# AI Agent Orientation

## Project summary

Vell is a human-readable markup language that produces a stable versioned AST. Tooling operates on the AST rather than source syntax. The repository contains the reference parser, formatter, WASM bindings, LSP server, renderers, editor extension, examples, and documentation. Vell is created, owned, and developed by Samin Yeasar as a solo developer project.

Vell syntax sits between simple prose markup and structured document systems. It includes headings, lists, tables, math, directives, variables, and extensions. Unknown extensions are preserved so documents can be processed safely by older tools.

Security and determinism are core constraints. Parsers must not panic, renderers must sanitize untrusted content, and formatting must be idempotent.

## Repository map

`crates/vell-core` defines AST, lexer, parser, errors, and tests. `crates/vell-fmt` formats parsed documents. `crates/vell-wasm` exposes parser APIs to WebAssembly. `crates/vell-lsp` provides editor features. `packages/vell-js` wraps WASM for TypeScript. `packages/vell-renderer-html` renders HTML. `packages/vell-renderer-pdf` defines PDF rendering integration. `packages/vell-vscode` provides VS Code integration. `spec` stores grammar, schema, and examples. `docs` stores language and implementation guides.

## Key concepts

A PEG grammar is an ordered deterministic grammar. The AST is the versioned tree returned by the parser. A span is a byte range in the original source. A directive is `@[Name](props)`. An extension is an unknown or namespaced directive preserved by the parser. A renderer converts AST nodes into an output format. The LSP provides editor diagnostics and actions. WASM bindings expose the Rust parser to JavaScript. A reactive variable is declared with `@var` and referenced with `@{name}`.

## Architecture invariants

The parser never panics. Increment the AST schema version for breaking node changes. The formatter is idempotent. Renderers must not fail on unknown nodes. The WASM API is the stable public API. All user supplied strings are untrusted before rendering.

## Crate and package responsibilities

`vell-core` must not depend on rendering or editor crates. `vell-fmt` depends on core and serializes ASTs. `vell-wasm` exposes stable JavaScript entry points. `vell-lsp` owns editor protocol behavior. TypeScript renderer packages must not reimplement parsing.

## How to add a feature

Update grammar and AST schema, implement parser support, add formatter output, update renderers, add tests and fixtures, update docs, and run workspace validation.

## Common pitfalls

Do not assume the lexer emits one token per character. Do not forget `Extension` nodes in renderers. Do not break formatter idempotency. Do not change node structure without considering the AST version.

## Testing philosophy

Tests should cover structure and diagnostics, not incidental formatting. Fixtures document stable language behavior. Renderer tests should use examples and unknown node fallbacks.

## Build commands reference

Use `cargo check --workspace`, `cargo test --workspace`, `cargo fmt --all`, `cargo clippy --all -- -D warnings`, `pnpm install`, and `pnpm build`.

## Roadmap priority

Follow `ROADMAP.md` for major implementation work. Finish parser correctness, real fixtures, HTML rendering, CLI support, formatter idempotency, and useful LSP behavior before implementing PDF output. The PDF renderer must be a direct custom AST-to-PDF renderer, not an HTML-to-PDF pipeline.
