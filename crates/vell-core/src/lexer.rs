// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Samin Yeasar

//! Streaming lexer for Vell source text.

use crate::ast::Span;
use std::collections::VecDeque;

/// Token variants emitted by the lexer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TokenKind {
    /// `=`.
    Equals,
    /// `>`.
    GreaterThan,
    /// `-`.
    Dash,
    /// `|`.
    Pipe,
    /// `+`.
    Plus,
    /// Single backtick.
    Backtick,
    /// Triple backtick fence.
    BacktickFence,
    /// Single dollar sign.
    DollarSign,
    /// Double dollar sign.
    DollarDouble,
    /// `*`.
    Asterisk,
    /// `/`.
    Slash,
    /// `_`.
    Underscore,
    /// `~`.
    Tilde,
    /// `^`.
    Caret,
    /// `,,`.
    DoubleComma,
    /// `@[`.
    AtBracket,
    /// `@{`.
    AtBrace,
    /// At-prefixed keyword such as `@var`.
    AtWord(String),
    /// `[`.
    BracketOpen,
    /// `]`.
    BracketClose,
    /// `(`.
    ParenOpen,
    /// `)`.
    ParenClose,
    /// `{`.
    BraceOpen,
    /// `}`.
    BraceClose,
    /// `!`.
    Bang,
    /// `:`.
    Colon,
    /// `::`.
    DoubleColon,
    /// Newline.
    Newline,
    /// Blank line.
    BlankLine,
    /// Indentation increased by one level.
    Indent,
    /// Indentation decreased by one level.
    Dedent,
    /// Text chunk.
    Text(String),
    /// Number token.
    Number(String),
    /// Identifier token.
    Ident(String),
    /// End of input.
    Eof,
}

/// A token with source location metadata.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Token {
    /// Token kind.
    pub kind: TokenKind,
    /// Byte span.
    pub span: Span,
    /// One-based line number.
    pub line: usize,
    /// One-based column number.
    pub column: usize,
}

/// Streaming lexer over UTF-8 Vell input.
pub struct Lexer<'a> {
    source: &'a str,
    pos: usize,
    line: usize,
    column: usize,
    pending: VecDeque<Token>,
    indents: Vec<usize>,
    at_line_start: bool,
    emitted_eof: bool,
}

impl<'a> Lexer<'a> {
    /// Creates a lexer for the provided source.
    pub fn new(source: &'a str) -> Self {
        Self {
            source,
            pos: 0,
            line: 1,
            column: 1,
            pending: VecDeque::new(),
            indents: vec![0],
            at_line_start: true,
            emitted_eof: false,
        }
    }

    fn make(&self, kind: TokenKind, start: usize, end: usize, line: usize, column: usize) -> Token {
        Token {
            kind,
            span: Span::new(start, end),
            line,
            column,
        }
    }

    fn starts_with(&self, value: &str) -> bool {
        self.source
            .get(self.pos..)
            .is_some_and(|remaining| remaining.starts_with(value))
    }

    fn advance_char(&mut self) -> Option<char> {
        let slice = self.source.get(self.pos..)?;
        let mut chars = slice.chars();
        let ch = chars.next()?;
        self.pos += ch.len_utf8();
        if ch == '\n' {
            self.line += 1;
            self.column = 1;
            self.at_line_start = true;
        } else {
            self.column += 1;
        }
        Some(ch)
    }

    fn advance_ascii(&mut self, count: usize) {
        for _ in 0..count {
            let _ = self.advance_char();
        }
    }

    fn handle_indent(&mut self) {
        if !self.at_line_start {
            return;
        }
        let start = self.pos;
        let line = self.line;
        let mut spaces = 0usize;
        while self.starts_with(" ") {
            self.advance_ascii(1);
            spaces += 1;
        }
        if self.starts_with("\n") || self.pos >= self.source.len() {
            return;
        }
        self.at_line_start = false;
        let current = self.indents.last().copied().unwrap_or(0);
        if spaces > current && spaces % 2 == 0 {
            self.indents.push(spaces);
            self.pending
                .push_back(self.make(TokenKind::Indent, start, self.pos, line, 1));
        } else if spaces < current {
            while self.indents.last().copied().unwrap_or(0) > spaces {
                let _ = self.indents.pop();
                self.pending
                    .push_back(self.make(TokenKind::Dedent, start, self.pos, line, 1));
            }
        }
    }
}

impl Iterator for Lexer<'_> {
    type Item = Token;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(token) = self.pending.pop_front() {
            return Some(token);
        }
        self.handle_indent();
        if let Some(token) = self.pending.pop_front() {
            return Some(token);
        }
        if self.pos >= self.source.len() {
            if self.indents.len() > 1 {
                let _ = self.indents.pop();
                return Some(self.make(
                    TokenKind::Dedent,
                    self.pos,
                    self.pos,
                    self.line,
                    self.column,
                ));
            }
            if self.emitted_eof {
                return None;
            }
            self.emitted_eof = true;
            return Some(self.make(TokenKind::Eof, self.pos, self.pos, self.line, self.column));
        }
        let start = self.pos;
        let line = self.line;
        let column = self.column;
        let kind = if self.starts_with("```") {
            self.advance_ascii(3);
            TokenKind::BacktickFence
        } else if self.starts_with("$$") {
            self.advance_ascii(2);
            TokenKind::DollarDouble
        } else if self.starts_with("@[") {
            self.advance_ascii(2);
            TokenKind::AtBracket
        } else if self.starts_with("@{") {
            self.advance_ascii(2);
            TokenKind::AtBrace
        } else if self.starts_with("::") {
            self.advance_ascii(2);
            TokenKind::DoubleColon
        } else if self.starts_with(",,") {
            self.advance_ascii(2);
            TokenKind::DoubleComma
        } else {
            let ch = self.advance_char()?;
            match ch {
                '\n' => {
                    let rest = self.source.get(self.pos..).unwrap_or_default();
                    if rest.starts_with('\n') {
                        TokenKind::BlankLine
                    } else {
                        TokenKind::Newline
                    }
                }
                '=' => TokenKind::Equals,
                '>' => TokenKind::GreaterThan,
                '-' => TokenKind::Dash,
                '|' => TokenKind::Pipe,
                '+' => TokenKind::Plus,
                '`' => TokenKind::Backtick,
                '$' => TokenKind::DollarSign,
                '*' => TokenKind::Asterisk,
                '/' => TokenKind::Slash,
                '_' => TokenKind::Underscore,
                '~' => TokenKind::Tilde,
                '^' => TokenKind::Caret,
                '[' => TokenKind::BracketOpen,
                ']' => TokenKind::BracketClose,
                '(' => TokenKind::ParenOpen,
                ')' => TokenKind::ParenClose,
                '{' => TokenKind::BraceOpen,
                '}' => TokenKind::BraceClose,
                '!' => TokenKind::Bang,
                ':' => TokenKind::Colon,
                '@' => {
                    let mut value = String::from("@");
                    while let Some(next) =
                        self.source.get(self.pos..).and_then(|s| s.chars().next())
                    {
                        if next.is_alphanumeric() || next == '_' {
                            value.push(next);
                            let _ = self.advance_char();
                        } else {
                            break;
                        }
                    }
                    TokenKind::AtWord(value)
                }
                c if c.is_ascii_digit() => {
                    let mut value = String::from(c);
                    while let Some(next) =
                        self.source.get(self.pos..).and_then(|s| s.chars().next())
                    {
                        if next.is_ascii_digit() || next == '.' {
                            value.push(next);
                            let _ = self.advance_char();
                        } else {
                            break;
                        }
                    }
                    TokenKind::Number(value)
                }
                c if c.is_alphabetic() || c == '_' => {
                    let mut value = String::from(c);
                    while let Some(next) =
                        self.source.get(self.pos..).and_then(|s| s.chars().next())
                    {
                        if next.is_alphanumeric() || next == '_' || next == '-' {
                            value.push(next);
                            let _ = self.advance_char();
                        } else {
                            break;
                        }
                    }
                    TokenKind::Ident(value)
                }
                c if c.is_whitespace() => TokenKind::Text(c.to_string()),
                other => TokenKind::Text(other.to_string()),
            }
        };
        Some(self.make(kind, start, self.pos, line, column))
    }
}
