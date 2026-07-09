// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Samin Yeasar

//! Command-line interface for the Vell language.
//!
//! Subcommands: parse, fmt, render html, validate.
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
        Command::Validate { input } => vell_cli::cmd_validate(&input),
    };

    match result {
        Ok(()) => process::exit(0),
        Err(_msg) => process::exit(1),
    }
}
