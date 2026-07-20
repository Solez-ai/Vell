#![allow(clippy::single_component_path_imports)]
// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Samin Yeasar

//! Command-line interface for the Vell language.
//!
//! Subcommands: parse, fmt, render, validate, and watch.
//! Use `vell --help` for usage.

use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::process;
use vell_cli;

/// Command-line tool for the Vell markup language.
#[derive(Parser)]
#[command(name = "vell", version, about = "Vell markup language toolkit")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Parse source and output the AST as JSON.
    Parse {
        /// Optional input file (reads from stdin if absent).
        input: Option<PathBuf>,
    },
    /// Format Vell source code.
    Fmt {
        /// Optional input file (reads from stdin if absent).
        input: Option<PathBuf>,
        /// Check if formatting is correct without writing.
        #[arg(long)]
        check: bool,
    },
    /// Render Vell source to output formats.
    Render {
        #[command(subcommand)]
        format: RenderFormat,
    },
    /// Validate Vell source and print diagnostics.
    Validate {
        /// Optional input file (reads from stdin if absent).
        input: Option<PathBuf>,
    },
    /// Watch a file and auto-rebuild on changes.
    Watch {
        /// Input file to watch.
        input: Option<PathBuf>,
        /// Output format (html, pdf, slides, epub).
        #[arg(long, default_value = "html", value_parser = clap::builder::PossibleValuesParser::new(["html", "pdf", "slides", "epub"]))]
        format: String,
        /// Output file path (defaults to stdout).
        #[arg(short = 'o')]
        output: Option<PathBuf>,
        /// Debounce interval in milliseconds.
        #[arg(long, default_value_t = 500)]
        debounce_ms: u64,
        /// Start a live-reload HTTP server on port 3000 (alias for --port 3000).
        #[arg(long, default_value_t = false)]
        serve: bool,
        /// Serve rebuilt HTML on this port (e.g. --port 3000). Ignored without --serve.
        #[arg(long, default_value_t = 3000)]
        port: u16,
    },
}

#[derive(Subcommand)]
enum RenderFormat {
    /// Render to HTML.
    Html {
        /// Optional input file (reads from stdin if absent).
        input: Option<PathBuf>,
        /// Output file path (prints to stdout if absent).
        #[arg(short = 'o')]
        output: Option<PathBuf>,
    },
    /// Render to PDF.
    Pdf {
        /// Input file (reads from stdin if absent).
        input: Option<PathBuf>,
        /// Output file path (prints to stdout if absent).
        #[arg(short = 'o')]
        output: Option<PathBuf>,
    },
    /// Render to slides (reveal.js HTML).
    Slides {
        /// Input file (reads from stdin if absent).
        input: Option<PathBuf>,
        /// Output file path (prints to stdout if absent).
        #[arg(short = 'o')]
        output: Option<PathBuf>,
    },
    /// Render to EPUB 3 (requires Node.js).
    Epub {
        /// Input file (reads from stdin if absent).
        input: Option<PathBuf>,
        /// Output .epub file path.
        output: Option<PathBuf>,
        /// Path to the vell-renderer-epub package (default: packages/vell-renderer-epub).
        #[arg(long)]
        renderer_path: Option<PathBuf>,
    },
    /// Render to LaTeX.
    Latex {
        input: Option<PathBuf>,
        #[arg(short = 'o')]
        output: Option<PathBuf>,
    },
    /// Render to JATS XML.
    Jats {
        input: Option<PathBuf>,
        #[arg(short = 'o')]
        output: Option<PathBuf>,
    },
    /// Render to a DOCX document.
    Docx {
        input: Option<PathBuf>,
        #[arg(short = 'o')]
        output: PathBuf,
    },
}

fn main() {
    let cli = Cli::parse();
    let result = match cli.command {
        Command::Parse { input } => vell_cli::cmd_parse(&input),
        Command::Fmt { input, check } => vell_cli::cmd_fmt(&input, check),
        Command::Render {
            format: RenderFormat::Html { input, output },
        } => vell_cli::cmd_render_html(&input, &output),
        Command::Render {
            format: RenderFormat::Pdf { input, output },
        } => vell_cli::cmd_render_pdf(&input, &output),
        Command::Render {
            format: RenderFormat::Slides { input, output },
        } => vell_cli::cmd_render_slides(&input, &output),
        Command::Render {
            format:
                RenderFormat::Epub {
                    input,
                    output,
                    renderer_path,
                },
        } => vell_cli::cmd_render_epub(&input, &output, &renderer_path),
        Command::Render {
            format: RenderFormat::Latex { input, output },
        } => vell_cli::cmd_render_latex(&input, &output),
        Command::Render {
            format: RenderFormat::Jats { input, output },
        } => vell_cli::cmd_render_jats(&input, &output),
        Command::Render {
            format: RenderFormat::Docx { input, output },
        } => vell_cli::cmd_render_docx(&input, &output),
        Command::Watch {
            input,
            format,
            output,
            debounce_ms,
            serve,
            port,
        } => {
            let actual_port = if serve { port } else { 0 };
            vell_cli::watch::cmd_watch(&input, &format, &output, debounce_ms, actual_port)
        }
        Command::Validate { input } => vell_cli::cmd_validate(&input),
    };

    match result {
        Ok(()) => process::exit(0),
        Err(_msg) => process::exit(1),
    }
}
