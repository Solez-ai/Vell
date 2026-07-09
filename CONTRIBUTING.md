# Contributing to Vell

## Code of conduct reference

Vell is created, owned, and maintained by Samin Yeasar as a solo developer project. People who submit issues, suggestions, or patches are expected to communicate respectfully and focus discussion on technical outcomes.

## Development environment setup

Use Rust stable with edition 2021 support, wasm-pack for WebAssembly builds, Node.js 20 or later, and pnpm 9 or later.

## Repository structure walkthrough

`crates/vell-core` contains the AST, lexer, and parser. `crates/vell-fmt` contains canonical formatting. `crates/vell-wasm` exposes parser APIs to JavaScript. `crates/vell-lsp` implements editor protocol support. `packages` contains TypeScript bindings, renderers, and VS Code integration. `spec` contains grammar, schema, and examples. `docs` contains user and implementer documentation.

## How to run tests

Run `cargo test --workspace` for Rust crates and `pnpm test` for TypeScript packages.

## How to run the LSP server locally

Build with `cargo build -p vell-lsp`, then configure an editor client to launch `target/debug/vell-lsp`.

## How to build the WASM module

Run `wasm-pack build crates/vell-wasm --target web` or `wasm-pack build crates/vell-wasm --target nodejs`.

## Coding standards

Rust code uses edition 2021, must be formatted with rustfmt, pass clippy without warnings, and document public items. TypeScript code uses strict mode and typed exports.

## Commit message format

Use Conventional Commits such as `feat: add table renderer` or `fix: report unclosed math delimiter`.

## Pull request process

Open focused pull requests with tests or fixtures for behavior changes. Include validation commands and results. Accepted changes are incorporated under the project's existing copyright notice, copyright (C) 2026 Samin Yeasar, and the AGPLv3-or-later license.

## How to add a new built-in directive

Update the parser built-in list, AST handling if needed, formatter output, renderer behavior, LSP hover text, grammar examples, and documentation.

## How to add a new renderer

Consume the AST schema, escape untrusted strings, preserve unknown nodes with fallback output, and add tests using `spec/examples`.

## How to add test fixtures

Place `.vl` input and expected `.json` output under `crates/vell-core/src/tests/fixtures`. Keep fixtures small and named after the behavior under test.

## Issue reporting guidelines

Include the Vell source, expected behavior, actual diagnostics or output, and tool versions.
