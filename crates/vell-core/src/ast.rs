// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Samin Yeasar

//! Versioned abstract syntax tree for Vell documents.

use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::fmt::{self, Display, Formatter};

/// Current AST schema version.
pub const AST_VERSION: u32 = 1;

/// Byte offsets into the original source text.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Span {
    /// Inclusive byte offset where the node starts.
    pub start: usize,
    /// Exclusive byte offset where the node ends.
    pub end: usize,
}

impl Span {
    /// Creates a new source span.
    pub const fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }
}

impl Display for Span {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}..{}", self.start, self.end)
    }
}

/// A complete Vell document.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Document {
    /// AST schema version, currently 1.
    pub version: u32,
    /// Top-level block nodes.
    pub children: Vec<Node>,
    /// Document-level metadata inferred from declarations and directives.
    pub metadata: DocumentMetadata,
    /// Source span for the entire document.
    pub span: Span,
}

impl Default for Document {
    fn default() -> Self {
        Self {
            version: AST_VERSION,
            children: Vec::new(),
            metadata: DocumentMetadata::default(),
            span: Span::default(),
        }
    }
}

impl Display for Document {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        for (index, child) in self.children.iter().enumerate() {
            if index > 0 {
                writeln!(f)?;
            }
            write!(f, "{child}")?;
        }
        Ok(())
    }
}

/// Metadata attached to a Vell document.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct DocumentMetadata {
    /// Optional document title.
    pub title: Option<String>,
    /// Optional author string.
    pub author: Option<String>,
    /// Optional ISO-like date string.
    pub date: Option<String>,
    /// Optional BCP-47 language tag.
    pub lang: Option<String>,
    /// Reactive variables declared in document order.
    pub variables: HashMap<String, JsonValue>,
}

impl Display for DocumentMetadata {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "metadata")
    }
}

/// Alignment for table cells.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Alignment {
    /// Left alignment.
    Left,
    /// Center alignment.
    Center,
    /// Right alignment.
    Right,
}

impl Display for Alignment {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Left => write!(f, "left"),
            Self::Center => write!(f, "center"),
            Self::Right => write!(f, "right"),
        }
    }
}

/// Property value used by directives and inline components.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PropValue {
    /// String property.
    String(String),
    /// Numeric property.
    Number(f64),
    /// Boolean property.
    Bool(bool),
    /// Variable reference property without evaluating it.
    Variable(String),
    /// Null property.
    Null,
}

impl Display for PropValue {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::String(value) => write!(f, "\"{}\"", value.replace('"', "\\\"")),
            Self::Number(value) => write!(f, "{value}"),
            Self::Bool(value) => write!(f, "{value}"),
            Self::Variable(value) => write!(f, "@{{{value}}}"),
            Self::Null => write!(f, "null"),
        }
    }
}

/// A list item containing nested block nodes.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ListItem {
    /// Child blocks inside the item.
    pub children: Vec<Node>,
    /// Task-list state when present.
    pub checked: Option<bool>,
    /// Source span.
    pub span: Span,
}

impl Display for ListItem {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        for child in &self.children {
            write!(f, "{child}")?;
        }
        Ok(())
    }
}

/// A table cell with inline content and merge metadata.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TableCell {
    /// Inline content in the cell.
    pub children: Vec<InlineNode>,
    /// Number of columns covered by this cell.
    pub colspan: u32,
    /// Number of rows covered by this cell.
    pub rowspan: u32,
    /// Optional alignment.
    pub align: Option<Alignment>,
    /// Source span.
    pub span: Span,
}

impl Display for TableCell {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", format_inline_nodes(&self.children))
    }
}

/// A definition-list item.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DefinitionItem {
    /// Term being defined.
    pub term: Vec<InlineNode>,
    /// Definition block content.
    pub definition: Vec<Node>,
    /// Source span.
    pub span: Span,
}

impl Display for DefinitionItem {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, ":: {}", format_inline_nodes(&self.term))
    }
}

/// Block-level AST nodes.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Node {
    /// Heading node.
    Heading {
        level: u8,
        children: Vec<InlineNode>,
        id: Option<String>,
        span: Span,
    },
    /// Paragraph node.
    Paragraph {
        children: Vec<InlineNode>,
        span: Span,
    },
    /// Blockquote node, optionally representing an admonition.
    Blockquote {
        children: Vec<Node>,
        admonition_type: Option<String>,
        span: Span,
    },
    /// Fenced code block.
    CodeBlock {
        lang: Option<String>,
        source: String,
        executable: bool,
        span: Span,
    },
    /// Block math containing raw LaTeX.
    MathBlock { source: String, span: Span },
    /// Ordered or unordered list.
    List {
        ordered: bool,
        start: Option<u32>,
        items: Vec<ListItem>,
        span: Span,
    },
    /// Table with headers and rows.
    Table {
        headers: Vec<TableCell>,
        rows: Vec<Vec<TableCell>>,
        span: Span,
    },
    /// Horizontal rule.
    HorizontalRule { span: Span },
    /// Definition list.
    DefinitionList {
        items: Vec<DefinitionItem>,
        span: Span,
    },
    /// Link reference definition.
    ReferenceDefinition {
        id: String,
        url: String,
        title: Option<String>,
        span: Span,
    },
    /// Footnote definition.
    FootnoteDefinition {
        marker: String,
        children: Vec<Node>,
        span: Span,
    },
    /// Reactive variable declaration.
    VarDeclaration {
        name: String,
        value: JsonValue,
        span: Span,
    },
    /// Experimental for-loop block.
    ForLoop {
        variable: String,
        iterable: String,
        children: Vec<Node>,
        span: Span,
    },
    /// Experimental conditional block.
    IfBlock {
        condition: String,
        consequent: Vec<Node>,
        alternate: Option<Vec<Node>>,
        span: Span,
    },
    /// Built-in directive.
    Directive {
        name: String,
        props: HashMap<String, PropValue>,
        children: Vec<Node>,
        span: Span,
    },
    /// Unknown or namespaced extension.
    Extension {
        name: String,
        props: HashMap<String, PropValue>,
        children: Vec<Node>,
        raw_source: String,
        span: Span,
    },
}

impl Node {
    /// Returns the span for this node.
    pub const fn span(&self) -> Span {
        match self {
            Self::Heading { span, .. }
            | Self::Paragraph { span, .. }
            | Self::Blockquote { span, .. }
            | Self::CodeBlock { span, .. }
            | Self::MathBlock { span, .. }
            | Self::List { span, .. }
            | Self::Table { span, .. }
            | Self::HorizontalRule { span }
            | Self::DefinitionList { span, .. }
            | Self::ReferenceDefinition { span, .. }
            | Self::FootnoteDefinition { span, .. }
            | Self::VarDeclaration { span, .. }
            | Self::ForLoop { span, .. }
            | Self::IfBlock { span, .. }
            | Self::Directive { span, .. }
            | Self::Extension { span, .. } => *span,
        }
    }
}

impl Display for Node {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Heading {
                level, children, ..
            } => write!(
                f,
                "{} {}",
                "=".repeat(usize::from(*level)),
                format_inline_nodes(children)
            ),
            Self::Paragraph { children, .. } => write!(f, "{}", format_inline_nodes(children)),
            Self::Blockquote {
                children,
                admonition_type,
                ..
            } => {
                if let Some(kind) = admonition_type {
                    writeln!(f, "> [!{kind}]")?;
                }
                for child in children {
                    for line in child.to_string().lines() {
                        writeln!(f, "> {line}")?;
                    }
                }
                Ok(())
            }
            Self::CodeBlock { lang, source, .. } => write!(
                f,
                "```{}\n{}\n```",
                lang.clone().unwrap_or_default(),
                source.trim_end()
            ),
            Self::MathBlock { source, .. } => write!(f, "$$\n{}\n$$", source.trim()),
            Self::List {
                ordered,
                start,
                items,
                ..
            } => {
                for (index, item) in items.iter().enumerate() {
                    if *ordered {
                        let base = start.unwrap_or(1);
                        writeln!(f, "{}. {}", base + u32::try_from(index).unwrap_or(0), item)?;
                    } else {
                        writeln!(f, "- {item}")?;
                    }
                }
                Ok(())
            }
            Self::Table { headers, rows, .. } => {
                writeln!(
                    f,
                    "| {} |",
                    headers
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(" | ")
                )?;
                if !headers.is_empty() {
                    writeln!(
                        f,
                        "|{}|",
                        headers.iter().map(|_| "---").collect::<Vec<_>>().join("|")
                    )?;
                }
                for row in rows {
                    writeln!(
                        f,
                        "| {} |",
                        row.iter()
                            .map(ToString::to_string)
                            .collect::<Vec<_>>()
                            .join(" | ")
                    )?;
                }
                Ok(())
            }
            Self::HorizontalRule { .. } => write!(f, "---"),
            Self::DefinitionList { items, .. } => write!(
                f,
                "{}",
                items
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join("\n")
            ),
            Self::ReferenceDefinition { id, url, title, .. } => {
                if let Some(title) = title {
                    write!(f, "[{id}]: {url} \"{title}\"")
                } else {
                    write!(f, "[{id}]: {url}")
                }
            }
            Self::FootnoteDefinition {
                marker, children, ..
            } => write!(
                f,
                "[^{marker}]: {}",
                children
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(" ")
            ),
            Self::VarDeclaration { name, value, .. } => write!(f, "@var {name} = {value}"),
            Self::ForLoop {
                variable,
                iterable,
                children,
                ..
            } => write!(
                f,
                "@for {variable} in @{{{iterable}}} {{\n{}\n}}",
                children
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join("\n\n")
            ),
            Self::IfBlock {
                condition,
                consequent,
                alternate,
                ..
            } => {
                write!(
                    f,
                    "@if {condition} {{\n{}\n}}",
                    consequent
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join("\n\n")
                )?;
                if let Some(alt) = alternate {
                    write!(
                        f,
                        " else {{\n{}\n}}",
                        alt.iter()
                            .map(ToString::to_string)
                            .collect::<Vec<_>>()
                            .join("\n\n")
                    )?;
                }
                Ok(())
            }
            Self::Directive {
                name,
                props,
                children,
                ..
            } => write!(
                f,
                "@[{name}]({}){}",
                format_props(props),
                format_directive_children(children)
            ),
            Self::Extension {
                name,
                props,
                children,
                ..
            } => write!(
                f,
                "@[{name}]({}){}",
                format_props(props),
                format_directive_children(children)
            ),
        }
    }
}

/// Inline AST nodes.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum InlineNode {
    /// Plain text.
    Text { value: String, span: Span },
    /// Strong emphasis.
    Bold {
        children: Vec<InlineNode>,
        span: Span,
    },
    /// Emphasis.
    Italic {
        children: Vec<InlineNode>,
        span: Span,
    },
    /// Underlined content.
    Underline {
        children: Vec<InlineNode>,
        span: Span,
    },
    /// Struck content.
    Strikethrough {
        children: Vec<InlineNode>,
        span: Span,
    },
    /// Inline code.
    Code { value: String, span: Span },
    /// Superscript content.
    Superscript {
        children: Vec<InlineNode>,
        span: Span,
    },
    /// Subscript content.
    Subscript {
        children: Vec<InlineNode>,
        span: Span,
    },
    /// Inline link.
    Link {
        href: String,
        title: Option<String>,
        children: Vec<InlineNode>,
        span: Span,
    },
    /// Reference-style link.
    LinkRef {
        id: String,
        children: Vec<InlineNode>,
        span: Span,
    },
    /// Inline image.
    Image {
        src: String,
        alt: String,
        title: Option<String>,
        span: Span,
    },
    /// Reference-style image.
    ImageRef { id: String, alt: String, span: Span },
    /// Inline math containing raw LaTeX.
    MathInline { source: String, span: Span },
    /// Variable interpolation.
    VarInterpolation { name: String, span: Span },
    /// Inline component.
    InlineComponent {
        name: String,
        props: HashMap<String, PropValue>,
        span: Span,
    },
    /// Citation reference.
    Citation { key: String, span: Span },
    /// Footnote reference.
    FootnoteRef { marker: String, span: Span },
    /// Soft line break.
    SoftBreak { span: Span },
    /// Hard line break.
    HardBreak { span: Span },
}

impl InlineNode {
    /// Returns the span for this inline node.
    pub const fn span(&self) -> Span {
        match self {
            Self::Text { span, .. }
            | Self::Bold { span, .. }
            | Self::Italic { span, .. }
            | Self::Underline { span, .. }
            | Self::Strikethrough { span, .. }
            | Self::Code { span, .. }
            | Self::Superscript { span, .. }
            | Self::Subscript { span, .. }
            | Self::Link { span, .. }
            | Self::LinkRef { span, .. }
            | Self::Image { span, .. }
            | Self::ImageRef { span, .. }
            | Self::MathInline { span, .. }
            | Self::VarInterpolation { span, .. }
            | Self::InlineComponent { span, .. }
            | Self::Citation { span, .. }
            | Self::FootnoteRef { span, .. }
            | Self::SoftBreak { span }
            | Self::HardBreak { span } => *span,
        }
    }
}

/// Characters and sequences that start inline markup in Vell.
/// When these appear in Text nodes, they must be backslash-escaped
/// so the formatted output can be correctly re-parsed.
const INLINE_DELIMITER_STARTS: &[&str] = &[
    "\\", "*", "/", "_", "~", "^", ",,", "`", "$", "@{", "@[", "[[", "[^", "![", "[",
];

/// Returns the text with inline delimiter characters backslash-escaped.
pub fn escape_inline_text(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < text.len() {
        let rest = &text[i..];
        let mut matched = false;
        for &prefix in INLINE_DELIMITER_STARTS {
            if rest.starts_with(prefix) {
                out.push('\\');
                out.push_str(prefix);
                i += prefix.len();
                matched = true;
                break;
            }
        }
        if !matched {
            if let Some(ch) = rest.chars().next() {
                out.push(ch);
                i += ch.len_utf8();
            } else {
                break;
            }
        }
    }
    out
}

impl Display for InlineNode {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Text { value, .. } => write!(f, "{}", escape_inline_text(value)),
            Self::Bold { children, .. } => write!(f, "*{}*", format_inline_nodes(children)),
            Self::Italic { children, .. } => write!(f, "/{}/", format_inline_nodes(children)),
            Self::Underline { children, .. } => write!(f, "_{}_", format_inline_nodes(children)),
            Self::Strikethrough { children, .. } => {
                write!(f, "~{}~", format_inline_nodes(children))
            }
            Self::Code { value, .. } => write!(f, "`{value}`"),
            Self::Superscript { children, .. } => write!(f, "^{}^", format_inline_nodes(children)),
            Self::Subscript { children, .. } => write!(f, ",,{},,", format_inline_nodes(children)),
            Self::Link { href, children, .. } => {
                write!(f, "[{}]({href})", format_inline_nodes(children))
            }
            Self::LinkRef { id, children, .. } => {
                write!(f, "[{}][{id}]", format_inline_nodes(children))
            }
            Self::Image { src, alt, .. } => write!(f, "![{alt}]({src})"),
            Self::ImageRef { id, alt, .. } => write!(f, "![{alt}][{id}]"),
            Self::MathInline { source, .. } => write!(f, "${source}$"),
            Self::VarInterpolation { name, .. } => write!(f, "@{{{name}}}"),
            Self::InlineComponent { name, props, .. } => {
                write!(f, "@[{name}]({})", format_props(props))
            }
            Self::Citation { key, .. } => write!(f, "[[{key}]]"),
            Self::FootnoteRef { marker, .. } => write!(f, "[^{marker}]"),
            Self::SoftBreak { .. } => writeln!(f),
            Self::HardBreak { .. } => writeln!(f),
        }
    }
}

/// Data-free node discriminator.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeKind {
    /// Heading.
    Heading,
    /// Paragraph.
    Paragraph,
    /// Blockquote.
    Blockquote,
    /// Code block.
    CodeBlock,
    /// Math block.
    MathBlock,
    /// List.
    List,
    /// Table.
    Table,
    /// Horizontal rule.
    HorizontalRule,
    /// Definition list.
    DefinitionList,
    /// Reference definition.
    ReferenceDefinition,
    /// Footnote definition.
    FootnoteDefinition,
    /// Variable declaration.
    VarDeclaration,
    /// For loop.
    ForLoop,
    /// If block.
    IfBlock,
    /// Directive.
    Directive,
    /// Extension.
    Extension,
}

impl From<Node> for NodeKind {
    fn from(value: Node) -> Self {
        (&value).into()
    }
}

impl From<&Node> for NodeKind {
    fn from(value: &Node) -> Self {
        match value {
            Node::Heading { .. } => Self::Heading,
            Node::Paragraph { .. } => Self::Paragraph,
            Node::Blockquote { .. } => Self::Blockquote,
            Node::CodeBlock { .. } => Self::CodeBlock,
            Node::MathBlock { .. } => Self::MathBlock,
            Node::List { .. } => Self::List,
            Node::Table { .. } => Self::Table,
            Node::HorizontalRule { .. } => Self::HorizontalRule,
            Node::DefinitionList { .. } => Self::DefinitionList,
            Node::ReferenceDefinition { .. } => Self::ReferenceDefinition,
            Node::FootnoteDefinition { .. } => Self::FootnoteDefinition,
            Node::VarDeclaration { .. } => Self::VarDeclaration,
            Node::ForLoop { .. } => Self::ForLoop,
            Node::IfBlock { .. } => Self::IfBlock,
            Node::Directive { .. } => Self::Directive,
            Node::Extension { .. } => Self::Extension,
        }
    }
}

impl Display for NodeKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

/// Formats inline nodes back to normalized Vell text.
pub fn format_inline_nodes(nodes: &[InlineNode]) -> String {
    nodes
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("")
}

/// Formats directive properties deterministically.
pub fn format_props(props: &HashMap<String, PropValue>) -> String {
    let mut pairs = props.iter().collect::<Vec<_>>();
    pairs.sort_by(|(left, _), (right, _)| left.cmp(right));
    pairs
        .into_iter()
        .map(|(key, value)| format!("{key}={value}"))
        .collect::<Vec<_>>()
        .join(" ")
}

fn format_directive_children(children: &[Node]) -> String {
    if children.is_empty() {
        String::new()
    } else {
        format!(
            " {{\n{}\n}}",
            children
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("\n\n")
        )
    }
}
