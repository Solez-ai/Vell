<p align="center"><img src="logo-no-bg.png" alt="Vell logo" width="160"></p>

<h1 align="center">Vell</h1>

<p align="center">
  <strong>A document markup language — Markdown's readability, LaTeX's power, plus reactivity</strong>
</p>

<p align="center">
  <em>Created and developed by <strong>Samin Yeasar</strong></em>
</p>

---

**Vell** is a document and markup language designed to bridge the gap between Markdown's simplicity and LaTeX's expressive power. It features a **versioned, deterministic abstract syntax tree (AST)**, a Rust reference parser, WebAssembly bindings, HTML and PDF renderers, a canonical formatter, an LSP server, and VS Code integration.

## Why Vell?

| Feature | Markdown | LaTeX | Vell |
|---------|----------|-------|------|
| Readable source | ✅ | ❌ | ✅ |
| Deterministic output | ❌ | ✅ | ✅ |
| Extensible directives | ❌ | ✅ (packages) | ✅ (native) |
| Variables & reactivity | ❌ | ❌ | ✅ |
| Math support | ❌ | ✅ | ✅ (MathML) |
| Tables | Basic | Verbose | ✅ (pipe + grid) |
| Formatter | Inconsistent | Limited | ✅ (idempotent) |
| LSP support | Basic | Complex | ✅ |
| PDF output | Via converters | Native | ✅ (direct AST→PDF) |

## Quick Start

### Installation

```bash
# Rust tools
cargo install vell-cli

# Node.js packages
npm install @vell-lang/vell-js @vell-lang/renderer-html

# VS Code extension
# Install "Vell" from the VS Code marketplace
```

### Usage

```bash
# Parse a .vl file
vell parse document.vl

# Format a .vl file
vell fmt document.vl

# Render to HTML
vell render html document.vl > output.html

# Validate
vell validate document.vl
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

== Tables & Code

| Feature | Status |
|---------|--------|
| Parser  | ✅     |
| CLI     | ✅     |
| LSP     | ✅     |

```rust
fn main() {
    println!("Hello, Vell!");
}
```

> [!TIP]
> Vell is designed for technical writing, academic papers,
> documentation, and presentations.
```

## Project Architecture

```
┌─────────────────────────────────────────────────────────┐
│                    Vell Source (.vl)                     │
└─────────────────────┬───────────────────────────────────┘
                      │
                      ▼
┌─────────────────────────────────────────────────────────┐
│              vell-core (Rust parser)                     │
│  ┌─────────┐  ┌──────────┐  ┌──────────┐               │
│  │  Lexer  │─▶│  Parser  │─▶│   AST    │               │
│  └─────────┘  └──────────┘  └──────────┘               │
└─────────────────────┬───────────────────────────────────┘
                      │
          ┌───────────┼───────────┐
          │           │           │
          ▼           ▼           ▼
┌──────────────┐ ┌────────┐ ┌──────────┐
│  vell-wasm   │ │vell-fmt│ │ vell-lsp │
│  (WASM API)  │ │(canon. │ │ (LSP     │
│              │ │formatter│ │ server)  │
└──────┬───────┘ └────────┘ └──────────┘
       │
       ▼
┌──────────────────────────────────────┐
│         TypeScript Packages          │
│  ┌─────────┐ ┌──────────────┐        │
│  │ vell-js │ │ renderer-html│        │
│  │ (WASM   │ │ (HTML5)      │        │
│  │ wrapper)│ │              │        │
│  └─────────┘ │ renderer-pdf │        │
│              │ (direct PDF) │        │
│              └──────────────┘        │
│  ┌────────────────────────────┐      │
│  │ vell-vscode (VS Code       │      │
│  │ extension + syntax hl)     │      │
│  └────────────────────────────┘      │
└──────────────────────────────────────┘
```

## Key Features

### ✨ Language

- **Readable syntax** — Headings (`=`), bold (`*`), italic (`/`), underline (`_`), strikethrough (`~`), code (`` ` ``), links, images, tables, and more
- **PEG grammar** — Formal grammar guarantees deterministic, linear-time O(n) parsing
- **Versioned AST** — The same source always produces the same AST, versioned for forward compatibility
- **Math support** — LaTeX math (`$...$`, `$$...$$`) rendered as MathML in HTML
- **Tables** — Both pipe tables (Markdown-style) and grid tables (reStructuredText-style) with alignment
- **Variables & reactivity** — `@var`, `@{name}`, `@for`, `@if` for dynamic documents
- **Directives** — `@[Figure]`, `@[Code]`, `@[Meta]`, `@[Theme]`, and 9 more built-in directives
- **Extensions** — Namespaced `@[org/Name](props)` for custom plugins

### 🛠️ Tooling

- **CLI** — `vell parse`, `vell fmt`, `vell render html`, `vell validate` with stdin/stdout support
- **Formatter** — Idempotent canonical formatting (`format(parse(format(parse(source))))` is stable)
- **LSP server** — Real diagnostics, hover info, context-aware completions, go-to-definition, document symbols, formatting
- **VS Code extension** — Syntax highlighting + LSP integration for `.vl` files

### 📦 Packages

- **vell-core** (Rust) — Reference parser, AST, formatter, validator
- **vell-wasm** (Rust/WASM) — WebAssembly bindings for browser and Node.js
- **vell-js** (TypeScript) — WASM wrapper with typed API
- **vell-renderer-html** (TypeScript) — HTML5 renderer with MathML, URL sanitization, footnote support
- **vell-renderer-pdf** (TypeScript) — Direct AST-to-PDF renderer (no HTML intermediary) using pdfkit
- **vell-vscode** (TypeScript) — VS Code extension with syntax highlighting and LSP client

## Documentation

| Document | Description |
|----------|-------------|
| [Language Reference](docs/language-reference.md) | Complete language syntax reference |
| [AST Reference](docs/ast-reference.md) | Full AST node reference with JSON schemas |
| [Renderer Guide](docs/renderer-guide.md) | How to build renderers for the Vell AST |
| [Extension Authoring](docs/extension-authoring.md) | How to create and distribute extensions |
| [Specification](spec/grammar.peg) | Formal PEG grammar |
| [AST Schema](spec/ast-schema.json) | JSON Schema for AST validation |
| [Roadmap](ROADMAP.md) | Development roadmap and future plans |

## Development

### Prerequisites

- Rust stable (latest)
- Node.js 20 or later
- wasm-pack (for WASM builds)

### Setup

```bash
# Clone the repository
git clone https://github.com/samin/vell.git
cd vell

# Build and test Rust crates
cargo build --workspace
cargo test --workspace

# Build TypeScript packages
npm install
npm run build
```

### Project Status

All 7 ROADMAP phases are complete:

| Phase | Status | Description |
|-------|--------|-------------|
| 1 | ✅ | Parser correctness — all fixtures pass |
| 2 | ✅ | Real fixture tests for all node types |
| 3 | ✅ | HTML renderer with MathML |
| 4 | ✅ | CLI with parse, fmt, render, validate |
| 5 | ✅ | Formatter idempotency tests |
| 6 | ✅ | LSP with diagnostics, hover, completions, go-to-def |
| 7 | ✅ | Direct PDF renderer (AST → PDF, no HTML) |

## License

Vell is copyright (C) 2026 **Samin Yeasar** and is licensed under the **GNU Affero General Public License v3.0 or later**. See [LICENSE](LICENSE).

---

<p align="center">
  <em>Built by Samin Yeasar · Solo developer project</em>
</p>
