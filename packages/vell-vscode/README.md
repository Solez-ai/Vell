# Vell for Visual Studio Code

Vell language support for VS Code — syntax highlighting, diagnostics, code completion, and more.

## Features

- **Syntax Highlighting** — Full grammar for `.vl` files with semantic token coloring
- **Diagnostics** — Real-time error and warning reporting as you type
- **Code Completion** — Smart completions for directives, variables, and references
- **Go to Definition** — Jump to variable declarations and reference definitions
- **Hover Information** — See variable values and directive documentation on hover
- **References** — Find all references to a variable or label
- **Rename** — Rename variables and labels across the document
- **Format Document** — Canonical Vell formatting (via `vell fmt`)
- **Export to HTML** — Quick export from the command palette

## Requirements

The extension requires the `vell-lsp` binary to be installed on your system:

```bash
cargo install --path crates/vell-cli
```

Make sure `vell-lsp` is available in your PATH.

## Extension Settings

This extension contributes the following commands:

* `vell.formatDocument`: Format the current Vell document
* `vell.exportHtml`: Export the current document to HTML

## Known Issues

- This extension is in **preview** status. The LSP server requires a locally installed `vell-lsp` binary.
- WASM-based client-side parsing is not yet integrated into the extension.

## Release Notes

### 0.1.0

Initial preview release:
- Syntax highlighting for `.vl` files
- LSP client integration
- Format and export commands

---

**Made by Samin Yeasar** — [Vell Language](https://vell-lang.dev)
