// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Samin Yeasar

//! Language Server Protocol implementation for Vell.
//!
//! Phase 14 features: incremental sync, folding ranges, code actions,
//! references, rename, hierarchical document symbols.
#![allow(deprecated)]
#![allow(clippy::collapsible_if)]
#![allow(clippy::needless_borrow)]
#![allow(clippy::ptr_arg)]
#![allow(clippy::clone_on_copy)]
#![allow(clippy::useless_format)]
#![allow(clippy::needless_pass_by_value)]
#![allow(clippy::redundant_pattern_matching)]
#![allow(clippy::needless_pass_by_value)]
#![allow(clippy::collapsible_if)]
#![allow(clippy::collapsible_match)]
#![allow(clippy::needless_borrow)]
#![allow(unused_variables)]

//! references, rename, hierarchical document symbols.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::RwLock;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};
use vell_core::{parse_document, parse_document_from, validate, Document, InlineNode, Node, ParseError, PropValue, Span};

/// Cached document state for an open file.
#[derive(Clone, Default)]
struct VellDocument {
    source: String,
    parsed: Option<Document>,
    #[allow(dead_code)]
    errors: Vec<ParseError>,
}

/// A raw semantic token before relative-position encoding.
// Uses tower_lsp::lsp_types::SemanticToken for the final encoded output.
/// LSP backend state.
struct Backend {
    client: Option<Client>,
    documents: Arc<RwLock<HashMap<Url, VellDocument>>>,
}

impl Backend {
    /// Converts a byte offset into an LSP Position (0-based line, 0-based character).
    fn byte_to_position(source: &str, byte_offset: usize) -> Position {
        let mut line = 0u32;
        let mut col = 0u32;
        for (i, ch) in source.char_indices() {
            if i >= byte_offset {
                break;
            }
            if ch == '\n' {
                line += 1;
                col = 0;
            } else {
                col += 1;
            }
        }
        Position::new(line, col)
    }

    /// Converts a vell_core::Span (byte offsets) into an LSP Range.
    fn span_to_range(source: &str, span: &vell_core::Span) -> Range {
        Range {
            start: Self::byte_to_position(source, span.start),
            end: Self::byte_to_position(source, span.end),
        }
    }

    /// Converts an LSP Range to a (start_byte, end_byte) pair.
    fn range_to_byte_range(source: &str, range: &Range) -> (usize, usize) {
        let start = Self::position_to_byte(source, range.start);
        let end = Self::position_to_byte(source, range.end);
        (start, end)
    }

    /// Returns a list of all variable names declared in the document.
    fn declared_variables(doc: &Document) -> Vec<String> {
        doc.children
            .iter()
            .filter_map(|node| {
                if let Node::VarDeclaration { name, .. } = node {
                    Some(name.clone())
                } else {
                    None
                }
            })
            .collect()
    }

    /// Returns all variable reference spans within the document (source byte ranges).
    fn find_var_references<'a>(doc: &'a Document, name: &str) -> Vec<&'a InlineNode> {
        let mut refs = Vec::new();
        for node in &doc.children {
            Self::collect_var_refs(node, name, &mut refs);
        }
        refs
    }

    fn collect_var_refs<'a>(node: &'a Node, name: &str, refs: &mut Vec<&'a InlineNode>) {
        match node {
            Node::Paragraph { children, .. } | Node::Heading { children, .. } => {
                for child in children {
                    Self::collect_var_refs_inline(child, name, refs);
                }
            }
            Node::Blockquote { children, .. } | Node::ForLoop { children, .. } => {
                for child in children {
                    Self::collect_var_refs(child, name, refs);
                }
            }
            Node::IfBlock {
                consequent,
                alternate,
                ..
            } => {
                for child in consequent {
                    Self::collect_var_refs(child, name, refs);
                }
                if let Some(alt) = alternate {
                    for child in alt {
                        Self::collect_var_refs(child, name, refs);
                    }
                }
            }
            Node::Directive { children, .. } | Node::Extension { children, .. } => {
                for child in children {
                    Self::collect_var_refs(child, name, refs);
                }
            }
            Node::List { items, .. } => {
                for item in items {
                    for child in &item.children {
                        Self::collect_var_refs(child, name, refs);
                    }
                }
            }
            Node::DefinitionList { items, .. } => {
                for item in items {
                    for child in &item.definition {
                        Self::collect_var_refs(child, name, refs);
                    }
                }
            }
            Node::Table { headers, rows, .. } => {
                for cell in headers {
                    for child in &cell.children {
                        Self::collect_var_refs_inline(child, name, refs);
                    }
                }
                for row in rows {
                    for cell in row {
                        for child in &cell.children {
                            Self::collect_var_refs_inline(child, name, refs);
                        }
                    }
                }
            }
            _ => {}
        }
    }

    fn collect_var_refs_inline<'a>(
        node: &'a InlineNode,
        name: &str,
        refs: &mut Vec<&'a InlineNode>,
    ) {
        if let InlineNode::VarInterpolation { name: n, .. } = node {
            if n == name {
                refs.push(node);
            }
        }
        // Recurse into child inlines
        match node {
            InlineNode::Bold { children, .. }
            | InlineNode::Italic { children, .. }
            | InlineNode::Underline { children, .. }
            | InlineNode::Strikethrough { children, .. }
            | InlineNode::Superscript { children, .. }
            | InlineNode::Subscript { children, .. }
            | InlineNode::Link { children, .. }
            | InlineNode::LinkRef { children, .. } => {
                for child in children {
                    Self::collect_var_refs_inline(child, name, refs);
                }
            }
            _ => {}
        }
    }

    /// Finds the heading at a given position.
    fn find_heading_at<'a>(doc: &'a Document, pos: Position, source: &str) -> Option<&'a Node> {
        doc.children.iter().find(|node| {
            if let Node::Heading { span, .. } = node {
                let range = Self::span_to_range(source, span);
                range.start <= pos && pos <= range.end
            } else {
                false
            }
        })
    }

    /// Looks up a variable declaration by name.
    fn find_variable_decl<'a>(doc: &'a Document, name: &str) -> Option<&'a Node> {
        doc.children.iter().find(|node| {
            if let Node::VarDeclaration { name: n, .. } = node {
                n == name
            } else {
                false
            }
        })
    }

    /// Finds a variable declaration at the given position.
    fn find_variable_decl_at<'a>(
        doc: &'a Document,
        pos: Position,
        source: &str,
    ) -> Option<&'a Node> {
        doc.children.iter().find(|node| {
            if let Node::VarDeclaration { name: _, span, .. } = node {
                let range = Self::span_to_range(source, span);
                range.start <= pos && pos <= range.end
            } else {
                false
            }
        })
    }

    /// Collect code lenses for @var declarations showing reference counts.
    fn collect_code_lenses(doc: &Document, source: &str, uri: &Url) -> Vec<CodeLens> {
        let mut lenses = Vec::new();
        for node in &doc.children {
            if let Node::VarDeclaration { name, span, .. } = node {
                let range = Self::span_to_range(source, span);
                let refs = Self::find_var_references(doc, name);
                let count = refs.len();
                let title = if count == 1 {
                    "1 reference".to_string()
                } else {
                    format!("{} references", count)
                };
                lenses.push(CodeLens {
                    range,
                    command: Some(Command {
                        title,
                        command: String::new(),
                        arguments: None,
                    }),
                    data: None,
                });
            }
        }
        lenses
    }

    /// Parse a hex color string (e.g., "#ff8800", "#abc", "#ff8800cc") into an lsp Color.
    fn hex_to_color(hex: &str) -> Option<Color> {
        let hex = hex.trim_start_matches('#');
        // Expand short forms (3/4 hex digits) to full 6/8 form by doubling each digit
        let expanded: String = match hex.len() {
            3 => hex.chars().flat_map(|c| [c, c]).collect(),
            4 => hex.chars().flat_map(|c| [c, c]).collect(),
            6 => hex.to_string(),
            8 => hex.to_string(),
            _ => return None,
        };

        let r = u8::from_str_radix(&expanded[0..2], 16).ok()? as f32 / 255.0;
        let g = u8::from_str_radix(&expanded[2..4], 16).ok()? as f32 / 255.0;
        let b = u8::from_str_radix(&expanded[4..6], 16).ok()? as f32 / 255.0;
        let a: f32 = if expanded.len() >= 8 {
            u8::from_str_radix(&expanded[6..8], 16).ok()? as f32 / 255.0
        } else {
            1.0
        };

        Some(Color {
            red: r,
            green: g,
            blue: b,
            alpha: a,
        })
    }

    /// Collect all color values in the document as ColorInformation.
    fn collect_document_colors(source: &str) -> Vec<ColorInformation> {
        let mut colors = Vec::new();
        // Match hex color patterns: #rgb, #rrggbb, #rgba, #rrggbbaa
        let mut pos = 0usize;
        let bytes = source.as_bytes();
        while pos < source.len() {
            if bytes[pos] == b'#' {
                // Determine the hex string length
                let start = pos;
                pos += 1;
                let hex_start = pos;
                while pos < source.len()
                    && ((bytes[pos] >= b'0' && bytes[pos] <= b'9')
                        || (bytes[pos] >= b'a' && bytes[pos] <= b'f')
                        || (bytes[pos] >= b'A' && bytes[pos] <= b'F'))
                {
                    pos += 1;
                }
                let hex_len = pos - hex_start;
                if hex_len == 3 || hex_len == 4 || hex_len == 6 || hex_len == 8 {
                    let hex_str = &source[hex_start..pos];
                    if let Some(color) = Self::hex_to_color(&format!("#{hex_str}")) {
                        let range = Self::span_to_range(source, &vell_core::Span::new(start, pos));
                        colors.push(ColorInformation { range, color });
                    }
                }
            } else {
                pos += 1;
            }
        }
        colors
    }

    /// Collect document links from Link and Image inline nodes, plus bare URLs in raw text.
    fn collect_document_links(doc: &Document, source: &str) -> Vec<DocumentLink> {
        let mut links = Vec::new();
        for node in &doc.children {
            Self::collect_links_node(node, source, &mut links);
        }
        links
    }

    fn collect_links_node(node: &Node, source: &str, links: &mut Vec<DocumentLink>) {
        match node {
            Node::Heading { children, .. } | Node::Paragraph { children, .. } => {
                for child in children {
                    Self::collect_links_inline(child, source, links);
                }
            }
            Node::Blockquote { children, .. } | Node::ForLoop { children, .. } => {
                for child in children {
                    Self::collect_links_node(child, source, links);
                }
            }
            Node::IfBlock {
                consequent,
                alternate,
                ..
            } => {
                for child in consequent {
                    Self::collect_links_node(child, source, links);
                }
                if let Some(alt) = alternate {
                    for child in alt {
                        Self::collect_links_node(child, source, links);
                    }
                }
            }
            Node::Directive { children, .. } | Node::Extension { children, .. } => {
                for child in children {
                    Self::collect_links_node(child, source, links);
                }
            }
            Node::List { items, .. } => {
                for item in items {
                    for child in &item.children {
                        Self::collect_links_node(child, source, links);
                    }
                }
            }
            Node::DefinitionList { items, .. } => {
                for item in items {
                    for child in &item.definition {
                        Self::collect_links_node(child, source, links);
                    }
                }
            }
            Node::Table { headers, rows, .. } => {
                for cell in headers {
                    for child in &cell.children {
                        Self::collect_links_inline(child, source, links);
                    }
                }
                for row in rows {
                    for cell in row {
                        for child in &cell.children {
                            Self::collect_links_inline(child, source, links);
                        }
                    }
                }
            }
            _ => {}
        }
    }

    fn collect_links_inline(node: &InlineNode, source: &str, links: &mut Vec<DocumentLink>) {
        match node {
            InlineNode::Link { href, span, .. } if !href.is_empty() => {
                if let Ok(url) = Url::parse(href) {
                    let range = Self::span_to_range(source, span);
                    links.push(DocumentLink {
                        range,
                        tooltip: Some(href.clone()),
                        target: Some(url),
                        data: None,
                    });
                }
            }
            InlineNode::Image { src, span, .. } if !src.is_empty() => {
                if let Ok(url) = Url::parse(src) {
                    let range = Self::span_to_range(source, span);
                    links.push(DocumentLink {
                        range,
                        tooltip: Some(src.clone()),
                        target: Some(url),
                        data: None,
                    });
                }
            }
            // Recurse into children for formatting wrappers
            InlineNode::Bold { children, .. }
            | InlineNode::Italic { children, .. }
            | InlineNode::Underline { children, .. }
            | InlineNode::Strikethrough { children, .. }
            | InlineNode::Superscript { children, .. }
            | InlineNode::Subscript { children, .. }
            | InlineNode::LinkRef { children, .. } => {
                for child in children {
                    Self::collect_links_inline(child, source, links);
                }
            }
            _ => {}
        }
    }

    /// Try to find a variable reference at the given position in source.
    fn find_var_ref_at(source: &str, pos: Position) -> Option<String> {
        let byte_offset = Self::position_to_byte(source, pos);
        let before = &source[..byte_offset];
        let after = &source[byte_offset..];

        // Find the last "@{" before the cursor
        let last_open = before.rfind("@{");
        // Find the first "}" after the cursor
        let first_close = after.find('}');

        if let (Some(open), Some(close)) = (last_open, first_close) {
            let name_start = open + 2; // after "@{"
            let name_end = byte_offset + close;
            if name_start <= byte_offset && byte_offset <= name_end {
                let name = &source[name_start..name_end];
                return Some(name.to_string());
            }
        }

        None
    }
    /// Converts an LSP Position to a byte offset in source.
    fn position_to_byte(source: &str, pos: Position) -> usize {
        let mut line = 0u32;
        let mut col = 0u32;
        for (i, ch) in source.char_indices() {
            if line == pos.line && col == pos.character {
                return i;
            }
            if ch == '\n' {
                line += 1;
                col = 0;
            } else {
                col += 1;
            }
        }
        source.len()
    }

    /// Collect folding ranges from the AST.
    fn collect_folding_ranges(doc: &Document, source: &str) -> Vec<FoldingRange> {
        let mut ranges = Vec::new();
        for node in &doc.children {
            Self::collect_folding_node(node, source, &mut ranges, &doc.children);
        }
        ranges
    }

    fn collect_folding_node(
        node: &Node,
        source: &str,
        ranges: &mut Vec<FoldingRange>,
        siblings: &[Node],
    ) {
        let span = node.span();
        match node {
            Node::Heading { level, .. } => {
                let start_line = Self::byte_to_position(source, span.start).line;
                let end_line = Self::find_next_heading_end_line(level, siblings, node, source);

                if end_line > start_line + 1 {
                    ranges.push(FoldingRange {
                        start_line: start_line + 1,
                        end_line,
                        kind: Some(FoldingRangeKind::Region),
                        start_character: None,
                        end_character: None,
                        collapsed_text: None,
                    });
                }
            }
            Node::CodeBlock { source: code, .. } => {
                if !code.is_empty() {
                    let start = Self::byte_to_position(source, span.start).line;
                    let end = Self::byte_to_position(source, span.end).line;
                    if end > start + 1 {
                        ranges.push(FoldingRange {
                            start_line: start,
                            end_line: end,
                            kind: Some(FoldingRangeKind::Region),
                            start_character: None,
                            end_character: None,
                            collapsed_text: None,
                        });
                    }
                }
            }
            Node::MathBlock { .. } => {
                let start = Self::byte_to_position(source, span.start).line;
                let end = Self::byte_to_position(source, span.end).line;
                if end > start + 1 {
                    ranges.push(FoldingRange {
                        start_line: start,
                        end_line: end,
                        kind: Some(FoldingRangeKind::Region),
                        start_character: None,
                        end_character: None,
                        collapsed_text: None,
                    });
                }
            }
            Node::Directive { children, .. } | Node::Extension { children, .. } => {
                if !children.is_empty() {
                    let start = Self::byte_to_position(source, span.start).line;
                    let end = Self::byte_to_position(source, span.end).line;
                    if end > start + 1 {
                        ranges.push(FoldingRange {
                            start_line: start,
                            end_line: end,
                            kind: Some(FoldingRangeKind::Region),
                            start_character: None,
                            end_character: None,
                            collapsed_text: None,
                        });
                    }
                }
                for child in children {
                    Self::collect_folding_node(child, source, ranges, children);
                }
            }
            Node::Blockquote { children, .. } => {
                let start = Self::byte_to_position(source, span.start).line;
                let end = Self::byte_to_position(source, span.end).line;
                if end > start {
                    ranges.push(FoldingRange {
                        start_line: start,
                        end_line: end,
                        kind: Some(FoldingRangeKind::Region),
                        start_character: None,
                        end_character: None,
                        collapsed_text: None,
                    });
                }
                for child in children {
                    Self::collect_folding_node(child, source, ranges, children);
                }
            }
            Node::List { items, .. } if items.len() > 1 => {
                let start = Self::byte_to_position(source, span.start).line;
                let end = Self::byte_to_position(source, span.end).line;
                if end > start + 1 {
                    ranges.push(FoldingRange {
                        start_line: start,
                        end_line: end,
                        kind: Some(FoldingRangeKind::Region),
                        start_character: None,
                        end_character: None,
                        collapsed_text: None,
                    });
                }
            }
            Node::ForLoop { children, .. }
            | Node::IfBlock {
                consequent: children,
                ..
            } => {
                let start = Self::byte_to_position(source, span.start).line;
                let end = Self::byte_to_position(source, span.end).line;
                if end > start + 1 {
                    ranges.push(FoldingRange {
                        start_line: start,
                        end_line: end,
                        kind: Some(FoldingRangeKind::Region),
                        start_character: None,
                        end_character: None,
                        collapsed_text: None,
                    });
                }
                for child in children {
                    Self::collect_folding_node(child, source, ranges, children);
                }
            }
            _ => {}
        }
    }

    /// Collect semantic tokens from the document AST.
    fn collect_semantic_tokens(doc: &Document, source: &str) -> Vec<SemanticToken> {
        let mut tokens = Vec::new();
        for node in &doc.children {
            Self::collect_semantic_node(node, source, &mut tokens);
        }
        tokens
    }

    fn collect_semantic_node(node: &Node, source: &str, tokens: &mut Vec<SemanticToken>) {
        let span = node.span();
        match node {
            Node::Heading {
                level, children, ..
            } => {
                // Tokenize the heading markers
                if let Some(text) = source.get(span.start..span.end) {
                    let eq_count = text.chars().take_while(|c| *c == '=').count() as u32;
                    if eq_count > 0 {
                        tokens.push(SemanticToken {
                            delta_line: 0,
                            delta_start: 0,
                            length: eq_count,
                            token_type: 0, // heading
                            token_modifiers_bitset: (*level as u32).saturating_sub(1),
                        });
                    }
                }
                for child in children {
                    Self::collect_semantic_inline(child, source, &span, tokens);
                }
            }
            Node::Paragraph { children, .. } => {
                for child in children {
                    Self::collect_semantic_inline(child, source, &span, tokens);
                }
            }
            Node::Directive { name, children, .. } | Node::Extension { name, children, .. } => {
                // Tokenize @[name]
                let name_start = span.start + 2; // skip "@["
                let name_end = name_start + name.len();
                tokens.push(SemanticToken {
                    delta_line: 0,
                    delta_start: 0,
                    length: 2,     // "@["
                    token_type: 5, // punctuation
                    token_modifiers_bitset: 0,
                });
                tokens.push(SemanticToken {
                    delta_line: 0,
                    delta_start: 2,
                    length: name.len() as u32,
                    token_type: 1, // directive
                    token_modifiers_bitset: 0,
                });
                for child in children {
                    Self::collect_semantic_node(child, source, tokens);
                }
            }
            Node::CodeBlock { source: code, .. } => {
                if !code.is_empty() {
                    tokens.push(SemanticToken {
                        delta_line: 0,
                        delta_start: 0,
                        length: 3,     // ```
                        token_type: 3, // code
                        token_modifiers_bitset: 0,
                    });
                }
            }
            Node::MathBlock { .. } => {
                tokens.push(SemanticToken {
                    delta_line: 0,
                    delta_start: 0,
                    length: 2,     // $$
                    token_type: 4, // math
                    token_modifiers_bitset: 0,
                });
            }
            Node::VarDeclaration { name, .. } => {
                // Tokenize @var name
                tokens.push(SemanticToken {
                    delta_line: 0,
                    delta_start: 0,
                    length: 4,     // @var
                    token_type: 2, // keyword
                    token_modifiers_bitset: 0,
                });
                tokens.push(SemanticToken {
                    delta_line: 0,
                    delta_start: 5, // space after @var
                    length: name.len() as u32,
                    token_type: 6, // variable
                    token_modifiers_bitset: 0,
                });
            }
            Node::ForLoop { variable, .. } => {
                tokens.push(SemanticToken {
                    delta_line: 0,
                    delta_start: 0,
                    length: 4,     // @for
                    token_type: 2, // keyword
                    token_modifiers_bitset: 0,
                });
                tokens.push(SemanticToken {
                    delta_line: 0,
                    delta_start: 5,
                    length: variable.len() as u32,
                    token_type: 6, // variable
                    token_modifiers_bitset: 0,
                });
            }
            Node::IfBlock { condition, .. } => {
                tokens.push(SemanticToken {
                    delta_line: 0,
                    delta_start: 0,
                    length: 3,     // @if
                    token_type: 2, // keyword
                    token_modifiers_bitset: 0,
                });
                // The condition is dynamic — just mark it
                if !condition.is_empty() {
                    tokens.push(SemanticToken {
                        delta_line: 0,
                        delta_start: 4, // space after @if
                        length: condition.len() as u32,
                        token_type: 6, // variable
                        token_modifiers_bitset: 0,
                    });
                }
            }
            _ => {}
        }
    }

    fn collect_semantic_inline(
        node: &InlineNode,
        source: &str,
        parent_span: &vell_core::Span,
        tokens: &mut Vec<SemanticToken>,
    ) {
        // Calculate position relative to parent for the encoded token
        let (line, col_start) = Self::inline_pos_relative(source, parent_span, node.span());
        match node {
            InlineNode::VarInterpolation { name, .. } => {
                // @{name}
                let len = 2 + name.len() + 1; // "@{" + name + "}"
                tokens.push(SemanticToken {
                    delta_line: line,
                    delta_start: col_start,
                    length: len as u32,
                    token_type: 6, // variable
                    token_modifiers_bitset: 0,
                });
            }
            InlineNode::MathInline { source: math, .. } => {
                let len = 1 + math.len() + 1; // "$" + math + "$"
                tokens.push(SemanticToken {
                    delta_line: line,
                    delta_start: col_start,
                    length: len as u32,
                    token_type: 4, // math
                    token_modifiers_bitset: 0,
                });
            }
            InlineNode::Code { value, .. } => {
                let len = 1 + value.len() + 1; // "`" + value + "`"
                tokens.push(SemanticToken {
                    delta_line: line,
                    delta_start: col_start,
                    length: len as u32,
                    token_type: 3, // code
                    token_modifiers_bitset: 0,
                });
            }
            InlineNode::Link { href, children, .. } => {
                // Tokenize the URL part
                if !href.is_empty() {
                    let link_span = node.span();
                    let href_start = link_span.end - href.len() - 1;
                    let href_span = vell_core::Span::new(href_start, link_span.end - 1);
                    let (hl, hc) = Self::inline_pos_relative(source, parent_span, href_span);
                    tokens.push(SemanticToken {
                        delta_line: hl,
                        delta_start: hc,
                        length: href.len() as u32,
                        token_type: 7,
                        token_modifiers_bitset: 0,
                    });
                }
                for child in children {
                    Self::collect_semantic_inline(child, source, parent_span, tokens);
                }
            }
            // Recurse into children for all formatting inline nodes
            InlineNode::Bold { children, .. }
            | InlineNode::Italic { children, .. }
            | InlineNode::Underline { children, .. }
            | InlineNode::Strikethrough { children, .. }
            | InlineNode::Superscript { children, .. }
            | InlineNode::Subscript { children, .. }
            | InlineNode::LinkRef { children, .. } => {
                for child in children {
                    Self::collect_semantic_inline(child, source, parent_span, tokens);
                }
            }
            _ => {}
        }
    }

    /// Compute (relative_line, relative_col_start) for an inline node within its parent.
    fn inline_pos_relative(
        source: &str,
        parent_span: &vell_core::Span,
        node_span: vell_core::Span,
    ) -> (u32, u32) {
        let parent_start = Self::byte_to_position(source, parent_span.start);
        let node_start = Self::byte_to_position(source, node_span.start);
        let line = node_start.line - parent_start.line;
        let col = if line == 0 {
            node_start.character - parent_start.character
        } else {
            node_start.character
        };
        (line, col)
    }

    /// Compute format-on-type edits.
    fn compute_on_type_format(source: &str, position: Position, ch: char) -> Option<Vec<TextEdit>> {
        let byte_offset = Self::position_to_byte(source, position);
        let line = Self::get_line(source, position.line);

        if ch == '}' {
            // When user types '}', auto-dedent to match the opening @[Name](...) or @for/@if
            // Find the matching opening line
            let before = &source[..byte_offset];
            // Count braces to find the matching one
            let mut depth = 0u32;
            for (i, c) in before.char_indices().rev() {
                if c == '}' {
                    depth += 1;
                } else if c == '{' {
                    if depth == 0 {
                        // Found matching '{' - check the line before for the directive
                        let before_pos = Self::byte_to_position(source, i);
                        let open_line = Self::get_line(source, before_pos.line);
                        let indent = open_line.len() - open_line.trim_start().len();
                        let line_indent = line.len() - line.trim_start().len();
                        if indent != line_indent {
                            // Only suggest if indentation doesn't match
                            let new_text = " ".repeat(indent) + "}";
                            return Some(vec![TextEdit {
                                range: Range {
                                    start: Position::new(position.line, 0),
                                    end: Position::new(position.line + 1, 0),
                                },
                                new_text,
                            }]);
                        }
                        return None;
                    }
                    depth -= 1;
                }
            }
            None
        } else if ch == '\n' {
            // When user presses Enter, auto-indent the new line
            // Copy indentation from the current line
            let current_indent = line.len() - line.trim_start().len();
            let trimmed = line.trim_start();

            // Check if line ends with an opening brace -> add extra indent
            if trimmed.ends_with('{') {
                let new_indent = " ".repeat(current_indent + 2);
                return Some(vec![TextEdit {
                    range: Range {
                        start: position,
                        end: position,
                    },
                    new_text: format!("{}", new_indent),
                }]);
            }

            // Check if line is a list item -> continue list
            if trimmed.starts_with("- ") || trimmed.starts_with("* ") || trimmed.starts_with("+ ") {
                let marker = &trimmed[..2];
                return Some(vec![TextEdit {
                    range: Range {
                        start: position,
                        end: position,
                    },
                    new_text: format!("{}{}", " ".repeat(current_indent), marker),
                }]);
            }

            // Check if line starts with a number marker "1. " -> continue numbering
            if trimmed.starts_with(|c: char| c.is_ascii_digit()) && trimmed.contains(". ") {
                if let Some(dot_pos) = trimmed.find(". ") {
                    let num_str = &trimmed[..dot_pos];
                    if let Ok(num) = num_str.parse::<u32>() {
                        return Some(vec![TextEdit {
                            range: Range {
                                start: position,
                                end: position,
                            },
                            new_text: format!("{}{}. ", " ".repeat(current_indent), num + 1),
                        }]);
                    }
                }
            }

            // Default: copy current line's indentation
            if current_indent > 0 {
                return Some(vec![TextEdit {
                    range: Range {
                        start: position,
                        end: position,
                    },
                    new_text: format!("{}", " ".repeat(current_indent)),
                }]);
            }

            None
        } else {
            None
        }
    }

    /// Compute format-on-type edits.
    fn find_next_heading_end_line(
        current_level: &u8,
        siblings: &[Node],
        current_node: &Node,
        source: &str,
    ) -> u32 {
        let mut found_self = false;
        for sibling in siblings {
            if std::ptr::eq(sibling, current_node) {
                found_self = true;
                continue;
            }
            if !found_self {
                continue;
            }
            if let Node::Heading {
                level: next_level,
                span: next_span,
                ..
            } = sibling
            {
                if *next_level <= *current_level {
                    let next_pos = Self::byte_to_position(source, next_span.start);
                    return next_pos.line.saturating_sub(1);
                }
            }
        }
        Self::byte_to_position(source, source.len()).line
    }

    fn collect_code_actions(
        doc: &Document,
        source: &str,
        range: &Range,
        uri: &Url,
    ) -> Vec<CodeAction> {
        let mut actions = Vec::new();
        let (start_byte, end_byte) = Self::range_to_byte_range(source, range);
        let selected_text = &source[start_byte..end_byte];

        // Action 1: Wrap selection in a heading if it looks like text
        if !selected_text.is_empty() && !selected_text.starts_with('=') {
            // Suggest wrapping in headings of different levels
            for level in 1..=3 {
                let eqs = "=".repeat(level);
                actions.push(CodeAction {
                    title: format!("Wrap in level-{} heading", level),
                    kind: Some(CodeActionKind::new("vell.wrap.heading")),
                    edit: Some(WorkspaceEdit {
                        changes: Some(HashMap::from([(
                            uri.clone(),
                            vec![TextEdit {
                                range: Range {
                                    start: range.start,
                                    end: range.end,
                                },
                                new_text: format!("{} {}\n", eqs, selected_text),
                            }],
                        )])),
                        ..WorkspaceEdit::default()
                    }),
                    ..CodeAction::default()
                });
            }
        }

        // Action 2: Check for missing @[Meta] directive (no metadata at all)
        let has_meta = doc
            .children
            .iter()
            .any(|n| matches!(n, Node::Directive { name, .. } if name == "Meta"));
        if !has_meta {
            // Only suggest if cursor is near the beginning of the document
            if range.start.line <= 2 {
                // Detect title
                let title = doc
                    .children
                    .first()
                    .and_then(|n| {
                        if let Node::Heading {
                            level: 1, children, ..
                        } = n
                        {
                            Some(vell_core::format_inline_nodes(children))
                        } else {
                            None
                        }
                    })
                    .unwrap_or_else(|| "Untitled".to_string());

                actions.push(CodeAction {
                    title: "Add @[Meta] directive with document metadata".to_string(),
                    kind: Some(CodeActionKind::new("vell.meta.add")),
                    edit: Some(WorkspaceEdit {
                        changes: Some(HashMap::from([(
                            uri.clone(),
                            vec![TextEdit {
                                range: Range {
                                    start: Position::new(1, 0),
                                    end: Position::new(1, 0),
                                },
                                new_text: format!("@[Meta](title=\"{}\")\n", title),
                            }],
                        )])),
                        ..WorkspaceEdit::default()
                    }),
                    ..CodeAction::default()
                });
            }
        }

        // Action 3: Convert to bullet list if the selection looks like items
        if !selected_text.is_empty() && range.start.line != range.end.line {
            let has_list_marker = selected_text
                .lines()
                .any(|l| l.trim().starts_with("- ") || l.trim().starts_with("* "));
            if !has_list_marker {
                let converted: String = selected_text
                    .lines()
                    .filter(|l| !l.trim().is_empty())
                    .map(|l| format!("- {}", l.trim()))
                    .collect::<Vec<_>>()
                    .join("\n");

                actions.push(CodeAction {
                    title: "Convert to bullet list".to_string(),
                    kind: Some(CodeActionKind::new("vell.list.convert")),
                    edit: Some(WorkspaceEdit {
                        changes: Some(HashMap::from([(
                            uri.clone(),
                            vec![TextEdit {
                                range: range.clone(),
                                new_text: converted,
                            }],
                        )])),
                        ..WorkspaceEdit::default()
                    }),
                    ..CodeAction::default()
                });
            }
        }

        // Action 4: Fix undefined variable — suggest adding @var declaration
        // Scan for undefined variable references in the selection range
        let mut seen_vars: HashSet<String> = HashSet::new();
        let scope_start_byte = Self::position_to_byte(source, range.start);
        let scope_end_byte = Self::position_to_byte(source, range.end);
        let scope_text = &source[scope_start_byte..scope_end_byte];

        // Find @{...} patterns that might be undefined (deduplicated)
        let mut var_start = 0usize;
        while let Some(open) = scope_text[var_start..].find("@{") {
            let abs_open = var_start + open;
            if let Some(close) = scope_text[abs_open + 2..].find('}') {
                let var_name = &scope_text[abs_open + 2..abs_open + 2 + close];
                if !var_name.is_empty()
                    && var_name.chars().all(|c| c.is_alphanumeric() || c == '_')
                    && seen_vars.insert(var_name.to_string())
                {
                    // Check if this variable is actually declared in the document
                    let is_declared = doc.children.iter().any(|n| {
                        if let Node::VarDeclaration { name: n_name, .. } = n {
                            n_name == var_name
                        } else {
                            false
                        }
                    });
                    if !is_declared {
                        let var_name_owned = var_name.to_string();
                        actions.push(CodeAction {
                            title: format!("Add @var {} declaration", var_name),
                            kind: Some(CodeActionKind::new("vell.var.add")),
                            edit: Some(WorkspaceEdit {
                                changes: Some(HashMap::from([(
                                    uri.clone(),
                                    vec![TextEdit {
                                        range: Range {
                                            start: Position::new(0, 0),
                                            end: Position::new(0, 0),
                                        },
                                        new_text: format!("@var {var_name_owned} = \"\"\n"),
                                    }],
                                )])),
                                ..WorkspaceEdit::default()
                            }),
                            ..CodeAction::default()
                        });
                    }
                }
                var_start = abs_open + 2 + close + 1;
            } else {
                break;
            }
        }

        actions
    }

    /// Collect hierarchical document symbols.
    fn collect_document_symbols(doc: &Document, source: &str) -> Vec<DocumentSymbol> {
        let mut symbols = Vec::new();
        Self::collect_symbols_recursive(&doc.children, source, &mut symbols, 0);
        symbols
    }

    fn collect_symbols_recursive(
        nodes: &[Node],
        source: &str,
        symbols: &mut Vec<DocumentSymbol>,
        _depth: usize,
    ) {
        for node in nodes {
            match node {
                Node::Heading {
                    level, children, ..
                } => {
                    let span = node.span();
                    let range = Self::span_to_range(source, &span);
                    let text = vell_core::format_inline_nodes(children);

                    let mut symbol = DocumentSymbol {
                        name: text,
                        detail: Some(format!("H{}", level)),
                        kind: match level {
                            1 => SymbolKind::MODULE,
                            2 => SymbolKind::NAMESPACE,
                            3 => SymbolKind::PACKAGE,
                            _ => SymbolKind::STRING,
                        },
                        tags: None,
                        deprecated: None,
                        range,
                        selection_range: range,
                        children: None,
                    };

                    // Collect children (sub-headings and directives)
                    let mut child_symbols = Vec::new();
                    Self::collect_heading_children(node, source, &mut child_symbols);
                    if !child_symbols.is_empty() {
                        symbol.children = Some(child_symbols);
                    }

                    symbols.push(symbol);
                }
                Node::VarDeclaration { name, .. } => {
                    let span = node.span();
                    let range = Self::span_to_range(source, &span);
                    symbols.push(DocumentSymbol {
                        name: format!("@var {name}"),
                        detail: Some("Variable".to_string()),
                        kind: SymbolKind::VARIABLE,
                        tags: None,
                        deprecated: None,
                        range,
                        selection_range: range,
                        children: None,
                    });
                }
                Node::Directive { name, children, .. } | Node::Extension { name, children, .. } => {
                    let span = node.span();
                    let range = Self::span_to_range(source, &span);
                    let mut symbol = DocumentSymbol {
                        name: format!("@[{name}]"),
                        detail: Some("Directive".to_string()),
                        kind: SymbolKind::FUNCTION,
                        tags: None,
                        deprecated: None,
                        range,
                        selection_range: range,
                        children: None,
                    };

                    // Recursively add child symbols from directive body
                    let mut child_symbols = Vec::new();
                    Self::collect_symbols_recursive(children, source, &mut child_symbols, 1);
                    if !child_symbols.is_empty() {
                        symbol.children = Some(child_symbols);
                    }

                    symbols.push(symbol);
                }
                Node::FootnoteDefinition { marker, .. } => {
                    let span = node.span();
                    let range = Self::span_to_range(source, &span);
                    symbols.push(DocumentSymbol {
                        name: format!("[^{marker}]"),
                        detail: Some("Footnote".to_string()),
                        kind: SymbolKind::PROPERTY,
                        tags: None,
                        deprecated: None,
                        range,
                        selection_range: range,
                        children: None,
                    });
                }
                Node::ReferenceDefinition { id, .. } => {
                    let span = node.span();
                    let range = Self::span_to_range(source, &span);
                    symbols.push(DocumentSymbol {
                        name: format!("[{id}]"),
                        detail: Some("Reference".to_string()),
                        kind: SymbolKind::CONSTANT,
                        tags: None,
                        deprecated: None,
                        range,
                        selection_range: range,
                        children: None,
                    });
                }
                Node::ForLoop {
                    variable, children, ..
                } => {
                    let span = node.span();
                    let range = Self::span_to_range(source, &span);
                    let mut symbol = DocumentSymbol {
                        name: format!("@for {variable}"),
                        detail: Some("Loop".to_string()),
                        kind: SymbolKind::NAMESPACE,
                        tags: None,
                        deprecated: None,
                        range,
                        selection_range: range,
                        children: None,
                    };
                    let mut child_symbols = Vec::new();
                    Self::collect_symbols_recursive(children, source, &mut child_symbols, 1);
                    if !child_symbols.is_empty() {
                        symbol.children = Some(child_symbols);
                    }
                    symbols.push(symbol);
                }
                Node::IfBlock {
                    condition,
                    consequent,
                    alternate,
                    ..
                } => {
                    let span = node.span();
                    let range = Self::span_to_range(source, &span);
                    let mut symbol = DocumentSymbol {
                        name: format!("@if {condition}"),
                        detail: Some("Conditional".to_string()),
                        kind: SymbolKind::NAMESPACE,
                        tags: None,
                        deprecated: None,
                        range,
                        selection_range: range,
                        children: None,
                    };
                    let mut child_symbols = Vec::new();
                    Self::collect_symbols_recursive(consequent, source, &mut child_symbols, 1);
                    if let Some(alt) = alternate {
                        Self::collect_symbols_recursive(alt, source, &mut child_symbols, 1);
                    }
                    if !child_symbols.is_empty() {
                        symbol.children = Some(child_symbols);
                    }
                    symbols.push(symbol);
                }
                _ => {}
            }
        }
    }

    /// Collect child symbols for a heading (sub-headings at higher levels).
    fn collect_heading_children(_parent: &Node, _source: &str, _symbols: &mut Vec<DocumentSymbol>) {
        // Heading children are disabled for now — real hierarchy would
        // require tracking doc.children between headings at consecutive levels.
    }

    /// Collect workspace symbols matching a query across all open documents.
    fn collect_workspace_symbols(
        nodes: &[Node],
        source: &str,
        query: &str,
        uri: &Url,
        file_name: &str,
        symbols: &mut Vec<SymbolInformation>,
    ) {
        for node in nodes {
            let span = node.span();
            let location = Location {
                uri: uri.clone(),
                range: Self::span_to_range(source, &span),
            };

            match node {
                Node::Heading {
                    level, children, ..
                } => {
                    let text = vell_core::format_inline_nodes(children);
                    let kind = match level {
                        1 => SymbolKind::MODULE,
                        2 => SymbolKind::NAMESPACE,
                        3 => SymbolKind::PACKAGE,
                        _ => SymbolKind::STRING,
                    };
                    if query.is_empty() || text.to_lowercase().contains(query) {
                        symbols.push(SymbolInformation {
                            name: text,
                            kind,
                            tags: None,
                            container_name: Some(file_name.to_string()),
                            location: location.clone(),
                            deprecated: None,
                        });
                    }
                }
                Node::VarDeclaration { name, .. } => {
                    if query.is_empty() || name.to_lowercase().contains(query) {
                        symbols.push(SymbolInformation {
                            name: format!("@var {}", name),
                            kind: SymbolKind::VARIABLE,
                            tags: None,
                            container_name: Some(file_name.to_string()),
                            location: location.clone(),
                            deprecated: None,
                        });
                    }
                }
                Node::Directive { name, children, .. } | Node::Extension { name, children, .. } => {
                    if query.is_empty() || name.to_lowercase().contains(query) {
                        symbols.push(SymbolInformation {
                            name: format!("@[{}]", name),
                            kind: SymbolKind::FUNCTION,
                            tags: None,
                            container_name: Some(file_name.to_string()),
                            location: location.clone(),
                            deprecated: None,
                        });
                    }
                    Self::collect_workspace_symbols(
                        children, source, query, uri, file_name, symbols,
                    );
                }
                Node::ForLoop {
                    variable, children, ..
                } => {
                    if query.is_empty() || variable.to_lowercase().contains(query) {
                        symbols.push(SymbolInformation {
                            name: format!("@for {}", variable),
                            kind: SymbolKind::NAMESPACE,
                            tags: None,
                            container_name: Some(file_name.to_string()),
                            location: location.clone(),
                            deprecated: None,
                        });
                    }
                    Self::collect_workspace_symbols(
                        children, source, query, uri, file_name, symbols,
                    );
                }
                Node::IfBlock {
                    consequent,
                    alternate,
                    ..
                } => {
                    Self::collect_workspace_symbols(
                        consequent, source, query, uri, file_name, symbols,
                    );
                    if let Some(alt) = alternate {
                        Self::collect_workspace_symbols(
                            alt, source, query, uri, file_name, symbols,
                        );
                    }
                }
                Node::Blockquote { children, .. } => {
                    Self::collect_workspace_symbols(
                        children, source, query, uri, file_name, symbols,
                    );
                }
                _ => {}
            }
        }
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::INCREMENTAL,
                )),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                completion_provider: Some(CompletionOptions {
                    trigger_characters: Some(vec![
                        "@".to_string(),
                        "[".to_string(),
                        "{".to_string(),
                        "=".to_string(),
                    ]),
                    ..CompletionOptions::default()
                }),
                document_formatting_provider: Some(OneOf::Left(true)),
                document_symbol_provider: Some(OneOf::Left(true)),
                definition_provider: Some(OneOf::Left(true)),
                references_provider: Some(OneOf::Left(true)),
                signature_help_provider: Some(SignatureHelpOptions {
                    trigger_characters: Some(vec!["(".to_string()]),
                    retrigger_characters: Some(vec![",".to_string()]),
                    work_done_progress_options: WorkDoneProgressOptions {
                        work_done_progress: None,
                    },
                }),
                rename_provider: Some(OneOf::Left(true)),
                folding_range_provider: Some(FoldingRangeProviderCapability::Simple(true)),
                code_action_provider: Some(CodeActionProviderCapability::Simple(true)),
                semantic_tokens_provider: Some(
                    SemanticTokensServerCapabilities::SemanticTokensOptions(
                        SemanticTokensOptions {
                            legend: SemanticTokensLegend {
                                token_types: vec![
                                    SemanticTokenType::new("heading"),
                                    SemanticTokenType::new("directive"),
                                    SemanticTokenType::new("keyword"),
                                    SemanticTokenType::new("code"),
                                    SemanticTokenType::new("math"),
                                    SemanticTokenType::new("punctuation"),
                                    SemanticTokenType::new("variable"),
                                    SemanticTokenType::new("string"),
                                ],
                                token_modifiers: vec![
                                    SemanticTokenModifier::new("heading1"),
                                    SemanticTokenModifier::new("heading2"),
                                    SemanticTokenModifier::new("heading3"),
                                    SemanticTokenModifier::new("heading4"),
                                    SemanticTokenModifier::new("heading5"),
                                    SemanticTokenModifier::new("heading6"),
                                ],
                            },
                            range: Some(true),
                            full: Some(SemanticTokensFullOptions::Bool(true)),
                            ..SemanticTokensOptions::default()
                        },
                    ),
                ),
                document_link_provider: Some(DocumentLinkOptions {
                    resolve_provider: Some(false),
                    work_done_progress_options: WorkDoneProgressOptions {
                        work_done_progress: None,
                    },
                }),
                color_provider: Some(ColorProviderCapability::Simple(true)),
                code_lens_provider: Some(CodeLensOptions {
                    resolve_provider: Some(false),
                }),
                workspace: Some(WorkspaceServerCapabilities {
                    workspace_folders: Some(WorkspaceFoldersServerCapabilities {
                        supported: Some(true),
                        change_notifications: Some(OneOf::Left(true)),
                    }),
                    ..WorkspaceServerCapabilities::default()
                }),
                document_on_type_formatting_provider: Some(DocumentOnTypeFormattingOptions {
                    first_trigger_character: "}".to_string(),
                    more_trigger_character: Some(vec!["\n".to_string()]),
                }),
                inlay_hint_provider: Some(OneOf::Left(true)),
                linked_editing_range_provider: Some(LinkedEditingRangeServerCapabilities::Simple(
                    true,
                )),
                call_hierarchy_provider: Some(CallHierarchyServerCapability::Simple(true)),
                ..ServerCapabilities::default()
            },
            ..InitializeResult::default()
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        if let Some(ref client) = self.client {
            client
                .log_message(MessageType::INFO, "Vell language server initialized")
                .await;
        }

        // Register file watchers for .vl files
        let watchers = vec![FileSystemWatcher {
            glob_pattern: GlobPattern::String("**/*.vl".to_string()),
            kind: Some(WatchKind::Create | WatchKind::Change | WatchKind::Delete),
        }];
        let options = DidChangeWatchedFilesRegistrationOptions { watchers };
        let registration = Registration {
            id: "vell-watcher".to_string(),
            method: "workspace/didChangeWatchedFiles".to_string(),
            register_options: Some(serde_json::to_value(options).unwrap()),
        };
        if let Some(ref client) = self.client {
            let _ = client.register_capability(vec![registration]).await;
        }
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let uri = params.text_document.uri;
        // Remove the document from cache
        self.documents.write().await.remove(&uri);
        // Clear diagnostics for the closed document
        if let Some(ref client) = self.client {
            client.publish_diagnostics(uri, Vec::new(), None).await;
        }
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        self.update_document(params.text_document.uri, params.text_document.text)
            .await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri;
        // Apply incremental changes if they have a range; otherwise use full text
        // Save the last change text before consuming the iterator
        let last_change = params.content_changes.into_iter().last();
        if let Some(change) = last_change {
            if let Some(range) = change.range {
                // Incremental update: apply the edit to the cached source
                let mut docs = self.documents.write().await;
                if let Some(vd) = docs.get_mut(&uri) {
                    let (start_byte, end_byte) = Self::range_to_byte_range(&vd.source, &range);
                    let mut new_source = vd.source.clone();
                    new_source.replace_range(start_byte..end_byte, &change.text);
                    drop(docs);
                    self.update_document_incremental(uri, new_source, start_byte).await;
                    return;
                }
                drop(docs);
            }
            // Full update fallback for changes without a range
            self.update_document(uri, change.text).await;
        }
    }

    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        // Re-validate on save to ensure diagnostics are up-to-date
        let uri = params.text_document.uri.clone();
        let file_name = uri
            .path()
            .rsplit('/')
            .next()
            .unwrap_or("unknown")
            .to_string();

        if let Some((text, _)) = {
            let docs = self.documents.read().await;
            docs.get(&uri)
                .map(|vd| (vd.source.clone(), vd.errors.len()))
        } {
            self.update_document(uri.clone(), text).await;
        }

        if let Some(ref client) = self.client {
            client
                .log_message(MessageType::INFO, format!("Saved {}", file_name))
                .await;
        }
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let uri = params.text_document_position_params.text_document.uri;
        let pos = params.text_document_position_params.position;
        let docs = self.documents.read().await;

        let Some(vd) = docs.get(&uri) else {
            return Ok(None);
        };
        let Some(ref doc) = vd.parsed else {
            return Ok(None);
        };

        let source = &vd.source;

        // 1. Check if cursor is on a variable reference @{name}
        if let Some(var_name) = Self::find_var_ref_at(source, pos) {
            if let Some(decl) = Self::find_variable_decl(doc, &var_name) {
                if let Node::VarDeclaration { value, .. } = decl {
                    let value_str = match value {
                        serde_json::Value::String(s) => format!("\"{s}\""),
                        serde_json::Value::Null => "null".to_string(),
                        other => other.to_string(),
                    };
                    return Ok(Some(Hover {
                        contents: HoverContents::Scalar(MarkedString::String(format!(
                            "@var {var_name} = {value_str}"
                        ))),
                        range: None,
                    }));
                }
            }
            return Ok(Some(Hover {
                contents: HoverContents::Scalar(MarkedString::String(format!(
                    "Variable `{var_name}` — not declared in this document."
                ))),
                range: None,
            }));
        }

        // 2. Check if cursor is on a heading
        if let Some(heading) = Self::find_heading_at(doc, pos, source) {
            if let Node::Heading {
                level, children, ..
            } = heading
            {
                let text = vell_core::format_inline_nodes(children);
                let level_label = match level {
                    1 => "Title (level 1)",
                    2 => "Section (level 2)",
                    3 => "Subsection (level 3)",
                    4 => "Sub-subsection (level 4)",
                    _ => "Heading",
                };
                return Ok(Some(Hover {
                    contents: HoverContents::Scalar(MarkedString::String(format!(
                        "## {}\n\n{} — `{}`",
                        text,
                        level_label,
                        "=".repeat(usize::from(*level))
                    ))),
                    range: None,
                }));
            }
        }

        // 3. Check if cursor is on a directive @[Name]
        for node in &doc.children {
            if let Node::Directive { name, span, .. } | Node::Extension { name, span, .. } = node {
                let range = Self::span_to_range(source, span);
                if range.start <= pos && pos <= range.end {
                    let desc = builtin_directive_description(name);
                    return Ok(Some(Hover {
                        contents: HoverContents::Scalar(MarkedString::String(format!(
                            "**`@{0}`**\n\n{1}",
                            name, desc
                        ))),
                        range: None,
                    }));
                }
            }
        }

        // 4. Check for inline nodes (var interpolation, inline math, etc.)
        for node in &doc.children {
            if let Node::Paragraph { children, span } = node {
                let para_range = Self::span_to_range(source, span);
                if para_range.start <= pos && pos <= para_range.end {
                    if let Some(hover) = Self::hover_inline(children, source, pos) {
                        return Ok(Some(hover));
                    }
                }
            }
        }

        Ok(None)
    }

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        let uri = params.text_document_position.text_document.uri;
        let pos = params.text_document_position.position;
        let docs = self.documents.read().await;

        let Some(vd) = docs.get(&uri) else {
            return Ok(Some(empty_completions()));
        };
        let source = &vd.source;
        let parsed = vd.parsed.as_ref();

        // Determine what the user is typing by looking at the line
        let line = Self::get_line(source, pos.line);
        let prefix = Self::prefix_before_cursor(&line, pos.character);

        let mut items = Vec::new();

        // If typing starts with "@", suggest directives and variable references
        if prefix.starts_with('@') {
            // Built-in directives
            for (name, desc) in BUILTIN_DIRECTIVES {
                items.push(CompletionItem {
                    label: format!("@[{}]", name),
                    detail: Some(desc.to_string()),
                    kind: Some(CompletionItemKind::KEYWORD),
                    ..CompletionItem::default()
                });
            }

            // Variable declarations
            items.push(CompletionItem {
                label: "@var".to_string(),
                detail: Some("Variable declaration".to_string()),
                kind: Some(CompletionItemKind::KEYWORD),
                ..CompletionItem::default()
            });

            // for and if blocks
            items.push(CompletionItem {
                label: "@for".to_string(),
                detail: Some("For loop block".to_string()),
                kind: Some(CompletionItemKind::KEYWORD),
                ..CompletionItem::default()
            });
            items.push(CompletionItem {
                label: "@if".to_string(),
                detail: Some("Conditional if block".to_string()),
                kind: Some(CompletionItemKind::KEYWORD),
                ..CompletionItem::default()
            });
        }

        // If typing starts with "=", suggest headings
        if prefix.starts_with('=') || prefix.is_empty() {
            for level in 1..=4 {
                let eqs = "=".repeat(level);
                let label = format!("{} Heading {}", eqs, level_label(level));
                items.push(CompletionItem {
                    label: format!("{} ", eqs),
                    detail: Some(label),
                    kind: Some(CompletionItemKind::SNIPPET),
                    insert_text: Some(format!("{} ", eqs)),
                    ..CompletionItem::default()
                });
            }
        }

        // If typing starts with "@{", suggest declared variables
        if prefix.contains("@{") || prefix.contains("@") {
            if let Some(ref doc) = parsed {
                for name in Self::declared_variables(doc) {
                    items.push(CompletionItem {
                        label: format!("@{{{}}}", name),
                        detail: Some("Variable reference".to_string()),
                        kind: Some(CompletionItemKind::VARIABLE),
                        insert_text: Some(format!("{{{}}}", name)),
                        ..CompletionItem::default()
                    });
                }
            }
        }

        // Suggest variable names when at "@{"
        if prefix.ends_with("@{") || prefix.ends_with("{") {
            if let Some(ref doc) = parsed {
                for name in Self::declared_variables(doc) {
                    items.push(CompletionItem {
                        label: format!("{}", name),
                        detail: Some("Insert variable name".to_string()),
                        kind: Some(CompletionItemKind::VARIABLE),
                        ..CompletionItem::default()
                    });
                }
            }
        }

        // Suggest common math symbols inside $ or $$
        if vd.source.contains('$') {
            let in_math = Self::cursor_in_math(&vd.source, pos);
            if in_math {
                for (symbol, desc) in MATH_SYMBOLS {
                    items.push(CompletionItem {
                        label: symbol.to_string(),
                        detail: Some(desc.to_string()),
                        kind: Some(CompletionItemKind::SNIPPET),
                        ..CompletionItem::default()
                    });
                }
            }
        }

        // Suggest admonition types after "> [!"
        if prefix.contains("[!") {
            for kind in &["NOTE", "TIP", "IMPORTANT", "WARNING", "CAUTION"] {
                items.push(CompletionItem {
                    label: format!("[!{}]", kind),
                    detail: Some(format!("Admonition: {}", kind)),
                    kind: Some(CompletionItemKind::SNIPPET),
                    ..CompletionItem::default()
                });
            }
        }

        Ok(Some(CompletionResponse::Array(items)))
    }

    async fn formatting(&self, params: DocumentFormattingParams) -> Result<Option<Vec<TextEdit>>> {
        let docs = self.documents.read().await;
        let Some(vd) = docs.get(&params.text_document.uri) else {
            return Ok(None);
        };
        match vell_fmt::format_source(&vd.source) {
            Ok(text) => {
                let line_count = vd.source.lines().count().max(1) as u32;
                Ok(Some(vec![TextEdit {
                    range: Range {
                        start: Position::new(0, 0),
                        end: Position::new(line_count, 0),
                    },
                    new_text: text,
                }]))
            }
            Err(_) => Ok(None),
        }
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        let uri = params.text_document_position_params.text_document.uri;
        let pos = params.text_document_position_params.position;
        let docs = self.documents.read().await;

        let Some(vd) = docs.get(&uri) else {
            return Ok(None);
        };
        let Some(ref doc) = vd.parsed else {
            return Ok(None);
        };

        let source = &vd.source;

        // Check if cursor is on a variable reference @{name}
        if let Some(var_name) = Self::find_var_ref_at(source, pos) {
            if let Some(decl) = Self::find_variable_decl(doc, &var_name) {
                let span = decl.span();
                let range = Self::span_to_range(source, &span);
                return Ok(Some(GotoDefinitionResponse::Scalar(Location {
                    uri: uri.clone(),
                    range,
                })));
            }
        }

        // Check if cursor is on a footnote reference [^marker] inside a paragraph
        for node in &doc.children {
            if let Node::Paragraph { children, span } = node {
                let para_range = Self::span_to_range(source, span);
                if para_range.start <= pos && pos <= para_range.end {
                    for inline in children {
                        if let InlineNode::FootnoteRef {
                            marker,
                            span: ref_span,
                        } = inline
                        {
                            let ref_range = Self::span_to_range(source, ref_span);
                            if ref_range.start <= pos && pos <= ref_range.end {
                                for def in &doc.children {
                                    if let Node::FootnoteDefinition {
                                        marker: def_marker,
                                        span: def_span,
                                        ..
                                    } = def
                                    {
                                        if def_marker == marker {
                                            return Ok(Some(GotoDefinitionResponse::Scalar(
                                                Location {
                                                    uri: uri.clone(),
                                                    range: Self::span_to_range(source, def_span),
                                                },
                                            )));
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(None)
    }

    /// Find references to a variable or symbol.
    async fn references(&self, params: ReferenceParams) -> Result<Option<Vec<Location>>> {
        let uri = params.text_document_position.text_document.uri;
        let pos = params.text_document_position.position;
        let docs = self.documents.read().await;

        let Some(vd) = docs.get(&uri) else {
            return Ok(None);
        };
        let Some(ref doc) = vd.parsed else {
            return Ok(None);
        };

        let source = &vd.source;
        let mut locations: Vec<Location> = Vec::new();

        // Check if cursor is on a variable reference
        if let Some(var_name) = Self::find_var_ref_at(source, pos) {
            // Add the declaration location
            if let Some(decl) = Self::find_variable_decl(doc, &var_name) {
                let span = decl.span();
                let range = Self::span_to_range(source, &span);
                locations.push(Location {
                    uri: uri.clone(),
                    range,
                });
            }

            // Add all reference locations
            let refs = Self::find_var_references(doc, &var_name);
            for r in refs {
                let span = r.span();
                let range = Self::span_to_range(source, &span);
                locations.push(Location {
                    uri: uri.clone(),
                    range,
                });
            }
        }

        // Check if cursor is on a variable declaration
        if let Some(decl) = Self::find_variable_decl_at(doc, pos, source) {
            if let Node::VarDeclaration { name, .. } = decl {
                // Add declaration location
                let span = decl.span();
                let range = Self::span_to_range(source, &span);
                locations.push(Location {
                    uri: uri.clone(),
                    range,
                });

                // Add all reference locations
                let refs = Self::find_var_references(doc, name);
                for r in refs {
                    let span = r.span();
                    let range = Self::span_to_range(source, &span);
                    locations.push(Location {
                        uri: uri.clone(),
                        range,
                    });
                }
            }
        }

        if locations.is_empty() {
            Ok(None)
        } else {
            Ok(Some(locations))
        }
    }

    /// Prepare rename: validate that the symbol can be renamed.
    async fn prepare_rename(
        &self,
        params: TextDocumentPositionParams,
    ) -> Result<Option<PrepareRenameResponse>> {
        let uri = params.text_document.uri.clone();
        let pos = params.position;
        let docs = self.documents.read().await;

        let Some(vd) = docs.get(&uri) else {
            return Ok(None);
        };
        let source = &vd.source;

        // Check if cursor is on a variable reference @{name}
        if let Some(var_name) = Self::find_var_ref_at(source, pos) {
            let byte_offset = Self::position_to_byte(source, pos);
            let before = &source[..byte_offset];
            let last_open = before.rfind("@{").unwrap_or(0);
            let name_start = last_open + 2;
            let name_end = name_start + var_name.len();
            let range = Self::span_to_range(source, &vell_core::Span::new(name_start, name_end));
            return Ok(Some(PrepareRenameResponse::Range(range)));
        }

        // Check if cursor is on a variable declaration
        if let Some(ref doc) = vd.parsed {
            if let Some(decl) = Self::find_variable_decl_at(doc, pos, source) {
                if let Node::VarDeclaration { name, span, .. } = decl {
                    let range = Self::span_to_range(source, span);
                    return Ok(Some(PrepareRenameResponse::Range(range)));
                }
            }
        }

        Ok(None)
    }

    /// Rename a symbol.
    /// Provide signature help for @[Directive](...) calls showing parameter names and types.
    async fn signature_help(&self, params: SignatureHelpParams) -> Result<Option<SignatureHelp>> {
        let uri = params.text_document_position_params.text_document.uri;
        let pos = params.text_document_position_params.position;
        let docs = self.documents.read().await;

        let Some(vd) = docs.get(&uri) else {
            return Ok(None);
        };
        let source = &vd.source;
        let line = Self::get_line(source, pos.line);
        let before_cursor = Self::prefix_before_cursor(line, pos.character);

        // Check if we're inside a directive call: @[Name](...)
        // Look backwards for @[Name] pattern
        if let Some(open_paren) = before_cursor.rfind('(') {
            let before_paren = &before_cursor[..open_paren];
            // Find the @[...] before the paren
            if let Some(close_bracket) = before_paren.rfind(']') {
                if let Some(open_bracket) = before_paren[..close_bracket].rfind("@[") {
                    let name = &before_paren[open_bracket + 2..close_bracket];
                    if let Some(info) = directive_signature(name) {
                        // Count commas after the opening paren to determine active parameter
                        let after_paren = &before_cursor[open_paren + 1..];
                        let param_idx = after_paren.matches(',').count() as u32;

                        return Ok(Some(SignatureHelp {
                            signatures: vec![info],
                            active_signature: Some(0),
                            active_parameter: Some(param_idx),
                        }));
                    }
                }
            }
        }

        Ok(None)
    }

    async fn rename(&self, params: RenameParams) -> Result<Option<WorkspaceEdit>> {
        let uri = params.text_document_position.text_document.uri;
        let pos = params.text_document_position.position;
        let new_name = params.new_name;
        let docs = self.documents.read().await;

        let Some(vd) = docs.get(&uri) else {
            return Ok(None);
        };
        let Some(ref doc) = vd.parsed else {
            return Ok(None);
        };

        let source = &vd.source;
        let mut changes: Vec<TextEdit> = Vec::new();

        // Check if cursor is on a variable reference @{name}
        if let Some(var_name) = Self::find_var_ref_at(source, pos) {
            // Rename declaration
            if let Some(decl) = Self::find_variable_decl(doc, &var_name) {
                let span = decl.span();
                // Only rename the name part of @var name = value
                if let Node::VarDeclaration {
                    name: _,
                    span: decl_span,
                    ..
                } = decl
                {
                    let decl_text = &source[decl_span.start..decl_span.end];
                    if let Some(eq_pos) = decl_text.find('=') {
                        // Find the name after "@var "
                        let after_var = decl_text.strip_prefix("@var ").unwrap_or("");
                        let name_end = after_var.find('=').unwrap_or(decl_text.len());
                        let name_start_off = decl_span.start + 5; // "@var " = 5 chars
                        let name_end_off = name_start_off + name_end;
                        changes.push(TextEdit {
                            range: Self::span_to_range(
                                source,
                                &vell_core::Span::new(name_start_off, name_end_off),
                            ),
                            new_text: new_name.clone(),
                        });
                    }
                }
            }

            // Rename all references
            let refs = Self::find_var_references(doc, &var_name);
            for r in refs {
                let span = r.span();
                // Inside @{name}, only replace the name portion
                let start = span.start + 2; // skip "@{"
                let end = span.end - 1; // skip "}"
                if start < end {
                    changes.push(TextEdit {
                        range: Self::span_to_range(source, &vell_core::Span::new(start, end)),
                        new_text: new_name.clone(),
                    });
                }
            }
        }

        // Check if cursor is on a variable declaration
        if let Some(decl) = Self::find_variable_decl_at(doc, pos, source) {
            if let Node::VarDeclaration { name, span, .. } = decl {
                // Rename declaration
                let decl_text = &source[span.start..span.end];
                if let Some(eq_pos) = decl_text.find('=') {
                    let after_var = decl_text.strip_prefix("@var ").unwrap_or("");
                    let name_end = after_var.find('=').unwrap_or(decl_text.len());
                    let name_start_off = span.start + 5;
                    let name_end_off = name_start_off + name_end;
                    changes.push(TextEdit {
                        range: Self::span_to_range(
                            source,
                            &vell_core::Span::new(name_start_off, name_end_off),
                        ),
                        new_text: new_name.clone(),
                    });
                }

                // Rename all references
                let refs = Self::find_var_references(doc, name);
                for r in refs {
                    let span = r.span();
                    let start = span.start + 2; // skip "@{"
                    let end = span.end - 1; // skip "}"
                    if start < end {
                        changes.push(TextEdit {
                            range: Self::span_to_range(source, &vell_core::Span::new(start, end)),
                            new_text: new_name.clone(),
                        });
                    }
                }
            }
        }

        if changes.is_empty() {
            Ok(None)
        } else {
            Ok(Some(WorkspaceEdit {
                changes: Some(HashMap::from([(uri, changes)])),
                ..WorkspaceEdit::default()
            }))
        }
    }

    /// Provide folding ranges for collapsible regions.
    async fn folding_range(&self, params: FoldingRangeParams) -> Result<Option<Vec<FoldingRange>>> {
        let docs = self.documents.read().await;
        let Some(vd) = docs.get(&params.text_document.uri) else {
            return Ok(None);
        };
        let Some(ref doc) = vd.parsed else {
            return Ok(None);
        };

        let ranges = Self::collect_folding_ranges(doc, &vd.source);
        Ok(Some(ranges))
    }

    /// Provide semantic tokens for syntax highlighting.
    async fn semantic_tokens_full(
        &self,
        params: SemanticTokensParams,
    ) -> Result<Option<SemanticTokensResult>> {
        let docs = self.documents.read().await;
        let Some(vd) = docs.get(&params.text_document.uri) else {
            return Ok(None);
        };
        let Some(ref doc) = vd.parsed else {
            return Ok(None);
        };

        let raw = Self::collect_semantic_tokens(doc, &vd.source);
        // Encode tokens using relative positions into tower_lsp::lsp_types::SemanticToken structs
        let mut data = Vec::with_capacity(raw.len());
        let mut prev_line = 0u32;
        let mut prev_col = 0u32;
        for token in &raw {
            let line = prev_line + token.delta_line;
            let col_start = if token.delta_line == 0 {
                prev_col + token.delta_start
            } else {
                token.delta_start
            };
            data.push(tower_lsp::lsp_types::SemanticToken {
                delta_line: token.delta_line,
                delta_start: token.delta_start,
                length: token.length,
                token_type: token.token_type,
                token_modifiers_bitset: token.token_modifiers_bitset,
            });
            prev_line = line;
            prev_col = col_start;
        }

        Ok(Some(SemanticTokensResult::Tokens(SemanticTokens {
            result_id: None,
            data,
        })))
    }

    /// Provide range-limited semantic tokens.
    async fn semantic_tokens_range(
        &self,
        params: SemanticTokensRangeParams,
    ) -> Result<Option<SemanticTokensRangeResult>> {
        // For simplicity, delegate to full and filter by range
        let docs = self.documents.read().await;
        let Some(vd) = docs.get(&params.text_document.uri) else {
            return Ok(None);
        };
        let Some(ref doc) = vd.parsed else {
            return Ok(None);
        };

        let raw = Self::collect_semantic_tokens(doc, &vd.source);
        let req_range = params.range;
        let mut data = Vec::with_capacity(raw.len());
        let mut prev_line = 0u32;
        let mut prev_col = 0u32;
        for token in &raw {
            let line = prev_line + token.delta_line;
            let col_start = if token.delta_line == 0 {
                prev_col + token.delta_start
            } else {
                token.delta_start
            };

            // Check if this token is within the requested range
            if line >= req_range.start.line && line <= req_range.end.line {
                data.push(tower_lsp::lsp_types::SemanticToken {
                    delta_line: token.delta_line,
                    delta_start: token.delta_start,
                    length: token.length,
                    token_type: token.token_type,
                    token_modifiers_bitset: token.token_modifiers_bitset,
                });
            }

            prev_line = line;
            prev_col = col_start;
        }

        Ok(Some(SemanticTokensRangeResult::Tokens(SemanticTokens {
            result_id: None,
            data,
        })))
    }

    /// Handle format-on-type: auto-indent when typing certain characters.
    async fn on_type_formatting(
        &self,
        params: DocumentOnTypeFormattingParams,
    ) -> Result<Option<Vec<TextEdit>>> {
        let uri = params.text_document_position.text_document.uri;
        let pos = params.text_document_position.position;
        let ch = params.ch.chars().next().unwrap_or('\n');
        let docs = self.documents.read().await;
        let Some(vd) = docs.get(&uri) else {
            return Ok(None);
        };
        let edits = Self::compute_on_type_format(&vd.source, pos, ch);
        Ok(edits)
    }

    /// Provide code actions for diagnostics and range-based fixes.
    async fn code_action(&self, params: CodeActionParams) -> Result<Option<CodeActionResponse>> {
        let uri = params.text_document.uri;
        let docs = self.documents.read().await;
        let Some(vd) = docs.get(&uri) else {
            return Ok(None);
        };
        let Some(ref doc) = vd.parsed else {
            return Ok(None);
        };

        let actions = Self::collect_code_actions(doc, &vd.source, &params.range, &uri);

        if actions.is_empty() {
            return Ok(None);
        }

        let response: Vec<CodeActionOrCommand> = actions
            .into_iter()
            .map(CodeActionOrCommand::CodeAction)
            .collect();
        Ok(Some(response))
    }

    /// Provide document links (clickable URLs) in the document.
    async fn document_link(&self, params: DocumentLinkParams) -> Result<Option<Vec<DocumentLink>>> {
        let docs = self.documents.read().await;
        let Some(vd) = docs.get(&params.text_document.uri) else {
            return Ok(None);
        };
        let Some(ref doc) = vd.parsed else {
            return Ok(None);
        };

        let links = Self::collect_document_links(doc, &vd.source);
        if links.is_empty() {
            Ok(None)
        } else {
            Ok(Some(links))
        }
    }

    /// Provide color information for color values in the document.
    async fn document_color(&self, params: DocumentColorParams) -> Result<Vec<ColorInformation>> {
        let docs = self.documents.read().await;
        let Some(vd) = docs.get(&params.text_document.uri) else {
            return Ok(Vec::new());
        };
        let colors = Self::collect_document_colors(&vd.source);
        Ok(colors)
    }

    /// Provide color presentations for editing a color value.
    async fn color_presentation(
        &self,
        params: ColorPresentationParams,
    ) -> Result<Vec<ColorPresentation>> {
        let color = params.color;
        // Compute common representations
        let r = (color.red * 255.0).round() as u8;
        let g = (color.green * 255.0).round() as u8;
        let b = (color.blue * 255.0).round() as u8;
        let a = (color.alpha * 255.0).round() as u8;

        let mut presentations = Vec::new();

        // Short hex when alpha is 1.0
        if a == 255 {
            presentations.push(ColorPresentation {
                label: format!("#{:02x}{:02x}{:02x}", r, g, b),
                text_edit: None,
                additional_text_edits: None,
            });
            // Short form if possible
            if r % 17 == 0 && g % 17 == 0 && b % 17 == 0 {
                presentations.push(ColorPresentation {
                    label: format!("#{:x}{:x}{:x}", r / 17, g / 17, b / 17),
                    text_edit: None,
                    additional_text_edits: None,
                });
            }
        } else {
            presentations.push(ColorPresentation {
                label: format!("#{:02x}{:02x}{:02x}{:02x}", r, g, b, a),
                text_edit: None,
                additional_text_edits: None,
            });
        }

        // RGB function form
        if a == 255 {
            presentations.push(ColorPresentation {
                label: format!("rgb({r}, {g}, {b})"),
                text_edit: None,
                additional_text_edits: None,
            });
        } else {
            let a_f32 = color.alpha;
            presentations.push(ColorPresentation {
                label: format!("rgba({r}, {g}, {b}, {a_f32:.2})"),
                text_edit: None,
                additional_text_edits: None,
            });
        }

        Ok(presentations)
    }

    /// Provide code lenses showing reference counts on @var declarations.
    async fn code_lens(&self, params: CodeLensParams) -> Result<Option<Vec<CodeLens>>> {
        let docs = self.documents.read().await;
        let Some(vd) = docs.get(&params.text_document.uri) else {
            return Ok(None);
        };
        let Some(ref doc) = vd.parsed else {
            return Ok(None);
        };

        let lenses = Self::collect_code_lenses(doc, &vd.source, &params.text_document.uri);
        if lenses.is_empty() {
            Ok(None)
        } else {
            Ok(Some(lenses))
        }
    }

    async fn document_symbol(
        &self,
        params: DocumentSymbolParams,
    ) -> Result<Option<DocumentSymbolResponse>> {
        let docs = self.documents.read().await;
        let Some(vd) = docs.get(&params.text_document.uri) else {
            return Ok(None);
        };
        let Some(ref doc) = vd.parsed else {
            return Ok(None);
        };

        let symbols = Self::collect_document_symbols(doc, &vd.source);
        Ok(Some(DocumentSymbolResponse::Nested(symbols)))
    }

    /// Search workspace symbols across all open documents.
    async fn symbol(
        &self,
        params: WorkspaceSymbolParams,
    ) -> Result<Option<Vec<SymbolInformation>>> {
        let query = params.query.to_lowercase();
        let docs = self.documents.read().await;
        let mut symbols = Vec::new();

        for (uri, vd) in docs.iter() {
            let Some(ref doc) = vd.parsed else {
                continue;
            };
            let file_name = uri.path().rsplit('/').next().unwrap_or("unknown");
            Self::collect_workspace_symbols(
                &doc.children,
                &vd.source,
                &query,
                uri,
                file_name,
                &mut symbols,
            );
        }

        if symbols.is_empty() {
            Ok(None)
        } else {
            Ok(Some(symbols))
        }
    }

    /// React to file system changes (create, modify, delete .vl files).
    async fn did_change_watched_files(&self, params: DidChangeWatchedFilesParams) {
        for change in params.changes {
            let uri = change.uri;
            if uri.path().ends_with(".vl") {
                match change.typ {
                    FileChangeType::CREATED | FileChangeType::CHANGED => {
                        // Try to read and validate the file
                        if let Ok(text) = tokio::fs::read_to_string(uri.path()).await {
                            self.update_document(uri.clone(), text).await;
                        }
                    }
                    FileChangeType::DELETED => {
                        // Clear diagnostics for deleted files
                        self.documents.write().await.remove(&uri);
                        if let Some(ref client) = self.client {
                            client.publish_diagnostics(uri, Vec::new(), None).await;
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    /// Provide inlay hints showing type information for @var declarations.
    async fn inlay_hint(&self, params: InlayHintParams) -> Result<Option<Vec<InlayHint>>> {
        let docs = self.documents.read().await;
        let Some(vd) = docs.get(&params.text_document.uri) else {
            return Ok(None);
        };
        let Some(ref doc) = vd.parsed else {
            return Ok(None);
        };

        let source = &vd.source;
        let range = params.range;
        let mut hints = Vec::new();

        for node in &doc.children {
            if let Node::VarDeclaration {
                name, span, value, ..
            } = node
            {
                let node_range = Self::span_to_range(source, span);
                // Only show hints for declarations in the requested range
                if node_range.start >= range.start && node_range.end <= range.end {
                    let type_label = match value {
                        serde_json::Value::String(_) => ": string",
                        serde_json::Value::Number(n) if n.is_f64() => ": number",
                        serde_json::Value::Bool(_) => ": boolean",
                        serde_json::Value::Array(_) => ": array",
                        serde_json::Value::Object(_) => ": object",
                        serde_json::Value::Null => ": null",
                        _ => ": unknown",
                    };
                    // Position the hint right after the variable name
                    let name_end_offset = span.start + 5 + name.len(); // "@var " = 5
                    let hint_pos = Self::byte_to_position(source, name_end_offset);
                    hints.push(InlayHint {
                        position: hint_pos,
                        label: InlayHintLabel::String(type_label.to_string()),
                        kind: Some(InlayHintKind::TYPE),
                        padding_left: Some(true),
                        padding_right: Some(false),
                        tooltip: Some(InlayHintTooltip::String(format!(
                            "@var {} has type {}",
                            name, type_label
                        ))),
                        text_edits: None,
                        data: None,
                    });
                }
            }
        }

        if hints.is_empty() {
            Ok(None)
        } else {
            Ok(Some(hints))
        }
    }

    /// Provide linked editing ranges for @for loop variable names.
    async fn linked_editing_range(
        &self,
        params: LinkedEditingRangeParams,
    ) -> Result<Option<LinkedEditingRanges>> {
        let docs = self.documents.read().await;
        let Some(vd) = docs.get(&params.text_document_position_params.text_document.uri) else {
            return Ok(None);
        };
        let Some(ref doc) = vd.parsed else {
            return Ok(None);
        };

        let source = &vd.source;
        let pos = params.text_document_position_params.position;

        // Look for @for loops where the cursor is on the variable name
        for node in &doc.children {
            if let Node::ForLoop {
                variable,
                span,
                children,
                ..
            } = node
            {
                let node_range = Self::span_to_range(source, span);
                if node_range.start <= pos && pos <= node_range.end {
                    // Find the variable name span in the source
                    // @for {variable} in @{items} {
                    let node_text = &source[span.start..span.end];
                    if let Some(in_pos) = node_text.find(" in ") {
                        let var_name_start = span.start + 5; // after "@for "
                        let var_name_end = var_name_start + variable.len();
                        let var_range = Self::span_to_range(
                            source,
                            &vell_core::Span::new(var_name_start, var_name_end),
                        );

                        // Only respond if cursor is on the variable declaration
                        if var_range.start <= pos && pos <= var_range.end {
                            let mut ranges = vec![var_range];

                            // Find all @{variable} references inside the loop body
                            for child in children {
                                Self::collect_var_refs_for_loop(
                                    child,
                                    variable,
                                    source,
                                    &mut ranges,
                                );
                            }

                            return Ok(Some(LinkedEditingRanges {
                                ranges,
                                word_pattern: Some("[a-zA-Z_][a-zA-Z0-9_]*".to_string()),
                            }));
                        }
                    }
                }
            }
        }

        Ok(None)
    }

    /// Provide call hierarchy items for @[Directive] calls.
    async fn prepare_call_hierarchy(
        &self,
        params: CallHierarchyPrepareParams,
    ) -> Result<Option<Vec<CallHierarchyItem>>> {
        let docs = self.documents.read().await;
        let Some(vd) = docs.get(&params.text_document_position_params.text_document.uri) else {
            return Ok(None);
        };
        let Some(ref doc) = vd.parsed else {
            return Ok(None);
        };

        let source = &vd.source;
        let pos = params.text_document_position_params.position;
        let uri = params.text_document_position_params.text_document.uri;

        // Check if cursor is on a directive @[Name]
        for node in &doc.children {
            if let Node::Directive { name, span, .. } | Node::Extension { name, span, .. } = node {
                let node_range = Self::span_to_range(source, span);
                if node_range.start <= pos && pos <= node_range.end {
                    let item = CallHierarchyItem {
                        name: format!("@[{}]", name),
                        kind: SymbolKind::FUNCTION,
                        tags: None,
                        detail: Some(builtin_directive_description(name).to_string()),
                        uri: uri.clone(),
                        range: node_range,
                        selection_range: node_range,
                        data: None,
                    };
                    return Ok(Some(vec![item]));
                }
            }
        }

        Ok(None)
    }

    #[allow(unused_variables)]
    async fn incoming_calls(
        &self,
        params: CallHierarchyIncomingCallsParams,
    ) -> Result<Option<Vec<CallHierarchyIncomingCall>>> {
        let docs = self.documents.read().await;
        let _item = params.item;

        // Incoming calls are not currently tracked across documents.
        // This could be extended to search all open documents for references
        // to the directive.
        Ok(None)
    }

    #[allow(unused_variables)]
    async fn outgoing_calls(
        &self,
        params: CallHierarchyOutgoingCallsParams,
    ) -> Result<Option<Vec<CallHierarchyOutgoingCall>>> {
        let docs = self.documents.read().await;
        let _item = params.item;

        // Outgoing calls are not currently tracked.
        // This could be extended to show which directives are called from
        // the selected directive's body.
        Ok(None)
    }
}

impl Backend {
    fn collect_var_refs_for_loop(
        node: &Node,
        variable: &str,
        source: &str,
        ranges: &mut Vec<Range>,
    ) {
        match node {
            Node::Paragraph { children, .. } | Node::Heading { children, .. } => {
                for child in children {
                    Self::collect_var_refs_for_loop_inline(child, variable, source, ranges);
                }
            }
            Node::Blockquote { children, .. } | Node::ForLoop { children, .. } => {
                for child in children {
                    Self::collect_var_refs_for_loop(child, variable, source, ranges);
                }
            }
            Node::IfBlock {
                consequent,
                alternate,
                ..
            } => {
                for child in consequent {
                    Self::collect_var_refs_for_loop(child, variable, source, ranges);
                }
                if let Some(alt) = alternate {
                    for child in alt {
                        Self::collect_var_refs_for_loop(child, variable, source, ranges);
                    }
                }
            }
            Node::Directive { children, .. } | Node::Extension { children, .. } => {
                for child in children {
                    Self::collect_var_refs_for_loop(child, variable, source, ranges);
                }
            }
            Node::List { items, .. } => {
                for item in items {
                    for child in &item.children {
                        Self::collect_var_refs_for_loop(child, variable, source, ranges);
                    }
                }
            }
            Node::DefinitionList { items, .. } => {
                for item in items {
                    for child in &item.definition {
                        Self::collect_var_refs_for_loop(child, variable, source, ranges);
                    }
                }
            }
            Node::Table { headers, rows, .. } => {
                for cell in headers {
                    for child in &cell.children {
                        Self::collect_var_refs_for_loop_inline(child, variable, source, ranges);
                    }
                }
                for row in rows {
                    for cell in row {
                        for child in &cell.children {
                            Self::collect_var_refs_for_loop_inline(child, variable, source, ranges);
                        }
                    }
                }
            }
            _ => {}
        }
    }

    fn collect_var_refs_for_loop_inline(
        node: &InlineNode,
        variable: &str,
        source: &str,
        ranges: &mut Vec<Range>,
    ) {
        if let InlineNode::VarInterpolation { name, span, .. } = node {
            if name == variable {
                ranges.push(Self::span_to_range(source, span));
            }
        }
        // Recurse into child inlines
        match node {
            InlineNode::Bold { children, .. }
            | InlineNode::Italic { children, .. }
            | InlineNode::Underline { children, .. }
            | InlineNode::Strikethrough { children, .. }
            | InlineNode::Superscript { children, .. }
            | InlineNode::Subscript { children, .. }
            | InlineNode::Link { children, .. }
            | InlineNode::LinkRef { children, .. } => {
                for child in children {
                    Self::collect_var_refs_for_loop_inline(child, variable, source, ranges);
                }
            }
            _ => {}
        }
    }

    /// Maps a ParseErrorKind to a numeric LSP diagnostic code.
    /// Maps a ParseErrorKind to a numeric LSP diagnostic code.
    fn diagnostic_code(kind: &vell_core::ParseErrorKind) -> i32 {
        match kind {
            vell_core::ParseErrorKind::UnexpectedToken => 1,
            vell_core::ParseErrorKind::UnterminatedDelimiter => 2,
            vell_core::ParseErrorKind::InvalidIndentation => 3,
            vell_core::ParseErrorKind::UndefinedReference => 4,
            vell_core::ParseErrorKind::MalformedDirective => 5,
            vell_core::ParseErrorKind::MalformedTable => 6,
            vell_core::ParseErrorKind::MalformedMath => 7,
            vell_core::ParseErrorKind::InvalidPropValue => 8,
        }
    }

    /// Incremental variant: only re-parses from start_byte, reusing existing
    /// metadata and variables. Falls back to full parse when no cached AST exists.
    async fn update_document_incremental(&self, uri: Url, source: String, start_byte: usize) {
        let existing_parsed = {
            let docs = self.documents.read().await;
            docs.get(&uri).and_then(|vd| vd.parsed.clone())
        };

        let (parsed, errors) = if let Some(existing_doc) = existing_parsed {
            let existing_variables: HashSet<String> = existing_doc.metadata.variables.keys().cloned().collect();
            // Find safe start byte: re-parse from the beginning of the child
            // block that contains start_byte, to avoid mid-block re-parsing.
            let safe_start = existing_doc.children
                .iter()
                .find(|child| child.span().end > start_byte)
                .map(|child| child.span().start)
                .unwrap_or(start_byte);

            match parse_document_from(&source, safe_start, &existing_doc.metadata, &existing_variables) {
                Ok(outcome) => {
                    let mut merged_children: Vec<Node> = existing_doc.children
                        .iter()
                        .filter(|child| child.span().end <= safe_start)
                        .cloned()
                        .collect();
                    merged_children.extend(outcome.document.children);

                    let doc = Document {
                        version: outcome.document.version,
                        children: merged_children,
                        metadata: outcome.document.metadata,
                        span: Span::new(0, source.len()),
                    };
                    (Some(doc), outcome.warnings)
                }
                Err(e) => {
                    let parsed = parse_document(&source).ok();
                    (parsed, vec![e])
                }
            }
        } else {
            let parsed = parse_document(&source).ok();
            (parsed, Vec::new())
        };

        let vd = VellDocument {
            source: source.clone(),
            parsed,
            errors: errors.clone(),
        };
        self.documents.write().await.insert(uri.clone(), vd);

        let diagnostics: Vec<Diagnostic> = errors
            .into_iter()
            .map(|error| {
                let range = Self::span_to_range(&source, &error.span);
                let is_undefined_ref = error.kind == vell_core::ParseErrorKind::UndefinedReference;
                let code = Self::diagnostic_code(&error.kind);

                let tags = if is_undefined_ref {
                    Some(vec![DiagnosticTag::UNNECESSARY])
                } else {
                    None
                };

                Diagnostic {
                    range,
                    severity: Some(if is_undefined_ref {
                        DiagnosticSeverity::WARNING
                    } else {
                        DiagnosticSeverity::ERROR
                    }),
                    code: Some(NumberOrString::Number(code)),
                    code_description: Some(CodeDescription {
                        href: Url::parse(&format!(
                            "https://vell-lang.dev/docs/diagnostics#vell{}",
                            code
                        ))
                        .ok()
                        .unwrap(),
                    }),
                    source: Some("vell".to_string()),
                    message: error.message,
                    tags,
                    related_information: None,
                    ..Diagnostic::default()
                }
            })
            .collect();

        if let Some(ref client) = self.client {
            client.publish_diagnostics(uri, diagnostics, None).await;
        }
    }

    /// Updates the document cache and publishes diagnostics with real ranges.
    async fn update_document(&self, uri: Url, text: String) {
        let errors = validate(&text);
        let parsed = parse_document(&text).ok();

        let vd = VellDocument {
            source: text.clone(),
            parsed,
            errors: errors.clone(),
        };

        self.documents.write().await.insert(uri.clone(), vd);

        let diagnostics: Vec<Diagnostic> = errors
            .into_iter()
            .map(|error| {
                let range = Self::span_to_range(&text, &error.span);
                let is_undefined_ref = error.kind == vell_core::ParseErrorKind::UndefinedReference;
                let code = Self::diagnostic_code(&error.kind);

                let tags = if is_undefined_ref {
                    Some(vec![DiagnosticTag::UNNECESSARY])
                } else {
                    None
                };

                Diagnostic {
                    range,
                    severity: Some(if is_undefined_ref {
                        DiagnosticSeverity::WARNING
                    } else {
                        DiagnosticSeverity::ERROR
                    }),
                    code: Some(NumberOrString::Number(code)),
                    code_description: Some(CodeDescription {
                        href: Url::parse(&format!(
                            "https://vell-lang.dev/docs/diagnostics#vell{}",
                            code
                        ))
                        .ok()
                        .unwrap(),
                    }),
                    source: Some("vell".to_string()),
                    tags,
                    message: if let Some(suggestion) = &error.suggestion {
                        format!("[vell{}] {}. Suggestion: {suggestion}", code, error.message)
                    } else {
                        format!("[vell{}] {}", code, error.message)
                    },
                    ..Diagnostic::default()
                }
            })
            .collect();

        if let Some(ref client) = self.client {
            client
                .publish_diagnostics(uri, diagnostics, None)
                .await;
        }
    }

    /// Returns the source text for a given line number.
    fn get_line(source: &str, line: u32) -> &str {
        source.lines().nth(line as usize).unwrap_or("")
    }

    /// Returns the text before the cursor on the current line.
    fn prefix_before_cursor(line: &str, character: u32) -> &str {
        let idx = character as usize;
        if idx >= line.len() {
            line
        } else {
            &line[..idx]
        }
    }

    /// Check if the cursor is inside a math expression (between $...$ or $$...$$).
    fn cursor_in_math(source: &str, pos: Position) -> bool {
        let byte_offset = Self::position_to_byte(source, pos);
        let before = &source[..byte_offset];

        // Count unescaped $ signs before cursor
        let dollar_count = before.matches('$').count();
        dollar_count % 2 == 1
    }

    /// Check inline children for hoverable content.
    fn hover_inline(children: &[InlineNode], source: &str, pos: Position) -> Option<Hover> {
        for child in children {
            let span = child.span();
            let range = Self::span_to_range(source, &span);
            if range.start <= pos && pos <= range.end {
                return Self::hover_inline_node(child);
            }
        }
        None
    }

    fn hover_inline_node(node: &InlineNode) -> Option<Hover> {
        match node {
            InlineNode::VarInterpolation { name, .. } => {
                Some(Hover {
                    contents: HoverContents::Scalar(MarkedString::String(format!(
                        "Variable reference: `@{{{}}}`\n\nUse **Go to Definition** to jump to the declaration.",
                        name
                    ))),
                    range: None,
                })
            }
            InlineNode::Link { href, title, .. } => {
                let mut text = format!("Link → `{href}`");
                if let Some(t) = title {
                    text.push_str(&format!("\n\nTitle: \"{t}\""));
                }
                Some(Hover {
                    contents: HoverContents::Scalar(MarkedString::String(text)),
                    range: None,
                })
            }
            InlineNode::Image { src, alt, .. } => {
                Some(Hover {
                    contents: HoverContents::Scalar(MarkedString::String(format!(
                        "Image: `{alt}`\n\nSource: `{src}`"
                    ))),
                    range: None,
                })
            }
            InlineNode::MathInline { source: math, .. } => {
                Some(Hover {
                    contents: HoverContents::Scalar(MarkedString::String(format!(
                        "Inline math: `${}$`",
                        math
                    ))),
                    range: None,
                })
            }
            InlineNode::FootnoteRef { marker, .. } => {
                Some(Hover {
                    contents: HoverContents::Scalar(MarkedString::String(format!(
                        "Footnote reference `[^{marker}]`\n\nUse **Go to Definition** to jump to the definition."
                    ))),
                    range: None,
                })
            }
            InlineNode::Citation { key, .. } => {
                Some(Hover {
                    contents: HoverContents::Scalar(MarkedString::String(format!(
                        "Citation: `[[{key}]]`"
                    ))),
                    range: None,
                })
            }
            InlineNode::InlineComponent { name, props, .. } => {
                let prop_str = props
                    .iter()
                    .map(|(k, v)| format!("{k}={}", format_prop_value(v)))
                    .collect::<Vec<_>>()
                    .join(" ");
                Some(Hover {
                    contents: HoverContents::Scalar(MarkedString::String(format!(
                        "Component `@[{name}]({prop_str})`"
                    ))),
                    range: None,
                })
            }
            InlineNode::Code { value, .. } => {
                Some(Hover {
                    contents: HoverContents::Scalar(MarkedString::String(format!(
                        "Inline code: `` `{value}` ``"
                    ))),
                    range: None,
                })
            }
            _ => None,
        }
    }
}

fn level_label(level: usize) -> &'static str {
    match level {
        1 => "Title",
        2 => "Section",
        3 => "Subsection",
        4 => "Sub-subsection",
        _ => "Heading",
    }
}

fn format_prop_value(value: &PropValue) -> String {
    match value {
        PropValue::String(s) => format!("\"{}\"", s),
        PropValue::Number(n) => n.to_string(),
        PropValue::Bool(b) => b.to_string(),
        PropValue::Variable(v) => format!("@{{{v}}}"),
        PropValue::Null => "null".to_string(),
    }
}

fn empty_completions() -> CompletionResponse {
    CompletionResponse::Array(Vec::new())
}

const BUILTIN_DIRECTIVES: &[(&str, &str)] = &[
    ("Figure", "Embed an image or figure with caption"),
    ("Code", "Display a code snippet with syntax highlighting"),
    (
        "Diagram",
        "Render a Mermaid, ASCII, or Graphviz DOT diagram",
    ),
    ("Chart", "Render a data visualization (bar chart)"),
    ("Plot", "Render a mathematical function plot via SVG"),
    ("Cite", "Insert a formatted citation reference"),
    ("Slide", "Define a slide section for presentations"),
    ("Animation", "Create an animated sequence of content"),
    ("Frame", "Frame content in a bordered box"),
    ("Layout", "Multi-column or grid layout definition"),
    ("Column", "Define a single column within a Layout"),
    (
        "Accessibility",
        "Alt text or ARIA attributes for accessible content",
    ),
    ("Theme", "Apply a visual theme to the document"),
    ("Meta", "Set document metadata (title, author, date, lang)"),
    ("Slider", "An interactive range slider for numeric values"),
    ("Template", "Apply a reusable template to the document"),
    ("Input", "A bound text or number input field"),
    ("Select", "A bound dropdown menu"),
    ("Checkbox", "A bound checkbox toggle"),
    ("Data", "Inline JSON data document for reactive variables"),
    ("Equation", "Numbered equation with optional label"),
    ("Chem", "Chemical equation with mhchem-style formatting"),
    ("Theorem", "Numbered theorem environment"),
    ("Proof", "Proof environment (unnumbered)"),
    ("Lemma", "Numbered lemma environment"),
    ("Corollary", "Numbered corollary environment"),
    ("Definition", "Numbered definition environment"),
    ("Remark", "Remark environment (unnumbered)"),
    ("Example", "Example environment (unnumbered)"),
    ("Conjecture", "Numbered conjecture environment"),
    ("Axiom", "Numbered axiom environment"),
    ("Proposition", "Numbered proposition environment"),
    ("Notation", "Notation environment (unnumbered)"),
    ("Include", "Include content from another Vell file"),
    ("Ref", "Cross-reference a labeled element"),
    ("Toc", "Generate a table of contents from document headings"),
    ("Lof", "Generate a list of figures"),
    ("Lot", "Generate a list of tables"),
    ("Align", "Multi-line equation alignment environment"),
    ("Matrix", "Matrix environment"),
    ("PMatrix", "Matrix with parentheses delimiters"),
    ("BMatrix", "Matrix with bracket delimiters"),
    ("VMatrix", "Matrix with vertical bar delimiters"),
    ("Cases", "Piecewise function cases environment"),
    (
        "Bibliography",
        "Bibliography manager: define entries, format citations, generate reference lists",
    ),
];

const MATH_SYMBOLS: &[(&str, &str)] = &[
    ("\\alpha", "Greek alpha (α)"),
    ("\\beta", "Greek beta (β)"),
    ("\\gamma", "Greek gamma (γ)"),
    ("\\delta", "Greek delta (δ)"),
    ("\\epsilon", "Greek epsilon (ε)"),
    ("\\theta", "Greek theta (θ)"),
    ("\\lambda", "Greek lambda (λ)"),
    ("\\mu", "Greek mu (μ)"),
    ("\\pi", "Greek pi (π)"),
    ("\\sigma", "Greek sigma (σ)"),
    ("\\omega", "Greek omega (ω)"),
    ("\\int", "Integral ∫"),
    ("\\sum", "Summation ∑"),
    ("\\prod", "Product ∏"),
    ("\\frac{}{}", "Fraction"),
    ("\\sqrt{}", "Square root"),
    ("\\infty", "Infinity ∞"),
    ("\\partial", "Partial derivative ∂"),
    ("\\rightarrow", "Right arrow →"),
    ("\\Rightarrow", "Double right arrow ⇒"),
    ("\\forall", "For all ∀"),
    ("\\exists", "There exists ∃"),
    ("\\in", "Element of ∈"),
    ("\\subset", "Subset ⊂"),
    ("\\subseteq", "Subset or equal ⊆"),
];

fn builtin_directive_description(name: &str) -> &'static str {
    BUILTIN_DIRECTIVES
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, desc)| *desc)
        .unwrap_or("Namespaced extension directive")
}

/// Returns the signature information for a directive, if known.
fn directive_signature(name: &str) -> Option<SignatureInformation> {
    let params = directive_parameters(name);
    if params.is_empty() {
        None
    } else {
        let label = format!(
            "@[{}]({})",
            name,
            params
                .iter()
                .map(|(n, t)| format!("{}: {}", n, t))
                .collect::<Vec<_>>()
                .join(", ")
        );
        Some(SignatureInformation {
            label,
            documentation: Some(Documentation::String(
                builtin_directive_description(name).to_string(),
            )),
            parameters: Some(
                params
                    .iter()
                    .map(|(n, t)| ParameterInformation {
                        label: ParameterLabel::Simple(format!("{}: {}", n, t)),
                        documentation: None,
                    })
                    .collect(),
            ),
            active_parameter: None,
        })
    }
}

/// Maps a directive name to its parameter list: (name, type) pairs.
fn directive_parameters(name: &str) -> Vec<(&'static str, &'static str)> {
    match name {
        "Figure" => vec![("src", "string"), ("caption", "string")],
        "Code" => vec![("source", "string"), ("lang", "string")],
        "Diagram" => vec![("type", "string"), ("source", "string")],
        "Chart" => vec![("title", "string"), ("data", "array")],
        "Plot" => vec![("fn", "string"), ("xmin", "number"), ("xmax", "number")],
        "Cite" => vec![("key", "string")],
        "Slide" => vec![("title", "string")],
        "Animation" => vec![("frames", "number")],
        "Frame" => vec![("title", "string")],
        "Layout" => vec![("columns", "number")],
        "Column" => vec![("width", "string")],
        "Accessibility" => vec![("alt", "string")],
        "Theme" => vec![("name", "string")],
        "Meta" => vec![
            ("title", "string"),
            ("author", "string"),
            ("date", "string"),
            ("lang", "string"),
        ],
        "Slider" => vec![("min", "number"), ("max", "number"), ("default", "number")],
        "Input" => vec![("label", "string")],
        "Select" => vec![("label", "string")],
        "Checkbox" => vec![("label", "string")],
        "Data" => vec![("source", "string")],
        "Equation" => vec![("label", "string")],
        "Theorem" => vec![("title", "string")],
        "Definition" => vec![("title", "string")],
        "Example" => vec![("title", "string")],
        "Remark" => vec![("title", "string")],
        "Lemma" => vec![("title", "string")],
        "Corollary" => vec![("title", "string")],
        "Conjecture" => vec![("title", "string")],
        "Axiom" => vec![("title", "string")],
        "Proposition" => vec![("title", "string")],
        "Proof" => vec![],
        "Notation" => vec![],
        "Include" => vec![("path", "string")],
        "Ref" => vec![("id", "string")],
        "Toc" => vec![],
        "Lof" => vec![],
        "Lot" => vec![],
        "Bibliography" => vec![("style", "string")],
        "Chem" => vec![("source", "string")],
        "Align" => vec![],
        "Matrix" => vec![],
        "PMatrix" => vec![],
        "BMatrix" => vec![],
        "VMatrix" => vec![],
        "Cases" => vec![],
        _ => vec![],
    }
}

#[tokio::main]
async fn main() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();
    let (service, socket) = LspService::new(|client| Backend {
        client: Some(client),
        documents: Arc::new(RwLock::new(HashMap::new())),
    });
    Server::new(stdin, stdout, socket).serve(service).await;
}


#[cfg(test)]
mod tests {
    use super::*;

    /// Test that opening a .vl file with a deliberate parse error
    /// stores the parse errors in the document cache.
    #[tokio::test]
    async fn test_diagnostics_on_parse_error() {
        let backend = Backend {
            client: None,
            documents: Arc::new(RwLock::new(HashMap::new())),
        };

        let uri = Url::parse("file:///test-diagnostics.vl").unwrap();
        // Invalid Vell source: unclosed bold delimiter
        let invalid_source = "= Heading

This is *bold and never closed.
";

        // Call did_open directly (bypasses the LSP transport layer)
        backend
            .did_open(DidOpenTextDocumentParams {
                text_document: TextDocumentItem {
                    uri: uri.clone(),
                    language_id: "vell".to_string(),
                    version: 1,
                    text: invalid_source.to_string(),
                },
            })
            .await;

        // Verify the document was cached with parse errors
        let docs = backend.documents.read().await;
        let vd = docs.get(&uri).expect("document should be cached");
        assert!(
            !vd.errors.is_empty(),
            "should have parse errors for invalid source (got {:?})",
            vd.errors
        );
        assert!(
            vd.parsed.is_none(),
            "parsed document should be None for invalid source"
        );
    }

    /// Test that opening a valid .vl file stores zero errors.
    #[tokio::test]
    async fn test_no_diagnostics_on_valid_source() {
        let backend = Backend {
            client: None,
            documents: Arc::new(RwLock::new(HashMap::new())),
        };

        let uri = Url::parse("file:///test-valid.vl").unwrap();
        let valid_source = "= Hello

A simple paragraph.
";

        backend
            .did_open(DidOpenTextDocumentParams {
                text_document: TextDocumentItem {
                    uri: uri.clone(),
                    language_id: "vell".to_string(),
                    version: 1,
                    text: valid_source.to_string(),
                },
            })
            .await;

        let docs = backend.documents.read().await;
        let vd = docs.get(&uri).expect("document should be cached");
        assert!(
            vd.errors.is_empty(),
            "should have zero errors for valid source (got {:?})",
            vd.errors
        );
        assert!(
            vd.parsed.is_some(),
            "parsed document should be Some for valid source"
        );
    }

    /// Test that the error range is correctly computed for a parse error.
    #[tokio::test]
    async fn test_error_has_valid_span() {
        let backend = Backend {
            client: None,
            documents: Arc::new(RwLock::new(HashMap::new())),
        };

        let uri = Url::parse("file:///test-span.vl").unwrap();
        // Unclosed code span: starts with backtick but never closes
        let invalid_source = "= Test

This is `code without closing.
";

        backend
            .did_open(DidOpenTextDocumentParams {
                text_document: TextDocumentItem {
                    uri: uri.clone(),
                    language_id: "vell".to_string(),
                    version: 1,
                    text: invalid_source.to_string(),
                },
            })
            .await;

        let docs = backend.documents.read().await;
        let vd = docs.get(&uri).expect("document should be cached");
        let err = vd.errors.first().expect("should have at least one error");
        // Verify the span points to a valid byte range
        assert!(
            err.span.start < err.span.end,
            "error span should have start < end"
        );
        assert!(
            err.span.end <= vd.source.len(),
            "error span end should be within source length"
        );
        assert!(
            !err.message.is_empty(),
            "error message should not be empty"
        );
    }

    /// Test that an incremental edit introducing a parse error updates diagnostics.
    /// Starts with valid source, sends an incremental change that uncloses bold,
    /// and verifies errors appear in the cached document.
    #[tokio::test]
    async fn test_diagnostics_after_incremental_edit() {
        let backend = Backend {
            client: None,
            documents: Arc::new(RwLock::new(HashMap::new())),
        };

        let uri = Url::parse("file:///test-incremental.vl").unwrap();
        let valid_source = "= Hello

A simple paragraph.
";

        // First, open a valid document (no errors)
        backend
            .did_open(DidOpenTextDocumentParams {
                text_document: TextDocumentItem {
                    uri: uri.clone(),
                    language_id: "vell".to_string(),
                    version: 1,
                    text: valid_source.to_string(),
                },
            })
            .await;

        // Verify the document starts clean
        {
            let docs = backend.documents.read().await;
            let vd = docs.get(&uri).expect("document should be cached after did_open");
            assert!(vd.errors.is_empty(), "valid source should have no errors");
            assert!(vd.parsed.is_some(), "valid source should be parseable");
        }

        // Send an incremental edit: replace "A simple paragraph." with "A *simple paragraph."
        // to introduce an unclosed bold delimiter at line 2
        let content_change = TextDocumentContentChangeEvent {
            range: Some(Range {
                start: Position::new(2, 0),
                end: Position::new(2, 18),
            }),
            range_length: None,
            text: "A *simple paragraph.".to_string(),
        };

        backend
            .did_change(DidChangeTextDocumentParams {
                text_document: VersionedTextDocumentIdentifier {
                    uri: uri.clone(),
                    version: 2,
                },
                content_changes: vec![content_change],
            })
            .await;

        // Verify the document now has parse errors (unclosed bold delimiter)
        let docs = backend.documents.read().await;
        let vd = docs.get(&uri).expect("document should still be cached after did_change");
        assert!(
            !vd.errors.is_empty(),
            "should have parse errors after incremental edit (got {:?})",
            vd.errors
        );
        assert!(
            vd.parsed.is_none(),
            "parsed document should be None after introducing parse error"
        );
        // Verify the source was actually updated
        assert!(
            vd.source.contains("*simple"),
            "source should contain the unclosed bold marker"
        );
    }
}
