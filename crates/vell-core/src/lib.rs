// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Samin Yeasar

//! Reference AST, lexer, parser, and diagnostics for the Vell language.

/// Abstract syntax tree definitions.
pub mod ast;
/// Human-readable parse error types.
pub mod error;
/// Streaming lexer for editor tooling and parser front ends.
pub mod lexer;
/// Recursive-descent parser that produces the canonical Vell AST.
pub mod parser;
/// Bibliography management: BibEntry data model, BibTeX parsing, DOI resolution.
pub mod bibliography;

pub use ast::*;
pub use error::{ParseError, ParseErrorKind};
pub use lexer::{Lexer, Token, TokenKind};
pub use parser::{parse_document, parse_document_with_warnings, slugify_inline, validate, ParseOutcome};

#[cfg(test)]
mod tests;
