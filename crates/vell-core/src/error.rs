// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Samin Yeasar

//! Parse error and diagnostic types.

use crate::ast::Span;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fmt::{self, Display, Formatter};

/// Structured parse error produced by the Vell parser.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParseError {
    /// Error category.
    pub kind: ParseErrorKind,
    /// Source span associated with the error.
    pub span: Span,
    /// Human-readable error message.
    pub message: String,
    /// Optional actionable suggestion.
    pub suggestion: Option<String>,
}

impl ParseError {
    /// Creates a parse error.
    pub fn new(
        kind: ParseErrorKind,
        span: Span,
        message: impl Into<String>,
        suggestion: Option<String>,
    ) -> Self {
        Self {
            kind,
            span,
            message: message.into(),
            suggestion,
        }
    }
}

/// Categories of parser diagnostics.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ParseErrorKind {
    /// The parser encountered a token that is not valid in the current position.
    UnexpectedToken,
    /// A balanced delimiter was opened and not closed.
    UnterminatedDelimiter,
    /// Indentation is not a multiple of two spaces or is otherwise inconsistent.
    InvalidIndentation,
    /// A reference is not declared in the document.
    UndefinedReference,
    /// A directive is malformed.
    MalformedDirective,
    /// A table is malformed.
    MalformedTable,
    /// A math delimiter or math block is malformed.
    MalformedMath,
    /// A property value cannot be parsed.
    InvalidPropValue,
}

impl Display for ParseErrorKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl Display for ParseError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        if let Some(suggestion) = &self.suggestion {
            write!(
                f,
                "{} at bytes {}: {} Suggestion: {}",
                self.kind, self.span, self.message, suggestion
            )
        } else {
            write!(f, "{} at bytes {}: {}", self.kind, self.span, self.message)
        }
    }
}

impl Error for ParseError {}
