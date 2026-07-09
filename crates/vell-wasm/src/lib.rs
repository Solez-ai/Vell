// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Samin Yeasar

//! WebAssembly bindings for the Vell parser.

use vell_core::{parse_document, validate as validate_source};
use wasm_bindgen::prelude::*;

/// Parses source and returns a JavaScript AST value or parse error value.
#[wasm_bindgen]
pub fn parse(source: &str) -> Result<JsValue, JsValue> {
    match parse_document(source) {
        Ok(document) => Ok(JsValue::from_str(
            &serde_json::to_string(&document).unwrap_or_else(|_| "{}".to_string()),
        )),
        Err(error) => Err(JsValue::from_str(
            &serde_json::to_string(&error).unwrap_or_else(|_| "{}".to_string()),
        )),
    }
}

/// Parses source and returns JSON containing either a document or errors.
#[wasm_bindgen]
pub fn parse_to_json(source: &str) -> String {
    match parse_document(source) {
        Ok(document) => {
            serde_json::json!({ "document": document, "errors": [], "warnings": [] }).to_string()
        }
        Err(error) => {
            serde_json::json!({ "document": null, "errors": [error], "warnings": [] }).to_string()
        }
    }
}

/// Returns the Vell parser version string.
#[wasm_bindgen]
pub fn get_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// Returns validation diagnostics as a JSON string value.
#[wasm_bindgen]
pub fn validate(source: &str) -> JsValue {
    JsValue::from_str(
        &serde_json::to_string(&validate_source(source)).unwrap_or_else(|_| "[]".to_string()),
    )
}
