// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Samin Yeasar

//! File watching module for `vell watch` command.
//! Watches a .vl file for changes and auto-rebuilds the output.
//! When `--port` is set, starts a tiny HTTP server with live-reload.

use notify::event::ModifyKind;
use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::HashSet;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::sync::mpsc;
use std::time::Duration;
use vell_core::*;

/// Watch command: watch a file and auto-rebuild on changes.
pub fn cmd_watch(
    input: &Option<PathBuf>,
    format: &str,
    output: &Option<PathBuf>,
    debounce_ms: u64,
    port: u16,
) -> Result<(), String> {
    let path = match input {
        Some(p) => p.clone(),
        None => return Err("Watch mode requires an input file".to_string()),
    };
    if !path.exists() {
        return Err(format!("File '{}' not found", path.display()));
    }

    let is_html_format = matches!(format, "html" | "slides");
    let serve = port > 0;
    if serve && !is_html_format {
        eprintln!(
            "[vell watch] Warning: --port is only supported for HTML/slides formats. Starting without server."
        );
    }
    let serve = serve && is_html_format;

    // Shared state for the HTTP server
    let latest_html: Arc<Mutex<String>> = Arc::new(Mutex::new(String::new()));
    let build_version: Arc<AtomicU64> = Arc::new(AtomicU64::new(0));

    // Do an initial build
    println!("[vell watch] Watching '{}'...", path.display());
    rebuild_and_store(&path, format, output, &latest_html, &build_version)?;

    // Start the HTTP server if requested
    if serve {
        let html = Arc::clone(&latest_html);
        let version = Arc::clone(&build_version);
        match start_server(html, version, port) {
            Ok(port_used) => {
                println!("[vell watch] Live preview at http://localhost:{}/", port_used);
            }
            Err(e) => {
                eprintln!("[vell watch] Failed to start server: {e}");
                return Err(e);
            }
        }
    }

    // Collect initial set of watch targets (main file + @[Include] dependencies)
    let base_dir = path.parent().unwrap_or(&path).to_path_buf();
    let mut watched_names: HashSet<String> = HashSet::new();
    watched_names.insert(
        path.file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default(),
    );
    // Scan for included files
    if let Ok(source) = std::fs::read_to_string(&path) {
        let included = collect_included_names(&source, &base_dir);
        watched_names.extend(included);
    }

    // Set up the file watcher (recursive to catch included files)
    let (tx, rx) = mpsc::channel::<Result<Event, notify::Error>>();
    let mut watcher = RecommendedWatcher::new(tx, Config::default())
        .map_err(|e| format!("Failed to create file watcher: {e}"))?;

    watcher
        .watch(&base_dir, RecursiveMode::Recursive)
        .map_err(|e| format!("Failed to watch '{}': {e}", base_dir.display()))?;

    let debounce = Duration::from_millis(debounce_ms);

    let num_targets = watched_names.len();
    if num_targets > 1 {
        println!(
            "[vell watch] Watching {} file(s) (including dependencies)...",
            num_targets
        );
    }
    println!("[vell watch] Waiting for changes (Ctrl+C to stop)...");

    loop {
        match rx.recv_timeout(debounce) {
            Ok(Ok(event)) => {
                let is_target = event.paths.iter().any(|p| {
                    p.file_name()
                        .map(|n| watched_names.contains(&n.to_string_lossy().to_string()))
                        .unwrap_or(false)
                });
                if !is_target {
                    continue;
                }
                match event.kind {
                    EventKind::Modify(ModifyKind::Data(_))
                    | EventKind::Modify(ModifyKind::Any)
                    | EventKind::Create(_) => {
                        println!("[vell watch] Change detected, rebuilding...");
                        match rebuild_and_store(&path, format, output, &latest_html, &build_version) {
                            Ok(()) => {
                                // Re-scan included files to catch new dependencies
                                if let Ok(source) = std::fs::read_to_string(&path) {
                                    let included = collect_included_names(&source, &base_dir);
                                    watched_names.clear();
                                    watched_names.insert(
                                        path.file_name()
                                            .map(|n| n.to_string_lossy().to_string())
                                            .unwrap_or_default(),
                                    );
                                    watched_names.extend(included);
                                }
                                let v = build_version.load(Ordering::Relaxed);
                                println!("[vell watch] Rebuild complete. (build #{v})");
                            }
                            Err(e) => eprintln!("[vell watch] Rebuild error: {e}"),
                        }
                    }
                    _ => {}
                }
            }
            Ok(Err(e)) => eprintln!("[vell watch] Watch error: {e}"),
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
    Ok(())
}

/// Starts a tiny HTTP server on the given port (or any available port if 0).
/// Returns the actual port the server is listening on.
fn start_server(
    html: Arc<Mutex<String>>,
    version: Arc<AtomicU64>,
    port: u16,
) -> Result<u16, String> {
    let bind_addr = format!("127.0.0.1:{port}");
    let listener = TcpListener::bind(&bind_addr)
        .map_err(|e| format!("Cannot bind to {bind_addr}: {e}"))?;

    let actual_port = listener.local_addr().map_err(|e| format!("{e}"))?.port();

    std::thread::spawn(move || {
        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {
                    let html = Arc::clone(&html);
                    let version = Arc::clone(&version);
                    std::thread::spawn(move || {
                        handle_client(stream, &html, &version);
                    });
                }
                Err(_) => break,
            }
        }
    });

    Ok(actual_port)
}

/// Handles a single HTTP client connection.
fn handle_client(mut stream: TcpStream, html: &Arc<Mutex<String>>, version: &Arc<AtomicU64>) {
    let mut buf = BufReader::new(&mut stream);
    let mut request_line = String::new();
    if buf.read_line(&mut request_line).is_err() {
        return;
    }

    let path = request_line
        .split_whitespace()
        .nth(1)
        .unwrap_or("/");

    // Read remaining headers (we don't need them, but must consume them)
    let mut header = String::new();
    loop {
        header.clear();
        if buf.read_line(&mut header).is_err() || header.trim().is_empty() {
            break;
        }
    }

    match path {
        "/_vell/version" => {
            let v = version.load(Ordering::Relaxed);
            let body = format!("{v}\n");
            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nCache-Control: no-cache\r\nAccess-Control-Allow-Origin: *\r\n\r\n{}",
                body.len(),
                body
            );
            let _ = stream.write_all(resp.as_bytes());
            let _ = stream.flush();
        }
        _ => {
            let body = html.lock().unwrap().clone();

            // Inject live-reload script before </body> if it looks like HTML
            let body = if body.contains("</body>") {
                body.replace("</body>", &format!(
                    r#"<script>
(function(){{
var v={v};
setInterval(function(){{
var x=new XMLHttpRequest();
x.open('GET','/_vell/version?t='+Date.now(),true);
x.onload=function(){{
var n=parseInt(x.responseText,10);
if(n>0&&n!==v){{v=n;location.reload();}}
}};
x.send();
}},1000);
}})();
</script>
</body>"#,
                    v = version.load(Ordering::Relaxed)
                ))
            } else {
                body
            };

            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nCache-Control: no-cache\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            let _ = stream.write_all(resp.as_bytes());
            let _ = stream.flush();
        }
    }

}

/// Rebuilds the output from the source file and stores the result.
fn rebuild_and_store(
    path: &Path,
    format: &str,
    output: &Option<PathBuf>,
    latest_html: &Arc<Mutex<String>>,
    build_version: &Arc<AtomicU64>,
) -> Result<(), String> {
    let source = std::fs::read_to_string(path)
        .map_err(|e| format!("Failed to read '{}': {e}", path.display()))?;
    let base = &Some(path.to_path_buf());
    let resolved = crate::resolve_includes(&source, base)?;
    let doc = parse_document(&resolved).map_err(|e| format!("Parse error: {e}"))?;

    match format {
        "html" => {
            let html = crate::render_document(&doc);
            *latest_html.lock().unwrap() = html.clone();
            write_output(html, output)?;
        }
        "pdf" => {
            let html = crate::render_document_pdf(&doc);
            *latest_html.lock().unwrap() = html.clone();
            write_output(html, output)?;
        }
        "slides" => {
            let html = crate::render_document_slides(&doc);
            *latest_html.lock().unwrap() = html.clone();
            write_output(html, output)?;
        }
        "epub" => {
            let ast_json = serde_json::to_string_pretty(&doc)
                .map_err(|e| format!("Failed to serialize AST: {e}"))?;
            let temp_dir = std::env::temp_dir();
            let temp_ast = temp_dir.join(format!("vell-epub-{}.json", std::process::id()));
            std::fs::write(&temp_ast, &ast_json)
                .map_err(|e| format!("Failed to write temp AST: {e}"))?;
            let epub_out = output
                .as_ref()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|| path.with_extension("epub").to_string_lossy().to_string());
            let pkg_dir = std::env::current_dir()
                .map_err(|e| format!("CWD: {e}"))?
                .join("packages")
                .join("vell-renderer-epub");
            let cli_script = pkg_dir.join("cli.js");
            if cli_script.exists() {
                let status = std::process::Command::new("node")
                    .arg(&cli_script)
                    .arg(&temp_ast)
                    .arg(&epub_out)
                    .status()
                    .map_err(|e| format!("Failed to run EPUB renderer: {e}"))?;
                if !status.success() {
                    eprintln!("EPUB renderer exited with error.");
                }
            } else {
                eprintln!("EPUB renderer not found at '{:?}'.", cli_script);
            }
            let _ = std::fs::remove_file(&temp_ast);
        }
        _ => return Err(format!("Unknown format '{format}'")),
    }

    build_version.fetch_add(1, Ordering::Relaxed);
    Ok(())
}

/// Scans source text for @[Include](path="...") directives and returns
/// the set of file names (not full paths) to watch for changes.
fn collect_included_names(source: &str, base_dir: &Path) -> HashSet<String> {
    let mut names = HashSet::new();
    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("@[Include](") && trimmed.contains("path=") {
            let after_open = trimmed.strip_prefix("@[Include](").unwrap_or_default();
            let props_content = after_open.trim_end_matches(')');
            // Find path="..."
            if let Some(path_val) = extract_include_path_value(props_content) {
                let p = PathBuf::from(&path_val);
                let resolved = if p.is_absolute() {
                    p
                } else {
                    base_dir.join(&p)
                };
                if let Some(name) = resolved.file_name() {
                    names.insert(name.to_string_lossy().to_string());
                }
            }
        }
    }
    names
}

/// Extracts the value from path="..." in a props string.
fn extract_include_path_value(props: &str) -> Option<String> {
    let start = props.find("path=\"")?;
    let value_start = start + 6; // length of path="
    let rest = props.get(value_start..)?;
    let end = rest.find('"')?;
    Some(rest.get(..end)?.to_string())
}

/// Writes content to file or prints to stdout.
fn write_output(content: String, output: &Option<PathBuf>) -> Result<(), String> {
    match output {
        Some(path) => std::fs::write(path, &content)
            .map_err(|e| format!("Failed to write '{}': {e}", path.display())),
        None => {
            println!("{content}");
            Ok(())
        }
    }
}
