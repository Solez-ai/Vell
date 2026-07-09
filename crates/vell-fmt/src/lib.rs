// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Samin Yeasar

//! Canonical formatter for Vell documents.

use vell_core::{
    format_inline_nodes, format_props, parse_document, Document, ListItem, Node, ParseError,
    TableCell,
};

/// Formats a parsed document into deterministic Vell source.
pub fn format(doc: &Document) -> String {
    let mut output = doc
        .children
        .iter()
        .map(format_node)
        .collect::<Vec<_>>()
        .join("\n\n");
    output = output
        .lines()
        .map(str::trim_end)
        .collect::<Vec<_>>()
        .join("\n");
    output.push('\n');
    output
}

/// Parses and formats source text.
pub fn format_source(source: &str) -> Result<String, ParseError> {
    parse_document(source).map(|doc| format(&doc))
}

fn format_node(node: &Node) -> String {
    match node {
        Node::Heading {
            level, children, ..
        } => format!(
            "{} {}",
            "=".repeat(usize::from(*level)),
            format_inline_nodes(children)
        ),
        Node::Paragraph { children, .. } => wrap_prose(&format_inline_nodes(children), 100),
        Node::Blockquote {
            children,
            admonition_type,
            ..
        } => {
            let mut lines = Vec::new();
            if let Some(kind) = admonition_type {
                lines.push(format!("> [!{kind}]"));
            }
            for child in children {
                for line in format_node(child).lines() {
                    lines.push(format!("> {line}"));
                }
            }
            lines.join("\n")
        }
        Node::CodeBlock { lang, source, .. } => format!(
            "```{}\n{}\n```",
            lang.clone().unwrap_or_default().to_ascii_lowercase(),
            source.trim_end()
        ),
        Node::MathBlock { source, .. } => format!("$$\n{}\n$$", source.trim()),
        Node::List {
            ordered,
            start,
            items,
            ..
        } => items
            .iter()
            .enumerate()
            .map(|(index, item)| format_list_item(*ordered, start.unwrap_or(1), index, item))
            .collect::<Vec<_>>()
            .join("\n"),
        Node::Table { headers, rows, .. } => format_table(headers, rows),
        Node::HorizontalRule { .. } => "---".to_string(),
        Node::DefinitionList { items, .. } => items
            .iter()
            .map(|item| {
                let def = item
                    .definition
                    .iter()
                    .map(format_node)
                    .collect::<Vec<_>>()
                    .join("\n");
                if def.is_empty() {
                    format!(":: {}", format_inline_nodes(&item.term))
                } else {
                    format!(
                        ":: {}\n   {}",
                        format_inline_nodes(&item.term),
                        def
                    )
                }
            })
            .collect::<Vec<_>>()
            .join("\n"),
        Node::ReferenceDefinition { id, url, title, .. } => title.as_ref().map_or_else(
            || format!("[{id}]: {url}"),
            |title| format!("[{id}]: {url} \"{title}\""),
        ),
        Node::FootnoteDefinition {
            marker, children, ..
        } => format!(
            "[^{marker}]: {}",
            children
                .iter()
                .map(format_node)
                .collect::<Vec<_>>()
                .join(" ")
        ),
        Node::VarDeclaration { name, value, .. } => format!("@var {name} = {value}"),
        Node::ForLoop {
            variable,
            iterable,
            children,
            ..
        } => format!(
            "@for {variable} in @{{{iterable}}} {{\n{}\n}}",
            indent(
                &children
                    .iter()
                    .map(format_node)
                    .collect::<Vec<_>>()
                    .join("\n\n")
            )
        ),
        Node::IfBlock {
            condition,
            consequent,
            alternate,
            ..
        } => {
            let mut out = format!(
                "@if {condition} {{\n{}\n}}",
                indent(
                    &consequent
                        .iter()
                        .map(format_node)
                        .collect::<Vec<_>>()
                        .join("\n\n")
                )
            );
            if let Some(alt) = alternate {
                out.push_str(&format!(
                    " else {{\n{}\n}}",
                    indent(&alt.iter().map(format_node).collect::<Vec<_>>().join("\n\n"))
                ));
            }
            out
        }
        Node::Directive {
            name,
            props,
            children,
            ..
        }
        | Node::Extension {
            name,
            props,
            children,
            ..
        } => {
            if children.is_empty() {
                format!("@[{name}]({})", format_props(props))
            } else {
                format!(
                    "@[{name}]({}) {{\n{}\n}}",
                    format_props(props),
                    indent(
                        &children
                            .iter()
                            .map(format_node)
                            .collect::<Vec<_>>()
                            .join("\n\n")
                    )
                )
            }
        }
    }
}

fn format_list_item(ordered: bool, start: u32, index: usize, item: &ListItem) -> String {
    let body = item
        .children
        .iter()
        .map(format_node)
        .collect::<Vec<_>>()
        .join(" ");
    if ordered {
        format!("{}. {body}", start + u32::try_from(index).unwrap_or(0))
    } else {
        format!("- {body}")
    }
}

fn format_table(headers: &[TableCell], rows: &[Vec<TableCell>]) -> String {
    let all_rows = std::iter::once(headers)
        .chain(rows.iter().map(Vec::as_slice))
        .collect::<Vec<_>>();
    let cols = all_rows.iter().map(|row| row.len()).max().unwrap_or(0);
    if cols == 0 {
        return String::new();
    }
    let mut widths = vec![3usize; cols];
    for row in &all_rows {
        for (i, cell) in row.iter().enumerate() {
            widths[i] = widths[i].max(format_inline_nodes(&cell.children).len());
        }
    }
    let row_fmt = |row: &[TableCell]| -> String {
        let mut out = String::from("|");
        for (i, width) in widths.iter().enumerate().take(cols) {
            let text = row
                .get(i)
                .map(|c| format_inline_nodes(&c.children))
                .unwrap_or_default();
            out.push_str(&format!(" {:width$} |", text, width = *width));
        }
        out
    };
    let sep = format!(
        "|{}|",
        widths
            .iter()
            .map(|w| "-".repeat(*w + 2))
            .collect::<Vec<_>>()
            .join("|")
    );
    let mut lines = vec![row_fmt(headers), sep];
    for row in rows {
        lines.push(row_fmt(row));
    }
    lines.join("\n")
}

fn indent(text: &str) -> String {
    text.lines()
        .map(|line| format!("  {line}"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Word-wrap text, keeping inline math expressions (`$...$`) atomic so that
/// they are never split across lines and never produce lines starting with `=`
/// or other block-introducing characters.
fn wrap_prose(text: &str, max: usize) -> String {
    // Group words: keep $...$ math expressions as single units
    let raw_words: Vec<&str> = text.split_whitespace().collect();
    let mut words: Vec<String> = Vec::new();
    let mut i = 0;
    while i < raw_words.len() {
        let word = raw_words[i];
        // Check if this word starts $ but doesn't contain a closing $ (except at position 0)
        let has_close_dollar = word.get(1..).map_or(false, |s| s.contains('$'));
        if word.starts_with('$') && !has_close_dollar {
            // Start of a multi-word math expression — collect until closing $
            let mut math_parts = vec![word];
            i += 1;
            while i < raw_words.len() {
                let part = raw_words[i];
                math_parts.push(part);
                i += 1;
                // A word has the closing $ if it contains $ anywhere beyond position 0
                if part.get(1..).map_or(false, |s| s.contains('$')) {
                    break;
                }
            }
            words.push(math_parts.join(" "));
        } else {
            words.push(word.to_string());
            i += 1;
        }
    }

    let mut out = String::new();
    let mut line_len = 0usize;
    for word in &words {
        if line_len > 0 && line_len + 1 + word.len() > max {
            out.push('\n');
            out.push_str(word);
            line_len = word.len();
        } else {
            if line_len > 0 {
                out.push(' ');
                line_len += 1;
            }
            out.push_str(word);
            line_len += word.len();
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Idempotency and snapshot tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use vell_core::parse_document;

    /// All valid fixture names for idempotency testing.
    const FIXTURES: &[&str] = &[
        "headings", "inline", "lists", "code_blocks", "math",
        "pipe_table", "grid_table_merged", "directives", "variables",
        "for_loop", "full_document", "nested_inline", "escaped_chars",
        "ref_defs", "footnote_defs", "if_block", "image_refs",
        "link_refs", "admonition", "def_list", "hrule_markers",
        "pipe_table_align", "list_nested", "code_blocks_no_lang",
        "math_inline", "inline_components", "empty_directive", "block_directive",
    ];

    /// All spec example names for idempotency testing.
    const SPEC_EXAMPLES: &[&str] = &[
        "01-basic", "02-math", "03-tables", "04-interactive", "05-extensions", "06-full-document",
        "07-math-advanced", "08-theorems-equations",
    ];

    /// Load a fixture source from the vell-core test fixtures directory.
    fn load_fixture(name: &str) -> String {
        let path = format!(
            "{}/../../crates/vell-core/src/tests/fixtures/{}.vl",
            env!("CARGO_MANIFEST_DIR"),
            name
        );
        std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("cannot read fixture {}.vl: {}", name, e))
    }

    /// Load a spec example source from the project spec directory.
    fn load_spec(name: &str) -> String {
        let path = format!(
            "{}/../../spec/examples/{}.vl",
            env!("CARGO_MANIFEST_DIR"),
            name
        );
        std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("cannot read spec {}.vl: {}", name, e))
    }

    /// Test that format is idempotent: format(parse(format(parse(source)))) == format(parse(source))
    fn assert_idempotent(label: &str, source: &str) {
        let doc1 = parse_document(source)
            .unwrap_or_else(|e| panic!("{label}: initial parse failed: {e:?}"));
        let fmt1 = format(&doc1);

        let doc2 = parse_document(&fmt1)
            .unwrap_or_else(|e| panic!("{label}: re-parse of formatted output failed: {e:?}"));
        let fmt2 = format(&doc2);

        assert_eq!(
            fmt1, fmt2,
            "{label}: formatter is NOT idempotent\n\
             fmt1 (first format):\n{fmt1}\n\
             fmt2 (second format):\n{fmt2}",
        );
    }

    #[test]
    fn idempotent_fixtures() {
        for name in FIXTURES {
            let source = load_fixture(name);
            assert_idempotent(&format!("fixture {name}"), &source);
        }
    }

    #[test]
    fn idempotent_spec_examples() {
        for name in SPEC_EXAMPLES {
            let source = load_spec(name);
            assert_idempotent(&format!("spec {name}"), &source);
        }
    }

    #[test]
    fn idempotent_empty_document() {
        assert_idempotent("empty", "");
    }

    #[test]
    fn idempotent_simple_paragraph() {
        assert_idempotent("simple para", "Hello world.\n");
    }

    #[test]
    fn idempotent_headings() {
        assert_idempotent("heading l1", "= Title\n");
        assert_idempotent("heading l2", "== Section\n");
        assert_idempotent("heading l3", "=== Subsection\n");
        assert_idempotent("multi heading", "= A\n\n== B\n");
    }

    #[test]
    fn idempotent_inline_markup() {
        assert_idempotent("bold", "*bold*\n");
        assert_idempotent("italic", "/italic/\n");
        assert_idempotent("code", "`code`\n");
        assert_idempotent("link", "[text](url)\n");
        assert_idempotent("mixed", "*bold* and /italic/\n");
    }

    #[test]
    fn idempotent_lists() {
        assert_idempotent("unordered list", "- Item 1\n- Item 2\n");
        assert_idempotent("ordered list", "1. First\n2. Second\n");
    }

    #[test]
    fn idempotent_tables() {
        assert_idempotent("pipe table", "| A | B |\n|---|---|\n| 1 | 2 |\n");
    }

    #[test]
    fn idempotent_code_blocks() {
        assert_idempotent("code block", "```rust\nfn main() {}\n```\n");
    }

    #[test]
    fn idempotent_math() {
        assert_idempotent("math block", "$$\nx^2\n$$\n");
        assert_idempotent("math inline", "$a^2$\n");
    }

    #[test]
    fn idempotent_directives() {
        assert_idempotent("directive", "@[Figure](src=\"a.png\")\n");
    }

    #[test]
    fn idempotent_horizontal_rules() {
        assert_idempotent("hrule", "---\n");
    }

    #[test]
    fn idempotent_blockquotes() {
        assert_idempotent("blockquote", "> Quoted.\n");
    }

    #[test]
    fn idempotent_admonition() {
        assert_idempotent("admonition", "> [!NOTE]\n> A note.\n");
    }

    #[test]
    fn idempotent_variables() {
        assert_idempotent("var decl", "@var x = 1\n");
        assert_idempotent("var ref", "@{x}\n");
    }

    #[test]
    fn idempotent_for_loop() {
        assert_idempotent("for loop", "@var items = [1, 2]\n@for item in @{items} {\n  Body.\n}\n");
    }

    #[test]
    fn idempotent_if_block() {
        assert_idempotent("if", "@if true {\n  Yes.\n}\n");
        assert_idempotent("if else", "@if true {\n  Yes.\n} else {\n  No.\n}\n");
    }

    #[test]
    fn idempotent_reference_definitions() {
        assert_idempotent("ref def", "[ref]: https://example.com\n");
    }

    #[test]
    fn idempotent_footnote_definitions() {
        assert_idempotent("footnote", "[^one]: Note.\n");
    }

    #[test]
    fn idempotent_definition_lists() {
        assert_idempotent("def list", ":: Term\n   Definition\n");
    }

    #[test]
    fn idempotent_extension() {
        assert_idempotent("extension", "@[myorg/Widget](data=1)\n");
    }

    #[test]
    fn format_strips_trailing_whitespace() {
        let source = "= Title   \n\nBody.  \n";
        let doc = parse_document(source).unwrap();
        let formatted = format(&doc);
        for line in formatted.lines() {
            assert_eq!(line, line.trim_end(), "line has trailing whitespace: {line:?}");
        }
    }

    #[test]
    fn format_ends_with_newline() {
        let source = "= Title\n";
        let doc = parse_document(source).unwrap();
        let formatted = format(&doc);
        assert!(formatted.ends_with('\n'), "formatted output must end with newline");
    }

    #[test]
    fn format_source_from_original() {
        let source = "= Title\n\nBody.\n";
        let result = format_source(source);
        assert!(result.is_ok(), "format_source failed: {:?}", result.err());
        let formatted = result.unwrap();
        assert!(formatted.contains("= Title"));
        assert!(formatted.contains("Body."));
    }

    #[test]
    fn format_heading_canonical() {
        let cases = ["= A\n", "== B\n", "=== C\n"];
        for input in &cases {
            let doc = parse_document(input).unwrap();
            let formatted = format(&doc);
            assert!(formatted.starts_with('='), "heading should start with =: {formatted:?}");
            assert!(formatted.ends_with('\n'), "heading should end with newline: {formatted:?}");
        }
    }

    #[test]
    fn format_preserves_document_structure() {
        let source = "= Title\n\nFirst paragraph.\n\n- Item\n\n```\ncode\n```\n";
        let doc = parse_document(source).unwrap();
        let formatted = format(&doc);
        // Should still have all blocks
        assert!(formatted.contains("= Title"), "missing heading");
        assert!(formatted.contains("First paragraph"), "missing paragraph");
        assert!(formatted.contains("- Item"), "missing list item");
        assert!(formatted.contains("code"), "missing code");
    }

    #[test]
    fn format_does_not_panic_on_any_fixture() {
        for name in FIXTURES {
            let source = load_fixture(name);
            let doc = parse_document(&source)
                .unwrap_or_else(|e| panic!("fixture {name} parse failed: {e:?}"));
            let _formatted = format(&doc);
        }
    }
}
