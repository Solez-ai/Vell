// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Samin Yeasar

//! Core library for the Vell command-line interface.
#![allow(clippy::single_component_path_imports)]
#![allow(clippy::needless_lifetimes)]
#![allow(clippy::needless_borrow)]
#![allow(clippy::too_many_arguments)]
//!
//! Exposes `cmd_parse`, `cmd_fmt`, `cmd_validate`, and `cmd_render_html`
//! for direct use and testing.

use std::cell::RefCell;
use std::collections::HashMap;
use std::fs;
use std::io::{self, Read};
use std::path::PathBuf;
use vell_core::*;
use vell_fmt;

/// Collects bibliography entries from all @[Bibliography] directives in the document.
fn collect_bibliography(doc: &Document) -> Option<vell_core::bibliography::Bibliography> {
    let mut combined: Option<vell_core::bibliography::Bibliography> = None;
    for node in &doc.children {
        if let Node::Directive { name, props, .. } = node {
            if name == "Bibliography" {
                let style = props
                    .get("style")
                    .and_then(|v| {
                        if let PropValue::String(s) = v {
                            Some(s.clone())
                        } else {
                            None
                        }
                    })
                    .unwrap_or_else(|| "apa".to_string());
                let title = props.get("title").and_then(|v| {
                    if let PropValue::String(s) = v {
                        Some(s.as_str())
                    } else {
                        None
                    }
                });

                // Try source prop first (YAML-like entries)
                if let Some(PropValue::String(source)) = props.get("source") {
                    let bib =
                        vell_core::bibliography::parse_bibliography_source(source, &style, title);
                    merge_bibliography(&mut combined, bib);
                }

                // Try bibtex prop (inline BibTeX)
                if let Some(PropValue::String(bibtex)) = props.get("bibtex") {
                    let bib = vell_core::bibliography::parse_bibtex_source(bibtex);
                    merge_bibliography(&mut combined, bib);
                }
            }
        }
    }
    combined
}

fn merge_bibliography(
    target: &mut Option<vell_core::bibliography::Bibliography>,
    source: vell_core::bibliography::Bibliography,
) {
    match target {
        Some(ref mut t) => {
            for (key, entry) in source.entries {
                if !t.entries.contains_key(&key) {
                    t.entries.insert(key.clone(), entry);
                    t.order.push(key);
                }
            }
        }
        None => *target = Some(source),
    }
}

/// Reads source text from a file or stdin.
pub fn read_source(input: &Option<PathBuf>) -> Result<String, String> {
    match input {
        Some(path) => fs::read_to_string(path)
            .map_err(|e| format!("Failed to read '{}': {}", path.display(), e)),
        None => {
            let mut buf = String::new();
            io::stdin()
                .read_to_string(&mut buf)
                .map_err(|e| format!("Failed to read stdin: {}", e))?;
            Ok(buf)
        }
    }
}

/// Parse command: parse source and output AST JSON.
pub fn cmd_parse(input: &Option<PathBuf>) -> Result<(), String> {
    let source = read_source(input)?;
    match parse_document(&source) {
        Ok(doc) => {
            let json = serde_json::to_string_pretty(&doc)
                .map_err(|e| format!("Failed to serialize AST: {}", e))?;
            println!("{}", json);
            Ok(())
        }
        Err(e) => {
            eprintln!("{}", e);
            Err(String::new())
        }
    }
}

/// Format command: format Vell source code.
pub fn cmd_fmt(input: &Option<PathBuf>, check: bool) -> Result<(), String> {
    let source = read_source(input)?;
    match vell_fmt::format_source(&source) {
        Ok(formatted) => {
            if check {
                if source == formatted {
                    Ok(())
                } else {
                    eprintln!("File is not formatted correctly.");
                    Err(String::new())
                }
            } else {
                print!("{}", formatted);
                Ok(())
            }
        }
        Err(e) => {
            eprintln!("{}", e);
            Err(String::new())
        }
    }
}

/// Validate command: print diagnostics.
pub fn cmd_validate(input: &Option<PathBuf>) -> Result<(), String> {
    let source = read_source(input)?;
    let diagnostics = validate(&source);
    if diagnostics.is_empty() {
        Ok(())
    } else {
        for diag in &diagnostics {
            eprintln!("{}", diag);
        }
        match parse_document(&source) {
            Ok(_) => Ok(()),
            Err(_) => Err(String::new()),
        }
    }
}

/// Render HTML command: parse and render to HTML.
pub fn cmd_render_html(input: &Option<PathBuf>, output: &Option<PathBuf>) -> Result<(), String> {
    let source = read_source(input)?;
    let resolved = resolve_includes(&source, input)?;
    let doc = parse_document(&resolved).map_err(|e| format!("Parse error: {}", e))?;
    let html = render_document(&doc);
    match output {
        Some(path) => fs::write(path, &html)
            .map_err(|e| format!("Failed to write '{}': {}", path.display(), e)),
        None => {
            println!("{}", html);
            Ok(())
        }
    }
}

/// Render PDF command: parse and render to PDF-friendly HTML with print CSS.
pub fn cmd_render_pdf(input: &Option<PathBuf>, output: &Option<PathBuf>) -> Result<(), String> {
    let source = read_source(input)?;
    let resolved = resolve_includes(&source, input)?;
    let doc = parse_document(&resolved).map_err(|e| format!("Parse error: {}", e))?;
    let html = render_document_pdf(&doc);
    match output {
        Some(path) => fs::write(path, &html)
            .map_err(|e| format!("Failed to write '{}': {}", path.display(), e)),
        None => {
            println!("{}", html);
            Ok(())
        }
    }
}

/// Render Slides command: parse and render to reveal.js slide deck HTML.
pub fn cmd_render_slides(input: &Option<PathBuf>, output: &Option<PathBuf>) -> Result<(), String> {
    let source = read_source(input)?;
    let resolved = resolve_includes(&source, input)?;
    let doc = parse_document(&resolved).map_err(|e| format!("Parse error: {}", e))?;
    let html = render_document_slides(&doc);
    match output {
        Some(path) => fs::write(path, &html)
            .map_err(|e| format!("Failed to write '{}': {}", path.display(), e)),
        None => {
            println!("{}", html);
            Ok(())
        }
    }
}

// ---------------------------------------------------------------------------
// HTML renderer
// ---------------------------------------------------------------------------

/// Resolves @[Include](path="...") directives by reading and merging referenced files.
/// This is done as a pre-processing step before parsing.
/// Uses a `resolved_set` to detect and prevent circular includes.
pub fn resolve_includes(source: &str, base_path: &Option<PathBuf>) -> Result<String, String> {
    let mut resolved_paths = std::collections::HashSet::new();
    resolve_includes_inner(source, base_path, &mut resolved_paths)
}

fn resolve_includes_inner(
    source: &str,
    base_path: &Option<PathBuf>,
    resolved_paths: &mut std::collections::HashSet<PathBuf>,
) -> Result<String, String> {
    let mut result = String::new();
    let base_dir = base_path
        .as_ref()
        .and_then(|p| p.parent())
        .map(|p| p.to_path_buf());

    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("@[Include](") && trimmed.contains("path=") {
            // Extract the path value
            let after_open = trimmed.strip_prefix("@[Include](").unwrap_or_default();
            let props_content = after_open.trim_end_matches(')');
            // Parse path=...
            if let Some(path_val) = extract_prop_value(props_content, "path") {
                let resolved = resolve_include_path(&path_val, &base_dir)?;

                // Check for circular includes
                if resolved_paths.contains(&resolved) {
                    return Err(format!(
                        "Circular include detected: '{}' is already being resolved",
                        resolved.display()
                    ));
                }
                resolved_paths.insert(resolved.clone());

                let included_source = if resolved.exists() {
                    fs::read_to_string(&resolved).map_err(|e| {
                        format!(
                            "Failed to read included file '{}': {}",
                            resolved.display(),
                            e
                        )
                    })?
                } else {
                    return Err(format!(
                        "Included file '{}' not found (resolved: {})",
                        path_val,
                        resolved.display()
                    ));
                };
                // Recursively resolve includes in the included file
                let resolved_content =
                    resolve_includes_inner(&included_source, &Some(resolved), resolved_paths)?;
                result.push_str(&resolved_content);
                result.push('\n');
            } else {
                result.push_str(line);
                result.push('\n');
            }
        } else {
            result.push_str(line);
            result.push('\n');
        }
    }
    Ok(result)
}

/// Extracts a quoted property value from a prop string like `path="foo.vl" other="bar"`.
fn extract_prop_value<'a>(props: &'a str, key: &str) -> Option<String> {
    // Pattern: key="value"
    let mut search = String::from(key);
    search.push_str(&String::from("=\""));
    let start = props.find(&search)?;
    let value_start = start + search.len();
    let rest = props.get(value_start..)?;
    let end = rest.find('"')?;
    Some(rest.get(..end)?.to_string())
}

/// Resolves an include path relative to the base directory.
fn resolve_include_path(path: &str, base_dir: &Option<PathBuf>) -> Result<PathBuf, String> {
    let p = PathBuf::from(path);
    if p.is_absolute() {
        Ok(p)
    } else if let Some(base) = base_dir {
        Ok(base.join(&p))
    } else {
        // No base dir — try relative to current directory
        Ok(p)
    }
}

/// Escape HTML special characters.
fn escape_html(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            c => out.push(c),
        }
    }
    out
}

/// Sanitize a URL to only allow safe schemes and relative paths.
fn sanitize_url(url: &str) -> String {
    let trimmed = url.trim();
    if trimmed.starts_with("http://")
        || trimmed.starts_with("https://")
        || trimmed.starts_with("mailto:")
    {
        return escape_html(trimmed);
    }
    if trimmed.starts_with('/') || trimmed.starts_with('#') || trimmed.starts_with('.') {
        return escape_html(trimmed);
    }
    if !trimmed.contains(':') {
        return escape_html(trimmed);
    }
    String::new()
}

/// A resolved cross-reference target.
#[derive(Clone, Debug)]
struct LabelTarget {
    /// Anchor ID for the destination (e.g., "eq-e:mass-energy").
    anchor_id: String,
    /// Display text to show in the reference (e.g., "(1)" or "Theorem 1").
    display_text: String,
}

/// Pre-compute equation and theorem numbers so that labels can be resolved.
/// Returns a map of label → target info.
fn collect_labels(doc: &Document) -> HashMap<String, LabelTarget> {
    let mut labels = HashMap::new();
    let mut eq_counter = 0u32;
    let mut thm_counter: HashMap<String, u32> = HashMap::new();

    for node in &doc.children {
        collect_labels_node(node, &mut labels, &mut eq_counter, &mut thm_counter);
    }
    labels
}

fn collect_labels_node(
    node: &Node,
    labels: &mut HashMap<String, LabelTarget>,
    eq_counter: &mut u32,
    thm_counter: &mut HashMap<String, u32>,
) {
    match node {
        Node::Directive {
            name,
            props,
            children,
            ..
        } => {
            if name == "Equation" {
                *eq_counter += 1;
                let eq_num = *eq_counter;
                if let Some(PropValue::String(label)) = props.get("label") {
                    let anchor = format!("eq-{}", label);
                    let display = format!("({})", eq_num);
                    labels.insert(
                        label.clone(),
                        LabelTarget {
                            anchor_id: anchor,
                            display_text: display,
                        },
                    );
                }
            }
            let theorem_names: &[&str] = &[
                "Theorem",
                "Proof",
                "Lemma",
                "Corollary",
                "Definition",
                "Remark",
                "Example",
                "Conjecture",
                "Axiom",
                "Proposition",
                "Notation",
            ];
            if theorem_names.contains(&name.as_str()) {
                let is_numbered =
                    !matches!(name.as_str(), "Proof" | "Remark" | "Example" | "Notation");
                let thm_num = if is_numbered {
                    let count = thm_counter.entry(name.clone()).or_insert(0);
                    *count += 1;
                    Some(*count)
                } else {
                    None
                };

                if let Some(PropValue::String(label)) = props.get("label") {
                    let anchor = format!("thm-{}", label);
                    let display = if let Some(num) = thm_num {
                        format!("{} {}", name, num)
                    } else {
                        name.clone()
                    };
                    // Also include extra name if present
                    let extra = props.get("name").and_then(|v| {
                        if let PropValue::String(s) = v {
                            Some(s.clone())
                        } else {
                            None
                        }
                    });
                    let display_text = if let Some(ref extra_name) = extra {
                        format!("{} ({})", display, extra_name)
                    } else {
                        display
                    };
                    labels.insert(
                        label.clone(),
                        LabelTarget {
                            anchor_id: anchor,
                            display_text,
                        },
                    );
                }
            }
            for child in children {
                collect_labels_node(child, labels, eq_counter, thm_counter);
            }
        }
        Node::Blockquote { children, .. } => {
            for child in children {
                collect_labels_node(child, labels, eq_counter, thm_counter);
            }
        }
        Node::List { items, .. } => {
            for item in items {
                for child in &item.children {
                    collect_labels_node(child, labels, eq_counter, thm_counter);
                }
            }
        }
        Node::DefinitionList { items, .. } => {
            for item in items {
                for child in &item.definition {
                    collect_labels_node(child, labels, eq_counter, thm_counter);
                }
            }
        }
        Node::ForLoop { children, .. }
        | Node::IfBlock {
            consequent: children,
            ..
        } => {
            for child in children {
                collect_labels_node(child, labels, eq_counter, thm_counter);
            }
            // Also traverse the alternate (else) branch
            if let Node::IfBlock {
                alternate: Some(alt),
                ..
            } = node
            {
                for child in alt {
                    collect_labels_node(child, labels, eq_counter, thm_counter);
                }
            }
        }
        _ => {}
    }
}

thread_local! {
    static RENDER_TOC_ENTRIES: RefCell<Vec<(u8, String, String)>> = const { RefCell::new(Vec::new()) };
    static RENDER_LOF_ENTRIES: RefCell<Vec<(String, String)>> = const { RefCell::new(Vec::new()) };
    static RENDER_LOT_ENTRIES: RefCell<Vec<(String, String)>> = const { RefCell::new(Vec::new()) };
    static RENDER_BIBLIOGRAPHY: RefCell<Option<vell_core::bibliography::Bibliography>> = const { RefCell::new(None) };
}

/// Renders a parsed Vell document to an HTML string.
pub fn render_document(doc: &Document) -> String {
    // Pre-collect TOC, LOF, LOT entries into thread-locals
    let toc_entries = collect_toc_entries_vec(doc);
    let lof_entries = collect_lof_entries(doc);
    let lot_entries = collect_lot_entries(doc);
    RENDER_TOC_ENTRIES.with(|e| *e.borrow_mut() = toc_entries);
    RENDER_LOF_ENTRIES.with(|e| *e.borrow_mut() = lof_entries);
    RENDER_LOT_ENTRIES.with(|e| *e.borrow_mut() = lot_entries);

    let mut html = String::new();
    let title = doc.metadata.title.as_deref().unwrap_or("Vell Document");
    html.push_str("<!doctype html>\n<html>\n<head>\n");
    html.push_str("<meta charset=\"utf-8\">\n");
    html.push_str(&format!("<title>{}</title>\n", escape_html(title)));
    html.push_str(VELL_CSS);
    html.push_str("</head>\n<body>\n");
    let mut footnotes = Vec::new();
    for node in &doc.children {
        collect_footnotes(node, &mut footnotes);
    }
    // Pre-collect labels for cross-reference resolution
    let labels = collect_labels(doc);
    // Render with equation counter and theorem counter
    // Pre-collect bibliography entries
    let bib = collect_bibliography(doc);
    RENDER_BIBLIOGRAPHY.with(|b| *b.borrow_mut() = bib);

    let mut eq_counter = 0u32;
    let mut thm_counter: HashMap<String, u32> = HashMap::new();
    for node in &doc.children {
        render_node(
            node,
            &mut html,
            0,
            &mut eq_counter,
            &mut thm_counter,
            &labels,
        );
    }
    render_footnotes_section(&footnotes, &mut html, &labels);
    html.push_str("</body>\n</html>\n");

    // Clean up thread-locals
    RENDER_TOC_ENTRIES.with(|e| e.borrow_mut().clear());
    RENDER_LOF_ENTRIES.with(|e| e.borrow_mut().clear());
    RENDER_LOT_ENTRIES.with(|e| e.borrow_mut().clear());
    RENDER_BIBLIOGRAPHY.with(|b| *b.borrow_mut() = None);
    html
}

/// Renders a Vell document to PDF-optimized HTML with print CSS, TOC, and page numbering.
pub fn render_document_pdf(doc: &Document) -> String {
    // Pre-collect entries
    RENDER_TOC_ENTRIES.with(|e| *e.borrow_mut() = collect_toc_entries_vec(doc));
    RENDER_LOF_ENTRIES.with(|e| *e.borrow_mut() = collect_lof_entries(doc));
    RENDER_LOT_ENTRIES.with(|e| *e.borrow_mut() = collect_lot_entries(doc));

    let mut html = String::new();
    let title = doc.metadata.title.as_deref().unwrap_or("Vell Document");
    html.push_str("<!doctype html>\n<html>\n<head>\n");
    html.push_str("<meta charset=\"utf-8\">\n");
    html.push_str(&format!("<title>{}</title>\n", escape_html(title)));
    html.push_str(VELL_CSS);
    html.push_str("</head>\n<body>\n");
    // Running page header
    html.push_str(&format!(
        "<div class=\"page-header\">{}</div>\n",
        escape_html(title)
    ));

    // Generate TOC (document-level auto-generated TOC)
    let toc_entries = collect_toc_entries_vec(doc);
    if !toc_entries.is_empty() {
        html.push_str("<nav class=\"toc\" role=\"toc\">\n");
        html.push_str("<h1>Table of Contents</h1>\n");
        for (level, text, id) in &toc_entries {
            let indent = (level - 1) as usize * 2;
            let indent_str = " ".repeat(indent);
            html.push_str(&format!(
                "{}<a href=\"#{}\">{}</a><br>\n",
                indent_str,
                escape_html(id),
                escape_html(text)
            ));
        }
        html.push_str("</nav>\n");
        html.push_str("<div class=\"page-break\"></div>\n");
    }

    let mut footnotes = Vec::new();
    for node in &doc.children {
        collect_footnotes(node, &mut footnotes);
    }
    let labels = collect_labels(doc);
    let mut eq_counter = 0u32;
    let mut thm_counter: HashMap<String, u32> = HashMap::new();
    for node in &doc.children {
        render_node(
            node,
            &mut html,
            0,
            &mut eq_counter,
            &mut thm_counter,
            &labels,
        );
    }
    render_footnotes_section(&footnotes, &mut html, &labels);
    html.push_str("</body>\n</html>\n");

    RENDER_TOC_ENTRIES.with(|e| e.borrow_mut().clear());
    RENDER_LOF_ENTRIES.with(|e| e.borrow_mut().clear());
    RENDER_LOT_ENTRIES.with(|e| e.borrow_mut().clear());
    html
}

/// Renders a Vell document to a reveal.js slide deck HTML.
pub fn render_document_slides(doc: &Document) -> String {
    // Pre-collect TOC/LOF/LOT entries
    RENDER_TOC_ENTRIES.with(|e| *e.borrow_mut() = collect_toc_entries_vec(doc));
    RENDER_LOF_ENTRIES.with(|e| *e.borrow_mut() = collect_lof_entries(doc));
    RENDER_LOT_ENTRIES.with(|e| *e.borrow_mut() = collect_lot_entries(doc));

    let mut slides_html = String::new();
    let title = doc.metadata.title.as_deref().unwrap_or("Vell Presentation");
    let mut has_slides = false;
    let labels = collect_labels(doc);
    let mut eq_counter = 0u32;
    let mut thm_counter: HashMap<String, u32> = HashMap::new();

    for node in &doc.children {
        match node {
            Node::Directive { name, children, .. } if name == "Slide" => {
                has_slides = true;
                slides_html.push_str("<section>\n");
                for child in children {
                    render_node(
                        child,
                        &mut slides_html,
                        0,
                        &mut eq_counter,
                        &mut thm_counter,
                        &labels,
                    );
                }
                slides_html.push_str("</section>\n");
            }
            // Non-Slide content before first slide becomes the title slide
            _ if !has_slides => {
                slides_html.push_str("<section>\n");
                render_node(
                    node,
                    &mut slides_html,
                    0,
                    &mut eq_counter,
                    &mut thm_counter,
                    &labels,
                );
                slides_html.push_str("</section>\n");
            }
            _ => {}
        }
    }

    // If no slides found, wrap entire document in a single slide
    if !has_slides && slides_html.is_empty() {
        slides_html.push_str("<section>\n");
        for node in &doc.children {
            render_node(
                node,
                &mut slides_html,
                0,
                &mut eq_counter,
                &mut thm_counter,
                &labels,
            );
        }
        slides_html.push_str("</section>\n");
    }

    let reveal_css = "https://cdn.jsdelivr.net/npm/reveal.js@5.0.5/dist/reveal.css";
    let reveal_theme = "https://cdn.jsdelivr.net/npm/reveal.js@5.0.5/dist/theme/white.css";
    let reveal_js = "https://cdn.jsdelivr.net/npm/reveal.js@5.0.5/dist/reveal.js";

    // Clean up
    RENDER_TOC_ENTRIES.with(|e| e.borrow_mut().clear());
    RENDER_LOF_ENTRIES.with(|e| e.borrow_mut().clear());
    RENDER_LOT_ENTRIES.with(|e| e.borrow_mut().clear());

    format!(
        "<!doctype html>\n<html>\n<head>\n\
<meta charset=\"utf-8\">\n\
<title>{}</title>\n\
<link rel=\"stylesheet\" href=\"{}\">\n\
<link rel=\"stylesheet\" href=\"{}\">\n\
<style>\n\
  .vell-slide {{ display: block; }}\n\
  .reveal section img {{ max-width: 100%; }}\n\
  .reveal table {{ font-size: 0.8em; }}\n\
  .reveal .math {{ font-size: 1.2em; }}\n\
  .reveal pre code {{ max-height: 500px; }}\n\
</style>\n\
</head>\n<body>\n\
<div class=\"reveal\">\n\
<div class=\"slides\">\n\
{}\n\
</div>\n</div>\n\
<script src=\"{}\"></script>\n\
<script>Reveal.initialize({{ \
  hash: true, \
  slideNumber: true, \
  transition: 'slide', \
  controls: true, \
  progress: true \
}});</script>\n\
</body>\n</html>\n",
        escape_html(&title),
        reveal_css,
        reveal_theme,
        slides_html,
        reveal_js
    )
}

/// Collects heading entries for table of contents generation.
fn collect_toc_entries(doc: &Document, entries: &mut Vec<(u8, String, String)>) {
    for node in &doc.children {
        if let Node::Heading {
            level,
            children,
            id,
            ..
        } = node
        {
            let text = format_inline_nodes(children);
            let id_str = id.clone().unwrap_or_else(|| slugify_inline(children));
            entries.push((*level, text, id_str));
        }
    }
}

/// Returns a Vec of TOC entries: (level, text, id).
fn collect_toc_entries_vec(doc: &Document) -> Vec<(u8, String, String)> {
    let mut entries = Vec::new();
    collect_toc_entries(doc, &mut entries);
    entries
}

/// Collects figure entries for List of Figures: (caption, id).
fn collect_lof_entries(doc: &Document) -> Vec<(String, String)> {
    let mut entries = Vec::new();
    for node in &doc.children {
        collect_lof_node(node, &mut entries);
    }
    entries
}

fn collect_lof_node(node: &Node, entries: &mut Vec<(String, String)>) {
    match node {
        Node::Directive {
            name,
            props,
            children,
            ..
        } => {
            if name == "Figure" {
                let caption = props
                    .get("caption")
                    .and_then(|v| {
                        if let PropValue::String(s) = v {
                            Some(s.clone())
                        } else {
                            None
                        }
                    })
                    .unwrap_or_default();
                let id = props
                    .get("id")
                    .and_then(|v| {
                        if let PropValue::String(s) = v {
                            Some(s.clone())
                        } else {
                            None
                        }
                    })
                    .unwrap_or_default();
                entries.push((caption, id));
            }
            for child in children {
                collect_lof_node(child, entries);
            }
        }
        Node::Blockquote { children, .. } => {
            for child in children {
                collect_lof_node(child, entries);
            }
        }
        Node::List { items, .. } => {
            for item in items {
                for child in &item.children {
                    collect_lof_node(child, entries);
                }
            }
        }
        _ => {}
    }
}

/// Collects table entries for List of Tables: (caption, id).
fn collect_lot_entries(doc: &Document) -> Vec<(String, String)> {
    let mut entries = Vec::new();
    for node in &doc.children {
        if let Node::Table { .. } = node {
            entries.push((String::new(), String::new()));
        }
        if let Node::Directive { name, props, .. } = node {
            if name == "Table" || name == "GridTable" {
                let caption = props
                    .get("caption")
                    .and_then(|v| {
                        if let PropValue::String(s) = v {
                            Some(s.clone())
                        } else {
                            None
                        }
                    })
                    .unwrap_or_default();
                let id = props
                    .get("id")
                    .and_then(|v| {
                        if let PropValue::String(s) = v {
                            Some(s.clone())
                        } else {
                            None
                        }
                    })
                    .unwrap_or_default();
                entries.push((caption, id));
            }
        }
    }
    entries
}

/// Formats a slice of inline nodes into a plain text string.
fn format_inline_nodes(children: &[InlineNode]) -> String {
    let mut text = String::new();
    for node in children {
        match node {
            InlineNode::Text { value, .. } => text.push_str(value),
            InlineNode::Bold { children, .. }
            | InlineNode::Italic { children, .. }
            | InlineNode::Underline { children, .. }
            | InlineNode::Strikethrough { children, .. }
            | InlineNode::Superscript { children, .. }
            | InlineNode::Subscript { children, .. } => {
                text.push_str(&format_inline_nodes(children));
            }
            InlineNode::Code { value, .. } => text.push_str(value),
            InlineNode::MathInline { source, .. } => text.push_str(source),
            InlineNode::SoftBreak { .. } | InlineNode::HardBreak { .. } => text.push(' '),
            _ => {}
        }
    }
    text
}

/// Creates a URL-safe slug from inline children (e.g. heading text).
fn slugify_inline(children: &[InlineNode]) -> String {
    let text = format_inline_nodes(children);
    text.to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == ' ' || *c == '-')
        .map(|c| if c == ' ' { '-' } else { c })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

/// Formats a chemical formula string with HTML subscript tags for numbers.
/// E.g. "H2O" → "H<sub>2</sub>O", "C6H12O6" → "C<sub>6</sub>H<sub>12</sub>O<sub>6</sub>".
fn format_chem_formula(source: &str) -> String {
    let mut out = String::new();
    let mut in_sub = false;
    for ch in source.chars() {
        match ch {
            '0'..='9' => {
                if !in_sub {
                    out.push_str("<sub>");
                    in_sub = true;
                }
                out.push(ch);
            }
            _ => {
                if in_sub {
                    out.push_str("</sub>");
                    in_sub = false;
                }
                match ch {
                    '&' => out.push_str("&amp;"),
                    '<' => out.push_str("&lt;"),
                    '>' => out.push_str("&gt;"),
                    '"' => out.push_str("&quot;"),
                    c => out.push(c),
                }
            }
        }
    }
    if in_sub {
        out.push_str("</sub>");
    }
    out
}

/// CSS styles for Vell rendering.
const VELL_CSS: &str = "<style>\n\
/* Base */\n\
body { font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Helvetica, Arial, sans-serif; line-height: 1.6; color: #1a202c; max-width: 800px; margin: 0 auto; padding: 1em; }\n\
code { font-family: 'SFMono-Regular', Consolas, 'Liberation Mono', Menlo, monospace; }\n\
pre { font-family: 'SFMono-Regular', Consolas, 'Liberation Mono', Menlo, monospace; background: #f7fafc; padding: 0.8em; overflow-x: auto; border-radius: 4px; }\n\
img { max-width: 100%; height: auto; }\n\
.vell-equation { margin: 0.8em 0; padding: 0.2em 0; }\n\
.eq-table { width: 100%; border: none; border-collapse: collapse; }\n\
.eq-table td { padding: 0; vertical-align: middle; }\n\
.eq-math { text-align: center; width: 90%; }\n\
.eq-number { text-align: right; width: 10%; padding-left: 1em; font-size: 0.9em; color: #555; }\n\
.vell-theorem { margin: 1em 0; padding: 0.6em 1em; border-left: 3px solid #3182ce; background: #f7fafc; }\n\
.vell-proof { border-left-color: #718096; background: #fefefe; }\n\
.vell-lemma { border-left-color: #38a169; background: #f0fff4; }\n\
.vell-corollary { border-left-color: #d69e2e; background: #fffff0; }\n\
.vell-definition { border-left-color: #805ad5; background: #faf5ff; }\n\
.vell-remark { border-left-color: #a0aec0; background: #f5f5f5; }\n\
.vell-example { border-left-color: #319795; background: #f0fdfa; }\n\
.vell-conjecture { border-left-color: #e53e3e; background: #fff5f5; }\n\
.vell-axiom { border-left-color: #dd6b20; background: #fffaf0; }\n\
.vell-proposition { border-left-color: #2b6cb0; background: #ebf8ff; }\n\
.theorem-label { font-weight: bold; font-style: italic; margin-bottom: 0.3em; color: #2d3748; }\n\
.theorem-body > :first-child { margin-top: 0; }\n\
.theorem-body > :last-child { margin-bottom: 0; }\n\
.vell-math-env { margin: 0.8em 0; padding: 0.4em 1em; background: #fdfdfd; border: 1px solid #e2e8f0; border-radius: 4px; }\n\
.vell-math-env math { display: block; margin: 0.4em 0; }\n\
.vell-ref { color: #2b6cb0; text-decoration: none; }\n\
.vell-ref:hover { text-decoration: underline; }\n\
.unresolved-ref { color: #e53e3e; font-style: italic; }\n\
.admonition { padding: 0.6em 1em; margin: 1em 0; border-left: 4px solid #3182ce; background: #ebf8ff; }\n\
.admonition.warning, .admonition.warn { border-left-color: #d69e2e; background: #fffff0; }\n\
.admonition.danger, .admonition.error { border-left-color: #e53e3e; background: #fff5f5; }\n\
.admonition.success, .admonition.tip { border-left-color: #38a169; background: #f0fff4; }\n\
/* Phase 8: Chemical equations */\n\
.vell-chem { margin: 0.6em 0; padding: 0.4em 1em; background: #f0fdfa; border: 1px solid #b2f5ea; border-radius: 4px; font-family: 'Courier New', monospace; }\n\
.vell-chem .chem-formula { font-size: 1.1em; font-weight: bold; color: #234e52; }\n\
/* Phase 9: TOC, LOF, LOT */\n\
.vell-toc, .vell-lof, .vell-lot { margin: 1em 0; padding: 0.6em 1em; background: #f7fafc; border: 1px solid #e2e8f0; border-radius: 4px; }\n\
.vell-toc h2, .vell-lof h2, .vell-lot h2 { font-size: 1.1em; margin: 0 0 0.5em 0; color: #2d3748; }\n\
.vell-toc .toc-list, .vell-lof .lof-list, .vell-lot .lot-list { padding-left: 1.5em; }\n\
.vell-toc .toc-list li, .vell-lof .lof-list li, .vell-lot .lot-list li { margin: 0.2em 0; }\n\
.toc-placeholder, .lof-placeholder, .lot-placeholder { color: #a0aec0; font-style: italic; list-style: none; }\n\
/* Phase 10: Diagrams & Charts */\n\
.vell-diagram { margin: 1em 0; padding: 1em; background: #f8f9fa; border: 1px solid #e2e8f0; border-radius: 4px; overflow-x: auto; }\n\
.vell-diagram .diagram-caption { font-size: 0.9em; color: #666; margin-top: 0.5em; font-style: italic; }\n\
.vell-diagram pre { margin: 0; white-space: pre; font-family: 'Courier New', monospace; font-size: 0.9em; line-height: 1.4; }\n\
.vell-diagram[data-type=\"mermaid\"] .mermaid { margin: 0; }\n\
.vell-diagram[data-type=\"ascii\"] pre { color: #333; }\n\
.vell-diagram[data-type=\"dot\"] .graphviz { margin: 0; }\n\
.vell-diagram[data-type=\"dot\"] pre.dot { color: #2b6cb0; }\n\
.vell-chart { margin: 1em 0; padding: 0.5em; overflow-x: auto; }\n\
.vell-chart svg { display: block; margin: 0 auto; }\n\
.chart-title { text-align: center; font-size: 1em; font-weight: bold; margin-bottom: 0.3em; color: #2d3748; }\n\
/* Phase 10: Function plots */\n\
.vell-plot { margin: 1em 0; padding: 0.5em; overflow-x: auto; }\n\
.vell-plot svg { display: block; margin: 0 auto; }\n\
/* Phase 12: Print CSS */\n\
@media print {\n\
  body { font-size: 11pt; line-height: 1.5; color: #000; background: #fff; max-width: none; padding: 0; }\n\
  @page { margin: 2.54cm; }\n\
  @page :first { margin-top: 2.54cm; }\n\
  h1, h2, h3, h4, h5, h6 { page-break-after: avoid; }\n\
  h1 { page-break-before: always; }\n\
  h1:first-of-type { page-break-before: avoid; }\n\
  table { page-break-inside: avoid; }\n\
  pre, blockquote { page-break-inside: avoid; }\n\
  img { page-break-inside: avoid; }\n\
  a { color: #000; text-decoration: none; }\n\
  a[href^=\"http\"]::after { content: \" (\" attr(href) \")\"; font-size: 0.8em; color: #555; }\n\
  .vell-slide { display: none; }\n\
  .vell-diagram { border: 1px solid #ddd; }\n\
  .vell-chart svg { max-width: 100%; }\n\
  .vell-plot svg { max-width: 100%; }\n\
  .vell-chem { border: 1px solid #b2f5ea; }\n\
  .vell-equation { page-break-inside: avoid; }\n\
  .footnotes { page-break-before: always; font-size: 0.85em; }\n\
  .page-break { page-break-before: always; }\n\
  .toc { page-break-after: always; }\n\
  /* Running header */\n\
  @page { @top-center { content: element(pageHeader); font-size: 9pt; color: #666; } }\n\
  .page-header { position: running(pageHeader); }\n\
  /* Running footer with page number */\n\
  @page { @bottom-center { content: counter(page); font-size: 9pt; color: #666; } }\n\
}\n\
</style>\n";

fn collect_footnotes(node: &Node, out: &mut Vec<(String, Vec<Node>)>) {
    if let Node::FootnoteDefinition {
        marker, children, ..
    } = node
    {
        out.push((marker.clone(), children.clone()));
    }
}

fn render_footnotes_section(
    footnotes: &[(String, Vec<Node>)],
    html: &mut String,
    labels: &HashMap<String, LabelTarget>,
) {
    if footnotes.is_empty() {
        return;
    }
    html.push_str("<section class=\"footnotes\">\n<h2>Footnotes</h2>\n");
    for (marker, children) in footnotes {
        html.push_str(&format!(
            "<p id=\"fn:{}\"><sup>{}</sup> ",
            escape_html(marker),
            escape_html(marker)
        ));
        for child in children {
            let mut _eq_tmp = 0;
            let mut _thm_tmp: HashMap<String, u32> = HashMap::new();
            render_node(child, html, 1, &mut _eq_tmp, &mut _thm_tmp, labels);
        }
        html.push_str("</p>\n");
    }
    html.push_str("</section>\n");
}

fn render_node(
    node: &Node,
    html: &mut String,
    _depth: usize,
    eq_counter: &mut u32,
    thm_counter: &mut HashMap<String, u32>,
    labels: &HashMap<String, LabelTarget>,
) {
    match node {
        Node::Heading {
            level,
            children,
            id,
            ..
        } => {
            let lvl = level.clamp(&1, &6);
            let id_attr = id
                .as_ref()
                .map(|v| format!(" id=\"{}\"", escape_html(v)))
                .unwrap_or_default();
            html.push_str(&format!("<h{}{}>", lvl, id_attr));
            render_inline_children(children, html, labels);
            html.push_str(&format!("</h{}>\n", lvl));
        }
        Node::Paragraph { children, .. } => {
            html.push_str("<p>");
            render_inline_children(children, html, labels);
            html.push_str("</p>\n");
        }
        Node::Blockquote {
            children,
            admonition_type,
            ..
        } => {
            if let Some(kind) = admonition_type {
                html.push_str(&format!(
                    "<blockquote class=\"admonition {}\">\n",
                    escape_html(kind)
                ));
            } else {
                html.push_str("<blockquote>\n");
            }
            for child in children {
                render_node(child, html, _depth + 1, eq_counter, thm_counter, labels);
            }
            html.push_str("</blockquote>\n");
        }
        Node::MathBlock { source, .. } => {
            let mathml = latex_to_mathml(source, true);
            html.push_str(&format!("<math display=\"block\">{}</math>\n", mathml));
        }
        Node::CodeBlock { lang, source, .. } => {
            let lang_class = lang
                .as_ref()
                .map(|l| format!(" class=\"language-{}\"", escape_html(l)))
                .unwrap_or_default();
            html.push_str(&format!("<pre{}><code>", lang_class));
            html.push_str(&escape_html(source));
            html.push_str("</code></pre>\n");
        }
        Node::List {
            ordered,
            start,
            items,
            ..
        } => {
            let tag = if *ordered { "ol" } else { "ul" };
            let start_attr = if *ordered {
                start
                    .map(|s| format!(" start=\"{}\"", s))
                    .unwrap_or_default()
            } else {
                String::new()
            };
            html.push_str(&format!("<{}{}>\n", tag, start_attr));
            for item in items {
                html.push_str("<li>");
                if let Some(checked) = item.checked {
                    html.push_str(if checked {
                        "<input type=\"checkbox\" checked disabled> "
                    } else {
                        "<input type=\"checkbox\" disabled> "
                    });
                }
                for child in &item.children {
                    render_node(child, html, _depth + 1, eq_counter, thm_counter, labels);
                }
                html.push_str("</li>\n");
            }
            html.push_str(&format!("</{}>\n", tag));
        }
        Node::Table { headers, rows, .. } => {
            html.push_str("<table>\n");
            if !headers.is_empty() {
                html.push_str("<thead>\n<tr>\n");
                for cell in headers {
                    render_table_cell(cell, html, true, labels);
                }
                html.push_str("</tr>\n</thead>\n");
            }
            if !rows.is_empty() {
                html.push_str("<tbody>\n");
                for row in rows {
                    html.push_str("<tr>\n");
                    for cell in row {
                        render_table_cell(cell, html, false, labels);
                    }
                    html.push_str("</tr>\n");
                }
                html.push_str("</tbody>\n");
            }
            html.push_str("</table>\n");
        }
        Node::HorizontalRule { .. } => html.push_str("<hr>\n"),
        Node::DefinitionList { items, .. } => {
            html.push_str("<dl>\n");
            for item in items {
                html.push_str("<dt>");
                render_inline_children(&item.term, html, labels);
                html.push_str("</dt>\n");
                for def in &item.definition {
                    html.push_str("<dd>");
                    render_node(def, html, _depth + 1, eq_counter, thm_counter, labels);
                    html.push_str("</dd>\n");
                }
            }
            html.push_str("</dl>\n");
        }
        Node::ReferenceDefinition { .. } => {}
        Node::FootnoteDefinition { .. } => {}
        Node::VarDeclaration { .. } => {}
        Node::ForLoop {
            variable,
            iterable,
            children,
            ..
        } => {
            html.push_str(&format!(
                "<div data-vell-for=\"{}\" data-vell-in=\"{}\">\n",
                escape_html(variable),
                escape_html(iterable)
            ));
            for child in children {
                render_node(child, html, _depth + 1, eq_counter, thm_counter, labels);
            }
            html.push_str("</div>\n");
        }
        Node::IfBlock {
            condition,
            consequent,
            alternate,
            ..
        } => {
            html.push_str(&format!(
                "<div data-vell-if=\"{}\">\n",
                escape_html(condition)
            ));
            for child in consequent {
                render_node(child, html, _depth + 1, eq_counter, thm_counter, labels);
            }
            if let Some(alt) = alternate {
                for child in alt {
                    render_node(child, html, _depth + 1, eq_counter, thm_counter, labels);
                }
            }
            html.push_str("</div>\n");
        }
        Node::Directive {
            name,
            props,
            children,
            ..
        } => {
            render_directive(
                name,
                props,
                children,
                html,
                _depth,
                eq_counter,
                thm_counter,
                labels,
            );
        }
        Node::Extension { name, children, .. } => {
            html.push_str(&format!(
                "<div class=\"vell-extension {}\">\n",
                escape_html(name)
            ));
            for child in children {
                render_node(child, html, _depth + 1, eq_counter, thm_counter, labels);
            }
            html.push_str("</div>\n");
        }
    }
}

fn render_directive(
    name: &str,
    props: &HashMap<String, PropValue>,
    children: &[Node],
    html: &mut String,
    _depth: usize,
    eq_counter: &mut u32,
    thm_counter: &mut HashMap<String, u32>,
    labels: &HashMap<String, LabelTarget>,
) {
    match name {
        "Template" => {
            let name = match props.get("name") {
                Some(PropValue::String(s)) => s.clone(),
                _ => String::new(),
            };
            let style = match props.get("style") {
                Some(PropValue::String(s)) => s.clone(),
                _ => String::new(),
            };
            let url = match props.get("url") {
                Some(PropValue::String(s)) => s.clone(),
                _ => String::new(),
            };
            if !url.is_empty() {
                html.push_str(&format!(
                    "<link rel=\"stylesheet\" href=\"{}\">\n",
                    sanitize_url(&url)
                ));
            }
            if !name.is_empty() {
                html.push_str(&format!(
                    "<meta name=\"vell-template\" content=\"{}\">\n",
                    escape_html(&name)
                ));
            }
            if !style.is_empty() {
                html.push_str(&format!("<style>\n{}\n</style>\n", style));
            }
            for child in children {
                render_node(child, html, _depth + 1, eq_counter, thm_counter, labels);
            }
        }
        "Ref" => {
            // Cross-reference: resolves label to equation/theorem number + URL
            let label = match props.get("label") {
                Some(PropValue::String(s)) => s.clone(),
                _ => String::new(),
            };
            if let Some(target) = labels.get(&label) {
                html.push_str(&format!(
                    "<a href=\"#{}\" class=\"vell-ref\">{}</a>",
                    escape_html(&target.anchor_id),
                    escape_html(&target.display_text)
                ));
            } else {
                html.push_str(&format!(
                    "<span class=\"unresolved-ref\">[?{}]</span>",
                    escape_html(&label)
                ));
            }
        }
        "Equation" => {
            // Get source from props or extract from children text
            let source = match props.get("source") {
                Some(PropValue::String(s)) => s.clone(),
                _ => {
                    // Fallback: extract text from children
                    let mut text = String::new();
                    for child in children {
                        if let Node::Paragraph {
                            children: inlines, ..
                        } = child
                        {
                            for inline in inlines {
                                if let InlineNode::Text { value, .. } = inline {
                                    text.push_str(value);
                                }
                            }
                        }
                    }
                    text
                }
            };
            let label = props.get("label").and_then(|v| {
                if let PropValue::String(s) = v {
                    Some(s.clone())
                } else {
                    None
                }
            });

            *eq_counter += 1;
            let eq_num = *eq_counter;

            let mathml = latex_to_mathml(&source, true);
            let id_attr = label
                .as_ref()
                .map(|l| format!(" id=\"eq-{}\"", escape_html(l)))
                .unwrap_or_default();
            let label_attr = label
                .as_ref()
                .map(|l| format!(" data-label=\"{}\"", escape_html(l)))
                .unwrap_or_default();

            html.push_str(&format!(
                "<div class=\"vell-equation\"{} data-number=\"{}\"{}>\n",
                id_attr, eq_num, label_attr
            ));
            html.push_str("<table class=\"eq-table\"><tr>\n");
            html.push_str(&format!(
                "<td class=\"eq-math\"><math display=\"block\">{}</math></td>\n",
                mathml
            ));
            html.push_str(&format!("<td class=\"eq-number\">({})</td>\n", eq_num));
            html.push_str("</tr></table>\n");
            html.push_str("</div>\n");
        }
        // Theorem environments: Theorem, Proof, Lemma, Corollary, Definition, etc.
        "Theorem" | "Proof" | "Lemma" | "Corollary" | "Definition" | "Remark" | "Example"
        | "Conjecture" | "Axiom" | "Proposition" | "Notation" => {
            let theme_name = escape_html(name);
            let theme_class = theme_name.to_lowercase();
            let extra = props.get("name").and_then(|v| {
                if let PropValue::String(s) = v {
                    Some(s.clone())
                } else {
                    None
                }
            });

            // Auto-number theorems (except Proof and Remark)
            let counter_key = if theme_name == "Proof"
                || theme_name == "Remark"
                || theme_name == "Example"
                || theme_name == "Notation"
            {
                None
            } else {
                let count = thm_counter.entry(theme_name.clone()).or_insert(0);
                *count += 1;
                Some(*count)
            };

            let theorem_label = props.get("label").and_then(|v| {
                if let PropValue::String(s) = v {
                    Some(s.clone())
                } else {
                    None
                }
            });
            let theorem_id = theorem_label
                .as_ref()
                .map(|l| format!(" id=\"thm-{}\"", escape_html(l)))
                .unwrap_or_default();

            html.push_str(&format!(
                "<div class=\"vell-theorem vell-{}\"{}>\n",
                theme_class, theorem_id
            ));
            html.push_str("<div class=\"theorem-label\">");
            html.push_str(&theme_name);
            if let Some(ref num) = counter_key {
                html.push_str(&format!(" {}", num));
            }
            if let Some(ref extra_name) = extra {
                html.push_str(&format!(" ({})", escape_html(extra_name)));
            }
            html.push_str("</div>\n");
            html.push_str("<div class=\"theorem-body\">\n");
            for child in children {
                render_node(child, html, _depth + 1, eq_counter, thm_counter, labels);
            }
            html.push_str("</div>\n");
            html.push_str("</div>\n");
        } // Phase 10: Diagram rendering (Mermaid, ASCII, Graphviz DOT)
        "Diagram" => {
            let diagram_type = match props.get("type") {
                Some(PropValue::String(s)) => escape_html(s),
                _ => String::from("general"),
            };
            let caption = match props.get("caption") {
                Some(PropValue::String(s)) => s.clone(),
                _ => String::new(),
            };
            // Extract source from children text
            let mut source_parts: Vec<String> = Vec::new();
            for child in children {
                if let Node::Paragraph {
                    children: inlines, ..
                } = child
                {
                    for inline in inlines {
                        if let InlineNode::Text { value, .. } = inline {
                            source_parts.push(value.clone());
                        }
                    }
                }
            }
            let source = source_parts.join("\n");

            html.push_str(&format!(
                "<div class=\"vell-diagram\" data-type=\"{}\">\n",
                diagram_type
            ));
            match diagram_type.as_str() {
                "mermaid" => {
                    html.push_str("<div class=\"mermaid\">\n");
                    html.push_str(&escape_html(&source));
                    html.push_str("\n</div>\n");
                }
                "dot" => {
                    html.push_str("<div class=\"graphviz\">\n");
                    html.push_str("<pre class=\"dot\">\n");
                    html.push_str(&escape_html(&source));
                    html.push_str("\n</pre>\n");
                    html.push_str("</div>\n");
                }
                _ => {
                    html.push_str("<pre>\n");
                    html.push_str(&escape_html(&source));
                    html.push_str("\n</pre>\n");
                }
            }
            if !caption.is_empty() {
                html.push_str(&format!(
                    "<div class=\"diagram-caption\">{}</div>\n",
                    escape_html(&caption)
                ));
            }
            html.push_str("</div>\n");
        }
        // Phase 11: Interactive form directives
        "Input" => {
            let var_name = match props.get("bind") {
                Some(PropValue::String(s)) => escape_html(s),
                _ => String::new(),
            };
            let input_type = match props.get("type") {
                Some(PropValue::String(s)) => escape_html(s),
                _ => String::from("text"),
            };
            let placeholder = match props.get("placeholder") {
                Some(PropValue::String(s)) => format!(" placeholder=\"{}\"", escape_html(s)),
                _ => String::new(),
            };
            let bind_attr = if !var_name.is_empty() {
                format!(" data-bind=\"{}\"", var_name)
            } else {
                String::new()
            };
            let label = match props.get("label") {
                Some(PropValue::String(s)) => format!("<label>{} ", escape_html(s)),
                _ => String::new(),
            };
            let label_close = if !label.is_empty() { "</label>" } else { "" };
            html.push_str(&format!(
                "{}<input type=\"{}\"{}{}>{}",
                label, input_type, placeholder, bind_attr, label_close
            ));
        }
        "Select" => {
            let var_name = match props.get("bind") {
                Some(PropValue::String(s)) => escape_html(s),
                _ => String::new(),
            };
            let bind_attr = if !var_name.is_empty() {
                format!(" data-bind=\"{}\"", var_name)
            } else {
                String::new()
            };
            let label = match props.get("label") {
                Some(PropValue::String(s)) => format!("<label>{} ", escape_html(s)),
                _ => String::new(),
            };
            let label_close = if !label.is_empty() { "</label>" } else { "" };
            // Options can be provided via an "options" prop or as children
            let options = match props.get("options") {
                Some(PropValue::String(s)) => s.clone(),
                _ => {
                    let mut opts = String::new();
                    for child in children {
                        if let Node::Paragraph {
                            children: inlines, ..
                        } = child
                        {
                            for inline in inlines {
                                if let InlineNode::Text { value, .. } = inline {
                                    for line in value.lines() {
                                        let line = line.trim();
                                        if !line.is_empty() {
                                            opts.push_str(&format!(
                                                "<option>{}</option>",
                                                escape_html(line)
                                            ));
                                        }
                                    }
                                }
                            }
                        }
                    }
                    opts
                }
            };
            html.push_str(&format!(
                "{}<select{}>\n{}\n</select>{}",
                label, bind_attr, options, label_close
            ));
        }
        "Checkbox" => {
            let var_name = match props.get("bind") {
                Some(PropValue::String(s)) => escape_html(s),
                _ => String::new(),
            };
            let bind_attr = if !var_name.is_empty() {
                format!(" data-bind=\"{}\"", var_name)
            } else {
                String::new()
            };
            let checked = match props.get("checked") {
                Some(PropValue::Bool(true)) => " checked",
                _ => "",
            };
            let label = match props.get("label") {
                Some(PropValue::String(s)) => format!(" <span>{}</span>", escape_html(s)),
                _ => String::new(),
            };
            html.push_str(&format!(
                "<label><input type=\"checkbox\"{}{}>{}</label>",
                bind_attr, checked, label
            ));
        }
        "Data" => {
            let data_json = match props.get("data") {
                Some(PropValue::String(s)) => s.clone(),
                _ => String::new(),
            };
            let source = match props.get("source") {
                Some(PropValue::String(s)) => escape_html(s),
                _ => String::new(),
            };
            if !data_json.is_empty() {
                // Raw JSON string — no HTML escaping (inside <script> tags, HTML entities are not decoded)
                html.push_str(&format!(
                    "<script type=\"application/json\" data-vell-init>{}</script>\n",
                    data_json
                ));
            } else if !source.is_empty() {
                // File reference — rendered as a meta tag for runtime
                html.push_str(&format!("<meta data-vell-data=\"{}\">\n", source));
            }
        }
        // Phase 10: Chart rendering (inline SVG bar chart)
        "Chart" => {
            let chart_type = match props.get("type") {
                Some(PropValue::String(s)) => s.clone(),
                _ => String::from("bar"),
            };
            let title = match props.get("title") {
                Some(PropValue::String(s)) => s.clone(),
                _ => String::new(),
            };
            // Extract data from children text
            let mut data_lines: Vec<(String, f64)> = Vec::new();
            for child in children {
                if let Node::Paragraph {
                    children: inlines, ..
                } = child
                {
                    for inline in inlines {
                        if let InlineNode::Text { value, .. } = inline {
                            for line in value.lines() {
                                let trimmed = line.trim();
                                if let Some(comma_pos) = trimmed.rfind(',') {
                                    let label =
                                        trimmed.get(..comma_pos).unwrap_or("").trim().to_string();
                                    let val_str = trimmed.get(comma_pos + 1..).unwrap_or("").trim();
                                    if let Ok(val) = val_str.parse::<f64>() {
                                        data_lines.push((label, val));
                                    }
                                }
                            }
                        }
                    }
                }
            }

            if !data_lines.is_empty() && chart_type == "bar" {
                let svg = render_bar_chart_svg(&data_lines, &title);
                html.push_str(&format!(
                    "<div class=\"vell-chart vell-chart-{}\">\n{}\n</div>\n",
                    escape_html(&chart_type),
                    svg
                ));
            } else {
                // Fallback: render as a table
                html.push_str("<div class=\"vell-chart\">\n");
                if !title.is_empty() {
                    html.push_str(&format!(
                        "<div class=\"chart-title\">{}</div>\n",
                        escape_html(&title)
                    ));
                }
                html.push_str("<table>\n");
                for (label, val) in &data_lines {
                    html.push_str(&format!(
                        "<tr><td>{}</td><td>{}</td></tr>\n",
                        escape_html(label),
                        val
                    ));
                }
                html.push_str("</table>\n");
                html.push_str("</div>\n");
            }
        }
        // Align, Align*, Matrix environments
        "Align" | "Align*" | "Matrix" | "PMatrix" | "BMatrix" | "VMatrix" | "Cases" => {
            let theme_name = escape_html(name);
            let source = match props.get("source") {
                Some(PropValue::String(s)) => s.clone(),
                _ => String::new(),
            };

            html.push_str(&format!(
                "<div class=\"vell-math-env vell-{}\">\n",
                theme_name.to_lowercase()
            ));
            if !source.is_empty() {
                let mathml = latex_to_mathml(&source, true);
                html.push_str(&format!("<math display=\"block\">{}</math>\n", mathml));
            }
            // Render children as description text
            for child in children {
                render_node(child, html, _depth + 1, eq_counter, thm_counter, labels);
            }
            html.push_str("</div>\n");
        }
        // Phase 8: Chemical equations with subscript formatting
        "Chem" => {
            let source = match props.get("source") {
                Some(PropValue::String(s)) => s.clone(),
                _ => {
                    // Extract from children
                    let mut text = String::new();
                    for child in children {
                        if let Node::Paragraph {
                            children: inlines, ..
                        } = child
                        {
                            for inline in inlines {
                                if let InlineNode::Text { value, .. } = inline {
                                    text.push_str(value);
                                }
                            }
                        }
                    }
                    text
                }
            };
            // Format: numbers become subscripts, other chars are HTML-escaped
            let formatted = format_chem_formula(&source);
            html.push_str("<div class=\"vell-chem\">\n");
            html.push_str(&format!(
                "<code class=\"chem-formula\">{}</code>\n",
                formatted
            ));
            html.push_str("</div>\n");
        }

        // Phase 9: Table of Contents (uses pre-collected entries from thread-local)
        "Toc" => {
            html.push_str("<nav class=\"vell-toc\" role=\"toc\">\n");
            html.push_str("<h2>Table of Contents</h2>\n");
            html.push_str("<ol class=\"toc-list\">\n");
            let entries = RENDER_TOC_ENTRIES.with(|e| e.borrow().clone());
            if entries.is_empty() {
                html.push_str("<li class=\"toc-placeholder\">(no headings found)</li>\n");
            } else {
                for (level, text, id) in &entries {
                    let indent = (level - 1) as usize * 2;
                    let indent_str = " ".repeat(indent);
                    html.push_str(&format!(
                        "{}<li><a href=\"#{}\">{}</a></li>\n",
                        indent_str,
                        escape_html(id),
                        escape_html(text)
                    ));
                }
            }
            html.push_str("</ol>\n");
            html.push_str("</nav>\n");
        }

        // Phase 9: List of Figures (uses pre-collected entries)
        "Lof" => {
            html.push_str("<nav class=\"vell-lof\" role=\"lof\">\n");
            html.push_str("<h2>List of Figures</h2>\n");
            html.push_str("<ul class=\"lof-list\">\n");
            let entries = RENDER_LOF_ENTRIES.with(|e| e.borrow().clone());
            if entries.is_empty() {
                html.push_str("<li class=\"lof-placeholder\">(no figures found)</li>\n");
            } else {
                for (caption, id) in &entries {
                    let id_attr = if id.is_empty() {
                        String::new()
                    } else {
                        format!(" href=\"#{}\"", escape_html(id))
                    };
                    html.push_str(&format!(
                        "<li><a{}>{}</a></li>\n",
                        id_attr,
                        if caption.is_empty() {
                            "(unnamed figure)".to_string()
                        } else {
                            escape_html(caption)
                        }
                    ));
                }
            }
            html.push_str("</ul>\n");
            html.push_str("</nav>\n");
        }

        // Phase 9: List of Tables (uses pre-collected entries)
        "Lot" => {
            html.push_str("<nav class=\"vell-lot\" role=\"lot\">\n");
            html.push_str("<h2>List of Tables</h2>\n");
            html.push_str("<ul class=\"lot-list\">\n");
            let entries = RENDER_LOT_ENTRIES.with(|e| e.borrow().clone());
            if entries.is_empty() {
                html.push_str("<li class=\"lot-placeholder\">(no tables found)</li>\n");
            } else {
                for (caption, id) in &entries {
                    let id_attr = if id.is_empty() {
                        String::new()
                    } else {
                        format!(" href=\"#{}\"", escape_html(id))
                    };
                    html.push_str(&format!(
                        "<li><a{}>{}</a></li>\n",
                        id_attr,
                        if caption.is_empty() {
                            "(unnamed table)".to_string()
                        } else {
                            escape_html(caption)
                        }
                    ));
                }
            }
            html.push_str("</ul>\n");
            html.push_str("</nav>\n");
        }

        // Phase 10: Function plot rendering
        "Plot" => {
            let fn_expr = match props.get("fn") {
                Some(PropValue::String(s)) => s.clone(),
                _ => String::from("sin(x)"),
            };
            let xmin = match props.get("xmin") {
                Some(PropValue::Number(n)) => *n,
                _ => -6.0,
            };
            let xmax = match props.get("xmax") {
                Some(PropValue::Number(n)) => *n,
                _ => 6.0,
            };
            let ymin = match props.get("ymin") {
                Some(PropValue::Number(n)) => *n,
                _ => -2.0,
            };
            let ymax = match props.get("ymax") {
                Some(PropValue::Number(n)) => *n,
                _ => 2.0,
            };
            let width = 500u32;
            let height = 250u32;
            // Render a placeholder SVG that describes the plot
            let mut svg = String::new();
            svg.push_str(&format!(
                "<svg width=\"{}px\" height=\"{}px\" viewBox=\"0 0 {} {}\" xmlns=\"http://www.w3.org/2000/svg\">\n",
                width, height, width, height
            ));
            svg.push_str(&format!(
                "<rect x=\"0\" y=\"0\" width=\"{}\" height=\"{}\" fill=\"#f8f9fa\" rx=\"4\"/>\n",
                width, height
            ));
            // Axes
            let pad = 40u32;
            let plot_w = width - 2 * pad;
            let plot_h = height - 2 * pad;
            let cx = pad + ((0.0 - xmin) / (xmax - xmin) * plot_w as f64) as u32;
            let cy = pad + ((ymax - 0.0) / (ymax - ymin) * plot_h as f64) as u32;
            // X-axis
            svg.push_str(&format!(
                "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"#a0aec0\" stroke-width=\"1\"/>\n",
                pad, cy, width - pad, cy
            ));
            // Y-axis
            svg.push_str(&format!(
                "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"#a0aec0\" stroke-width=\"1\"/>\n",
                cx, pad, cx, height - pad
            ));
            // Label
            svg.push_str(&format!(
                "<text x=\"{}\" y=\"20\" text-anchor=\"middle\" font-size=\"12\" fill=\"#2d3748\">Plot of {}</text>\n",
                width / 2, escape_html(&fn_expr)
            ));
            svg.push_str(&format!(
                "<text x=\"{}\" y=\"{}\" text-anchor=\"middle\" font-size=\"10\" fill=\"#718096\">Function plotting requires a JavaScript runtime for evaluation.</text>\n",
                width / 2, height - 8
            ));
            svg.push_str("</svg>\n");
            html.push_str(&format!("<div class=\"vell-plot\">\n{}\n</div>\n", svg));
        }

        // Default: generic directive handler
        _ => {
            html.push_str(&format!(
                "<div class=\"directive-{}\">\n",
                escape_html(name)
            ));
            for child in children {
                render_node(child, html, _depth + 1, eq_counter, thm_counter, labels);
            }
            html.push_str("</div>\n");
        }
    }
}

fn render_table_cell(
    cell: &TableCell,
    html: &mut String,
    is_header: bool,
    labels: &HashMap<String, LabelTarget>,
) {
    let tag = if is_header { "th" } else { "td" };
    let mut attrs = String::new();
    if cell.colspan > 1 {
        attrs.push_str(&format!(" colspan=\"{}\"", cell.colspan));
    }
    if cell.rowspan > 1 {
        attrs.push_str(&format!(" rowspan=\"{}\"", cell.rowspan));
    }
    if let Some(ref align) = cell.align {
        let val = match align {
            Alignment::Left => "left",
            Alignment::Center => "center",
            Alignment::Right => "right",
        };
        attrs.push_str(&format!(" align=\"{}\"", val));
    }
    html.push_str(&format!("<{}{}>", tag, attrs));
    render_inline_children(&cell.children, html, labels);
    html.push_str(&format!("</{}>\n", tag));
}

fn render_inline_children(
    children: &[InlineNode],
    html: &mut String,
    labels: &HashMap<String, LabelTarget>,
) {
    for node in children {
        render_inline(node, html, labels);
    }
}

fn render_inline(node: &InlineNode, html: &mut String, labels: &HashMap<String, LabelTarget>) {
    match node {
        InlineNode::Text { value, .. } => html.push_str(&escape_html(value)),
        InlineNode::Bold { children, .. } => {
            html.push_str("<strong>");
            render_inline_children(children, html, labels);
            html.push_str("</strong>");
        }
        InlineNode::Italic { children, .. } => {
            html.push_str("<em>");
            render_inline_children(children, html, labels);
            html.push_str("</em>");
        }
        InlineNode::Underline { children, .. } => {
            html.push_str("<u>");
            render_inline_children(children, html, labels);
            html.push_str("</u>");
        }
        InlineNode::Strikethrough { children, .. } => {
            html.push_str("<del>");
            render_inline_children(children, html, labels);
            html.push_str("</del>");
        }
        InlineNode::Code { value, .. } => {
            html.push_str("<code>");
            html.push_str(&escape_html(value));
            html.push_str("</code>");
        }
        InlineNode::Superscript { children, .. } => {
            html.push_str("<sup>");
            render_inline_children(children, html, labels);
            html.push_str("</sup>");
        }
        InlineNode::Subscript { children, .. } => {
            html.push_str("<sub>");
            render_inline_children(children, html, labels);
            html.push_str("</sub>");
        }
        InlineNode::Link {
            href,
            title,
            children,
            ..
        } => {
            let url = sanitize_url(href);
            let title_attr = title
                .as_ref()
                .map(|t| format!(" title=\"{}\"", escape_html(t)))
                .unwrap_or_default();
            html.push_str(&format!("<a href=\"{}\"{}>", url, title_attr));
            render_inline_children(children, html, labels);
            html.push_str("</a>");
        }
        InlineNode::LinkRef { id, children, .. } => {
            html.push_str(&format!(
                "<a href=\"{}\" class=\"link-ref\">",
                escape_html(id)
            ));
            render_inline_children(children, html, labels);
            html.push_str("</a>");
        }
        InlineNode::Image {
            src, alt, title, ..
        } => {
            let url = sanitize_url(src);
            let title_attr = title
                .as_ref()
                .map(|t| format!(" title=\"{}\"", escape_html(t)))
                .unwrap_or_default();
            html.push_str(&format!(
                "<img src=\"{}\" alt=\"{}\"{}>",
                url,
                escape_html(alt),
                title_attr
            ));
        }
        InlineNode::ImageRef { id, alt, .. } => {
            html.push_str(&format!(
                "<img src=\"{}\" alt=\"{}\" class=\"img-ref\">",
                sanitize_url(id),
                escape_html(alt)
            ));
        }
        InlineNode::MathInline { source, .. } => {
            let mathml = latex_to_mathml(source, false);
            html.push_str(&format!("<math display=\"inline\">{}</math>", mathml));
        }
        InlineNode::VarInterpolation { name, .. } => {
            html.push_str(&format!(
                "<span class=\"var\" data-vell-var=\"{}\">{}</span>",
                escape_html(name),
                escape_html(name)
            ));
        }
        InlineNode::InlineComponent { name, props, .. } => {
            if name == "Ref" {
                // Cross-reference directive used inline
                let label = match props.get("label") {
                    Some(PropValue::String(s)) => s.clone(),
                    _ => String::new(),
                };
                if let Some(target) = labels.get(&label) {
                    html.push_str(&format!(
                        "<a href=\"#{}\" class=\"vell-ref\">{}</a>",
                        escape_html(&target.anchor_id),
                        escape_html(&target.display_text)
                    ));
                } else {
                    html.push_str(&format!(
                        "<span class=\"unresolved-ref\">[?{}]</span>",
                        escape_html(&label)
                    ));
                }
            } else {
                let props_str = vell_core::format_props(props);
                html.push_str(&format!(
                    "<span class=\"component-{}\" data-props=\"{}\"></span>",
                    escape_html(name),
                    escape_html(&props_str)
                ));
            }
        }
        InlineNode::Citation { key, .. } => {
            html.push_str(&format!("<cite>{}</cite>", escape_html(key)));
        }
        InlineNode::FootnoteRef { marker, .. } => {
            html.push_str(&format!(
                "<sup><a href=\"#fn:{}\" id=\"fnref:{}\">{}</a></sup>",
                escape_html(marker),
                escape_html(marker),
                escape_html(marker)
            ));
        }
        InlineNode::SoftBreak { .. } => html.push_str("<br>\n"),
        InlineNode::HardBreak { .. } => html.push_str("<br>\n"),
    }
}

// ---------------------------------------------------------------------------
// Phase 10: SVG Bar Chart rendering
// ---------------------------------------------------------------------------

/// Renders a set of data points as an inline SVG bar chart.
fn render_bar_chart_svg(data: &[(String, f64)], title: &str) -> String {
    let width = 500u32;
    let height = 250u32;
    let padding_left = 60u32;
    let padding_right = 20u32;
    let padding_top = if title.is_empty() { 20u32 } else { 40u32 };
    let padding_bottom = 50u32;
    let chart_w = width.saturating_sub(padding_left + padding_right);
    let chart_h = height.saturating_sub(padding_top + padding_bottom);

    if data.is_empty() {
        return format!(
            "<svg width=\"{}px\" height=\"{}px\" viewBox=\"0 0 {} {}\" xmlns=\"http://www.w3.org/2000/svg\"></svg>",
            width, height, width, height
        );
    }

    let max_val = data.iter().map(|(_, v)| *v).fold(0.0f64, f64::max).max(1.0);
    let n = data.len() as f64;
    let bar_w = (chart_w as f64 / n * 0.7).max(8.0);
    let gap = chart_w as f64 / n * 0.3;

    let mut svg = String::new();
    svg.push_str(&format!(
        "<svg width=\"{}px\" height=\"{}px\" viewBox=\"0 0 {} {}\" xmlns=\"http://www.w3.org/2000/svg\">\n",
        width, height, width, height
    ));

    // Title
    if !title.is_empty() {
        svg.push_str(&format!(
            "<text x=\"{}\" y=\"22\" text-anchor=\"middle\" font-size=\"14\" font-weight=\"bold\" fill=\"#2d3748\">{}</text>\n",
            width / 2,
            escape_html(title)
        ));
    }

    // Y-axis
    let y_ticks = 5u32;
    svg.push_str(&format!(
        "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"#e2e8f0\" stroke-width=\"1\"/>\n",
        padding_left,
        padding_top,
        padding_left,
        padding_top + chart_h
    ));

    for i in 0..=y_ticks {
        let y = padding_top + chart_h - (chart_h as f64 * i as f64 / y_ticks as f64) as u32;
        let val = max_val * i as f64 / y_ticks as f64;
        svg.push_str(&format!(
            "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"#e2e8f0\" stroke-width=\"1\" stroke-dasharray=\"4,2\"/>\n",
            padding_left, y, padding_left + chart_w, y
        ));
        svg.push_str(&format!(
            "<text x=\"{}\" y=\"{}\" text-anchor=\"end\" font-size=\"10\" fill=\"#666\">{:.1}</text>\n",
            padding_left - 6, y + 3, val
        ));
    }

    // X-axis
    svg.push_str(&format!(
        "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"#a0aec0\" stroke-width=\"1\"/>\n",
        padding_left,
        padding_top + chart_h,
        padding_left + chart_w,
        padding_top + chart_h
    ));

    // Bars
    let colors = [
        "#3182ce", "#38a169", "#d69e2e", "#e53e3e", "#805ad5", "#319795", "#dd6b20", "#2b6cb0",
    ];
    for (i, (label, value)) in data.iter().enumerate() {
        let bar_h = if max_val > 0.0 {
            (chart_h as f64 * value / max_val) as u32
        } else {
            0
        };
        let x = padding_left + (i as f64 * (bar_w + gap) + gap / 2.0) as u32;
        let y = padding_top + chart_h - bar_h;
        let color = colors[i % colors.len()];

        svg.push_str(&format!(
            "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"{}\" rx=\"2\"/>\n",
            x, y, bar_w as u32, bar_h, color
        ));
        svg.push_str(&format!(
            "<text x=\"{}\" y=\"{}\" text-anchor=\"middle\" font-size=\"9\" fill=\"#4a5568\">{}</text>\n",
            x + bar_w as u32 / 2, padding_top + chart_h + 16, escape_html(label)
        ));
        svg.push_str(&format!(
            "<text x=\"{}\" y=\"{}\" text-anchor=\"middle\" font-size=\"9\" fill=\"#718096\">{}</text>\n",
            x + bar_w as u32 / 2, y - 4, value
        ));
    }

    svg.push_str("</svg>\n");
    svg
}

// ---------------------------------------------------------------------------
// LaTeX to MathML converter
// ---------------------------------------------------------------------------

/// Converts a LaTeX math expression to MathML markup.
fn latex_to_mathml(latex: &str, _is_block: bool) -> String {
    let mut stack: Vec<String> = Vec::new();
    let mut i = 0;
    let chars: Vec<char> = latex.chars().collect();

    while i < chars.len() {
        let ch = chars[i];
        match ch {
            '^' => {
                i += 1;
                let sup = parse_math_group_chars(&chars, &mut i);
                let base = stack.pop().unwrap_or_default();
                if base.is_empty() {
                    stack.push(format!("<msup><mrow/><mrow>{}</mrow></msup>", sup));
                } else {
                    stack.push(format!("<msup>{}{}</msup>", base, wrap_in_mrow(&sup)));
                }
            }
            '_' => {
                i += 1;
                let sub = parse_math_group_chars(&chars, &mut i);
                let base = stack.pop().unwrap_or_default();
                if base.is_empty() {
                    stack.push(format!("<msub><mrow/><mrow>{}</mrow></msub>", sub));
                } else {
                    stack.push(format!("<msub>{}{}</msub>", base, wrap_in_mrow(&sub)));
                }
            }
            '{' => {
                i += 1;
                let mut depth = 1usize;
                let mut group = String::new();
                while i < chars.len() && depth > 0 {
                    if chars[i] == '{' {
                        depth += 1;
                        if depth > 1 {
                            group.push('{');
                        }
                    } else if chars[i] == '}' {
                        depth -= 1;
                        if depth > 0 {
                            group.push('}');
                        }
                    } else {
                        group.push(chars[i]);
                    }
                    if depth > 0 {
                        i += 1;
                    }
                }
                i += 1;
                let converted = latex_to_mathml(&group, false);
                stack.push(wrap_in_mrow(&converted));
            }
            '}' => {
                i += 1;
            }
            '\\' => {
                i += 1;
                if i < chars.len() && (chars[i] == ' ' || chars[i] == '\n') {
                    i += 1;
                    continue;
                }
                let mut cmd = String::new();
                while i < chars.len() && chars[i].is_ascii_alphabetic() {
                    cmd.push(chars[i]);
                    i += 1;
                }
                if cmd.is_empty() {
                    // Check for single-char spacing shorthands: \,, \;, \:, \!
                    if i < chars.len() {
                        let sc = chars[i];
                        match sc {
                            ',' | ';' | ':' | '!' => {
                                i += 1;
                            }
                            _ => {
                                stack.push("<mtext>\\</mtext>".to_string());
                            }
                        }
                    } else {
                        stack.push("<mtext>\\</mtext>".to_string());
                    }
                } else {
                    stack.push(latex_cmd_to_mathml(&cmd, &chars, &mut i));
                }
            }
            ' ' | '\t' | '\n' | '\r' => {
                i += 1;
            }
            '~' => {
                stack.push("<mtext> </mtext>".to_string());
                i += 1;
            }
            _ => {
                if ch.is_ascii_digit() {
                    stack.push(format!("<mn>{}</mn>", ch));
                } else if ch.is_ascii_alphabetic() {
                    stack.push(format!("<mi>{}</mi>", ch));
                } else {
                    let op = match ch {
                        '+' => "+",
                        '-' => "\u{2212}",
                        '=' => "=",
                        '<' => "&lt;",
                        '>' => "&gt;",
                        '(' => "(",
                        ')' => ")",
                        '[' => "[",
                        ']' => "]",
                        ',' => ",",
                        '.' => ".",
                        '!' => "!",
                        '?' => "?",
                        '/' => "/",
                        '|' => "|",
                        '%' => "%",
                        ':' => ":",
                        ';' => ";",
                        '"' => "&quot;",
                        _ => {
                            i += 1;
                            continue;
                        }
                    };
                    stack.push(format!("<mo>{}</mo>", op));
                }
                i += 1;
            }
        }
    }

    if stack.len() == 1 {
        stack.remove(0)
    } else {
        format!("<mrow>{}</mrow>", stack.join(""))
    }
}

fn wrap_in_mrow(content: &str) -> String {
    if content.starts_with("<mrow>") && content.ends_with("</mrow>") {
        content.to_string()
    } else {
        format!("<mrow>{}</mrow>", content)
    }
}

fn parse_math_group_chars(chars: &[char], i: &mut usize) -> String {
    while *i < chars.len() && (chars[*i] == ' ' || chars[*i] == '\t') {
        *i += 1;
    }
    if *i >= chars.len() {
        return String::new();
    }
    if chars[*i] == '{' {
        *i += 1;
        let mut depth = 1usize;
        let mut content = String::new();
        while *i < chars.len() && depth > 0 {
            if chars[*i] == '{' {
                depth += 1;
                if depth > 1 {
                    content.push('{');
                }
            } else if chars[*i] == '}' {
                depth -= 1;
                if depth > 0 {
                    content.push('}');
                }
            } else {
                content.push(chars[*i]);
            }
            if depth > 0 {
                *i += 1;
            }
        }
        *i += 1;
        latex_to_mathml(&content, false)
    } else if *i < chars.len() {
        let c = chars[*i];
        *i += 1;
        if c.is_ascii_digit() {
            format!("<mn>{}</mn>", c)
        } else if c.is_ascii_alphabetic() {
            format!("<mi>{}</mi>", c)
        } else {
            let op = match c {
                '+' | '-' | '=' | '(' | ')' | '[' | ']' | ',' | '.' | '!' | '?' | '/' | '|' => {
                    c.to_string()
                }
                _ => return String::new(),
            };
            format!("<mo>{}</mo>", op)
        }
    } else {
        String::new()
    }
}

fn latex_cmd_to_mathml(cmd: &str, chars: &[char], i: &mut usize) -> String {
    match cmd {
        // Greek lowercase
        "alpha" | "beta" | "gamma" | "delta" | "epsilon" | "zeta" | "eta" | "theta" | "iota"
        | "kappa" | "lambda" | "mu" | "nu" | "xi" | "omicron" | "pi" | "rho" | "sigma" | "tau"
        | "upsilon" | "phi" | "chi" | "psi" | "omega" => format!("<mi>&{};</mi>", cmd),
        // Greek variants
        "varepsilon" => "<mi>&epsilon;</mi>".into(),
        "vartheta" => "<mi>&theta;</mi>".into(),
        "varrho" => "<mi>&rho;</mi>".into(),
        "varsigma" => "<mi>&sigmaf;</mi>".into(),
        "varphi" => "<mi>&phi;</mi>".into(),
        // Greek uppercase
        "Gamma" | "Delta" | "Theta" | "Lambda" | "Xi" | "Pi" | "Sigma" | "Phi" | "Psi"
        | "Omega" => format!("<mi>&{};</mi>", cmd),
        // Operators
        "int" => "<mo>&#x222B;</mo>".into(),
        "iint" => "<mo>&#x222C;</mo>".into(),
        "iiint" => "<mo>&#x222D;</mo>".into(),
        "sum" => "<mo>&#x2211;</mo>".into(),
        "prod" => "<mo>&#x220F;</mo>".into(),
        "coprod" => "<mo>&#x2210;</mo>".into(),
        "oint" => "<mo>&#x222E;</mo>".into(),
        "nabla" => "<mo>&#x2207;</mo>".into(),
        "partial" => "<mo>&#x2202;</mo>".into(),
        "infty" => "<mo>&#x221E;</mo>".into(),
        "times" => "<mo>&#x00D7;</mo>".into(),
        "div" => "<mo>&#x00F7;</mo>".into(),
        "pm" => "<mo>&#x00B1;</mo>".into(),
        "mp" => "<mo>&#x2213;</mo>".into(),
        "cdot" => "<mo>&#x00B7;</mo>".into(),
        "circ" => "<mo>&#x2218;</mo>".into(),
        "ast" => "<mo>&#x2217;</mo>".into(),
        "langle" => "<mo>&#x27E8;</mo>".into(),
        "rangle" => "<mo>&#x27E9;</mo>".into(),
        "lvert" => "<mo>|</mo>".into(),
        "rvert" => "<mo>|</mo>".into(),
        "star" => "<mo>&#x22C6;</mo>".into(),
        "otimes" => "<mo>&#x2297;</mo>".into(),
        "oplus" => "<mo>&#x2295;</mo>".into(),
        "ominus" => "<mo>&#x2296;</mo>".into(),
        "oslash" => "<mo>&#x2298;</mo>".into(),
        "odot" => "<mo>&#x2299;</mo>".into(),
        "cdots" => "<mo>&#x22EF;</mo>".into(),
        "ldots" => "<mo>&#x2026;</mo>".into(),
        "vdots" => "<mo>&#x22EE;</mo>".into(),
        "ddots" => "<mo>&#x22F1;</mo>".into(),
        // Relations
        "equiv" => "<mo>&#x2261;</mo>".into(),
        "approx" => "<mo>&#x2248;</mo>".into(),
        "sim" => "<mo>&#x223C;</mo>".into(),
        "simeq" => "<mo>&#x2243;</mo>".into(),
        "cong" => "<mo>&#x2245;</mo>".into(),
        "propto" => "<mo>&#x221D;</mo>".into(),
        "neq" => "<mo>&#x2260;</mo>".into(),
        "le" => "<mo>&#x2264;</mo>".into(),
        "ge" => "<mo>&#x2265;</mo>".into(),
        "ll" => "<mo>&#x226A;</mo>".into(),
        "gg" => "<mo>&#x226B;</mo>".into(),
        "prec" => "<mo>&#x227A;</mo>".into(),
        "succ" => "<mo>&#x227B;</mo>".into(),
        "preceq" => "<mo>&#x227C;</mo>".into(),
        "succeq" => "<mo>&#x227D;</mo>".into(),
        // Set symbols
        "subset" => "<mo>&#x2282;</mo>".into(),
        "supset" => "<mo>&#x2283;</mo>".into(),
        "subseteq" => "<mo>&#x2286;</mo>".into(),
        "supseteq" => "<mo>&#x2287;</mo>".into(),
        "cap" => "<mo>&#x2229;</mo>".into(),
        "cup" => "<mo>&#x222A;</mo>".into(),
        "setminus" => "<mo>&#x2216;</mo>".into(),
        "emptyset" => "<mo>&#x2205;</mo>".into(),
        "varnothing" => "<mo>&#x2205;</mo>".into(),
        "in" => "<mo>&#x2208;</mo>".into(),
        "notin" => "<mo>&#x2209;</mo>".into(),
        "ni" => "<mo>&#x220B;</mo>".into(),
        // Arrows
        "rightarrow" => "<mo>&#x2192;</mo>".into(),
        "leftarrow" => "<mo>&#x2190;</mo>".into(),
        "Rightarrow" => "<mo>&#x21D2;</mo>".into(),
        "Leftarrow" => "<mo>&#x21D0;</mo>".into(),
        "leftrightarrow" => "<mo>&#x2194;</mo>".into(),
        "Leftrightarrow" => "<mo>&#x21D4;</mo>".into(),
        "uparrow" => "<mo>&#x2191;</mo>".into(),
        "downarrow" => "<mo>&#x2193;</mo>".into(),
        "mapsto" => "<mo>&#x21A6;</mo>".into(),
        "implies" => "<mo>&#x21D2;</mo>".into(),
        "iff" => "<mo>&#x21D4;</mo>".into(),
        // Logical
        "forall" => "<mo>&#x2200;</mo>".into(),
        "exists" => "<mo>&#x2203;</mo>".into(),
        "nexists" => "<mo>&#x2204;</mo>".into(),
        "neg" => "<mo>&#x00AC;</mo>".into(),
        "lor" => "<mo>&#x2228;</mo>".into(),
        "land" => "<mo>&#x2227;</mo>".into(),
        "top" => "<mo>&#x22A4;</mo>".into(),
        "bot" => "<mo>&#x22A5;</mo>".into(),
        "vdash" => "<mo>&#x22A2;</mo>".into(),
        "mid" => "<mo>&#x2223;</mo>".into(),
        "models" => "<mo>&#x22A7;</mo>".into(),
        // Functions
        "sin" | "cos" | "tan" | "cot" | "sec" | "csc" | "log" | "ln" | "sinh" | "cosh" | "tanh"
        | "arcsin" | "arccos" | "arctan" | "det" | "dim" | "lim" | "max" | "min" | "sup"
        | "inf" | "exp" | "deg" | "arg" | "ker" | "hom" | "gcd" | "Pr" => {
            format!("<mi>{}</mi>", cmd)
        }
        // Fractions
        "frac" => {
            let num = parse_math_group_chars(chars, i);
            let den = parse_math_group_chars(chars, i);
            format!(
                "<mfrac>{}{}</mfrac>",
                wrap_in_mrow(&num),
                wrap_in_mrow(&den)
            )
        }
        // Roots
        "sqrt" => {
            let content = parse_math_group_chars(chars, i);
            format!("<msqrt>{}</msqrt>", wrap_in_mrow(&content))
        }
        // Accents
        "hat" => {
            let content = parse_math_group_chars(chars, i);
            format!("<mover>{}{}</mover>", wrap_in_mrow(&content), "<mo>^</mo>")
        }
        "tilde" => {
            let content = parse_math_group_chars(chars, i);
            format!("<mover>{}{}</mover>", wrap_in_mrow(&content), "<mo>~</mo>")
        }
        "bar" => {
            let content = parse_math_group_chars(chars, i);
            format!(
                "<mover>{}{}</mover>",
                wrap_in_mrow(&content),
                "<mo>&#x00AF;</mo>"
            )
        }
        "dot" => {
            let content = parse_math_group_chars(chars, i);
            format!("<mover>{}{}</mover>", wrap_in_mrow(&content), "<mo>.</mo>")
        }
        "ddot" => {
            let content = parse_math_group_chars(chars, i);
            format!("<mover>{}<mo>..</mo></mover>", wrap_in_mrow(&content))
        }
        "vec" => {
            let content = parse_math_group_chars(chars, i);
            format!(
                "<mover>{}{}</mover>",
                wrap_in_mrow(&content),
                "<mo>&#x2192;</mo>"
            )
        }
        // Blackboard bold
        "mathbb" => {
            let content = parse_math_group_chars(chars, i);
            // Strip HTML tags to extract the content letter
            let text: String = content
                .replace("<mi>", "")
                .replace("</mi>", "")
                .replace("<mn>", "")
                .replace("</mn>", "")
                .replace("<mo>", "")
                .replace("</mo>", "")
                .replace("<mrow>", "")
                .replace("</mrow>", "")
                .chars()
                .filter(|c| c.is_ascii_alphabetic())
                .collect();
            let letter = text.chars().next().unwrap_or('N').to_string();
            format!("<mi mathvariant=\"double-struck\">{}</mi>", letter)
        }
        // Calligraphic / script
        "mathcal" => {
            let content = parse_math_group_chars(chars, i);
            // Extract letter and wrap
            let text: String = content
                .replace("<mi>", "")
                .replace("</mi>", "")
                .replace("<mrow>", "")
                .replace("</mrow>", "")
                .chars()
                .filter(|c| c.is_ascii_alphabetic())
                .collect();
            let letter = text.chars().next().unwrap_or('A').to_string();
            format!("<mi mathvariant=\"script\">{}</mi>", letter)
        }
        // Roman/upright text
        "mathrm" => {
            let content = parse_math_group_chars(chars, i);
            format!("<mi mathvariant=\"normal\">{}</mi>", content)
        }
        // Bold text
        "mathbf" => {
            let content = parse_math_group_chars(chars, i);
            format!("<mi mathvariant=\"bold\">{}</mi>", content)
        }
        // Italic text
        "mathit" => {
            let content = parse_math_group_chars(chars, i);
            format!("<mi mathvariant=\"italic\">{}</mi>", content)
        }
        // Sans-serif
        "mathsf" => {
            let content = parse_math_group_chars(chars, i);
            format!("<mi mathvariant=\"sans-serif\">{}</mi>", content)
        }
        // Typewriter
        "mathtt" => {
            let content = parse_math_group_chars(chars, i);
            format!("<mi mathvariant=\"monospace\">{}</mi>", content)
        }
        // Binomial coefficient
        "binom" => {
            let num = parse_math_group_chars(chars, i);
            let den = parse_math_group_chars(chars, i);
            format!(
                "<mrow><mo>(</mo><mfrac linethickness=\"0\">{}{}</mfrac><mo>)</mo></mrow>",
                wrap_in_mrow(&num),
                wrap_in_mrow(&den)
            )
        }
        // Named operator
        "operatorname" => {
            let content = parse_math_group_chars(chars, i);
            format!("<mi>{}</mi>", content)
        }
        // Physics: bra-ket notation
        "bra" => {
            let content = parse_math_group_chars(chars, i);
            format!(
                "<mrow><mo>&#x27E8;</mo>{}<mo>|</mo></mrow>",
                wrap_in_mrow(&content)
            )
        }
        "ket" => {
            let content = parse_math_group_chars(chars, i);
            format!(
                "<mrow><mo>|</mo>{}<mo>&#x27E9;</mo></mrow>",
                wrap_in_mrow(&content)
            )
        }
        "braket" => {
            let content = parse_math_group_chars(chars, i);
            // Split on '|' if present, otherwise wrap the whole content
            if let Some(pipe_pos) = content.find('|') {
                let left = content.get(..pipe_pos).unwrap_or_default();
                let right = content.get(pipe_pos + 1..).unwrap_or_default();
                let left_conv = latex_to_mathml(left.trim(), false);
                let right_conv = latex_to_mathml(right.trim(), false);
                format!(
                    "<mrow><mo>&#x27E8;</mo>{}<mo>|</mo>{}<mo>&#x27E9;</mo></mrow>",
                    wrap_in_mrow(&left_conv),
                    wrap_in_mrow(&right_conv)
                )
            } else {
                format!(
                    "<mrow><mo>&#x27E8;</mo>{}<mo>&#x27E9;</mo></mrow>",
                    wrap_in_mrow(&content)
                )
            }
        }
        // Text in math
        "text" => {
            let content = parse_math_group_chars(chars, i);
            format!("<mtext>{}</mtext>", content)
        }
        // Spacing (long-form names)
        "quad" | "qquad" | "thinspace" | "medspace" | "thickspace" | "negthinspace"
        | "negmedspace" | "negthickspace" => String::new(),
        " " => "<mtext> </mtext>".into(),
        _ => format!("<mtext>\\{}</mtext>", cmd),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const SPEC_SOURCES: &[&str] = &[
        include_str!("../../../spec/examples/01-basic.vl"),
        include_str!("../../../spec/examples/02-math.vl"),
        include_str!("../../../spec/examples/03-tables.vl"),
        include_str!("../../../spec/examples/04-interactive.vl"),
        include_str!("../../../spec/examples/05-extensions.vl"),
        include_str!("../../../spec/examples/06-full-document.vl"),
        include_str!("../../../spec/examples/07-math-advanced.vl"),
        include_str!("../../../spec/examples/08-theorems-equations.vl"),
        include_str!("../../../spec/examples/09-diagrams.vl"),
        include_str!("../../../spec/examples/10-multi-format.vl"),
        include_str!("../../../spec/examples/11-extensions.vl"),
        include_str!("../../../spec/examples/12-new-features.vl"),
    ];

    #[test]
    fn render_spec_example_12_new_features() {
        let source = SPEC_SOURCES[11];
        let doc = parse_document(source)
            .unwrap_or_else(|e| panic!("spec example 12 failed to parse: {:?}", e));
        let html = render_document(&doc);
        assert!(!html.is_empty(), "spec example 12 produced empty HTML");
        assert!(
            html.contains("vell-chem"),
            "should contain chemical equation rendering"
        );
        assert!(html.contains("vell-toc"), "should contain TOC rendering");
        assert!(html.contains("vell-lof"), "should contain LOF rendering");
        assert!(html.contains("vell-lot"), "should contain LOT rendering");
        assert!(html.contains("vell-plot"), "should contain plot rendering");
        assert!(
            html.contains("vell-diagram"),
            "should contain diagram rendering"
        );
        assert!(html.contains("Phase 8-10"), "should contain page title");
    }

    #[test]
    fn render_all_spec_examples() {
        for (i, source) in SPEC_SOURCES.iter().enumerate() {
            let doc = parse_document(source)
                .unwrap_or_else(|e| panic!("spec example {} failed to parse: {:?}", i + 1, e));
            let html = render_document(&doc);
            assert!(
                !html.is_empty(),
                "spec example {} produced empty HTML",
                i + 1
            );
            assert!(
                html.starts_with("<!doctype html>"),
                "spec example {} missing doctype",
                i + 1
            );
            assert!(
                html.contains("</html>"),
                "spec example {} missing closing html",
                i + 1
            );
        }
    }

    #[test]
    fn render_document_contains_title() {
        let doc = parse_document("= My Title\n\nBody.\n").unwrap();
        let html = render_document(&doc);
        assert!(html.contains("<title>My Title</title>"));
    }

    #[test]
    fn render_math_block_produces_math_element() {
        let doc = parse_document("$$\nx^2\n$$\n").unwrap();
        let html = render_document(&doc);
        assert!(html.contains("<math display=\"block\">"));
    }

    #[test]
    fn render_math_inline_produces_math_element() {
        let doc = parse_document("Text with $x^2$ here.\n").unwrap();
        let html = render_document(&doc);
        assert!(html.contains("<math display=\"inline\">"));
    }

    #[test]
    fn render_footnotes_section_appears_at_end() {
        let doc = parse_document("Content[^one].\n\n[^one]: Footnote body.\n").unwrap();
        let html = render_document(&doc);
        assert!(html.contains("<section class=\"footnotes\">"));
        assert!(html.contains(">Footnote body.<"));
    }

    #[test]
    fn render_heading_gets_id() {
        let doc = parse_document("= Hello World\n").unwrap();
        let html = render_document(&doc);
        assert!(html.contains("id=\"hello-world\""));
    }

    #[test]
    fn render_empty_document() {
        let doc = parse_document("").unwrap();
        let html = render_document(&doc);
        assert!(html.contains("<!doctype html>"));
    }

    #[test]
    fn render_with_default_title() {
        let doc = parse_document("Just text.\n").unwrap();
        let html = render_document(&doc);
        assert!(html.contains("<title>Vell Document</title>"));
    }

    #[test]
    fn render_code_block() {
        let doc = parse_document("```rust\nfn main() {}\n```\n").unwrap();
        let html = render_document(&doc);
        assert!(html.contains("class=\"language-rust\""));
        assert!(html.contains("fn main()"));
    }

    #[test]
    fn render_table() {
        let doc = parse_document("| A | B |\n|---|---|\n| 1 | 2 |\n").unwrap();
        let html = render_document(&doc);
        assert!(html.contains("<table>"));
        assert!(html.contains("<th>"));
        assert!(html.contains("<td>"));
    }

    #[test]
    fn render_list() {
        let doc = parse_document("- Item 1\n- Item 2\n").unwrap();
        let html = render_document(&doc);
        assert!(html.contains("<ul>"));
        assert!(html.contains("<li>"));
    }

    #[test]
    fn render_directive() {
        let doc = parse_document("@[Figure](src=\"img.png\" alt=\"A figure\")\n").unwrap();
        let html = render_document(&doc);
        assert!(html.contains("class=\"directive-Figure\""));
    }

    #[test]
    fn render_blockquote() {
        let doc = parse_document("> Quoted.\n").unwrap();
        let html = render_document(&doc);
        assert!(html.contains("<blockquote>"));
    }

    #[test]
    fn render_admonition() {
        let doc = parse_document("> [!WARNING]\n> Careful!\n").unwrap();
        let html = render_document(&doc);
        assert!(html.contains("class=\"admonition WARNING\""));
    }

    #[test]
    fn cmd_fmt_with_temp_valid_file() {
        let source = "= Title\n\nBody.\n";
        let tmp = std::env::temp_dir().join("vell_test_fmt_ok.vl");
        std::fs::write(&tmp, source).unwrap();
        let result = cmd_fmt(&Some(tmp.clone()), true);
        assert!(result.is_ok(), "well-formed file should pass --check");
        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn cmd_fmt_check_passes() {
        // Create a temp file and test
        let source = "= Title\n\nBody.\n";
        let tmp = std::env::temp_dir().join("vell_test_fmt.vl");
        std::fs::write(&tmp, source).unwrap();
        let result = cmd_fmt(&Some(tmp.clone()), true);
        assert!(result.is_ok());
        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn cmd_format_is_idempotent_on_valid_source() {
        for source in SPEC_SOURCES {
            let doc = parse_document(source).unwrap();
            let formatted = vell_fmt::format(&doc);
            let doc2 = parse_document(&formatted).unwrap();
            let formatted2 = vell_fmt::format(&doc2);
            assert_eq!(
                formatted, formatted2,
                "formatter is not idempotent on spec example"
            );
        }
    }

    #[test]
    fn check_sanitize_url_safe() {
        assert_eq!(sanitize_url("https://example.com"), "https://example.com");
        assert_eq!(sanitize_url("http://example.com"), "http://example.com");
        assert_eq!(
            sanitize_url("mailto:user@example.com"),
            "mailto:user@example.com"
        );
        assert_eq!(sanitize_url("/relative/path"), "/relative/path");
    }

    #[test]
    fn check_sanitize_url_rejects_unsafe() {
        assert_eq!(sanitize_url("javascript:alert(1)"), "");
        assert_eq!(sanitize_url("data:text/html,<script>"), "");
    }

    #[test]
    fn check_escape_html() {
        assert_eq!(escape_html("Hello"), "Hello");
        assert_eq!(escape_html("<tag>"), "&lt;tag&gt;");
        assert_eq!(escape_html("a&b"), "a&amp;b");
        assert_eq!(escape_html("\"quote\""), "&quot;quote&quot;");
    }

    // -----------------------------------------------------------------------
    // Phase 8 snapshot tests
    // -----------------------------------------------------------------------

    #[test]
    fn render_equation_produces_numbered_div() {
        let doc = parse_document("@[Equation](source=\"E = mc^2\")\n").unwrap();
        let html = render_document(&doc);
        assert!(
            html.contains("class=\"vell-equation\""),
            "should have vell-equation class"
        );
        assert!(
            html.contains("data-number=\"1\""),
            "should have data-number=1"
        );
        assert!(
            html.contains("class=\"eq-number\""),
            "should have eq-number class"
        );
        assert!(
            html.contains("(1)"),
            "should display (1) as equation number"
        );
        assert!(html.contains("<math"), "should contain MathML");
    }

    #[test]
    fn render_equation_increments_counter() {
        let src = "@[Equation](source=\"a = b\")\n\n@[Equation](source=\"c = d\")\n";
        let doc = parse_document(src).unwrap();
        let html = render_document(&doc);
        assert!(html.contains("data-number=\"1\""), "first eq should be 1");
        assert!(html.contains("data-number=\"2\""), "second eq should be 2");
        assert!(html.contains("(1)"), "first eq number tag");
        assert!(html.contains("(2)"), "second eq number tag");
    }

    #[test]
    fn render_equation_with_label() {
        let doc = parse_document("@[Equation](source=\"E = mc^2\" label=e:mass-energy)\n").unwrap();
        let html = render_document(&doc);
        assert!(
            html.contains("data-label=\"e:mass-energy\""),
            "should have label attribute"
        );
    }

    #[test]
    fn render_theorem_produces_styled_block() {
        let doc = parse_document("@[Theorem](name=\"Pythagoras\") {\n  Body text.\n}\n").unwrap();
        let html = render_document(&doc);
        assert!(
            html.contains("class=\"vell-theorem vell-theorem\""),
            "should have theorem class"
        );
        assert!(
            html.contains("class=\"theorem-label\""),
            "should have label"
        );
        assert!(html.contains("Theorem 1"), "should have auto-numbered");
        assert!(html.contains("(Pythagoras)"), "should have name");
        assert!(html.contains("class=\"theorem-body\""), "should have body");
        assert!(html.contains("Body text."), "should contain body text");
    }

    #[test]
    fn render_theorem_increments_counter() {
        let src =
            "@[Theorem](name=\"A\") {\n  First.\n}\n\n@[Theorem](name=\"B\") {\n  Second.\n}\n";
        let doc = parse_document(src).unwrap();
        let html = render_document(&doc);
        assert!(
            html.find("Theorem 1").is_some(),
            "first theorem should be 1"
        );
        assert!(
            html.find("Theorem 2").is_some(),
            "second theorem should be 2"
        );
    }

    #[test]
    fn render_proof_has_no_number() {
        let doc = parse_document("@[Proof] {\n  A proof.\n}\n").unwrap();
        let html = render_document(&doc);
        assert!(
            html.contains("class=\"vell-theorem vell-proof\""),
            "should have proof class"
        );
        assert!(html.contains(">Proof</div>"), "should not have a number");
    }

    #[test]
    fn render_math_env_produces_mathml() {
        let doc = parse_document("@[Align](source=\"a &= b\\\\ c &= d\")\n").unwrap();
        let html = render_document(&doc);
        assert!(
            html.contains("class=\"vell-math-env vell-align\""),
            "should have math-env class"
        );
        assert!(html.contains("<math"), "should contain MathML");
    }

    #[test]
    fn render_css_is_included() {
        let doc = parse_document("Hello.\n").unwrap();
        let html = render_document(&doc);
        assert!(
            html.contains("vell-equation"),
            "CSS should include vell-equation"
        );
        assert!(
            html.contains("vell-theorem"),
            "CSS should include vell-theorem"
        );
        assert!(
            html.contains("vell-math-env"),
            "CSS should include vell-math-env"
        );
        assert!(html.contains("admonition"), "CSS should include admonition");
    }

    // -----------------------------------------------------------------------
    // End-to-end snapshot: spec/examples/08-theorems-equations.vl
    // -----------------------------------------------------------------------

    #[test]
    fn render_spec_example_08_contains_all_equations() {
        let source = SPEC_SOURCES[7];
        let doc = parse_document(source).unwrap();
        let html = render_document(&doc);

        // All 6 equations should be numbered 1 through 6
        for n in 1..=6 {
            assert!(
                html.contains(&format!("data-number=\"{}\"", n)),
                "equation {} should have data-number={}",
                n,
                n
            );
            assert!(
                html.contains(&format!("({})", n)),
                "equation {} should display ({}) in eq-number",
                n,
                n
            );
        }

        // First equation has a label
        assert!(
            html.contains("data-label=\"e:mass-energy\""),
            "eq 1 should have data-label"
        );
        assert!(
            html.contains("data-label=\"e:pythagoras\""),
            "eq 3 should have data-label"
        );

        // All equations get the vell-equation wrapper class
        assert_eq!(
            html.matches("class=\"vell-equation\"").count(),
            6,
            "all 6 equations should have vell-equation class"
        );
    }

    #[test]
    fn render_spec_example_08_contains_all_theorems() {
        let source = SPEC_SOURCES[7];
        let doc = parse_document(source).unwrap();
        let html = render_document(&doc);

        // Numbered theorems
        assert!(html.contains("Theorem 1"), "first Theorem should be 1");
        assert!(html.contains("(Pythagoras)"), "Theorem 1 should have name");
        assert!(html.contains("Theorem 2"), "second Theorem should be 2");
        assert!(
            html.contains("(Fundamental Theorem of Calculus)"),
            "Theorem 2 should have name"
        );
        assert!(html.contains("Lemma 1"), "Lemma should be 1");
        assert!(
            html.contains("(Triangle Inequality)"),
            "Lemma 1 should have name"
        );
        assert!(html.contains("Corollary 1"), "Corollary should be 1");
        assert!(
            html.contains("(Reverse Triangle Inequality)"),
            "Corollary 1 should have name"
        );
        assert!(html.contains("Definition 1"), "Definition should be 1");
        assert!(
            html.contains("(Prime Number)"),
            "Definition 1 should have name"
        );
        assert!(html.contains("Axiom 1"), "Axiom should be 1");
        assert!(
            html.contains("(Euclid's Fifth Postulate)"),
            "Axiom 1 should have name"
        );
        assert!(html.contains("Conjecture 1"), "Conjecture should be 1");
        assert!(
            html.contains("(Goldbach's Conjecture)"),
            "Conjecture 1 should have name"
        );
        assert!(html.contains("Proposition 1"), "Proposition should be 1");
        assert!(
            html.contains("(Sum of First n Naturals)"),
            "Proposition 1 should have name"
        );

        // Non-numbered theorems
        assert!(html.contains(">Proof</div>"), "Proof should have no number");
        assert!(
            html.contains(">Remark</div>"),
            "Remark should have no number"
        );
        assert!(
            html.contains(">Example</div>"),
            "Example should have no number"
        );
    }

    #[test]
    fn render_spec_example_08_contains_css_classes() {
        let source = SPEC_SOURCES[7];
        let doc = parse_document(source).unwrap();
        let html = render_document(&doc);

        // CSS classes for theorem types
        assert!(
            html.contains("vell-theorem vell-theorem"),
            "should have vell-theorem class"
        );
        assert!(
            html.contains("vell-theorem vell-proof"),
            "should have vell-proof class"
        );
        assert!(
            html.contains("vell-theorem vell-lemma"),
            "should have vell-lemma class"
        );
        assert!(
            html.contains("vell-theorem vell-corollary"),
            "should have vell-corollary class"
        );
        assert!(
            html.contains("vell-theorem vell-definition"),
            "should have vell-definition class"
        );
        assert!(
            html.contains("vell-theorem vell-remark"),
            "should have vell-remark class"
        );
        assert!(
            html.contains("vell-theorem vell-example"),
            "should have vell-example class"
        );
        assert!(
            html.contains("vell-theorem vell-axiom"),
            "should have vell-axiom class"
        );
        assert!(
            html.contains("vell-theorem vell-conjecture"),
            "should have vell-conjecture class"
        );
        assert!(
            html.contains("vell-theorem vell-proposition"),
            "should have vell-proposition class"
        );

        // Each theorem has a label and body
        let theorem_count = html.matches("class=\"theorem-label\"").count();
        assert_eq!(theorem_count, 12, "should have 12 theorem labels");
        let body_count = html.matches("class=\"theorem-body\"").count();
        assert_eq!(body_count, 12, "should have 12 theorem bodies");
    }

    #[test]
    fn render_spec_example_08_contains_math_environments() {
        let source = SPEC_SOURCES[7];
        let doc = parse_document(source).unwrap();
        let html = render_document(&doc);

        // Math environment CSS classes
        assert!(
            html.contains("vell-math-env vell-align"),
            "should have vell-align class"
        );
        assert!(
            html.contains("vell-math-env vell-pmatrix"),
            "should have vell-pmatrix class"
        );
        assert!(
            html.contains("vell-math-env vell-cases"),
            "should have vell-cases class"
        );

        // Align description text rendered from children
        assert!(
            html.contains("multi-line equations"),
            "Align children should render"
        );
        assert!(
            html.contains("parentheses delimiters"),
            "PMatrix children should render"
        );
        assert!(
            html.contains("piecewise function"),
            "Cases children should render"
        );
    }

    #[test]
    fn render_spec_example_08_contains_meta_and_heading() {
        let source = SPEC_SOURCES[7];
        let doc = parse_document(source).unwrap();
        let html = render_document(&doc);

        // Document title from Meta
        assert!(html.contains("<title>Phase 8: Professional Math</title>"));

        // Heading structure: H1 for title, H2 for sections
        assert!(html.contains("<h1 id=\"professional-math-in-vell\">"));
        assert!(html.contains("<h2 id=\"numbered-equations\">"));
        assert!(html.contains("<h2 id=\"theorem-environments\">"));
        assert!(html.contains("<h2 id=\"align-environment\">"));
        assert!(html.contains("<h2 id=\"matrix-environment\">"));
        assert!(html.contains("<h2 id=\"cases-environment\">"));
        assert!(html.contains("<h2 id=\"combined-usage\">"));

        // Admonition (NOTE)
        assert!(
            html.contains("class=\"admonition NOTE\""),
            "should have NOTE admonition"
        );
        assert!(
            html.contains("Cross-references resolve labels"),
            "should contain NOTE body"
        );
    }

    // -----------------------------------------------------------------------
    // Phase 9: Cross-reference tests
    // -----------------------------------------------------------------------

    #[test]
    fn render_ref_resolves_equation_label() {
        let doc = parse_document(
            "@[Equation](source=\"E = mc^2\" label=e:mass-energy)\n\nSee @[Ref](label=e:mass-energy).\n"
        ).unwrap();
        let html = render_document(&doc);
        // The ref should resolve to (1) with a link
        assert!(
            html.contains("class=\"vell-ref\""),
            "should have vell-ref class"
        );
        assert!(
            html.contains("href=\"#eq-e:mass-energy\""),
            "should link to equation anchor"
        );
        assert!(
            html.contains(">(1)<"),
            "should display (1) as resolved text"
        );
        // The equation should have an id for anchoring
        assert!(
            html.contains("id=\"eq-e:mass-energy\""),
            "equation should have id for anchor"
        );
    }

    #[test]
    fn render_ref_resolves_theorem_label() {
        let doc = parse_document(
            "@[Theorem](name=\"Test\" label=thm:test) {\n  Body.\n}\n\nSee @[Ref](label=thm:test).\n"
        ).unwrap();
        let html = render_document(&doc);
        assert!(
            html.contains("class=\"vell-ref\""),
            "should have vell-ref class"
        );
        assert!(
            html.contains("href=\"#thm-thm:test\""),
            "should link to theorem anchor"
        );
        assert!(
            html.contains(">Theorem 1 (Test)<"),
            "should display 'Theorem 1 (Test)' as resolved text"
        );
        assert!(
            html.contains("id=\"thm-thm:test\""),
            "theorem should have id for anchor"
        );
    }

    #[test]
    fn render_ref_shows_unresolved_when_label_missing() {
        let doc = parse_document("See @[Ref](label=nonexistent).\n").unwrap();
        let html = render_document(&doc);
        assert!(
            html.contains("class=\"unresolved-ref\""),
            "should have unresolved-ref class"
        );
        assert!(
            html.contains("[?nonexistent]"),
            "should show [?label] for unresolved ref"
        );
    }

    #[test]
    fn render_ref_multiple_refs_to_different_labels() {
        let doc = parse_document(
            "@[Equation](source=\"a = b\" label=eq:first)\n\n@[Equation](source=\"c = d\" label=eq:second)\n\nEq @[Ref](label=eq:first) and @[Ref](label=eq:second).\n"
        ).unwrap();
        let html = render_document(&doc);
        assert!(html.contains(">(1)<"), "first ref should show (1)");
        assert!(html.contains(">(2)<"), "second ref should show (2)");
        assert!(html.contains("href=\"#eq-eq:first\""), "first ref href");
        assert!(html.contains("href=\"#eq-eq:second\""), "second ref href");
    }

    #[test]
    fn render_spec_example_08_contains_cross_refs() {
        let source = SPEC_SOURCES[7];
        let doc = parse_document(source).unwrap();
        let html = render_document(&doc);
        // Should contain resolved refs to e:mass-energy and e:pythagoras
        assert!(
            html.contains("href=\"#eq-e:mass-energy\""),
            "ref to e:mass-energy"
        );
        assert!(
            html.contains("href=\"#eq-e:pythagoras\""),
            "ref to e:pythagoras"
        );
        assert!(
            html.contains("href=\"#thm-thm:pythagoras\""),
            "ref to thm:pythagoras"
        );
        assert!(html.contains("href=\"#thm-thm:ftc\""), "ref to thm:ftc");
    }

    // -----------------------------------------------------------------------
    // Phase 10: Diagram & Chart tests
    // -----------------------------------------------------------------------

    #[test]
    fn render_spec_example_08_equations_inside_theorems() {
        let source = SPEC_SOURCES[7];
        let doc = parse_document(source).unwrap();
        let html = render_document(&doc);

        // Equations inside Theorem 2 (Fundamental Theorem of Calculus): eq 4 and eq 5
        assert!(
            html.contains("data-number=\"4\""),
            "eq inside Theorem 2 should be 4"
        );
        assert!(
            html.contains("data-number=\"5\""),
            "eq inside Theorem 2 should be 5"
        );

        // Equation inside Proof 2 (after Theorem 2): eq 6
        assert!(
            html.contains("data-number=\"6\""),
            "eq inside Proof 2 should be 6"
        );
    }

    #[test]
    fn render_diagram_mermaid_produces_mermaid_div() {
        let doc = parse_document(
            "@[Diagram](type=mermaid caption=\"A sequence diagram\") {\n  sequenceDiagram\n    A->>B: Hello\n}\n"
        ).unwrap();
        let html = render_document(&doc);
        assert!(
            html.contains("class=\"vell-diagram\""),
            "should have vell-diagram class"
        );
        assert!(
            html.contains("data-type=\"mermaid\""),
            "should have data-type=mermaid"
        );
        assert!(
            html.contains("class=\"mermaid\""),
            "should have mermaid class for Mermaid.js"
        );
        assert!(
            html.contains("sequenceDiagram"),
            "should contain diagram source"
        );
        assert!(
            html.contains("A sequence diagram"),
            "should contain caption"
        );
    }

    #[test]
    fn render_diagram_ascii_produces_pre_block() {
        let doc = parse_document("@[Diagram](type=ascii) {\n  #######\n  # Node #\n  #######\n}\n")
            .unwrap();
        let html = render_document(&doc);
        assert!(
            html.contains("class=\"vell-diagram\""),
            "should have vell-diagram class"
        );
        assert!(
            html.contains("data-type=\"ascii\""),
            "should have data-type=ascii"
        );
        assert!(
            html.contains("<pre>"),
            "should have pre block for ASCII art"
        );
        assert!(html.contains("#######"), "should contain ASCII art content");
    }

    #[test]
    fn render_diagram_general_defaults_to_pre() {
        let doc = parse_document("@[Diagram] {\n  Some diagram content\n}\n").unwrap();
        let html = render_document(&doc);
        assert!(
            html.contains("data-type=\"general\""),
            "should default to general type"
        );
        assert!(html.contains("<pre>"), "should use pre block");
    }

    #[test]
    fn render_bar_chart_produces_svg() {
        let doc = parse_document(
            "@[Chart](type=bar title=\"Sales\") {\n  Q1, 10\n  Q2, 25\n  Q3, 15\n}\n",
        )
        .unwrap();
        let html = render_document(&doc);
        assert!(html.contains("vell-chart"), "should have vell-chart class");
        assert!(html.contains("<svg"), "should contain SVG element");
        assert!(html.contains("viewBox"), "should have viewBox attribute");
        assert!(html.contains("Sales"), "should contain chart title");
    }

    #[test]
    fn render_chart_empty_data_falls_back() {
        let doc = parse_document("@[Chart](type=bar title=\"Empty\") {}\n").unwrap();
        let html = render_document(&doc);
        // With no data, it won't crash — may render empty div or fallback
        assert!(html.contains("vell-chart"), "should have vell-chart class");
    }

    #[test]
    fn render_spec_example_09_parses_cleanly() {
        let source = SPEC_SOURCES[8]; // Index 8 = 09-diagrams.vl
        let doc = parse_document(source).unwrap();
        let html = render_document(&doc);
        assert!(!html.is_empty(), "spec example 09 produced empty HTML");
        assert!(
            html.contains("vell-diagram"),
            "should contain diagram rendering"
        );
        assert!(
            html.contains("vell-chart"),
            "should contain chart rendering"
        );
        assert!(
            html.contains("Phase 10: Native Diagrams"),
            "should contain page title"
        );
    }

    // -----------------------------------------------------------------------
    // Phase 11: Interactive test
    // -----------------------------------------------------------------------

    #[test]
    fn render_input_produces_text_input() {
        let doc = parse_document(
            "@[Input](type=text bind=name label=\"Name\" placeholder=\"Enter name\")\n",
        )
        .unwrap();
        let html = render_document(&doc);
        assert!(
            html.contains("input type=\"text\""),
            "should have text input"
        );
        assert!(
            html.contains("data-bind=\"name\""),
            "should have data-bind attribute"
        );
        assert!(
            html.contains("placeholder=\"Enter name\""),
            "should have placeholder"
        );
        assert!(html.contains("Name"), "should have label");
    }

    #[test]
    fn render_select_produces_dropdown() {
        let doc =
            parse_document("@[Select](bind=opt label=\"Pick\") {\n  A\n  B\n  C\n}\n").unwrap();
        let html = render_document(&doc);
        assert!(html.contains("<select"), "should have select element");
        assert!(html.contains("data-bind=\"opt\""), "should have data-bind");
        assert!(html.contains("<option>A</option>"), "should have options");
        assert!(html.contains("Pick"), "should have label");
    }

    #[test]
    fn render_checkbox_produces_checkbox_input() {
        let doc = parse_document("@[Checkbox](bind=dark label=\"Enable dark mode\")\n").unwrap();
        let html = render_document(&doc);
        assert!(
            html.contains("type=\"checkbox\""),
            "should have checkbox input"
        );
        assert!(html.contains("data-bind=\"dark\""), "should have data-bind");
        assert!(html.contains("Enable dark mode"), "should have label");
    }

    #[test]
    fn render_data_produces_json_script() {
        let doc = parse_document("@[Data](data=\"{'x': 42, 'y': 'hello'}\")\n").unwrap();
        let html = render_document(&doc);
        assert!(
            html.contains("data-vell-init"),
            "should have data-vell-init attribute"
        );
    }

    #[test]
    fn render_interactive_variables_show_reactive_spans() {
        let doc = parse_document("Count: @{count}\n").unwrap();
        let html = render_document(&doc);
        assert!(
            html.contains("data-vell-var=\"count\""),
            "should have reactive span"
        );
        assert!(html.contains("class=\"var\""), "should have var class");
    }

    #[test]
    fn render_spec_example_09_diagrams_and_charts() {
        let source = SPEC_SOURCES[8];
        let doc = parse_document(source).unwrap();
        let html = render_document(&doc);
        assert!(!html.is_empty(), "spec example 09 produced empty HTML");
        assert!(
            html.contains("vell-diagram"),
            "should contain diagram rendering"
        );
        assert!(
            html.contains("vell-chart"),
            "should contain chart rendering"
        );
        assert!(
            html.contains("class=\"mermaid\""),
            "should contain mermaid divs"
        );
        assert!(
            html.contains("sequenceDiagram"),
            "should contain sequence diagram"
        );
        assert!(html.contains("flowchart TD"), "should contain flowchart");
        assert!(html.contains("<svg"), "should contain SVG bar charts");
        assert!(
            html.contains("Quarterly Revenue"),
            "should contain first chart title"
        );
        assert!(
            html.contains("Product Sales by Category"),
            "should contain second chart title"
        );
        assert!(
            html.contains("Phase 10: Native Diagrams"),
            "should contain page title"
        );
    }
}
