# Vell

<p align="center">
  <img src="logo-no-bg.png" alt="Vell logo" width="160">
</p>

Vell is a document markup language that combines Markdown's readability with LaTeX's expressive power. It produces a versioned, deterministic abstract syntax tree (AST), and includes a Rust reference parser, WebAssembly bindings, HTML and PDF renderers, a canonical formatter, an LSP server, and VS Code integration.

Created and developed by **Samin Yeasar**.

---

## Why Vell?

| Feature | Markdown | LaTeX | Vell |
|---------|----------|-------|------|
| Readable source | Yes | No | Yes |
| Deterministic output | No | Yes | Yes |
| Extensible directives | No | Yes (packages) | Yes (native) |
| Variables and reactivity | No | No | Yes |
| Math support | No | Yes | Yes (MathML) |
| Tables | Basic | Verbose | Yes (pipe + grid) |
| Formatter | Inconsistent | Limited | Yes (idempotent) |
| LSP support | Basic | Complex | Yes |
| PDF output | Via converters | Native | Yes (direct AST to PDF) |

## Quick Start

### Installation

```bash
cargo install --path crates/vell-cli
npm install @solez-ai/vell @solez-ai/vell-renderer-html
```

The npm package is currently a typed wrapper. Build `crates/vell-wasm` with `wasm-pack` and register the generated module with `setWasmModule()` before using the JavaScript parser API.

### Usage

```bash
vell parse document.vl        # Print AST as JSON
vell fmt document.vl           # Format to canonical style
vell render html document.vl   # Render to HTML5
vell validate document.vl      # Check for errors
```

### Example

```vell
@[Meta](title="Getting Started" author="You")

= Hello, Vell!

@var name = "Vell"
@var items = ["simple", "expressive", "extensible"]

Welcome to @{name}! This language is:

@for feature in @{items} {
  - @{feature}
}

== Math Support

Inline: $E = mc^2$

Block:
$$
\int_0^1 x^2 \, dx = \frac{1}{3}
$$

== Tables and Code

| Feature | Description |
|---------|-------------|
| Parser  | PEG grammar, linear-time O(n) |
| CLI     | Parse, format, render, validate |
| LSP     | Diagnostics, completions, hover, refactor |

```rust
fn main() {
    println!("Hello, Vell!");
}
```
```

## Architecture

```
Vell Source (.vl)
        |
        v
vell-core (Rust parser)
  Lexer -> Parser -> AST
        |
    -----+-----
    |    |    |
    v    v    v
vell-wasm  vell-fmt  vell-lsp
(WASM)   (fmt)    (LSP server)
    |
    v
TypeScript Packages
  vell-js         renderer-html
  (WASM wrapper)  (HTML5 + MathML)
                   renderer-pdf
                   (direct PDF)
  vell-vscode (VS Code extension)
```

## Features

### Language

- **Readable syntax** -- Headings (`=`), bold (`*`), italic (`/`), underline (`_`), strikethrough (`~`), code (`` ` ``), links, images, tables
- **PEG grammar** -- Formal grammar guarantees deterministic, linear-time O(n) parsing
- **Versioned AST** -- Same source always produces the same AST, versioned for forward compatibility
- **Math support** -- LaTeX math (`$...$`, `$$...$$`) rendered as MathML in HTML
- **Tables** -- Pipe tables (Markdown-style) and grid tables (reStructuredText-style) with alignment
- **Variables and reactivity** -- `@var`, `@{name}`, `@for`, `@if` for dynamic documents
- **Directives** -- `@[Figure]`, `@[Code]`, `@[Meta]`, `@[Theme]`, and 30+ built-in directives
- **Extensions** -- Namespaced `@[org/Name](props)` for custom plugins

### Tooling

- **CLI** -- `vell parse`, `vell fmt`, `vell render html`, `vell validate` with stdin/stdout support
- **Formatter** -- Idempotent canonical formatting
- **LSP server** -- Diagnostics, hover, completions, go-to-definition, references, rename, folding, semantic tokens, code actions, document symbols, workspace symbols, code lens, signature help, document links, color provider, inlay hints, linked editing ranges, call hierarchy, format-on-type
- **VS Code extension** -- Syntax highlighting with LSP integration

### Packages

| Package | Language | Description |
|---------|----------|-------------|
| vell-core | Rust | Reference parser, AST, formatter, validator |
| vell-wasm | Rust/WASM | WebAssembly bindings for browser and Node.js |
| `@solez-ai/vell` | TypeScript | Official WASM wrapper with typed parse, validate, and format APIs |
| vell-renderer-html | TypeScript | HTML5 renderer with MathML, URL sanitization, footnote support |
| vell-renderer-pdf | TypeScript | Direct AST-to-PDF renderer using pdfkit |
| `@solez-ai/vell-vscode` | TypeScript | VS Code extension with syntax highlighting and LSP client |

## Documentation

| Document | Description |
|----------|-------------|
| [Language Reference](docs/language-reference.md) | Complete language syntax reference |
| [AST Reference](docs/ast-reference.md) | Full AST node reference with JSON schemas |
| [Renderer Guide](docs/renderer-guide.md) | How to build renderers for the Vell AST |
| [Extension Authoring](docs/extension-authoring.md) | How to create and distribute extensions |
| [Specification](spec/grammar.peg) | Formal PEG grammar |
| [AST Schema](spec/ast-schema.json) | JSON Schema for AST validation |

### Documentation Overview

The **Language Reference** covers every aspect of Vell syntax -- block elements (headings, paragraphs, blockquotes, admonitions, code blocks, math blocks, lists, tables, horizontal rules, definition lists, reference definitions, footnote definitions), inline elements (bold, italic, underline, strikethrough, code, superscript, subscript, links, images, math, citations, footnotes), variables and reactivity (variable declarations, interpolation, for loops, if blocks), and all built-in directives with their properties and usage examples.

The **AST Reference** documents the complete abstract syntax tree structure with JSON schemas for every node type. It covers the document root, all 16 block node types, 19 inline node types, and supporting types (list items, table cells, definition items, property values, alignment). Each node includes its JSON representation, field descriptions, and schema validation rules.

The **Renderer Guide** is for anyone building a renderer that consumes the Vell AST. It covers the renderer contract, two-pass rendering for cross-references, node dispatch strategy, block and inline node rendering for every node type, safety requirements (HTML escaping, URL sanitization, XSS prevention), footnote handling, extension fallback strategies, and testing checklists.

The **Extension Authoring** guide explains how to create custom Vell directives using the extension system. It covers naming conventions, property schemas, block bodies, AST representation, adding extension support to renderers (HTML, PDF, custom), the plugin architecture with the extension registry API, LSP integration for extensions, best practices, and distribution guidance for npm packages.

## Development

### Prerequisites

- Rust stable (latest edition 2021)
- Node.js 20 or later
- wasm-pack (for WASM builds)

### Setup

```bash
git clone https://github.com/samin/vell.git
cd vell

cargo build --workspace
cargo test --workspace

npm install
npm run typecheck
npm test
npm run build
```

### Project Status

| Component | Status | Description |
|-----------|--------|-------------|
| Parser | Beta | Deterministic fixtures pass; fuzzing and compatibility certification remain |
| Fixture tests | Strong | Parser and formatter fixtures cover the current language surface |
| HTML renderer | Beta | Broad HTML5 and interactive support; behavioral browser tests remain |
| CLI | Beta | Parse, fmt, validate, watch, and publishing commands work |
| Formatter | Strong | Idempotency is verified across fixtures and specification examples |
| LSP server | Beta | Broad feature set; multi-editor and long-running stress tests remain |
| PDF renderer | Beta | Direct AST-to-PDF package with documented math and layout limitations |
| WASM bindings | Incomplete distribution | Rust bindings work, but the npm package does not yet bundle generated WASM |
| VS Code extension | Preview | Syntax highlighting and LSP client work from a source-built VSIX |

See `docsfr/docs/reliability.mdx` for production-readiness guarantees, limitations, and the prioritized path to `1.0.0`.

## License

Vell is copyright (C) 2026 **Samin Yeasar** and is licensed under the **GNU Affero General Public License v3.0 or later**. See [LICENSE](LICENSE).

---

*Built by Samin Yeasar -- Solo developer project*
