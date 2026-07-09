// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Samin Yeasar

//! Recursive-descent parser for Vell source text.

use crate::ast::{
    Alignment, Document, DocumentMetadata, InlineNode, ListItem, Node, PropValue, Span, TableCell,
    AST_VERSION,
};
use crate::error::{ParseError, ParseErrorKind};
use serde_json::Value as JsonValue;
use std::collections::{HashMap, HashSet};

/// Result that preserves warnings alongside the parsed document.
#[derive(Clone, Debug, PartialEq)]
pub struct ParseOutcome {
    /// Parsed document when no parse error occurred.
    pub document: Document,
    /// Non-fatal diagnostics such as unresolved runtime variables.
    pub warnings: Vec<ParseError>,
}

#[derive(Clone, Debug)]
struct Line {
    text: String,
    start: usize,
    end: usize,
}

/// Parses source into a document, returning the first fatal parse error.
pub fn parse_document(source: &str) -> Result<Document, ParseError> {
    parse_document_with_warnings(source).map(|outcome| outcome.document)
}

/// Parses source into a document and preserves non-fatal warnings.
pub fn parse_document_with_warnings(source: &str) -> Result<ParseOutcome, ParseError> {
    Parser::new(source).parse_document()
}

/// Validates source and returns parser diagnostics.
pub fn validate(source: &str) -> Vec<ParseError> {
    match parse_document_with_warnings(source) {
        Ok(outcome) => outcome.warnings,
        Err(error) => vec![error],
    }
}

struct Parser {
    source: String,
    lines: Vec<Line>,
    index: usize,
    metadata: DocumentMetadata,
    warnings: Vec<ParseError>,
    variables: HashSet<String>,
}

impl Parser {
    fn new(source: &str) -> Self {
        let mut lines = Vec::new();
        let mut start = 0usize;
        for part in source.split_inclusive('\n') {
            let end = start + part.len();
            let without_lf = part.strip_suffix('\n').unwrap_or(part);
            let text = without_lf
                .strip_suffix('\r')
                .unwrap_or(without_lf)
                .to_string();
            lines.push(Line { text, start, end });
            start = end;
        }
        if source.is_empty() || !source.ends_with('\n') {
            lines.push(Line {
                text: String::new(),
                start,
                end: start,
            });
        }
        Self {
            source: source.to_string(),
            lines,
            index: 0,
            metadata: DocumentMetadata::default(),
            warnings: Vec::new(),
            variables: HashSet::new(),
        }
    }

    fn parse_document(mut self) -> Result<ParseOutcome, ParseError> {
        let mut children = Vec::new();
        while self.index < self.lines.len() {
            if self.current_trim().is_empty() {
                self.index += 1;
                continue;
            }
            let node = self.parse_block()?;
            children.push(node);
        }
        let span = Span::new(0, self.source.len());
        Ok(ParseOutcome {
            document: Document {
                version: AST_VERSION,
                children,
                metadata: self.metadata,
                span,
            },
            warnings: self.warnings,
        })
    }

    fn current(&self) -> Option<&Line> {
        self.lines.get(self.index)
    }

    fn current_trim(&self) -> &str {
        self.current().map(|line| line.text.trim()).unwrap_or("")
    }

    fn parse_block(&mut self) -> Result<Node, ParseError> {
        let trim = self.current_trim().to_string();
        if trim.starts_with("```") {
            self.parse_code_fence()
        } else if trim == "$$" || (trim.starts_with("$$") && !trim.ends_with("$$")) {
            self.parse_math_block()
        } else if is_heading(&trim) {
            self.parse_heading()
        } else if trim.starts_with("> [!") {
            self.parse_admonition()
        } else if trim.starts_with('>') {
            self.parse_blockquote()
        } else if trim.starts_with('+') {
            self.parse_grid_table()
        } else if trim.starts_with('|') && self.next_is_pipe_separator() {
            self.parse_pipe_table()
        } else if is_hrule(&trim) {
            self.parse_hrule()
        } else if is_ordered_marker(&trim) {
            self.parse_list(true)
        } else if trim.starts_with("- ") {
            self.parse_list(false)
        } else if trim.starts_with(":: ") {
            self.parse_def_list()
        } else if trim.starts_with("@var ") {
            self.parse_var_decl()
        } else if trim.starts_with("@for ") {
            self.parse_for_loop()
        } else if trim.starts_with("@if ") {
            self.parse_if_block()
        } else if trim.starts_with("@[") {
            self.parse_directive()
        } else if trim.starts_with("[^") && trim.contains("]:") {
            self.parse_footnote_def()
        } else if trim.starts_with('[') && trim.contains("]:") {
            self.parse_ref_def()
        } else if trim.starts_with("$$") && trim.ends_with("$$") && trim.len() >= 4 {
            let current_span = self
                .current()
                .map(|l| Span::new(l.start, l.end))
                .unwrap_or_default();
            let line = self.current().cloned().ok_or_else(|| {
                self.error(
                    ParseErrorKind::MalformedMath,
                    current_span,
                    "Missing math block line",
                    None,
                )
            })?;
            self.index += 1;
            let source = trim.trim_matches('$').trim().to_string();
            Ok(Node::MathBlock {
                source,
                span: Span::new(line.start, line.end),
            })
        } else {
            self.parse_paragraph()
        }
    }

    fn parse_heading(&mut self) -> Result<Node, ParseError> {
        let line = self.current().cloned().ok_or_else(|| {
            let span = self
                .current()
                .map(|l| Span::new(l.start, l.end))
                .unwrap_or_default();
            self.error(
                ParseErrorKind::UnexpectedToken,
                span,
                "Expected heading",
                None,
            )
        })?;
        self.index += 1;
        let trimmed = line.text.trim_start();
        let level = trimmed.chars().take_while(|ch| *ch == '=').count();
        if level == 0 || level > 6 || trimmed.chars().nth(level).is_none_or(|ch| ch != ' ') {
            return Err(self.error(
                ParseErrorKind::UnexpectedToken,
                Span::new(line.start, line.end),
                "Headings must use one to six '=' characters followed by a space.",
                Some("Write headings as '= Title' or '== Section'.".to_string()),
            ));
        }
        let text = trimmed.get(level + 1..).unwrap_or_default();
        let children = self.parse_inline_sequence(text, line.start + level + 1)?;
        let id = Some(slugify_inline(&children));
        let heading = Node::Heading {
            level: u8::try_from(level).unwrap_or(6),
            children,
            id,
            span: Span::new(line.start, line.end),
        };
        if self.metadata.title.is_none() && level == 1 {
            self.metadata.title = Some(text.to_string());
        }
        Ok(heading)
    }

    fn parse_paragraph(&mut self) -> Result<Node, ParseError> {
        let start = self.current().map(|line| line.start).unwrap_or(0);
        let mut end = start;
        let mut parts = Vec::new();
        while self.index < self.lines.len() {
            let trim = self.current_trim();
            if trim.is_empty() || (!parts.is_empty() && starts_block(trim)) {
                break;
            }
            if let Some(line) = self.current() {
                end = line.end;
                parts.push(line.text.trim().to_string());
            }
            self.index += 1;
        }
        let text = parts.join("\n");
        let children = self.parse_inline_sequence(&text, start)?;
        Ok(Node::Paragraph {
            children,
            span: Span::new(start, end),
        })
    }

    fn parse_blockquote(&mut self) -> Result<Node, ParseError> {
        let start = self.current().map(|line| line.start).unwrap_or(0);
        let mut source = String::new();
        let mut end = start;
        while self.index < self.lines.len() && self.current_trim().starts_with('>') {
            if let Some(line) = self.current() {
                end = line.end;
                let content = line
                    .text
                    .trim_start()
                    .trim_start_matches('>')
                    .trim_start_matches('>')
                    .trim_start();
                source.push_str(content);
                source.push('\n');
            }
            self.index += 1;
        }
        let nested = parse_document(&source)?.children;
        Ok(Node::Blockquote {
            children: nested,
            admonition_type: None,
            span: Span::new(start, end),
        })
    }

    fn parse_admonition(&mut self) -> Result<Node, ParseError> {
        let start = self.current().map(|line| line.start).unwrap_or(0);
        let first = self.current_trim().to_string();
        let kind = first
            .strip_prefix("> [!")
            .and_then(|rest| rest.strip_suffix(']'))
            .unwrap_or("NOTE")
            .to_string();
        self.index += 1;
        let mut source = String::new();
        let mut end = start;
        while self.index < self.lines.len() && self.current_trim().starts_with('>') {
            if let Some(line) = self.current() {
                end = line.end;
                let content = line.text.trim_start().trim_start_matches('>').trim_start();
                source.push_str(content);
                source.push('\n');
            }
            self.index += 1;
        }
        let nested = parse_document(&source)?.children;
        Ok(Node::Blockquote {
            children: nested,
            admonition_type: Some(kind),
            span: Span::new(start, end),
        })
    }

    fn parse_code_fence(&mut self) -> Result<Node, ParseError> {
        let open = self.current().cloned().ok_or_else(|| {
            let span = self
                .current()
                .map(|l| Span::new(l.start, l.end))
                .unwrap_or_default();
            self.error(
                ParseErrorKind::UnexpectedToken,
                span,
                "Expected code fence",
                None,
            )
        })?;
        let lang = open
            .text
            .trim()
            .strip_prefix("```")
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| s.to_ascii_lowercase());
        self.index += 1;
        let mut source = String::new();
        let mut end = open.end;
        while self.index < self.lines.len() {
            let line = self.current().cloned().ok_or_else(|| {
                self.error(
                    ParseErrorKind::UnterminatedDelimiter,
                    Span::new(open.start, open.end),
                    "Unterminated code fence.",
                    Some("Close the block with ``` on its own line.".to_string()),
                )
            })?;
            if line.text.trim() == "```" {
                end = line.end;
                self.index += 1;
                return Ok(Node::CodeBlock {
                    lang,
                    source: source.trim_end_matches('\n').to_string(),
                    executable: false,
                    span: Span::new(open.start, end),
                });
            }
            source.push_str(&line.text);
            source.push('\n');
            end = line.end;
            self.index += 1;
        }
        Err(self.error(
            ParseErrorKind::UnterminatedDelimiter,
            Span::new(open.start, end),
            "Unterminated code fence: ``` was opened but never closed.",
            Some("Add a closing ``` line.".to_string()),
        ))
    }

    fn parse_math_block(&mut self) -> Result<Node, ParseError> {
        let open = self.current().cloned().ok_or_else(|| {
            let span = self
                .current()
                .map(|l| Span::new(l.start, l.end))
                .unwrap_or_default();
            self.error(
                ParseErrorKind::MalformedMath,
                span,
                "Expected math block",
                None,
            )
        })?;
        self.index += 1;
        let mut source = String::new();
        let mut end = open.end;
        while self.index < self.lines.len() {
            let line = self.current().cloned().ok_or_else(|| {
                self.error(
                    ParseErrorKind::MalformedMath,
                    Span::new(open.start, open.end),
                    "Unterminated math block.",
                    Some("Close the block with $$ on its own line.".to_string()),
                )
            })?;
            if line.text.trim() == "$$" {
                end = line.end;
                self.index += 1;
                return Ok(Node::MathBlock {
                    source: source.trim_end_matches('\n').to_string(),
                    span: Span::new(open.start, end),
                });
            }
            source.push_str(&line.text);
            source.push('\n');
            end = line.end;
            self.index += 1;
        }
        Err(self.error(
            ParseErrorKind::MalformedMath,
            Span::new(open.start, end),
            "Unterminated math block: $$ was opened but never closed.",
            Some("Add a closing $$ line.".to_string()),
        ))
    }

    fn parse_list(&mut self, ordered: bool) -> Result<Node, ParseError> {
        let start = self.current().map(|line| line.start).unwrap_or(0);
        let mut items = Vec::new();
        let mut end = start;
        let mut first_number = None;
        while self.index < self.lines.len() {
            let trim = self.current_trim().to_string();
            if ordered && !is_ordered_marker(&trim) || !ordered && !trim.starts_with("- ") {
                break;
            }
            let line = self.current().cloned().ok_or_else(|| {
                let span = self
                    .current()
                    .map(|l| Span::new(l.start, l.end))
                    .unwrap_or_default();
                self.error(
                    ParseErrorKind::UnexpectedToken,
                    span,
                    "Expected list item",
                    None,
                )
            })?;
            // Validate indentation is a multiple of 2 spaces
            let leading_spaces = line.text.len() - line.text.trim_start().len();
            if leading_spaces > 0 && leading_spaces % 2 != 0 {
                return Err(self.error(
                    ParseErrorKind::InvalidIndentation,
                    Span::new(line.start, line.end),
                    format!(
                        "List item indentation of {} spaces is not a multiple of 2.",
                        leading_spaces
                    ),
                    Some("Use 0, 2, 4, 6, etc. spaces for list item indentation.".to_string()),
                ));
            }
            end = line.end;
            let body = if ordered {
                let marker_end = trim.find(". ").unwrap_or(0);
                if first_number.is_none() {
                    first_number = trim.get(..marker_end).and_then(|s| s.parse::<u32>().ok());
                }
                trim.get(marker_end + 2..).unwrap_or_default().to_string()
            } else {
                trim.get(2..).unwrap_or_default().to_string()
            };
            let paragraph = Node::Paragraph {
                children: self.parse_inline_sequence(&body, line.start)?,
                span: Span::new(line.start, line.end),
            };
            items.push(ListItem {
                children: vec![paragraph],
                checked: None,
                span: Span::new(line.start, line.end),
            });
            self.index += 1;
        }
        Ok(Node::List {
            ordered,
            start: if ordered { first_number } else { None },
            items,
            span: Span::new(start, end),
        })
    }

    fn parse_pipe_table(&mut self) -> Result<Node, ParseError> {
        let start = self.current().map(|line| line.start).unwrap_or(0);
        let header_line = self.current().cloned().ok_or_else(|| {
            let span = self
                .current()
                .map(|l| Span::new(l.start, l.end))
                .unwrap_or_default();
            self.error(
                ParseErrorKind::MalformedTable,
                span,
                "Expected table header",
                None,
            )
        })?;
        let separator_line = self.lines.get(self.index + 1).cloned().ok_or_else(|| {
            self.error(
                ParseErrorKind::MalformedTable,
                Span::new(header_line.start, header_line.end),
                "Pipe table header must be followed by a separator row.",
                Some("Add a separator such as |---|---| after the header row.".to_string()),
            )
        })?;
        if !is_pipe_separator(separator_line.text.trim()) {
            return Err(self.error(
                ParseErrorKind::MalformedTable,
                Span::new(separator_line.start, separator_line.end),
                "Pipe table separator row is malformed.",
                Some("Use dashes and optional colons, for example |---|:---:|---:|.".to_string()),
            ));
        }
        let alignments = parse_pipe_alignments(separator_line.text.trim());
        let mut headers = self.parse_table_cells(&header_line.text, header_line.start)?;
        for (cell, align) in headers.iter_mut().zip(alignments.iter()) {
            cell.align = align.clone();
        }
        self.index += 2;
        let mut rows = Vec::new();
        let mut end = separator_line.end;
        while self.index < self.lines.len() && self.current_trim().starts_with('|') {
            let line = self.current().cloned().ok_or_else(|| {
                let span = self
                    .current()
                    .map(|l| Span::new(l.start, l.end))
                    .unwrap_or_default();
                self.error(
                    ParseErrorKind::MalformedTable,
                    span,
                    "Expected table row",
                    None,
                )
            })?;
            if is_pipe_separator(line.text.trim()) {
                break;
            }
            let mut cells = self.parse_table_cells(&line.text, line.start)?;
            if cells.len() != headers.len() {
                return Err(self.error(
                    ParseErrorKind::MalformedTable,
                    Span::new(line.start, line.end),
                    format!(
                        "Pipe table row has {} cells, but the header has {} cells.",
                        cells.len(),
                        headers.len()
                    ),
                    Some("Make every pipe table row use the same number of cells.".to_string()),
                ));
            }
            for (cell, align) in cells.iter_mut().zip(alignments.iter()) {
                cell.align = align.clone();
            }
            end = line.end;
            rows.push(cells);
            self.index += 1;
        }
        Ok(Node::Table {
            headers,
            rows,
            span: Span::new(start, end),
        })
    }

    fn parse_grid_table(&mut self) -> Result<Node, ParseError> {
        let start = self.current().map(|line| line.start).unwrap_or(0);
        let first_border = self.current().cloned().ok_or_else(|| {
            let span = self
                .current()
                .map(|l| Span::new(l.start, l.end))
                .unwrap_or_default();
            self.error(
                ParseErrorKind::MalformedTable,
                span,
                "Expected grid table border.",
                None,
            )
        })?;
        let boundaries = grid_boundaries(first_border.text.trim()).ok_or_else(|| {
            self.error(
                ParseErrorKind::MalformedTable,
                Span::new(first_border.start, first_border.end),
                "Grid table border is malformed.",
                Some("Use borders such as +-----+-----+.".to_string()),
            )
        })?;
        let mut parsed_rows = Vec::new();
        let mut end = first_border.end;
        while self.index < self.lines.len() && self.current_trim().starts_with('+') {
            let border = self.current().cloned().ok_or_else(|| {
                let span = self
                    .current()
                    .map(|l| Span::new(l.start, l.end))
                    .unwrap_or_default();
                self.error(
                    ParseErrorKind::MalformedTable,
                    span,
                    "Expected grid table border.",
                    None,
                )
            })?;
            if grid_boundaries(border.text.trim()) != Some(boundaries.clone()) {
                return Err(self.error(
                    ParseErrorKind::MalformedTable,
                    Span::new(border.start, border.end),
                    "Grid table borders must use consistent column boundaries.",
                    Some("Keep every +---+ border aligned to the same columns.".to_string()),
                ));
            }
            end = border.end;
            self.index += 1;
            if self.index >= self.lines.len() || !self.current_trim().starts_with('|') {
                break;
            }
            let row = self.current().cloned().ok_or_else(|| {
                let span = self
                    .current()
                    .map(|l| Span::new(l.start, l.end))
                    .unwrap_or_default();
                self.error(
                    ParseErrorKind::MalformedTable,
                    span,
                    "Expected grid table row.",
                    None,
                )
            })?;
            parsed_rows.push(self.parse_grid_row(&row, &boundaries)?);
            end = row.end;
            self.index += 1;
        }
        if parsed_rows.is_empty() {
            return Err(self.error(
                ParseErrorKind::MalformedTable,
                Span::new(start, end),
                "Grid table must contain at least one row.",
                None,
            ));
        }
        let headers = parsed_rows.first().cloned().unwrap_or_default();
        let rows = parsed_rows.into_iter().skip(1).collect();
        Ok(Node::Table {
            headers,
            rows,
            span: Span::new(start, end),
        })
    }

    fn parse_table_cells(
        &mut self,
        line: &str,
        offset: usize,
    ) -> Result<Vec<TableCell>, ParseError> {
        let trimmed = line.trim();
        if !trimmed.starts_with('|') || !trimmed.ends_with('|') {
            return Err(self.error(
                ParseErrorKind::MalformedTable,
                Span::new(offset, offset + line.len()),
                "Table rows must start and end with a pipe character.",
                None,
            ));
        }
        let inner = trimmed.trim_matches('|');
        let mut cells = Vec::new();
        let mut cursor = offset + line.find('|').unwrap_or(0) + 1;
        for cell in inner.split('|') {
            let content = cell.trim();
            cells.push(TableCell {
                children: self.parse_inline_sequence(content, cursor)?,
                colspan: 1,
                rowspan: 1,
                align: None,
                span: Span::new(cursor, cursor + cell.len()),
            });
            cursor += cell.len() + 1;
        }
        Ok(cells)
    }

    fn parse_grid_row(
        &mut self,
        line: &Line,
        boundaries: &[usize],
    ) -> Result<Vec<TableCell>, ParseError> {
        let trimmed = line.text.trim();
        if !trimmed.starts_with('|') || !trimmed.ends_with('|') {
            return Err(self.error(
                ParseErrorKind::MalformedTable,
                Span::new(line.start, line.end),
                "Grid table rows must start and end with '|'.",
                None,
            ));
        }
        let separators = trimmed
            .char_indices()
            .filter_map(|(index, ch)| (ch == '|').then_some(index))
            .collect::<Vec<_>>();
        if separators.first() != Some(&0) || separators.last() != boundaries.last() {
            return Err(self.error(
                ParseErrorKind::MalformedTable,
                Span::new(line.start, line.end),
                "Grid table row does not align with the table border.",
                Some("Align row pipe characters with the + characters in the border.".to_string()),
            ));
        }
        let mut cells = Vec::new();
        for pair in separators.windows(2) {
            let left = pair[0];
            let right = pair[1];
            let start_column = boundary_index(boundaries, left).ok_or_else(|| {
                self.error(
                    ParseErrorKind::MalformedTable,
                    Span::new(line.start, line.end),
                    "Grid table cell starts at a non-border column.",
                    None,
                )
            })?;
            let end_column = boundary_index(boundaries, right).ok_or_else(|| {
                self.error(
                    ParseErrorKind::MalformedTable,
                    Span::new(line.start, line.end),
                    "Grid table cell ends at a non-border column.",
                    None,
                )
            })?;
            let content = trimmed.get(left + 1..right).unwrap_or_default().trim();
            cells.push(TableCell {
                children: self.parse_inline_sequence(content, line.start + left + 1)?,
                colspan: u32::try_from(end_column.saturating_sub(start_column)).unwrap_or(1),
                rowspan: 1,
                align: None,
                span: Span::new(line.start + left, line.start + right + 1),
            });
        }
        Ok(cells)
    }

    fn parse_hrule(&mut self) -> Result<Node, ParseError> {
        let line = self.current().cloned().ok_or_else(|| {
            let span = self
                .current()
                .map(|l| Span::new(l.start, l.end))
                .unwrap_or_default();
            self.error(
                ParseErrorKind::UnexpectedToken,
                span,
                "Expected horizontal rule",
                None,
            )
        })?;
        self.index += 1;
        Ok(Node::HorizontalRule {
            span: Span::new(line.start, line.end),
        })
    }

    fn parse_def_list(&mut self) -> Result<Node, ParseError> {
        let start = self.current().map(|line| line.start).unwrap_or(0);
        let mut items = Vec::new();
        let mut end = start;
        while self.index < self.lines.len() && self.current_trim().starts_with(":: ") {
            let term_line = self.current().cloned().ok_or_else(|| {
                let span = self
                    .current()
                    .map(|l| Span::new(l.start, l.end))
                    .unwrap_or_default();
                self.error(
                    ParseErrorKind::UnexpectedToken,
                    span,
                    "Expected definition term",
                    None,
                )
            })?;
            self.index += 1;
            let term = term_line
                .text
                .trim()
                .get(3..)
                .unwrap_or_default()
                .to_string();
            let mut body = String::new();
            while self.index < self.lines.len()
                && self
                    .current()
                    .is_some_and(|line| line.text.starts_with("  "))
            {
                if let Some(line) = self.current() {
                    body.push_str(line.text.trim());
                    body.push('\n');
                    end = line.end;
                }
                self.index += 1;
            }
            let definition = if body.trim().is_empty() {
                Vec::new()
            } else {
                parse_document(&body)?.children
            };
            let term_nodes = self.parse_inline_sequence(&term, term_line.start)?;
            items.push(crate::ast::DefinitionItem {
                term: term_nodes,
                definition,
                span: Span::new(term_line.start, end.max(term_line.end)),
            });
        }
        Ok(Node::DefinitionList {
            items,
            span: Span::new(start, end),
        })
    }

    fn parse_var_decl(&mut self) -> Result<Node, ParseError> {
        let line = self.current().cloned().ok_or_else(|| {
            let span = self
                .current()
                .map(|l| Span::new(l.start, l.end))
                .unwrap_or_default();
            self.error(
                ParseErrorKind::UnexpectedToken,
                span,
                "Expected variable declaration",
                None,
            )
        })?;
        self.index += 1;
        let body = line.text.trim().strip_prefix("@var ").unwrap_or_default();
        let Some(eq_index) = body.find('=') else {
            return Err(self.error(
                ParseErrorKind::UnexpectedToken,
                Span::new(line.start, line.end),
                "Variable declarations require '='.",
                Some("Use @var name = value.".to_string()),
            ));
        };
        let name = body.get(..eq_index).unwrap_or_default().trim().to_string();
        if !is_ident(&name) {
            return Err(self.error(
                ParseErrorKind::UnexpectedToken,
                Span::new(line.start, line.end),
                "Variable name must be an identifier.",
                None,
            ));
        }
        let raw = body.get(eq_index + 1..).unwrap_or_default().trim();
        let value = parse_json_value(raw).map_err(|message| {
            self.error(
                ParseErrorKind::InvalidPropValue,
                Span::new(line.start, line.end),
                message,
                Some("Use JSON-compatible primitives or arrays.".to_string()),
            )
        })?;
        self.metadata.variables.insert(name.clone(), value.clone());
        self.variables.insert(name.clone());
        Ok(Node::VarDeclaration {
            name,
            value,
            span: Span::new(line.start, line.end),
        })
    }

    fn parse_for_loop(&mut self) -> Result<Node, ParseError> {
        let line = self.current().cloned().ok_or_else(|| {
            let span = self
                .current()
                .map(|l| Span::new(l.start, l.end))
                .unwrap_or_default();
            self.error(
                ParseErrorKind::UnexpectedToken,
                span,
                "Expected for loop",
                None,
            )
        })?;
        let header = line.text.trim();
        let Some(in_index) = header.find(" in ") else {
            return Err(self.error(
                ParseErrorKind::UnexpectedToken,
                Span::new(line.start, line.end),
                "For loops require 'in'.",
                Some("Use @for item in @{items} { ... }.".to_string()),
            ));
        };
        let variable = header
            .get(5..in_index)
            .unwrap_or_default()
            .trim()
            .to_string();
        let iterable = header
            .get(in_index + 4..)
            .unwrap_or_default()
            .trim()
            .trim_end_matches('{')
            .trim()
            .trim_start_matches("@{")
            .trim_end_matches('}')
            .to_string();
        self.index += 1;
        let (children, end) = self.parse_braced_children(line.start)?;
        Ok(Node::ForLoop {
            variable,
            iterable,
            children,
            span: Span::new(line.start, end),
        })
    }

    fn parse_if_block(&mut self) -> Result<Node, ParseError> {
        let line = self.current().cloned().ok_or_else(|| {
            let span = self
                .current()
                .map(|l| Span::new(l.start, l.end))
                .unwrap_or_default();
            self.error(
                ParseErrorKind::UnexpectedToken,
                span,
                "Expected if block",
                None,
            )
        })?;
        let condition = line
            .text
            .trim()
            .strip_prefix("@if ")
            .unwrap_or_default()
            .trim()
            .trim_end_matches('{')
            .trim()
            .to_string();
        self.index += 1;
        let (consequent, mut end) = self.parse_braced_children(line.start)?;
        let mut alternate = None;
        if self.index < self.lines.len()
            && (self.current_trim().starts_with("else") || self.current_trim() == "} else {")
        {
            let else_line = self.current().cloned().ok_or_else(|| {
                let span = self
                    .current()
                    .map(|l| Span::new(l.start, l.end))
                    .unwrap_or_default();
                self.error(
                    ParseErrorKind::UnexpectedToken,
                    span,
                    "Expected else block",
                    None,
                )
            })?;
            self.index += 1;
            let (alt, alt_end) = self.parse_braced_children(else_line.start)?;
            end = alt_end;
            alternate = Some(alt);
        }
        Ok(Node::IfBlock {
            condition,
            consequent,
            alternate,
            span: Span::new(line.start, end),
        })
    }

    fn parse_braced_children(&mut self, start: usize) -> Result<(Vec<Node>, usize), ParseError> {
        let mut body = String::new();
        let mut end = start;
        let mut depth = 1usize;
        while self.index < self.lines.len() {
            let line = self.current().cloned().ok_or_else(|| {
                self.error(
                    ParseErrorKind::UnexpectedToken,
                    Span::new(start, end),
                    "Unexpected end of block",
                    None,
                )
            })?;
            let trimmed = line.text.trim();
            if trimmed == "}" {
                depth = depth.saturating_sub(1);
                end = line.end;
                self.index += 1;
                if depth == 0 {
                    return Ok((parse_document(&body)?.children, end));
                }
                body.push_str(trimmed);
                body.push('\n');
                continue;
            }
            if trimmed == "} else {" && depth == 1 {
                end = line.end;
                return Ok((parse_document(&body)?.children, end));
            }
            if trimmed.ends_with('{') {
                depth += 1;
            }
            body.push_str(line.text.strip_prefix("  ").unwrap_or(line.text.as_str()));
            body.push('\n');
            end = line.end;
            self.index += 1;
        }
        Err(self.error(
            ParseErrorKind::UnexpectedToken,
            Span::new(start, end),
            "Block body is missing a closing '}'.",
            Some("Add a closing brace on its own line.".to_string()),
        ))
    }

    fn parse_directive(&mut self) -> Result<Node, ParseError> {
        let line = self.current().cloned().ok_or_else(|| {
            let span = self
                .current()
                .map(|l| Span::new(l.start, l.end))
                .unwrap_or_default();
            self.error(
                ParseErrorKind::MalformedDirective,
                span,
                "Expected directive",
                None,
            )
        })?;
        let trimmed = line.text.trim();

        // Check if directive has properties (Name](...)) or is prop-less (Name] { or Name] EOL)
        let (name, props, tail) = if let Some(close_name) = trimmed.find("](") {
            // Has properties: @[Name](key=value) { ... }
            let name = trimmed.get(2..close_name).unwrap_or_default().to_string();
            let rest = trimmed.get(close_name + 2..).unwrap_or_default();
            let Some(close_props) = find_matching_paren(rest) else {
                return Err(self.error(
                    ParseErrorKind::MalformedDirective,
                    Span::new(line.start, line.end),
                    "Directive property list is not closed.",
                    Some("Add a closing ')' after the property list.".to_string()),
                ));
            };
            let props_raw = rest.get(..close_props).unwrap_or_default();
            let props = parse_props(props_raw).map_err(|message| {
                self.error(
                    ParseErrorKind::InvalidPropValue,
                    Span::new(line.start, line.end),
                    message,
                    None,
                )
            })?;
            let tail = trimmed
                .get(close_name + 3 + close_props..)
                .unwrap_or_default()
                .trim();
            (name, props, tail)
        } else if let Some(close_name) = trimmed.find(']') {
            // No properties: @[Name] { ... } or @[Name]
            let name = trimmed.get(2..close_name).unwrap_or_default().to_string();
            let tail = trimmed.get(close_name + 1..).unwrap_or_default().trim();
            (name, HashMap::new(), tail)
        } else {
            return Err(self.error(
                ParseErrorKind::MalformedDirective,
                Span::new(line.start, line.end),
                "Directive name is missing closing ']'.",
                Some("Use @[Name](key=value) or @[Name] { body }.".to_string()),
            ));
        };

        self.index += 1;
        let (children, end) = if tail.starts_with('{') {
            if tail == "{" {
                self.parse_braced_children(line.start)?
            } else if tail.ends_with('}') {
                let inline_body = tail.trim_start_matches('{').trim_end_matches('}').trim();
                let children = if inline_body.is_empty() {
                    Vec::new()
                } else {
                    parse_document(inline_body)?.children
                };
                (children, line.end)
            } else {
                return Err(self.error(
                    ParseErrorKind::MalformedDirective,
                    Span::new(line.start, line.end),
                    "Directive body starts with '{' but does not close on the same line or as a block.",
                    Some("Use either @[Name](props) { body } or put the closing brace on its own line.".to_string()),
                ));
            }
        } else {
            (Vec::new(), line.end)
        };
        if name == "Meta" {
            apply_meta(&mut self.metadata, &props);
        }
        if builtin_directive(&name) {
            Ok(Node::Directive {
                name,
                props,
                children,
                span: Span::new(line.start, end),
            })
        } else {
            Ok(Node::Extension {
                name,
                props,
                children,
                raw_source: trimmed.to_string(),
                span: Span::new(line.start, end),
            })
        }
    }

    fn parse_ref_def(&mut self) -> Result<Node, ParseError> {
        let line = self.current().cloned().ok_or_else(|| {
            let span = self
                .current()
                .map(|l| Span::new(l.start, l.end))
                .unwrap_or_default();
            self.error(
                ParseErrorKind::UnexpectedToken,
                span,
                "Expected reference definition",
                None,
            )
        })?;
        self.index += 1;
        let trimmed = line.text.trim();
        let close = trimmed.find("]:").unwrap_or(0);
        let id = trimmed.get(1..close).unwrap_or_default().to_string();
        let rest = trimmed.get(close + 2..).unwrap_or_default().trim();
        let (url, title) = split_url_title(rest);
        Ok(Node::ReferenceDefinition {
            id,
            url,
            title,
            span: Span::new(line.start, line.end),
        })
    }

    fn parse_footnote_def(&mut self) -> Result<Node, ParseError> {
        let line = self.current().cloned().ok_or_else(|| {
            let span = self
                .current()
                .map(|l| Span::new(l.start, l.end))
                .unwrap_or_default();
            self.error(
                ParseErrorKind::UnexpectedToken,
                span,
                "Expected footnote definition",
                None,
            )
        })?;
        self.index += 1;
        let trimmed = line.text.trim();
        let close = trimmed.find("]:").unwrap_or(0);
        let marker = trimmed.get(2..close).unwrap_or_default().to_string();
        let body = trimmed.get(close + 2..).unwrap_or_default().trim();
        let children = vec![Node::Paragraph {
            children: self.parse_inline_sequence(body, line.start)?,
            span: Span::new(line.start, line.end),
        }];
        Ok(Node::FootnoteDefinition {
            marker,
            children,
            span: Span::new(line.start, line.end),
        })
    }

    fn next_is_pipe_separator(&self) -> bool {
        self.lines
            .get(self.index + 1)
            .is_some_and(|line| is_pipe_separator(line.text.trim()))
    }

    fn parse_inline_sequence(
        &mut self,
        text: &str,
        offset: usize,
    ) -> Result<Vec<InlineNode>, ParseError> {
        let mut stack = Vec::new();
        let (nodes, _) = self.parse_inline_until(text, 0, offset, None, &mut stack)?;
        Ok(nodes)
    }

    fn parse_inline_until(
        &mut self,
        text: &str,
        mut pos: usize,
        offset: usize,
        end: Option<&str>,
        stack: &mut Vec<&'static str>,
    ) -> Result<(Vec<InlineNode>, usize), ParseError> {
        let mut nodes = Vec::new();
        while pos < text.len() {
            if let Some(delim) = end {
                if text.get(pos..).is_some_and(|s| s.starts_with(delim)) {
                    return Ok((nodes, pos + delim.len()));
                }
            }
            let remaining = text.get(pos..).unwrap_or_default();
            if remaining.starts_with('\\') && remaining.len() > 1 {
                let ch = remaining.chars().nth(1).unwrap_or('\\');
                nodes.push(InlineNode::Text {
                    value: ch.to_string(),
                    span: Span::new(offset + pos, offset + pos + ch.len_utf8() + 1),
                });
                pos += 1 + ch.len_utf8();
            } else if remaining.starts_with('*') {
                nodes.push(self.parse_delimited(
                    text,
                    &mut pos,
                    offset,
                    "*",
                    "bold",
                    stack,
                    |children, span| InlineNode::Bold { children, span },
                )?);
            } else if remaining.starts_with('/') {
                nodes.push(self.parse_delimited(
                    text,
                    &mut pos,
                    offset,
                    "/",
                    "italic",
                    stack,
                    |children, span| InlineNode::Italic { children, span },
                )?);
            } else if remaining.starts_with('_') {
                nodes.push(self.parse_delimited(
                    text,
                    &mut pos,
                    offset,
                    "_",
                    "underline",
                    stack,
                    |children, span| InlineNode::Underline { children, span },
                )?);
            } else if remaining.starts_with('~') {
                nodes.push(self.parse_delimited(
                    text,
                    &mut pos,
                    offset,
                    "~",
                    "strike",
                    stack,
                    |children, span| InlineNode::Strikethrough { children, span },
                )?);
            } else if remaining.starts_with('^') {
                nodes.push(self.parse_delimited(
                    text,
                    &mut pos,
                    offset,
                    "^",
                    "superscript",
                    stack,
                    |children, span| InlineNode::Superscript { children, span },
                )?);
            } else if remaining.starts_with(",,") {
                nodes.push(self.parse_delimited(
                    text,
                    &mut pos,
                    offset,
                    ",,",
                    "subscript",
                    stack,
                    |children, span| InlineNode::Subscript { children, span },
                )?);
            } else if remaining.starts_with('`') {
                nodes.push(self.parse_raw_inline(
                    text,
                    &mut pos,
                    offset,
                    "`",
                    ParseErrorKind::UnterminatedDelimiter,
                    |value, span| InlineNode::Code { value, span },
                )?);
            } else if remaining.starts_with('$') {
                nodes.push(self.parse_raw_inline(
                    text,
                    &mut pos,
                    offset,
                    "$",
                    ParseErrorKind::MalformedMath,
                    |source, span| InlineNode::MathInline { source, span },
                )?);
            } else if remaining.starts_with("@{") {
                nodes.push(self.parse_var_ref(text, &mut pos, offset)?);
            } else if remaining.starts_with("@[") {
                nodes.push(self.parse_inline_component(text, &mut pos, offset)?);
            } else if remaining.starts_with("[[") {
                nodes.push(self.parse_citation(text, &mut pos, offset)?);
            } else if remaining.starts_with("[^") {
                nodes.push(self.parse_footnote_ref(text, &mut pos, offset)?);
            } else if remaining.starts_with("![") {
                nodes.push(self.parse_image(text, &mut pos, offset)?);
            } else if remaining.starts_with('[') {
                nodes.push(self.parse_link(text, &mut pos, offset)?);
            } else {
                let start = pos;
                let mut end_pos = pos;
                while end_pos < text.len() {
                    let s = text.get(end_pos..).unwrap_or_default();
                    if starts_inline(s) || end.is_some_and(|d| s.starts_with(d)) {
                        break;
                    }
                    if let Some(ch) = s.chars().next() {
                        end_pos += ch.len_utf8();
                    } else {
                        break;
                    }
                }
                if end_pos == start {
                    end_pos += 1;
                }
                let value = text.get(start..end_pos).unwrap_or_default().to_string();
                nodes.push(InlineNode::Text {
                    value,
                    span: Span::new(offset + start, offset + end_pos),
                });
                pos = end_pos;
            }
        }
        if let Some(delim) = end {
            Err(self.error(
                ParseErrorKind::UnterminatedDelimiter,
                Span::new(offset, offset + text.len()),
                format!("Unterminated inline delimiter '{delim}'."),
                Some(format!("Add a closing {delim}.")),
            ))
        } else {
            Ok((nodes, pos))
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn parse_delimited<F>(
        &mut self,
        text: &str,
        pos: &mut usize,
        offset: usize,
        delimiter: &'static str,
        name: &'static str,
        stack: &mut Vec<&'static str>,
        make: F,
    ) -> Result<InlineNode, ParseError>
    where
        F: FnOnce(Vec<InlineNode>, Span) -> InlineNode,
    {
        if stack.contains(&name) {
            return Err(self.error(
                ParseErrorKind::UnterminatedDelimiter,
                Span::new(offset + *pos, offset + *pos + delimiter.len()),
                format!("Self-nested {name} markup is not allowed."),
                Some(
                    "Close the current delimiter before opening another of the same kind."
                        .to_string(),
                ),
            ));
        }
        let start = *pos;
        *pos += delimiter.len();
        stack.push(name);
        let (children, consumed) =
            self.parse_inline_until(text, *pos, offset, Some(delimiter), stack)?;
        let _ = stack.pop();
        *pos = consumed;
        Ok(make(children, Span::new(offset + start, offset + *pos)))
    }

    fn parse_raw_inline<F>(
        &mut self,
        text: &str,
        pos: &mut usize,
        offset: usize,
        delimiter: &str,
        kind: ParseErrorKind,
        make: F,
    ) -> Result<InlineNode, ParseError>
    where
        F: FnOnce(String, Span) -> InlineNode,
    {
        let start = *pos;
        *pos += delimiter.len();
        let Some(close_rel) = text.get(*pos..).and_then(|s| s.find(delimiter)) else {
            return Err(self.error(
                kind,
                Span::new(offset + start, offset + text.len()),
                format!("Unterminated inline delimiter '{delimiter}'."),
                Some(format!("Add a closing {delimiter}.")),
            ));
        };
        let close = *pos + close_rel;
        let value = text.get(*pos..close).unwrap_or_default().to_string();
        *pos = close + delimiter.len();
        Ok(make(value, Span::new(offset + start, offset + *pos)))
    }

    fn parse_var_ref(
        &mut self,
        text: &str,
        pos: &mut usize,
        offset: usize,
    ) -> Result<InlineNode, ParseError> {
        let start = *pos;
        let Some(close_rel) = text.get(*pos + 2..).and_then(|s| s.find('}')) else {
            return Err(self.error(
                ParseErrorKind::UnterminatedDelimiter,
                Span::new(offset + start, offset + text.len()),
                "Variable interpolation is missing a closing '}'.",
                None,
            ));
        };
        let close = *pos + 2 + close_rel;
        let name = text.get(*pos + 2..close).unwrap_or_default().to_string();
        if !self.variables.contains(&name) {
            self.warnings.push(ParseError::new(
                ParseErrorKind::UndefinedReference,
                Span::new(offset + start, offset + close + 1),
                format!("Variable '{name}' is not declared before use."),
                Some(
                    "Declare it with @var before referencing it, or provide it at runtime."
                        .to_string(),
                ),
            ));
        }
        *pos = close + 1;
        Ok(InlineNode::VarInterpolation {
            name,
            span: Span::new(offset + start, offset + *pos),
        })
    }

    fn parse_inline_component(
        &mut self,
        text: &str,
        pos: &mut usize,
        offset: usize,
    ) -> Result<InlineNode, ParseError> {
        let start = *pos;
        let Some(close_name) = text.get(*pos..).and_then(|s| s.find("](")) else {
            return Err(self.error(
                ParseErrorKind::MalformedDirective,
                Span::new(offset + start, offset + text.len()),
                "Inline component is missing ]( after its name.",
                None,
            ));
        };
        let name_start = *pos + 2;
        let name_end = *pos + close_name;
        let name = text
            .get(name_start..name_end)
            .unwrap_or_default()
            .to_string();
        let props_start = name_end + 2;
        let Some(close_props_rel) = text.get(props_start..).and_then(find_matching_paren) else {
            return Err(self.error(
                ParseErrorKind::MalformedDirective,
                Span::new(offset + start, offset + text.len()),
                "Inline component properties are not closed.",
                None,
            ));
        };
        let props_end = props_start + close_props_rel;
        let props = parse_props(text.get(props_start..props_end).unwrap_or_default()).map_err(
            |message| {
                self.error(
                    ParseErrorKind::InvalidPropValue,
                    Span::new(offset + start, offset + props_end),
                    message,
                    None,
                )
            },
        )?;
        *pos = props_end + 1;
        Ok(InlineNode::InlineComponent {
            name,
            props,
            span: Span::new(offset + start, offset + *pos),
        })
    }

    fn parse_citation(
        &self,
        text: &str,
        pos: &mut usize,
        offset: usize,
    ) -> Result<InlineNode, ParseError> {
        let start = *pos;
        let Some(close_rel) = text.get(*pos + 2..).and_then(|s| s.find("]]")) else {
            return Err(self.error(
                ParseErrorKind::UnterminatedDelimiter,
                Span::new(offset + start, offset + text.len()),
                "Citation is missing closing ]].",
                None,
            ));
        };
        let close = *pos + 2 + close_rel;
        let key = text.get(*pos + 2..close).unwrap_or_default().to_string();
        *pos = close + 2;
        Ok(InlineNode::Citation {
            key,
            span: Span::new(offset + start, offset + *pos),
        })
    }

    fn parse_footnote_ref(
        &self,
        text: &str,
        pos: &mut usize,
        offset: usize,
    ) -> Result<InlineNode, ParseError> {
        let start = *pos;
        let Some(close_rel) = text.get(*pos + 2..).and_then(|s| s.find(']')) else {
            return Err(self.error(
                ParseErrorKind::UnterminatedDelimiter,
                Span::new(offset + start, offset + text.len()),
                "Footnote reference is missing closing ].",
                None,
            ));
        };
        let close = *pos + 2 + close_rel;
        let marker = text.get(*pos + 2..close).unwrap_or_default().to_string();
        *pos = close + 1;
        Ok(InlineNode::FootnoteRef {
            marker,
            span: Span::new(offset + start, offset + *pos),
        })
    }

    fn parse_image(
        &mut self,
        text: &str,
        pos: &mut usize,
        offset: usize,
    ) -> Result<InlineNode, ParseError> {
        let start = *pos;
        let Some(close_alt_rel) = text.get(*pos + 2..).and_then(|s| s.find(']')) else {
            return Err(self.error(
                ParseErrorKind::UnterminatedDelimiter,
                Span::new(offset + start, offset + text.len()),
                "Image alt text is missing closing ].",
                None,
            ));
        };
        let alt_end = *pos + 2 + close_alt_rel;
        let alt = text.get(*pos + 2..alt_end).unwrap_or_default().to_string();
        let after = text.get(alt_end + 1..).unwrap_or_default();
        if after.starts_with('(') {
            let Some(close_url_rel) = after.get(1..).and_then(|s| s.find(')')) else {
                return Err(self.error(
                    ParseErrorKind::UnterminatedDelimiter,
                    Span::new(offset + start, offset + text.len()),
                    "Image URL is missing closing ).",
                    None,
                ));
            };
            let inside = after.get(1..1 + close_url_rel).unwrap_or_default();
            let (src, title) = split_url_title(inside);
            *pos = alt_end + 2 + close_url_rel + 1;
            Ok(InlineNode::Image {
                src,
                alt,
                title,
                span: Span::new(offset + start, offset + *pos),
            })
        } else if after.starts_with('[') {
            let Some(close_ref_rel) = after.get(1..).and_then(|s| s.find(']')) else {
                return Err(self.error(
                    ParseErrorKind::UnterminatedDelimiter,
                    Span::new(offset + start, offset + text.len()),
                    "Image reference is missing closing ].",
                    None,
                ));
            };
            let id = after
                .get(1..1 + close_ref_rel)
                .unwrap_or_default()
                .to_string();
            *pos = alt_end + 2 + close_ref_rel + 1;
            Ok(InlineNode::ImageRef {
                id,
                alt,
                span: Span::new(offset + start, offset + *pos),
            })
        } else {
            Err(self.error(
                ParseErrorKind::UnexpectedToken,
                Span::new(offset + start, offset + alt_end),
                "Image must be followed by a URL or reference.",
                None,
            ))
        }
    }

    fn parse_link(
        &mut self,
        text: &str,
        pos: &mut usize,
        offset: usize,
    ) -> Result<InlineNode, ParseError> {
        let start = *pos;
        let Some(close_text_rel) = text.get(*pos + 1..).and_then(|s| s.find(']')) else {
            return Err(self.error(
                ParseErrorKind::UnterminatedDelimiter,
                Span::new(offset + start, offset + text.len()),
                "Link text is missing closing ].",
                None,
            ));
        };
        let text_end = *pos + 1 + close_text_rel;
        let link_text = text.get(*pos + 1..text_end).unwrap_or_default();
        let children = self.parse_inline_sequence(link_text, offset + *pos + 1)?;
        let after = text.get(text_end + 1..).unwrap_or_default();
        if after.starts_with('(') {
            let Some(close_url_rel) = after.get(1..).and_then(|s| s.find(')')) else {
                return Err(self.error(
                    ParseErrorKind::UnterminatedDelimiter,
                    Span::new(offset + start, offset + text.len()),
                    "Link URL is missing closing ).",
                    None,
                ));
            };
            let inside = after.get(1..1 + close_url_rel).unwrap_or_default();
            let (href, title) = split_url_title(inside);
            *pos = text_end + 2 + close_url_rel + 1;
            Ok(InlineNode::Link {
                href,
                title,
                children,
                span: Span::new(offset + start, offset + *pos),
            })
        } else if after.starts_with('[') {
            let Some(close_ref_rel) = after.get(1..).and_then(|s| s.find(']')) else {
                return Err(self.error(
                    ParseErrorKind::UnterminatedDelimiter,
                    Span::new(offset + start, offset + text.len()),
                    "Link reference is missing closing ].",
                    None,
                ));
            };
            let id = after
                .get(1..1 + close_ref_rel)
                .unwrap_or_default()
                .to_string();
            *pos = text_end + 2 + close_ref_rel + 1;
            Ok(InlineNode::LinkRef {
                id,
                children,
                span: Span::new(offset + start, offset + *pos),
            })
        } else {
            Err(self.error(
                ParseErrorKind::UnexpectedToken,
                Span::new(offset + start, offset + text_end),
                "Link must be followed by a URL or reference.",
                None,
            ))
        }
    }

    fn error(
        &self,
        kind: ParseErrorKind,
        span: Span,
        message: impl Into<String>,
        suggestion: Option<String>,
    ) -> ParseError {
        ParseError::new(kind, span, message, suggestion)
    }
}

fn is_heading(trim: &str) -> bool {
    trim.starts_with('=') && trim.contains(' ')
}

fn starts_block(trim: &str) -> bool {
    trim.starts_with("```")
        || trim == "$$"
        || is_heading(trim)
        || trim.starts_with('>')
        || trim.starts_with('+')
        || trim.starts_with('|')
        || is_hrule(trim)
        || is_ordered_marker(trim)
        || trim.starts_with("- ")
        || trim.starts_with(":: ")
        || trim.starts_with("@var ")
        || trim.starts_with("@for ")
        || trim.starts_with("@if ")
        || trim.starts_with("@[")
        || (trim.starts_with("[^") && trim.contains("]:"))
        || (trim.starts_with('[') && trim.contains("]:"))
}

fn is_hrule(trim: &str) -> bool {
    trim.len() >= 3 && trim.chars().all(|ch| ch == '-')
}

fn is_ordered_marker(trim: &str) -> bool {
    let Some(dot) = trim.find(". ") else {
        return false;
    };
    dot > 0
        && trim
            .get(..dot)
            .is_some_and(|s| s.chars().all(|ch| ch.is_ascii_digit()))
}

fn is_pipe_separator(trim: &str) -> bool {
    trim.starts_with('|')
        && trim
            .chars()
            .all(|ch| ch == '|' || ch == '-' || ch == ':' || ch == ' ')
}

fn parse_pipe_alignments(separator: &str) -> Vec<Option<Alignment>> {
    separator
        .trim_matches('|')
        .split('|')
        .map(|cell| {
            let trimmed = cell.trim();
            let left = trimmed.starts_with(':');
            let right = trimmed.ends_with(':');
            match (left, right) {
                (true, true) => Some(Alignment::Center),
                (true, false) => Some(Alignment::Left),
                (false, true) => Some(Alignment::Right),
                (false, false) => None,
            }
        })
        .collect()
}

fn grid_boundaries(border: &str) -> Option<Vec<usize>> {
    if !border.starts_with('+') || !border.ends_with('+') {
        return None;
    }
    if !border.chars().all(|ch| ch == '+' || ch == '-') {
        return None;
    }
    let boundaries = border
        .char_indices()
        .filter_map(|(index, ch)| (ch == '+').then_some(index))
        .collect::<Vec<_>>();
    (boundaries.len() >= 2).then_some(boundaries)
}

fn boundary_index(boundaries: &[usize], value: usize) -> Option<usize> {
    boundaries.iter().position(|boundary| *boundary == value)
}

fn starts_inline(s: &str) -> bool {
    ["*", "/", "_", "~", "^", ",,", "`", "$", "@{", "@["]
        .iter()
        .any(|prefix| s.starts_with(prefix))
        || s.starts_with("[[")
        || s.starts_with("[^")
        || s.starts_with("![")
        || s.starts_with('[')
}

fn is_ident(value: &str) -> bool {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first.is_alphabetic() || first == '_') && chars.all(|ch| ch.is_alphanumeric() || ch == '_')
}

fn parse_json_value(raw: &str) -> Result<JsonValue, String> {
    serde_json::from_str(raw).or_else(|_| {
        if raw == "true" || raw == "false" || raw == "null" || raw.starts_with('[') {
            serde_json::from_str(raw).map_err(|err| format!("Invalid JSON value: {err}"))
        } else {
            Ok(JsonValue::String(raw.trim_matches('"').to_string()))
        }
    })
}

fn parse_props(raw: &str) -> Result<HashMap<String, PropValue>, String> {
    let mut props = HashMap::new();
    let mut pos = 0usize;
    while pos < raw.len() {
        while raw.get(pos..).is_some_and(|s| s.starts_with(' ')) {
            pos += 1;
        }
        if pos >= raw.len() {
            break;
        }
        let key_start = pos;
        while pos < raw.len() {
            let s = raw.get(pos..).unwrap_or_default();
            if s.starts_with('=') || s.starts_with(' ') {
                break;
            }
            if let Some(ch) = s.chars().next() {
                pos += ch.len_utf8();
            } else {
                break;
            }
        }
        let key = raw
            .get(key_start..pos)
            .unwrap_or_default()
            .trim()
            .to_string();
        while raw.get(pos..).is_some_and(|s| s.starts_with(' ')) {
            pos += 1;
        }
        if !raw.get(pos..).is_some_and(|s| s.starts_with('=')) {
            return Err(format!("Property '{key}' is missing '='."));
        }
        pos += 1;
        while raw.get(pos..).is_some_and(|s| s.starts_with(' ')) {
            pos += 1;
        }
        let value = if raw.get(pos..).is_some_and(|s| s.starts_with('"')) {
            pos += 1;
            let start = pos;
            let Some(close_rel) = raw.get(pos..).and_then(|s| s.find('"')) else {
                return Err(format!("Property '{key}' string is not closed."));
            };
            let end = pos + close_rel;
            let value = PropValue::String(raw.get(start..end).unwrap_or_default().to_string());
            pos = end + 1;
            value
        } else if raw.get(pos..).is_some_and(|s| s.starts_with("@{")) {
            pos += 2;
            let start = pos;
            let Some(close_rel) = raw.get(pos..).and_then(|s| s.find('}')) else {
                return Err(format!(
                    "Property '{key}' variable reference is not closed."
                ));
            };
            let end = pos + close_rel;
            let value = PropValue::Variable(raw.get(start..end).unwrap_or_default().to_string());
            pos = end + 1;
            value
        } else {
            let start = pos;
            while pos < raw.len() && !raw.get(pos..).is_some_and(|s| s.starts_with(' ')) {
                if let Some(ch) = raw.get(pos..).and_then(|s| s.chars().next()) {
                    pos += ch.len_utf8();
                } else {
                    break;
                }
            }
            let token = raw.get(start..pos).unwrap_or_default();
            if token == "true" {
                PropValue::Bool(true)
            } else if token == "false" {
                PropValue::Bool(false)
            } else if token == "null" {
                PropValue::Null
            } else if let Ok(number) = token.parse::<f64>() {
                PropValue::Number(number)
            } else {
                PropValue::String(token.to_string())
            }
        };
        if !key.is_empty() {
            props.insert(key, value);
        }
    }
    Ok(props)
}

fn find_matching_paren(input: &str) -> Option<usize> {
    let mut in_string = false;
    for (index, ch) in input.char_indices() {
        if ch == '"' {
            in_string = !in_string;
        } else if ch == ')' && !in_string {
            return Some(index);
        }
    }
    None
}

fn split_url_title(input: &str) -> (String, Option<String>) {
    let trimmed = input.trim();
    if let Some(quote) = trimmed.find('"') {
        let url = trimmed.get(..quote).unwrap_or_default().trim().to_string();
        let title = trimmed.get(quote + 1..).and_then(|rest| {
            rest.find('"')
                .map(|end| rest.get(..end).unwrap_or_default().to_string())
        });
        (url, title)
    } else {
        (trimmed.to_string(), None)
    }
}

fn builtin_directive(name: &str) -> bool {
    matches!(
        name,
        "Figure"
            | "Code"
            | "Diagram"
            | "Chart"
            | "Cite"
            | "Slide"
            | "Animation"
            | "Frame"
            | "Layout"
            | "Column"
            | "Accessibility"
            | "Theme"
            | "Meta"
            | "Slider"
            // Phase 8: Professional Math Engine
            | "Equation"
            | "Align"
            | "Matrix"
            | "PMatrix"
            | "BMatrix"
            | "VMatrix"
            | "Cases"
            | "Chem"
            // Theorem environments
            | "Theorem"
            | "Proof"
            | "Lemma"
            | "Corollary"
            | "Definition"
            | "Remark"
            | "Example"
            | "Conjecture"
            | "Axiom"
            | "Proposition"
            | "Notation"
            // Phase 9: Cross-references & Bibliography
            | "Ref"
            | "Toc"
            | "Lof"
            | "Lot"
            // Phase 10: Native Diagrams
            | "Plot"
            // Phase 11: Interactive Documents
            | "Input"
            | "Select"
            | "Checkbox"
            | "Data"
            // Phase 12: Multi-Format Publishing
            | "Include"
            // Phase 13: Package & Extension Ecosystem
            | "Template"
            // Phase 9: Bibliography
            | "Bibliography"
    )
}

fn apply_meta(metadata: &mut DocumentMetadata, props: &HashMap<String, PropValue>) {
    for (key, value) in props {
        let string_value = match value {
            PropValue::String(value) | PropValue::Variable(value) => Some(value.clone()),
            PropValue::Number(value) => Some(value.to_string()),
            PropValue::Bool(value) => Some(value.to_string()),
            PropValue::Null => None,
        };
        match (key.as_str(), string_value) {
            ("title", Some(value)) => metadata.title = Some(value),
            ("author", Some(value)) => metadata.author = Some(value),
            ("date", Some(value)) => metadata.date = Some(value),
            ("lang", Some(value)) => metadata.lang = Some(value),
            _ => {}
        }
    }
}

pub fn slugify_inline(nodes: &[InlineNode]) -> String {
    let raw = crate::ast::format_inline_nodes(nodes);
    let mut slug = String::new();
    let mut last_dash = false;
    for ch in raw.chars() {
        if ch.is_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
            last_dash = false;
        } else if !last_dash {
            slug.push('-');
            last_dash = true;
        }
    }
    slug.trim_matches('-').to_string()
}
