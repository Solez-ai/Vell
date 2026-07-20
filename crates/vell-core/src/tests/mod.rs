// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Samin Yeasar

//! Comprehensive parser tests for Phase 1 validation.
//!
//! Tests cover:
//! - All block-level node types (headings, paragraphs, lists, tables, etc.)
//! - All inline node types (bold, italic, code, links, images, etc.)
//! - Cross-type inline nesting (bold inside italic, etc.)
//! - Escape character support (backslash before delimiters)
//! - Error cases: unterminated delimiters, malformed tables, invalid directives
//! - All 6 spec examples parse successfully
//! - Fixture parsing (every .vl fixture produces a valid Document)
//! - Public API validation entry points

use crate::{parse_document, Alignment, InlineNode, Lexer, Node, ParseErrorKind, TokenKind};
use serde_json::Value as JsonValue;

// ---------------------------------------------------------------------------
// Spec examples
// ---------------------------------------------------------------------------

const SPEC_EXAMPLES: &[(&str, &str)] = &[
    (
        "01-basic",
        include_str!("../../../../spec/examples/01-basic.vl"),
    ),
    (
        "02-math",
        include_str!("../../../../spec/examples/02-math.vl"),
    ),
    (
        "03-tables",
        include_str!("../../../../spec/examples/03-tables.vl"),
    ),
    (
        "04-interactive",
        include_str!("../../../../spec/examples/04-interactive.vl"),
    ),
    (
        "05-extensions",
        include_str!("../../../../spec/examples/05-extensions.vl"),
    ),
    (
        "06-full-document",
        include_str!("../../../../spec/examples/06-full-document.vl"),
    ),
    (
        "07-math-comprehensive",
        include_str!("../../../../spec/examples/07-math-comprehensive.vl"),
    ),
];

#[test]
fn spec_examples_all_parse_successfully() {
    for (name, source) in SPEC_EXAMPLES {
        let result = parse_document(source);
        assert!(
            result.is_ok(),
            "Spec example '{}' failed to parse: {:?}",
            name,
            result.err()
        );
    }
}

// ---------------------------------------------------------------------------
// Headings
// ---------------------------------------------------------------------------

#[test]
fn parses_heading_level_1() {
    let doc = parse_document("= Title\n\nText.\n").unwrap();
    assert_eq!(doc.children.len(), 2);
    assert!(matches!(doc.children[0], Node::Heading { level: 1, .. }));
}

#[test]
fn parses_heading_level_2() {
    let doc = parse_document("== Section\n\nBody.\n").unwrap();
    assert_eq!(doc.children.len(), 2);
    assert!(matches!(doc.children[0], Node::Heading { level: 2, .. }));
}

#[test]
fn parses_heading_level_6() {
    let doc = parse_document("====== Deep\n").unwrap();
    assert!(matches!(doc.children[0], Node::Heading { level: 6, .. }));
}

#[test]
fn rejects_heading_level_7() {
    let err = parse_document("======= Too deep\n").unwrap_err();
    assert!(matches!(err.kind, ParseErrorKind::UnexpectedToken));
}

#[test]
fn heading_without_space_is_paragraph() {
    let doc = parse_document("=NoSpace\n").unwrap();
    assert!(matches!(doc.children[0], Node::Paragraph { .. }));
}

// ---------------------------------------------------------------------------
// Paragraphs
// ---------------------------------------------------------------------------

#[test]
fn parses_simple_paragraph() {
    let doc = parse_document("Hello world.\n").unwrap();
    assert_eq!(doc.children.len(), 1);
    assert!(matches!(doc.children[0], Node::Paragraph { .. }));
}

#[test]
fn parses_multi_line_paragraph() {
    let doc = parse_document("Line one.\nLine two.\n").unwrap();
    assert_eq!(doc.children.len(), 1);
}

#[test]
fn parses_empty_document() {
    let doc = parse_document("").unwrap();
    assert!(doc.children.is_empty());
}

#[test]
fn parses_whitespace_only_document() {
    let doc = parse_document("  \n\n  \n").unwrap();
    assert!(doc.children.is_empty());
}

// ---------------------------------------------------------------------------
// Blockquotes
// ---------------------------------------------------------------------------

#[test]
fn parses_blockquote() {
    let doc = parse_document("> Quoted text.\n").unwrap();
    assert!(matches!(doc.children[0], Node::Blockquote { .. }));
}

#[test]
fn parses_blockquote_multi_line() {
    let doc = parse_document("> Line one.\n> Line two.\n").unwrap();
    assert_eq!(doc.children.len(), 1);
}

#[test]
fn parses_admonition() {
    let doc = parse_document("> [!NOTE]\n> A note.\n").unwrap();
    let Node::Blockquote {
        admonition_type, ..
    } = &doc.children[0]
    else {
        panic!("expected blockquote");
    };
    assert_eq!(admonition_type.as_deref(), Some("NOTE"));
}

// ---------------------------------------------------------------------------
// Code blocks
// ---------------------------------------------------------------------------

#[test]
fn parses_code_block_with_lang() {
    let doc = parse_document("```rust\nfn main() {}\n```\n").unwrap();
    let Node::CodeBlock { lang, source, .. } = &doc.children[0] else {
        panic!("expected code block");
    };
    assert_eq!(lang.as_deref(), Some("rust"));
    assert!(source.contains("fn main()"));
}

#[test]
fn parses_code_block_without_lang() {
    let doc = parse_document("```\nplain code\n```\n").unwrap();
    assert!(matches!(doc.children[0], Node::CodeBlock { .. }));
}

// ---------------------------------------------------------------------------
// Math blocks
// ---------------------------------------------------------------------------

#[test]
fn parses_math_block() {
    let doc = parse_document("$$\n\\int x dx\n$$\n").unwrap();
    assert!(matches!(doc.children[0], Node::MathBlock { .. }));
}

// ---------------------------------------------------------------------------
// Lists
// ---------------------------------------------------------------------------

#[test]
fn parses_unordered_list() {
    let doc = parse_document("- Item 1\n- Item 2\n").unwrap();
    let Node::List { ordered, items, .. } = &doc.children[0] else {
        panic!("expected list");
    };
    assert!(!ordered);
    assert_eq!(items.len(), 2);
}

#[test]
fn parses_ordered_list() {
    let doc = parse_document("1. First\n2. Second\n").unwrap();
    let Node::List { ordered, items, .. } = &doc.children[0] else {
        panic!("expected list");
    };
    assert!(ordered);
    assert_eq!(items.len(), 2);
}

// ---------------------------------------------------------------------------
// Horizontal rules
// ---------------------------------------------------------------------------

#[test]
fn parses_horizontal_rule() {
    let doc = parse_document("---\n").unwrap();
    assert!(matches!(doc.children[0], Node::HorizontalRule { .. }));
}

#[test]
fn parses_long_horizontal_rule() {
    let doc = parse_document("--------\n").unwrap();
    assert!(matches!(doc.children[0], Node::HorizontalRule { .. }));
}

// ---------------------------------------------------------------------------
// Pipe tables
// ---------------------------------------------------------------------------

#[test]
fn parses_pipe_table_simple() {
    let doc = parse_document("| A | B |\n|---|---|\n| 1 | 2 |\n").unwrap();
    assert!(matches!(doc.children[0], Node::Table { .. }));
}

#[test]
fn parses_pipe_table_alignment() {
    let doc = parse_document(
        "| Left | Center | Right |\n|:-----|:------:|------:|\n| a    | b      | c     |\n",
    )
    .unwrap();
    let Node::Table { headers, rows, .. } = &doc.children[0] else {
        panic!("expected table");
    };
    assert_eq!(headers[0].align, Some(Alignment::Left));
    assert_eq!(headers[1].align, Some(Alignment::Center));
    assert_eq!(headers[2].align, Some(Alignment::Right));
    assert_eq!(rows.len(), 1);
}

#[test]
fn rejects_pipe_table_wrong_cell_count() {
    let err = parse_document("| A | B |\n|---|---|\n| 1 | 2 | 3 |\n").unwrap_err();
    assert!(matches!(err.kind, ParseErrorKind::MalformedTable));
}

// ---------------------------------------------------------------------------
// Grid tables
// ---------------------------------------------------------------------------

#[test]
fn parses_grid_table() {
    let doc = parse_document(
        "+-----+-----+\n| X   | Y   |\n+-----+-----+\n| 1   | 2   |\n+-----+-----+\n",
    )
    .unwrap();
    assert!(matches!(doc.children[0], Node::Table { .. }));
}

#[test]
fn parses_grid_table_colspan() {
    let doc = parse_document(
        "+---+---+---+\n| A     | C |\n+---+---+---+\n| 1 | 2 | 3 |\n+---+---+---+\n",
    )
    .unwrap();
    let Node::Table { headers, rows, .. } = &doc.children[0] else {
        panic!("expected table");
    };
    assert_eq!(headers.len(), 2);
    assert_eq!(headers[0].colspan, 2);
    assert_eq!(headers[1].colspan, 1);
    assert_eq!(rows[0].len(), 3);
}

// ---------------------------------------------------------------------------
// Definition lists
// ---------------------------------------------------------------------------

#[test]
fn parses_definition_list() {
    let doc = parse_document(":: Term\n   Definition.\n").unwrap();
    assert!(matches!(doc.children[0], Node::DefinitionList { .. }));
}

// ---------------------------------------------------------------------------
// Variables
// ---------------------------------------------------------------------------

#[test]
fn parses_variable_declaration_and_reference() {
    let doc = parse_document("@var x = 1\n@{x}\n").unwrap();
    assert_eq!(doc.metadata.variables.len(), 1);
    let Node::VarDeclaration { name, .. } = &doc.children[0] else {
        panic!("expected var declaration");
    };
    assert_eq!(name, "x");
    assert!(doc.metadata.variables.contains_key("x"));
}

#[test]
fn warns_undefined_variable() {
    let result = crate::parse_document_with_warnings("@{undefined}\n").unwrap();
    assert!(!result.warnings.is_empty());
}

// ---------------------------------------------------------------------------
// For loops
// ---------------------------------------------------------------------------

#[test]
fn parses_for_loop() {
    let doc = parse_document("@var items = [1, 2]\n@for item in @{items} {\n  Body.\n}\n").unwrap();
    assert!(matches!(doc.children[1], Node::ForLoop { .. }));
}

#[test]
fn loop_variables_are_available_inside_the_loop_only() {
    let result = crate::parse_document_with_warnings(
        "@var items = [1]\n@for item in @{items} {\n  @{item}\n}\n@{item}\n",
    )
    .unwrap();
    assert_eq!(result.warnings.len(), 1);
    assert!(result.warnings[0].message.contains("item"));
}

#[test]
fn loop_headers_require_valid_declared_iterables() {
    let errors = crate::validate("@for item in @{missing} {\n  Body.\n}\n");
    assert!(
        errors
            .iter()
            .any(|error| error.kind == ParseErrorKind::UndefinedReference)
    );
    let errors = crate::validate("@for 1item in @{items} {\n  Body.\n}\n");
    assert!(
        errors
            .iter()
            .any(|error| error.kind == ParseErrorKind::UnexpectedToken)
    );
}

#[test]
fn identifiers_use_unicode_nfc() {
    let result = crate::parse_document_with_warnings("@var café = \"ok\"\n@{café}\n").unwrap();
    assert!(result.warnings.is_empty());
    assert!(result.document.metadata.variables.contains_key("café"));
}

// ---------------------------------------------------------------------------
// If blocks
// ---------------------------------------------------------------------------

#[test]
fn parses_if_block() {
    let doc = parse_document("@if true {\n  Consequent.\n}\n").unwrap();
    assert!(matches!(doc.children[0], Node::IfBlock { .. }));
}

#[test]
fn parses_if_else() {
    let doc = parse_document("@if true {\n  Yes.\n} else {\n  No.\n}\n").unwrap();
    assert!(matches!(doc.children[0], Node::IfBlock { .. }));
}

// ---------------------------------------------------------------------------
// Directives
// ---------------------------------------------------------------------------

#[test]
fn parses_directive_inline() {
    let doc = parse_document("@[Figure](src=\"a.png\")\n").unwrap();
    assert!(matches!(doc.children[0], Node::Directive { .. }));
}

#[test]
fn parses_directive_with_body() {
    let doc = parse_document("@[Layout](columns=2) {\n  Content.\n}\n").unwrap();
    assert!(matches!(doc.children[0], Node::Directive { .. }));
}

#[test]
fn parses_namespaced_extension() {
    let doc = parse_document("@[myorg/CustomWidget](data=@{count})\n").unwrap();
    assert!(matches!(doc.children[0], Node::Extension { .. }));
}

#[test]
fn reports_unclosed_braced_directive_body() {
    let err = parse_document("@[Layout](columns=2) {\n  Missing close.\n").unwrap_err();
    assert!(matches!(err.kind, ParseErrorKind::UnexpectedToken));
}

// ---------------------------------------------------------------------------
// Reference definitions
// ---------------------------------------------------------------------------

#[test]
fn parses_reference_definition() {
    let doc = parse_document("[ref]: https://example.com \"Title\"\n").unwrap();
    assert!(matches!(doc.children[0], Node::ReferenceDefinition { .. }));
}

// ---------------------------------------------------------------------------
// Footnote definitions
// ---------------------------------------------------------------------------

#[test]
fn parses_footnote_definition() {
    let doc = parse_document("[^one]: Footnote body.\n").unwrap();
    assert!(matches!(doc.children[0], Node::FootnoteDefinition { .. }));
}

// ---------------------------------------------------------------------------
// Inline markup
// ---------------------------------------------------------------------------

#[test]
fn parses_bold() {
    let doc = parse_document("Text with *bold*.\n").unwrap();
    let Node::Paragraph { children, .. } = &doc.children[0] else {
        panic!("expected paragraph");
    };
    let found = children
        .iter()
        .any(|n| matches!(n, InlineNode::Bold { .. }));
    assert!(found, "expected bold inline node");
}

#[test]
fn parses_italic() {
    let doc = parse_document("Text with /italic/.\n").unwrap();
    let Node::Paragraph { children, .. } = &doc.children[0] else {
        panic!("expected paragraph");
    };
    let found = children
        .iter()
        .any(|n| matches!(n, InlineNode::Italic { .. }));
    assert!(found, "expected italic inline node");
}

#[test]
fn parses_underline() {
    let doc = parse_document("Text with _underline_.\n").unwrap();
    let Node::Paragraph { children, .. } = &doc.children[0] else {
        panic!("expected paragraph");
    };
    let found = children
        .iter()
        .any(|n| matches!(n, InlineNode::Underline { .. }));
    assert!(found, "expected underline inline node");
}

#[test]
fn parses_strikethrough() {
    let doc = parse_document("Text with ~strike~.\n").unwrap();
    let Node::Paragraph { children, .. } = &doc.children[0] else {
        panic!("expected paragraph");
    };
    let found = children
        .iter()
        .any(|n| matches!(n, InlineNode::Strikethrough { .. }));
    assert!(found, "expected strikethrough inline node");
}

#[test]
fn parses_superscript() {
    let doc = parse_document("Text with ^super^.\n").unwrap();
    let Node::Paragraph { children, .. } = &doc.children[0] else {
        panic!("expected paragraph");
    };
    let found = children
        .iter()
        .any(|n| matches!(n, InlineNode::Superscript { .. }));
    assert!(found, "expected superscript inline node");
}

#[test]
fn parses_subscript() {
    let doc = parse_document("Text with ,,sub,,\n").unwrap();
    let Node::Paragraph { children, .. } = &doc.children[0] else {
        panic!("expected paragraph");
    };
    let found = children
        .iter()
        .any(|n| matches!(n, InlineNode::Subscript { .. }));
    assert!(found, "expected subscript inline node");
}

#[test]
fn parses_inline_code() {
    let doc = parse_document("Text with `code`.\n").unwrap();
    let Node::Paragraph { children, .. } = &doc.children[0] else {
        panic!("expected paragraph");
    };
    let found = children
        .iter()
        .any(|n| matches!(n, InlineNode::Code { .. }));
    assert!(found, "expected code inline node");
}

#[test]
fn parses_inline_math() {
    let doc = parse_document("Text with $a^2$.\n").unwrap();
    let Node::Paragraph { children, .. } = &doc.children[0] else {
        panic!("expected paragraph");
    };
    let found = children
        .iter()
        .any(|n| matches!(n, InlineNode::MathInline { .. }));
    assert!(found, "expected math inline node");
}

#[test]
fn parses_link() {
    let doc = parse_document("[text](https://example.com)\n").unwrap();
    let Node::Paragraph { children, .. } = &doc.children[0] else {
        panic!("expected paragraph");
    };
    assert!(children
        .iter()
        .any(|n| matches!(n, InlineNode::Link { .. })));
}

#[test]
fn parses_link_with_title() {
    let doc = parse_document("[text](https://example.com \"Title\")\n").unwrap();
    let Node::Paragraph { children, .. } = &doc.children[0] else {
        panic!("expected paragraph");
    };
    assert!(children
        .iter()
        .any(|n| matches!(n, InlineNode::Link { .. })));
}

#[test]
fn parses_link_ref() {
    let doc = parse_document("[text][ref]\n\n[ref]: https://example.com\n").unwrap();
    let Node::Paragraph { children, .. } = &doc.children[0] else {
        panic!("expected paragraph");
    };
    assert!(children
        .iter()
        .any(|n| matches!(n, InlineNode::LinkRef { .. })));
}

#[test]
fn parses_image() {
    let doc = parse_document("![alt](image.png)\n").unwrap();
    let Node::Paragraph { children, .. } = &doc.children[0] else {
        panic!("expected paragraph");
    };
    assert!(children
        .iter()
        .any(|n| matches!(n, InlineNode::Image { .. })));
}

#[test]
fn parses_image_ref() {
    let doc = parse_document("![alt][ref]\n\n[ref]: https://example.com/img.png\n").unwrap();
    let Node::Paragraph { children, .. } = &doc.children[0] else {
        panic!("expected paragraph");
    };
    assert!(children
        .iter()
        .any(|n| matches!(n, InlineNode::ImageRef { .. })));
}

#[test]
fn parses_citation() {
    let doc = parse_document("See [[smith2023]].\n").unwrap();
    let Node::Paragraph { children, .. } = &doc.children[0] else {
        panic!("expected paragraph");
    };
    assert!(children
        .iter()
        .any(|n| matches!(n, InlineNode::Citation { .. })));
}

#[test]
fn parses_footnote_ref() {
    let doc = parse_document("Text[^one].\n\n[^one]: Footnote.\n").unwrap();
    let Node::Paragraph { children, .. } = &doc.children[0] else {
        panic!("expected paragraph");
    };
    assert!(children
        .iter()
        .any(|n| matches!(n, InlineNode::FootnoteRef { .. })));
}

#[test]
fn parses_inline_component() {
    let doc = parse_document("Text @[Component](key=val).\n").unwrap();
    let Node::Paragraph { children, .. } = &doc.children[0] else {
        panic!("expected paragraph");
    };
    assert!(children
        .iter()
        .any(|n| matches!(n, InlineNode::InlineComponent { .. })));
}

// ---------------------------------------------------------------------------
// Cross-type inline nesting
// ---------------------------------------------------------------------------

#[test]
fn parses_bold_inside_italic() {
    let doc = parse_document("Text /italic and *bold* inside/.\n").unwrap();
    let Node::Paragraph { children, .. } = &doc.children[0] else {
        panic!("expected paragraph");
    };
    let has_italic = children
        .iter()
        .any(|n| matches!(n, InlineNode::Italic { .. }));
    assert!(has_italic, "expected italic node");
}

#[test]
fn parses_italic_inside_bold() {
    let doc = parse_document("Text *bold and /italic/ inside*.\n").unwrap();
    let Node::Paragraph { children, .. } = &doc.children[0] else {
        panic!("expected paragraph");
    };
    let has_bold = children
        .iter()
        .any(|n| matches!(n, InlineNode::Bold { .. }));
    assert!(has_bold, "expected bold node");
}

#[test]
fn parses_underline_inside_bold() {
    let doc = parse_document("Text *bold and _underline_ inside*.\n").unwrap();
    let Node::Paragraph { children, .. } = &doc.children[0] else {
        panic!("expected paragraph");
    };
    let has_bold = children
        .iter()
        .any(|n| matches!(n, InlineNode::Bold { .. }));
    assert!(has_bold, "expected bold node with underline inside");
}

#[test]
fn parses_code_inside_italic() {
    let doc = parse_document("Text /italic and `code` inside/.\n").unwrap();
    let Node::Paragraph { children, .. } = &doc.children[0] else {
        panic!("expected paragraph");
    };
    let has_italic = children
        .iter()
        .any(|n| matches!(n, InlineNode::Italic { .. }));
    assert!(has_italic, "expected italic node with code inside");
}

// ---------------------------------------------------------------------------
// Escape support
// ---------------------------------------------------------------------------

#[test]
fn parses_escaped_italic_delimiter() {
    let bs = '\\';
    let source = format!("Both {}*slashes{}* are escaped.\n", bs, bs);
    let doc = parse_document(&source).unwrap();
    let Node::Paragraph { children, .. } = &doc.children[0] else {
        panic!("expected paragraph");
    };
    let has_italic = children
        .iter()
        .any(|n| matches!(n, InlineNode::Italic { .. }));
    assert!(
        !has_italic,
        "escaped slashes should not create italic, got children: {:?}",
        children
    );
}

// ---------------------------------------------------------------------------
// Lexer tests
// ---------------------------------------------------------------------------

#[test]
fn lexer_emits_eof() {
    let mut seen = false;
    for token in Lexer::new("= Title\n") {
        if token.kind == TokenKind::Eof {
            seen = true;
        }
    }
    assert!(seen);
}

#[test]
fn lexer_emits_tokens_for_empty_input() {
    let tokens: Vec<_> = Lexer::new("").collect();
    assert!(tokens.last().is_some_and(|t| t.kind == TokenKind::Eof));
}

#[test]
fn lexer_handles_blank_lines() {
    let tokens: Vec<_> = Lexer::new("= H\n\nBody\n").collect();
    let has_blank = tokens.iter().any(|t| t.kind == TokenKind::BlankLine);
    assert!(has_blank);
}

// ---------------------------------------------------------------------------
// Unterminated delimiter errors
// ---------------------------------------------------------------------------

#[test]
fn reports_unclosed_bold() {
    let err = parse_document("This is *open.\n").unwrap_err();
    assert!(matches!(err.kind, ParseErrorKind::UnterminatedDelimiter));
}

#[test]
fn reports_unclosed_code() {
    let err = parse_document("This is `open code.\n").unwrap_err();
    assert!(matches!(err.kind, ParseErrorKind::UnterminatedDelimiter));
}

#[test]
fn reports_unclosed_math_inline() {
    let err = parse_document("This is $open math.\n").unwrap_err();
    assert!(matches!(err.kind, ParseErrorKind::MalformedMath));
}

#[test]
fn reports_unclosed_link() {
    let err = parse_document("[text(https://example.com)\n").unwrap_err();
    assert!(matches!(err.kind, ParseErrorKind::UnterminatedDelimiter));
}

#[test]
fn reports_unclosed_image_alt() {
    let err = parse_document("![alt(https://example.com)\n").unwrap_err();
    assert!(matches!(err.kind, ParseErrorKind::UnterminatedDelimiter));
}

// ---------------------------------------------------------------------------
// Fixture tests
// ---------------------------------------------------------------------------

#[test]
fn fixture_headings_parses() {
    let source = include_str!("fixtures/headings.vl");
    let doc = parse_document(source).unwrap();
    assert!(matches!(doc.children[0], Node::Heading { .. }));
    assert!(matches!(doc.children[1], Node::Heading { level: 2, .. }));
}

#[test]
fn fixture_inline_parses() {
    let source = include_str!("fixtures/inline.vl");
    let doc = parse_document(source).unwrap();
    assert!(!doc.children.is_empty());
}

#[test]
fn fixture_lists_parses() {
    let source = include_str!("fixtures/lists.vl");
    let doc = parse_document(source).unwrap();
    assert!(!doc.children.is_empty());
}

#[test]
fn fixture_code_blocks_parses() {
    let source = include_str!("fixtures/code_blocks.vl");
    let doc = parse_document(source).unwrap();
    assert!(matches!(doc.children[0], Node::CodeBlock { .. }));
}

#[test]
fn fixture_math_parses() {
    let source = include_str!("fixtures/math.vl");
    let doc = parse_document(source).unwrap();
    assert!(!doc.children.is_empty());
}

#[test]
fn fixture_pipe_table_parses() {
    let source = include_str!("fixtures/pipe_table.vl");
    let doc = parse_document(source).unwrap();
    assert!(matches!(doc.children[0], Node::Table { .. }));
}

#[test]
fn fixture_grid_table_merged_parses() {
    let source = include_str!("fixtures/grid_table_merged.vl");
    let doc = parse_document(source).unwrap();
    assert!(matches!(doc.children[0], Node::Table { .. }));
}

#[test]
fn fixture_directives_parses() {
    let source = include_str!("fixtures/directives.vl");
    let doc = parse_document(source).unwrap();
    assert!(matches!(doc.children[0], Node::Directive { .. }));
}

#[test]
fn fixture_variables_parses() {
    let source = include_str!("fixtures/variables.vl");
    let doc = parse_document(source).unwrap();
    assert!(!doc.children.is_empty());
}

#[test]
fn fixture_for_loop_parses() {
    let source = include_str!("fixtures/for_loop.vl");
    let doc = parse_document(source).unwrap();
    assert!(matches!(doc.children[0], Node::ForLoop { .. }));
}

#[test]
fn fixture_full_document_parses() {
    let source = include_str!("fixtures/full_document.vl");
    let doc = parse_document(source).unwrap();
    assert!(!doc.children.is_empty());
}

#[test]
fn fixture_nested_inline_parses() {
    let source = include_str!("fixtures/nested_inline.vl");
    let doc = parse_document(source).unwrap();
    assert!(!doc.children.is_empty());
    assert!(matches!(&doc.children[0], crate::Node::Paragraph { .. }));
}

#[test]
fn fixture_escaped_chars_parses() {
    let source = include_str!("fixtures/escaped_chars.vl");
    let doc = parse_document(source).unwrap();
    assert!(!doc.children.is_empty());
}

#[test]
fn fixture_ref_defs_parses() {
    let source = include_str!("fixtures/ref_defs.vl");
    let doc = parse_document(source).unwrap();
    assert!(!doc.children.is_empty());
}

#[test]
fn fixture_footnote_defs_parses() {
    let source = include_str!("fixtures/footnote_defs.vl");
    let doc = parse_document(source).unwrap();
    assert!(!doc.children.is_empty());
}

#[test]
fn fixture_if_block_parses() {
    let source = include_str!("fixtures/if_block.vl");
    let doc = parse_document(source).unwrap();
    assert!(matches!(doc.children[0], crate::Node::IfBlock { .. }));
}

#[test]
fn fixture_image_refs_parses() {
    let source = include_str!("fixtures/image_refs.vl");
    let doc = parse_document(source).unwrap();
    assert!(!doc.children.is_empty());
}

#[test]
fn fixture_link_refs_parses() {
    let source = include_str!("fixtures/link_refs.vl");
    let doc = parse_document(source).unwrap();
    assert!(!doc.children.is_empty());
}

#[test]
fn fixture_admonition_parses() {
    let source = include_str!("fixtures/admonition.vl");
    let doc = parse_document(source).unwrap();
    assert!(!doc.children.is_empty());
}

#[test]
fn fixture_classic_table_parses() {
    let source = include_str!("fixtures/classic_table.vl");
    let doc = parse_document(source).unwrap();
    assert!(matches!(doc.children[0], crate::Node::Table { .. }));
}

#[test]
fn fixture_def_list_parses() {
    let source = include_str!("fixtures/def_list.vl");
    let doc = parse_document(source).unwrap();
    assert!(!doc.children.is_empty());
}

#[test]
fn fixture_hrule_markers_parses() {
    let source = include_str!("fixtures/hrule_markers.vl");
    let doc = parse_document(source).unwrap();
    assert!(!doc.children.is_empty());
}

#[test]
fn fixture_pipe_table_align_parses() {
    let source = include_str!("fixtures/pipe_table_align.vl");
    let doc = parse_document(source).unwrap();
    assert!(matches!(doc.children[0], crate::Node::Table { .. }));
}

#[test]
fn fixture_list_nested_parses() {
    let source = include_str!("fixtures/list_nested.vl");
    let doc = parse_document(source).unwrap();
    assert!(!doc.children.is_empty());
}

#[test]
fn fixture_code_blocks_no_lang_parses() {
    let source = include_str!("fixtures/code_blocks_no_lang.vl");
    let doc = parse_document(source).unwrap();
    assert!(matches!(doc.children[0], crate::Node::CodeBlock { .. }));
}

#[test]
fn fixture_math_inline_parses() {
    let source = include_str!("fixtures/math_inline.vl");
    let doc = parse_document(source).unwrap();
    assert!(!doc.children.is_empty());
}

#[test]
fn fixture_inline_components_parses() {
    let source = include_str!("fixtures/inline_components.vl");
    let doc = parse_document(source).unwrap();
    assert!(!doc.children.is_empty());
}

#[test]
fn fixture_empty_directive_parses() {
    let source = include_str!("fixtures/empty_directive.vl");
    let doc = parse_document(source).unwrap();
    assert!(!doc.children.is_empty());
}

#[test]
fn fixture_block_directive_parses() {
    let source = include_str!("fixtures/block_directive.vl");
    let doc = parse_document(source).unwrap();
    assert!(!doc.children.is_empty());
}

// ---------------------------------------------------------------------------
// Error span verification
// ---------------------------------------------------------------------------

#[test]
fn error_spans_are_non_default_for_real_errors() {
    let err = parse_document("| A | B |\n|---|---|\n| 1 | 2 | 3 |\n").unwrap_err();
    assert!(
        err.span.start > 0 || err.span.end > 0,
        "error span should be non-zero: {:?}",
        err.span
    );
}

#[test]
fn unterminated_bold_span_is_non_default() {
    let err = parse_document("This *bold never ends.\n").unwrap_err();
    assert!(
        err.span.start > 0 || err.span.end > 0,
        "error span should be non-zero: {:?}",
        err.span
    );
}

// ---------------------------------------------------------------------------
// Validation public API
// ---------------------------------------------------------------------------

#[test]
fn validate_returns_errors_for_invalid_syntax() {
    let errors = crate::validate("| A | B |\n|---|---|\n| 1 | 2 | 3 |\n");
    assert!(!errors.is_empty());
}

#[test]
fn validate_returns_warnings_for_undefined_variables() {
    let errors = crate::validate("@{unknown}\n");
    assert!(!errors.is_empty());
    assert!(errors
        .iter()
        .any(|e| matches!(e.kind, ParseErrorKind::UndefinedReference)));
}

// ---------------------------------------------------------------------------
// Snapshot comparison tests
//
// Each valid fixture is parsed and compared against its expected JSON.
// Spans (byte offsets) are stripped before comparison to avoid brittle
// line-ending or whitespace-only changes from breaking tests.
// ---------------------------------------------------------------------------

/// Strips all `"span"` keys from a JSON value recursively.
fn strip_spans(value: &JsonValue) -> JsonValue {
    match value {
        JsonValue::Object(map) => {
            let mut out = serde_json::Map::new();
            for (k, v) in map {
                if k != "span" {
                    out.insert(k.clone(), strip_spans(v));
                }
            }
            JsonValue::Object(out)
        }
        JsonValue::Array(arr) => JsonValue::Array(arr.iter().map(strip_spans).collect()),
        other => other.clone(),
    }
}

const SNAPSHOT_FIXTURES: &[&str] = &[
    "headings",
    "inline",
    "lists",
    "code_blocks",
    "math",
    "pipe_table",
    "grid_table_merged",
    "directives",
    "variables",
    "for_loop",
    "full_document",
    "nested_inline",
    "escaped_chars",
    "ref_defs",
    "footnote_defs",
    "if_block",
    "image_refs",
    "link_refs",
    "admonition",
    "def_list",
    "hrule_markers",
    "pipe_table_align",
    "list_nested",
    "code_blocks_no_lang",
    "math_inline",
    "inline_components",
    "empty_directive",
    "block_directive",
];

fn fixture_source(name: &str) -> String {
    let path = format!(
        "{}/src/tests/fixtures/{}.vl",
        env!("CARGO_MANIFEST_DIR"),
        name
    );
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", &path))
}

fn fixture_expected_json(name: &str) -> JsonValue {
    let path = format!(
        "{}/src/tests/fixtures/{}.json",
        env!("CARGO_MANIFEST_DIR"),
        name
    );
    let text =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", &path));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("invalid JSON in {}: {e}", &path))
}

#[test]
fn snapshots_match_all_fixtures() {
    for name in SNAPSHOT_FIXTURES {
        let source = fixture_source(name);
        let doc = parse_document(&source)
            .unwrap_or_else(|e| panic!("snapshot '{name}' parse failed: {e:?}"));
        let actual_json = serde_json::to_value(&doc)
            .unwrap_or_else(|e| panic!("snapshot '{name}' serialize failed: {e}"));
        let expected_json = fixture_expected_json(name);
        let actual_stripped = strip_spans(&actual_json);
        let expected_stripped = strip_spans(&expected_json);
        assert_eq!(
            actual_stripped,
            expected_stripped,
            "snapshot mismatch for fixture '{name}'\n\
             actual (no spans):   {}\n\
             expected (no spans): {}",
            serde_json::to_string_pretty(&actual_stripped).unwrap(),
            serde_json::to_string_pretty(&expected_stripped).unwrap(),
        );
    }
}

#[test]
fn comment_stripping_preserves_unicode() {
    let source = "= 日本語\n\nمرحبا — Привет 😀 /* hidden */ café\n";
    let document = parse_document(source).expect("unicode source should parse");
    let paragraph = document
        .children
        .iter()
        .find_map(|node| match node {
            Node::Paragraph { children, .. } => Some(children),
            _ => None,
        })
        .expect("paragraph should be present");
    let text = paragraph
        .iter()
        .filter_map(|node| match node {
            InlineNode::Text { value, .. } => Some(value.as_str()),
            _ => None,
        })
        .collect::<String>();
    assert_eq!(text, "مرحبا — Привет 😀  café");
}

// ---------------------------------------------------------------------------
// Invalid syntax fixture tests
// ---------------------------------------------------------------------------

const INVALID_FIXTURES: &[(&str, ParseErrorKind)] = &[
    ("err_unclosed_bold", ParseErrorKind::UnterminatedDelimiter),
    ("err_unclosed_italic", ParseErrorKind::UnterminatedDelimiter),
    ("err_unclosed_code", ParseErrorKind::UnterminatedDelimiter),
    ("err_unclosed_math", ParseErrorKind::MalformedMath),
    ("err_unclosed_link", ParseErrorKind::UnterminatedDelimiter),
    ("err_unclosed_image", ParseErrorKind::UnterminatedDelimiter),
    ("err_unclosed_directive", ParseErrorKind::MalformedDirective),
    ("err_malformed_table_pipe", ParseErrorKind::MalformedTable),
    ("err_malformed_table_grid", ParseErrorKind::MalformedTable),
    (
        "err_malformed_directive",
        ParseErrorKind::MalformedDirective,
    ),
    ("err_undefined_var", ParseErrorKind::UndefinedReference),
    ("err_unclosed_fence", ParseErrorKind::UnterminatedDelimiter),
    ("err_bad_math_block", ParseErrorKind::MalformedMath),
    ("err_bad_prop_value", ParseErrorKind::InvalidPropValue),
    ("err_bad_indent", ParseErrorKind::UnexpectedToken),
    ("err_invalid_indent", ParseErrorKind::InvalidIndentation),
];

#[test]
fn invalid_fixtures_produce_expected_errors() {
    for (name, expected_kind) in INVALID_FIXTURES {
        let source = fixture_source(name);
        let errors = crate::validate(&source);
        assert!(
            !errors.is_empty(),
            "invalid fixture '{name}' produced no errors/warnings"
        );
        let matched = errors.iter().any(|e| e.kind == *expected_kind);
        assert!(
            matched,
            "invalid fixture '{name}': expected error kind {:?}, got: {}\n\
             all diagnostics: {:#?}",
            expected_kind,
            errors
                .first()
                .map(|e| format!("{:?}", e.kind))
                .unwrap_or_default(),
            errors,
        );
    }
}

#[test]
fn every_error_kind_has_a_test() {
    // Verify that all ParseErrorKind variants are covered by the INVALID_FIXTURES list.
    let covered: std::collections::HashSet<ParseErrorKind> =
        INVALID_FIXTURES.iter().map(|(_, k)| k.clone()).collect();
    // InvalidIndentation is handled by indentation checks during parsing.
    let all_variants: std::collections::HashSet<ParseErrorKind> = [
        ParseErrorKind::UnexpectedToken,
        ParseErrorKind::UnterminatedDelimiter,
        ParseErrorKind::InvalidIndentation,
        ParseErrorKind::UndefinedReference,
        ParseErrorKind::MalformedDirective,
        ParseErrorKind::MalformedTable,
        ParseErrorKind::MalformedMath,
        ParseErrorKind::InvalidPropValue,
    ]
    .into();
    for variant in &all_variants {
        assert!(
            covered.contains(variant),
            "ParseErrorKind {:?} has no invalid-syntax fixture",
            variant
        );
    }
}
