// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Samin Yeasar
//
//! Helper test to regenerate JSON fixture files from .vl source.
//!
//! Run: cargo test --test gen_fixtures -- --nocapture

use std::fs;
use std::path::Path;

#[test]
fn generate_fixtures() {
    let fixture_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("tests")
        .join("fixtures");

    // Valid fixture pairs (should parse successfully)
    let valid_fixtures: &[&str] = &[
        "nested_inline",
        "escaped_chars",
        "ref_defs",
        "footnote_defs",
        "if_block",
        "image_refs",
        "link_refs",
        "admonition",
        "classic_table",
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

    // Invalid fixture pairs (should produce a parse error or validation warning)
    let invalid_fixtures: &[&str] = &[
        "err_unclosed_bold",
        "err_unclosed_italic",
        "err_unclosed_code",
        "err_unclosed_math",
        "err_unclosed_link",
        "err_unclosed_image",
        "err_unclosed_directive",
        "err_malformed_table_pipe",
        "err_malformed_table_grid",
        "err_malformed_directive",
        "err_undefined_var",
        "err_bad_indent",
        "err_unclosed_fence",
        "err_bad_math_block",
        "err_bad_prop_value",
    ];

    for name in valid_fixtures {
        let vl_path = fixture_dir.join(format!("{name}.vl"));
        let json_path = fixture_dir.join(format!("{name}.json"));

        let source = fs::read_to_string(&vl_path)
            .unwrap_or_else(|e| panic!("cannot read {}: {}", vl_path.display(), e));

        match vell_core::parse_document(&source) {
            Ok(doc) => {
                let json = serde_json::to_string_pretty(&doc)
                    .unwrap_or_else(|e| panic!("serialization error for {name}: {e}"));
                fs::write(&json_path, json + "\n")
                    .unwrap_or_else(|e| panic!("cannot write {}: {}", json_path.display(), e));
                eprintln!("OK   {} (valid)", name);
            }
            Err(err) => {
                panic!("PARSE FAILED for {name} (expected success): {:?}", err);
            }
        }
    }

    for name in invalid_fixtures {
        let vl_path = fixture_dir.join(format!("{name}.vl"));
        let json_path = fixture_dir.join(format!("{name}.json"));

        let source = fs::read_to_string(&vl_path)
            .unwrap_or_else(|e| panic!("cannot read {}: {}", vl_path.display(), e));

        // First try parse_document (fatal errors)
        match vell_core::parse_document(&source) {
            Err(err) => {
                let error_obj = serde_json::json!({
                    "error_kind": format!("{:?}", err.kind),
                    "message_contains": err.message,
                });
                let json = serde_json::to_string_pretty(&error_obj)
                    .unwrap_or_else(|e| panic!("serialization error for {name}: {e}"));
                fs::write(&json_path, json + "\n")
                    .unwrap_or_else(|e| panic!("cannot write {}: {}", json_path.display(), e));
                eprintln!("OK   {} -> parse_error: {:?}", name, err.kind);
            }
            Ok(_doc) => {
                // parse_document succeeded; check validate for warnings
                let warnings = vell_core::validate(&source);
                if let Some(warn) = warnings.first() {
                    let error_obj = serde_json::json!({
                        "error_kind": format!("{:?}", warn.kind),
                        "message_contains": warn.message,
                    });
                    let json = serde_json::to_string_pretty(&error_obj)
                        .unwrap_or_else(|e| panic!("serialization error for {name}: {e}"));
                    fs::write(&json_path, json + "\n")
                        .unwrap_or_else(|e| panic!("cannot write {}: {}", json_path.display(), e));
                    eprintln!("OK   {} -> warning: {:?}", name, warn.kind);
                } else {
                    panic!(
                        "UNEXPECTED SUCCESS for {name} (expected error/warning): {}",
                        serde_json::to_string_pretty(&_doc).unwrap()
                    );
                }
            }
        }
    }

    eprintln!(
        "\nDone. Generated {} valid + {} invalid fixture JSON files.",
        valid_fixtures.len(),
        invalid_fixtures.len()
    );
}
