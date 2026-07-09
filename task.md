# Vell Language — Built Prompt

You are being asked to build **Vell**, a next-generation document and markup language. This is a substantial engineering project. Read every word of this prompt before writing a single line of code. Understand the architecture, the goals, and the constraints. Then execute the full plan in order.

---

## Project Identity

**Name:** Vell
**File extension:** `.vl`
**License:** GNU Affero General Public License v3.0 (AGPLv3)
**Repository layout:** monorepo

Vell is a human-readable markup language designed to sit between Markdown's simplicity and LaTeX's expressive power. It is deterministic, extensible, fast to parse, and designed with interactivity and multi-format export as first-class concerns — not afterthoughts.

The language produces a universal, versioned Abstract Syntax Tree (AST). All rendering, exporting, and tooling operates on that AST. The source syntax and the AST are separate contracts. Renderers depend on the AST schema, not on the source text.

---

## Design Principles

These principles are non-negotiable. Every architectural decision must be consistent with them.

1. **Human-readable first.** A `.vl` file must be legible to a person who has never read the spec. Syntax constructs must be intuitive from context.
2. **Unambiguous grammar.** The grammar is a Parsing Expression Grammar (PEG). Every valid input has exactly one parse tree. There are no context-sensitive rules. No edge cases. No 652-item conformance test suites like CommonMark needed.
3. **Linear-time parsing.** The tokenizer and parser together must run in O(n) time relative to input length. Packrat memoization is acceptable to achieve this for PEG.
4. **Deterministic output.** The same source file always produces the same AST, the same rendered output, and the same formatted source. There is no environment-dependent behavior.
5. **Extensible without breaking.** The `@` directive syntax is the extension mechanism. Core grammar never changes to accommodate new features. Extensions declare typed schemas. Unknown extension nodes are preserved as opaque `Extension` nodes, not parse errors.
6. **Interactive by design.** Variables, reactive bindings, and interactive component nodes are first-class AST node types. Interactivity is not bolted on via JavaScript embeds.
7. **Portable implementation.** The reference parser is written in Rust and compiled to WebAssembly. Every other language consumes the WASM binary. Nobody re-implements the parser.
8. **Tooling parity.** The language is only as good as its editor support. An LSP server, a formatter, and a VS Code extension ship alongside the parser.

---

## Repository Structure

Create the following monorepo structure exactly. Do not deviate.

```
vell/
├── LICENSE                          # AGPLv3 full text
├── README.md                        # Professional, descriptive, no emojis
├── CONTRIBUTING.md                  # Contribution guide
├── AGENT.md                         # AI agent orientation document
├── CHANGELOG.md                     # Initially empty, version 0.1.0 entry
├── .gitignore
├── Cargo.toml                       # Workspace root
├── package.json                     # Workspace root for JS/TS packages
│
├── crates/
│   ├── vell-core/                   # Reference parser and AST (Rust)
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── lexer.rs             # Tokenizer
│   │       ├── parser.rs            # PEG parser producing AST
│   │       ├── ast.rs               # AST node type definitions
│   │       ├── error.rs             # Parse error types
│   │       └── tests/
│   │           ├── mod.rs
│   │           ├── lexer_tests.rs
│   │           ├── parser_tests.rs
│   │           └── fixtures/        # .vl test fixture files
│   │
│   ├── vell-wasm/                   # WASM bindings (Rust + wasm-bindgen)
│   │   ├── Cargo.toml
│   │   └── src/
│   │       └── lib.rs
│   │
│   ├── vell-fmt/                    # Canonical formatter (Rust)
│   │   ├── Cargo.toml
│   │   └── src/
│   │       └── lib.rs
│   │
│   └── vell-lsp/                    # LSP server (Rust)
│       ├── Cargo.toml
│       └── src/
│           └── main.rs
│
├── packages/
│   ├── vell-js/                     # JavaScript/TypeScript bindings
│   │   ├── package.json
│   │   ├── tsconfig.json
│   │   └── src/
│   │       ├── index.ts             # Public API
│   │       └── wasm.ts              # WASM loader
│   │
│   ├── vell-renderer-html/          # HTML renderer (TypeScript)
│   │   ├── package.json
│   │   └── src/
│   │       └── index.ts
│   │
│   ├── vell-renderer-pdf/           # PDF renderer (TypeScript, uses pdfkit or similar)
│   │   ├── package.json
│   │   └── src/
│   │       └── index.ts
│   │
│   └── vell-vscode/                 # VS Code extension
│       ├── package.json
│       └── src/
│           └── extension.ts
│
├── spec/
│   ├── grammar.peg                  # Formal PEG grammar specification
│   ├── ast-schema.json              # JSON Schema for the AST
│   └── examples/                   # Annotated .vl example files
│       ├── 01-basic.vl
│       ├── 02-math.vl
│       ├── 03-tables.vl
│       ├── 04-interactive.vl
│       ├── 05-extensions.vl
│       └── 06-full-document.vl
│
└── docs/
    ├── language-reference.md
    ├── ast-reference.md
    ├── renderer-guide.md
    └── extension-authoring.md
```

---

## The Vell Language Specification

This is the authoritative syntax definition. Implement it exactly.

### Block-Level Syntax

Block-level constructs are identified by the first non-whitespace character(s) on a line. Block parsing dispatches in O(1) on the line prefix.

```
= Heading level 1
== Heading level 2
=== Heading level 3
==== Heading level 4

> Blockquote line
>> Nested blockquote

- Unordered list item
  - Nested list item (two-space indent per level)

1. Ordered list item
2. Second ordered list item

` ` ` language
Code block (fenced with triple backtick)
` ` `

$$ LaTeX math source $$    (block math, on its own line)

| Table row (pipe-delimited)
| Another row

+--------+--------+
| Grid   | Table  |    (grid table for merged cells)
+--------+--------+

---                      (horizontal rule)

@[DirectiveName](prop=value prop2="string value") {
  Optional body content, parsed as Vell inline
}

:: term                  (definition list term)
   Definition content here (indented)

> [!NOTE]                (admonition, GitHub-style but formalized)
> Content of the note

```

Blank lines separate block-level nodes. A blank line always terminates the current block.

### Inline Syntax

Inline constructs appear within block content. Delimiters are balanced.

```
*bold text*
/italic text/
_underlined text_
~strikethrough~
`inline code`
^superscript^
,,subscript,,

[link text](https://url.example)
[link text][ref-id]
![alt text](image-url.png)
![alt text][ref-id]

$inline math$

@{variableName}              (variable interpolation)
@[ComponentName](prop=value) (interactive component, inline)

[[citation-key]]             (citation reference)
[^footnote-marker]           (footnote reference)
```

Reference definitions (for links and images) are block-level:
```
[ref-id]: https://url.example "Optional title"
```

Footnote definitions are block-level:
```
[^footnote-marker]: Footnote content here.
```

### Variables and Reactivity

Variables are declared at block level and create a reactive scope for the entire document below the declaration.

```
@var count = 0
@var name = "World"
@var items = [1, 2, 3]

The count is currently @{count}.

@[Slider](min=0 max=100 bind=count label="Count")

@for item in @{items} {
  - Item value: @{item}
}

@if @{count} > 50 {
  Count is above 50.
} else {
  Count is 50 or below.
}
```

Variable values are JSON-compatible primitives and arrays. Objects are not supported in v0.1. The `@for` and `@if` constructs are optional features (declare them as `ExperimentalFeature` nodes in the AST if the renderer does not support them).

### Math

Math uses LaTeX syntax within `$...$` (inline) and `$$...$$` (block) delimiters. The parser does not interpret math content — it captures the raw LaTeX source string and stores it in a `MathInline` or `MathBlock` AST node. Renderers are responsible for rendering math from that string. The HTML renderer should emit MathML. Other renderers may use the raw string.

### Tables — Grid Syntax

For merged cells, Vell uses an ASCII grid table:

```
+----------+----------+----------+
| Header A | Header B            |
+----------+----------+----------+
| Cell 1   | Cell 2   | Cell 3   |
+----------+----------+----------+
| Cell 4              | Cell 6   |
+----------+----------+----------+
```

A cell that spans multiple columns is indicated by the absence of a `|` separator where one would otherwise appear. The parser calculates `colspan` and `rowspan` by analyzing the grid geometry.

Simple pipe tables (no merging) are also supported:

```
| Column A | Column B | Column C |
|----------|----------|----------|
| Value 1  | Value 2  | Value 3  |
```

### Directives and Extensions

The `@[Name](props) { body }` syntax is the universal extension mechanism. Core built-in directives use the same syntax:

```
@[Figure](src="image.png" alt="Description" caption="A figure caption")

@[Code](lang=python run=true) {
  print("Hello, Vell")
}

@[Diagram](type=flowchart) {
  A -> B -> C
  B -> D
}

@[Cite](key=smith2023 style=apa)

@[Slide]() {
  Slide content here. Each @[Slide] block becomes one slide in presentations.
}

@[Animation](duration=2s loop=true) {
  @[Frame](at=0s) { Initial state }
  @[Frame](at=1s) { Midpoint state }
  @[Frame](at=2s) { Final state }
}

@[Layout](columns=2) {
  Left column content.

  @[Column]()
  Right column content.
}

@[Accessibility](role=figure label="A bar chart showing quarterly revenue")

@[Theme](name=dark accent=#5B8DEF)

@[Meta](author="Samin Yeasar" date=2026-01-15 lang=en)
```

Custom extensions register under their own namespace:

```
@[myorg/CustomWidget](data=@{someVar})
```

---

## AST Schema

Define these node types in `crates/vell-core/src/ast.rs`. Every node must implement `Serialize` and `Deserialize` (serde). Every node carries `span` (byte offset start and end in the source) for editor tooling.

### Core Node Types

```rust
pub struct Document {
    pub version: u32,         // AST schema version, currently 1
    pub children: Vec<Node>,
    pub metadata: DocumentMetadata,
}

pub struct DocumentMetadata {
    pub title: Option<String>,
    pub author: Option<String>,
    pub date: Option<String>,
    pub lang: Option<String>,
    pub variables: HashMap<String, JsonValue>,
}

// Block nodes
Heading { level: u8, children: Vec<InlineNode>, id: Option<String> }
Paragraph { children: Vec<InlineNode> }
Blockquote { children: Vec<Node>, admonition_type: Option<String> }
CodeBlock { lang: Option<String>, source: String, executable: bool }
MathBlock { source: String }   // raw LaTeX
List { ordered: bool, start: Option<u32>, items: Vec<ListItem> }
ListItem { children: Vec<Node>, checked: Option<bool> }
Table { headers: Vec<TableCell>, rows: Vec<Vec<TableCell>> }
TableCell { children: Vec<InlineNode>, colspan: u32, rowspan: u32, align: Option<Alignment> }
HorizontalRule
DefinitionList { items: Vec<DefinitionItem> }
DefinitionItem { term: Vec<InlineNode>, definition: Vec<Node> }
ReferenceDefinition { id: String, url: String, title: Option<String> }
FootnoteDefinition { marker: String, children: Vec<Node> }
VarDeclaration { name: String, value: JsonValue }
ForLoop { variable: String, iterable: String, children: Vec<Node> }
IfBlock { condition: String, consequent: Vec<Node>, alternate: Option<Vec<Node>> }
Directive { name: String, props: HashMap<String, PropValue>, children: Vec<Node> }
Extension { name: String, props: HashMap<String, PropValue>, children: Vec<Node>, raw_source: String }

// Inline nodes
Text { value: String }
Bold { children: Vec<InlineNode> }
Italic { children: Vec<InlineNode> }
Underline { children: Vec<InlineNode> }
Strikethrough { children: Vec<InlineNode> }
Code { value: String }
Superscript { children: Vec<InlineNode> }
Subscript { children: Vec<InlineNode> }
Link { href: String, title: Option<String>, children: Vec<InlineNode> }
LinkRef { id: String, children: Vec<InlineNode> }
Image { src: String, alt: String, title: Option<String> }
ImageRef { id: String, alt: String }
MathInline { source: String }
VarInterpolation { name: String }
InlineComponent { name: String, props: HashMap<String, PropValue> }
Citation { key: String }
FootnoteRef { marker: String }
SoftBreak
HardBreak
```

All node types are wrapped in the `Node` and `InlineNode` enums. The AST schema version is `1` for this release.

---

## Formal PEG Grammar

Write the following as `spec/grammar.peg`. This is the normative grammar specification. It is also the document from which the Rust parser is derived.

```peg
Document    <- Block* EOF

Block       <- BlankLine*
               ( Heading / Admonition / Blockquote / CodeFence
               / MathBlock / GridTable / PipeTable / HRule
               / OrderedList / UnorderedList / DefList
               / VarDecl / ForLoop / IfBlock / Directive
               / FootnoteDef / RefDef / Paragraph )
               BlankLine*

Heading     <- '='+ ' ' InlineLine NEWLINE
Admonition  <- '>' ' ' '[!' [A-Z]+ ']' NEWLINE ('>' ' ' InlineLine NEWLINE)*
Blockquote  <- ('>' '>'? ' '? InlineLine NEWLINE)+
CodeFence   <- '`'`'`' LangId? NEWLINE (!('`'`'`') AnyLine)* '`'`'`' NEWLINE
MathBlock   <- '$$' NEWLINE (!('$$') AnyLine)* '$$' NEWLINE
HRule       <- '---' '-'* NEWLINE

GridTable   <- GridBorder (GridRow GridBorder)+ 
GridBorder  <- '+' ('-'+ '+')+  NEWLINE
GridRow     <- '|' (GridCell '|')+ NEWLINE
GridCell    <- ' ' (!('|') Char)* ' '

PipeTable   <- PipeRow PipeSep PipeRow*
PipeRow     <- '|' (PipeCell '|')+ NEWLINE
PipeSep     <- '|' ('-'+ '|')+ NEWLINE

OrderedList <- (Digit+ '.' ' ' ListItemBody NEWLINE)+
UnorderedList <- ('-' ' ' ListItemBody NEWLINE (INDENT ListItemBody NEWLINE)*)+
DefList     <- ('::' ' ' InlineLine NEWLINE INDENT AnyLine+ DEDENT)+

VarDecl     <- '@var' ' ' Ident ' '* '=' ' '* JsonValue NEWLINE
ForLoop     <- '@for' ' ' Ident ' ' 'in' ' ' VarRef NEWLINE INDENT Block+ DEDENT
IfBlock     <- '@if' ' ' Expr NEWLINE INDENT Block+ DEDENT 
               ('@else' NEWLINE INDENT Block+ DEDENT)?

Directive   <- '@[' DirectiveName '](' Props ')' (' ' '{' NEWLINE INDENT Block+ DEDENT '}')?
Extension   <- '@[' ExtName '/' DirectiveName '](' Props ')' (' ' '{' NEWLINE INDENT Block+ DEDENT '}')?

RefDef      <- '[' RefId ']' ':' ' ' URL Title? NEWLINE
FootnoteDef <- '[^' Marker ']:' ' ' InlineLine NEWLINE

Paragraph   <- InlineLine (InlineLine)* NEWLINE

InlineLine  <- Inline+ NEWLINE
Inline      <- Bold / Italic / Underline / Strike / Code / Super / Sub
             / MathInline / VarRef / InlineComp / Citation / FootnoteRef
             / Link / LinkRef / Image / ImageRef / Text

Bold        <- '*' Inline+ '*'
Italic      <- '/' Inline+ '/'
Underline   <- '_' Inline+ '_'
Strike      <- '~' Inline+ '~'
Code        <- '`' (!('`') Char)+ '`'
Super       <- '^' Inline+ '^'
Sub         <- ',,' Inline+ ',,'
MathInline  <- '$' (!('$') Char)+ '$'
VarRef      <- '@{' Ident '}'
InlineComp  <- '@[' DirectiveName '](' Props ')'
Citation    <- '[[' CitationKey ']]'
FootnoteRef <- '[^' Marker ']'
Link        <- '[' Inline+ '](' URL Title? ')'
LinkRef     <- '[' Inline+ '][' RefId ']'
Image       <- '![' AltText '](' URL Title? ')'
ImageRef    <- '![' AltText '][' RefId ']'
Text        <- Char+
```

---

## Implementation Tasks

Execute these tasks in order. Do not skip ahead.

### Task 1: Repository scaffolding

Create every file and directory in the repository structure. Initialize `Cargo.toml` workspace, `package.json` workspace (pnpm or npm workspaces), and `.gitignore`. Write the `LICENSE` file with the complete AGPLv3 text. Do not truncate it.

### Task 2: AGPLv3 License

Write the complete, untruncated GNU Affero General Public License Version 3.0 text into the `LICENSE` file. Include every section from the preamble through the "How to Apply These Terms" section. This is a legal document; do not paraphrase or abbreviate it.

Add the SPDX header to every source file:
```
// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Samin Yeasar
```

### Task 3: AST definitions (`crates/vell-core/src/ast.rs`)

Implement all node types listed in the AST Schema section above. Use Rust enums and structs with `serde` derives. Every node carries a `Span { start: usize, end: usize }` field for source location tracking. Implement `Display` for every node type that produces a normalized text representation (useful for testing and debugging).

Add `version: u32` to the `Document` node. Implement a `NodeKind` enum that mirrors the node type names without data (used for pattern matching in renderers without needing to destructure). Implement `From<Node> for NodeKind`.

### Task 4: Lexer (`crates/vell-core/src/lexer.rs`)

Implement a streaming, zero-copy lexer. Token types:

```rust
pub enum TokenKind {
    Equals, GreaterThan, Dash, Pipe, Plus,
    Backtick, BacktickFence,           // ` and ```
    DollarSign, DollarDouble,          // $ and $$
    Asterisk, Slash, Underscore, Tilde,
    Caret, DoubleComma,
    AtBracket,                         // @[
    AtBrace,                           // @{
    AtWord,                            // @var, @for, @if, @else
    BracketOpen, BracketClose,
    ParenOpen, ParenClose,
    BraceOpen, BraceClose,
    Bang,                              // !
    Colon, DoubleColon,
    Newline, BlankLine,
    Indent, Dedent,
    Text(String),
    Number(String),
    Ident(String),
    Eof,
}
```

The lexer must be a proper iterator. It must track line and column numbers for every token. It must handle UTF-8 input. Indent/Dedent tokens are emitted based on consistent 2-space indentation.

### Task 5: Parser (`crates/vell-core/src/parser.rs`)

Implement a recursive descent PEG parser that consumes the token stream from the lexer and produces a `Document` AST node.

Requirements:
- All parse functions return `Result<Node, ParseError>`
- `ParseError` carries source location, an error message, and an optional suggestion for fixing the error
- The parser never panics. All error paths return `Err`.
- Block-level dispatch is a match on the first token of each line — O(1)
- Inline parsing handles nested delimiters correctly. Bold inside italic is valid. Self-nesting (bold inside bold) produces a parse error.
- The grid table parser must correctly compute `colspan` and `rowspan` by analyzing the `+` and `-` geometry
- Variable declarations populate a scope that inline `VarRef` nodes can reference for validation (emit a warning, not an error, for undefined variable references — they may be defined by a runtime)
- Unknown `@[name]` directives that are not in the built-in directive list produce `Extension` nodes, not errors

Implement the following parse functions at minimum:
`parse_document`, `parse_block`, `parse_heading`, `parse_paragraph`, `parse_blockquote`, `parse_code_fence`, `parse_math_block`, `parse_list`, `parse_grid_table`, `parse_pipe_table`, `parse_directive`, `parse_var_decl`, `parse_for_loop`, `parse_if_block`, `parse_inline`, `parse_inline_sequence`, `parse_bold`, `parse_italic`, `parse_math_inline`, `parse_link`, `parse_image`, `parse_var_ref`, `parse_citation`

### Task 6: Error types (`crates/vell-core/src/error.rs`)

```rust
pub struct ParseError {
    pub kind: ParseErrorKind,
    pub span: Span,
    pub message: String,
    pub suggestion: Option<String>,
}

pub enum ParseErrorKind {
    UnexpectedToken,
    UnterminatedDelimiter,
    InvalidIndentation,
    UndefinedReference,
    MalformedDirective,
    MalformedTable,
    MalformedMath,
    InvalidPropValue,
}
```

Implement `std::error::Error` and `Display` for `ParseError`. Error messages must be human-readable and actionable. Good: "Unterminated bold delimiter: '*' opened at line 4, column 12 was never closed." Bad: "parse error at offset 47".

### Task 7: Comprehensive tests

Write tests in `crates/vell-core/src/tests/`. Every node type must have at least one round-trip test (parse source → AST → verify AST structure). Every error type must have at least one test that verifies the correct error is returned. Every table format must have tests including merged cells.

Write fixture files in `crates/vell-core/src/tests/fixtures/`. Each fixture is a `.vl` file paired with a `.json` file containing the expected AST output.

Fixtures to write:
- `headings.vl` / `headings.json`
- `inline.vl` / `inline.json`
- `lists.vl` / `lists.json`
- `code_blocks.vl` / `code_blocks.json`
- `math.vl` / `math.json`
- `pipe_table.vl` / `pipe_table.json`
- `grid_table_merged.vl` / `grid_table_merged.json`
- `directives.vl` / `directives.json`
- `variables.vl` / `variables.json`
- `for_loop.vl` / `for_loop.json`
- `full_document.vl` / `full_document.json`

### Task 8: WASM bindings (`crates/vell-wasm/src/lib.rs`)

Use `wasm-bindgen` to expose the parser to JavaScript. Expose the following functions:

```rust
#[wasm_bindgen]
pub fn parse(source: &str) -> Result<JsValue, JsValue>
// Returns the AST as a JSON-serialized JsValue, or a ParseError as JsValue

#[wasm_bindgen]
pub fn parse_to_json(source: &str) -> String
// Returns AST as JSON string; errors embedded in the JSON

#[wasm_bindgen]
pub fn get_version() -> String
// Returns the Vell parser version string

#[wasm_bindgen]
pub fn validate(source: &str) -> JsValue
// Returns array of diagnostics (errors and warnings) as JsValue
```

Configure `wasm-pack` build in `crates/vell-wasm/Cargo.toml`. Target: `web` and `nodejs` (both builds must work).

### Task 9: Formatter (`crates/vell-fmt/src/lib.rs`)

The formatter takes a `Document` AST and produces canonical Vell source text. It is deterministic: formatting the same AST always produces the same output. Formatting is idempotent: formatting already-formatted source produces identical source.

Formatting rules:
- Headings: `=` prefix, single space, content, newline
- Blank lines: exactly one blank line between top-level blocks, zero between list items of the same list
- Inline bold: `*content*` with no spaces inside delimiters
- Inline italic: `/content/` with no spaces inside delimiters
- Code blocks: triple backtick, language identifier (lowercase), newline, content, triple backtick
- Math blocks: `$$` on its own line, content, `$$` on its own line
- Directives: `@[Name](prop=value)` with props in the order they were declared, normalized
- Tables: normalized column widths (each column as wide as its widest cell + 1 space padding each side)
- Lists: `-` for unordered (even if source used `*` or `+`), `1.` `2.` etc for ordered
- Maximum line length: 100 characters for prose (soft wrap at word boundaries); no limit for code, math, or tables
- Trailing whitespace: none on any line
- Final newline: always present

Expose `format(doc: &Document) -> String` and `format_source(source: &str) -> Result<String, ParseError>`.

### Task 10: LSP server (`crates/vell-lsp/src/main.rs`)

Implement a Language Server Protocol server using the `tower-lsp` crate. Implement the following LSP features:

**Diagnostics:** On every `textDocument/didOpen` and `textDocument/didChange`, parse the document and emit diagnostics for all `ParseError` values. Diagnostics carry range (line/column), severity (Error for parse errors, Warning for undefined variable references), and message.

**Hover:** On `textDocument/hover`, if the cursor is on a directive name, return documentation for that directive. If on a variable reference, return the variable's declared type and value.

**Completion:** On `textDocument/completion`, provide:
- Block-level completions at line start (all heading levels, directive names, list syntax)
- Directive prop completions for built-in directives
- Variable name completions after `@{`

**Formatting:** On `textDocument/formatting`, run the formatter and return the text edits.

**Go to definition:** On `textDocument/definition`, if on a `LinkRef`, navigate to the `ReferenceDefinition`. If on a `FootnoteRef`, navigate to the `FootnoteDefinition`.

**Document symbols:** On `textDocument/documentSymbol`, return all headings as a hierarchical symbol tree.

The LSP server must handle incremental document changes efficiently. Maintain a cache of parsed documents keyed by URI.

### Task 11: HTML renderer (`packages/vell-renderer-html/src/index.ts`)

Implement a TypeScript renderer that accepts a `Document` AST (as a JavaScript object deserialized from the WASM output) and produces an HTML string or DOM fragment.

Rendering rules:
- `Heading { level: 1 }` → `<h1 id="slug-from-content">...</h1>` (auto-generate IDs from heading text)
- `Paragraph` → `<p>...</p>`
- `Blockquote` → `<blockquote>...</blockquote>`
- `CodeBlock { lang }` → `<pre><code class="language-{lang}">...</code></pre>`
- `MathBlock { source }` → `<math display="block">` ... MathML from source `</math>` (use mathml-to-mathml or a LaTeX-to-MathML library)
- `MathInline { source }` → `<math display="inline">` ... MathML `</math>`
- `List { ordered: true }` → `<ol>` / `<ul>` with `<li>` items
- `Table` → `<table>` with `<thead>`, `<tbody>`, `<th>`, `<td>` with `colspan` and `rowspan` attributes
- `HorizontalRule` → `<hr>`
- `Bold` → `<strong>`
- `Italic` → `<em>`
- `Underline` → `<u>`
- `Strikethrough` → `<del>`
- `Code` (inline) → `<code>`
- `Superscript` → `<sup>`
- `Subscript` → `<sub>`
- `Link` → `<a href="..." title="...">` (sanitize href)
- `Image` → `<img src="..." alt="..." title="...">`
- `VarInterpolation` → `<span data-vell-var="{name}" data-vell-value="{resolvedValue}">...</span>`
- `InlineComponent` → `<vell-component name="{name}" ...props as data attributes>`
- `Citation` → `<cite data-key="{key}">[{n}]</cite>`
- `FootnoteRef` → `<sup><a href="#fn-{marker}">[n]</a></sup>`
- `FootnoteDefinition` → rendered at document end in `<section class="footnotes">`
- `Directive { name: "Figure" }` → `<figure>` with `<img>` and `<figcaption>`
- `Directive { name: "Slide" }` → `<section class="vell-slide">`
- `Directive { name: "Layout", props: { columns: 2 } }` → `<div class="vell-layout vell-cols-2">`
- `Directive { name: "Accessibility" }` → add `role` and `aria-label` to the next sibling element
- Unknown `Directive` / `Extension` → `<div class="vell-extension" data-name="{name}" ...props as data attributes>`

The HTML output must be valid HTML5. All user-supplied string values must be HTML-escaped. hrefs must be sanitized (allow only `http:`, `https:`, `mailto:`, and relative paths; reject `javascript:` and `data:` schemes).

Produce both `render(doc: VellDocument): string` (full HTML) and `renderToFragment(doc: VellDocument): DocumentFragment` (live DOM, browser-only) exports.

### Task 12: JavaScript/TypeScript bindings (`packages/vell-js/src/index.ts`)

Wrap the WASM module with a clean TypeScript API:

```typescript
export interface VellDocument { /* mirrors AST */ }
export interface ParseError { kind: string; span: { start: number; end: number }; message: string; suggestion?: string }
export interface ParseResult { document: VellDocument; errors: ParseError[]; warnings: ParseError[] }

export function parse(source: string): ParseResult
export function parseOrThrow(source: string): VellDocument
export function validate(source: string): ParseError[]
export function format(source: string): string
export function getVersion(): string
```

Write full TypeScript type definitions for every AST node type. Export them all. The type definitions must be 1:1 with the Rust AST.

### Task 13: VS Code extension (`packages/vell-vscode/src/extension.ts`)

Implement a VS Code extension that:
- Registers `.vl` as a language with `vell` as the language identifier
- Connects to the `vell-lsp` language server binary (or the WASM LSP fallback if the binary is unavailable)
- Provides syntax highlighting via a TextMate grammar (`syntaxes/vell.tmLanguage.json`)
- Adds a "Vell: Format Document" command
- Adds a "Vell: Export to HTML" command
- Registers file icons for `.vl` files

Write `syntaxes/vell.tmLanguage.json` with scopes for:
- `markup.heading.vell` (heading lines)
- `markup.bold.vell` (bold `*...*`)
- `markup.italic.vell` (italic `/.../ `)
- `markup.code.vell` (inline code)
- `markup.code.block.vell` (code fences)
- `string.other.math.vell` (math `$...$` and `$$...$$`)
- `entity.name.tag.directive.vell` (directive names in `@[Name]`)
- `variable.other.vell` (variable references `@{name}`)
- `comment.line.vell` (if comments are added later, reserve the scope)
- `keyword.control.vell` (`@var`, `@for`, `@if`, `@else`)

### Task 14: Spec files

Write `spec/grammar.peg` as the complete normative PEG grammar (use the grammar defined in this prompt as the basis, and make it complete and internally consistent).

Write `spec/ast-schema.json` as a complete JSON Schema (draft-07) for the AST. Every node type must be represented as a `$defs` entry with all required and optional properties documented.

Write all six example files in `spec/examples/`:
- `01-basic.vl`: headings, paragraphs, lists, links, images, code blocks, blockquotes, horizontal rules
- `02-math.vl`: inline math, block math, multiple equations, math in headings
- `03-tables.vl`: simple pipe table, grid table with row and column merges
- `04-interactive.vl`: `@var` declarations, `@[Slider]`, `@{var}` interpolation, `@for` loop, `@if` conditional
- `05-extensions.vl`: all built-in directives, one custom extension namespace
- `06-full-document.vl`: a realistic research-style document using every feature

### Task 15: Documentation

Write `docs/language-reference.md` — a complete reference for the Vell syntax. Cover every construct. Include examples for every feature. Use Vell syntax in the examples (fenced as `vl` code blocks within the Markdown). Sections: Overview, Block Elements, Inline Elements, Math, Tables, Directives, Variables and Reactivity, Extensions, Accessibility Metadata, Export Targets.

Write `docs/ast-reference.md` — a complete reference for the AST node types. For every node: the Rust struct/enum definition, the JSON representation, which source syntax produces it, and notes on how renderers should handle it.

Write `docs/renderer-guide.md` — a guide for building a new renderer. Cover: receiving an AST, walking the tree, handling unknown nodes, handling `Extension` nodes, the fallback rendering contract, testing a renderer.

Write `docs/extension-authoring.md` — a guide for building a Vell extension. Cover: the extension schema declaration, registering with the plugin registry, providing a renderer adapter, providing LSP documentation.

### Task 16: README.md

Write a professional, comprehensive README. Requirements:
- No emojis anywhere in the document
- No informal language
- Professional technical writing throughout
- Sections: Project overview, Design principles, Language syntax (with extensive examples), Installation, Usage (CLI, library, VS Code), Architecture overview, Rendering targets, Extension system, Contributing, License
- Include realistic `.vl` syntax examples in fenced code blocks
- Include the full pipeline diagram description in prose
- Do not use marketing language ("powerful", "amazing", "blazing fast") — describe capabilities factually

### Task 17: CONTRIBUTING.md

Write a comprehensive contributor guide. Sections:
- Code of conduct reference
- Development environment setup (Rust toolchain version, wasm-pack, Node.js version, pnpm)
- Repository structure walkthrough
- How to run tests (`cargo test`, `pnpm test`)
- How to run the LSP server locally
- How to build the WASM module
- Coding standards: Rust (edition 2021, `clippy` clean, `rustfmt` formatted, all public items documented), TypeScript (strict mode, ESLint clean, all exports typed)
- Commit message format (Conventional Commits)
- Pull request process
- How to add a new built-in directive
- How to add a new renderer
- How to add test fixtures
- Issue reporting guidelines

### Task 18: AGENT.md

Write a comprehensive orientation document for AI coding agents. This file exists so that any AI assistant given access to this repository can immediately understand the codebase and contribute effectively.

Sections:

**Project summary** — What Vell is in three paragraphs.

**Repository map** — Every directory and its purpose in one sentence each.

**Key concepts** — Define: PEG grammar, AST, span, directive, extension, renderer, LSP, WASM binding, reactive variable. Each definition is two to four sentences.

**Architecture invariants** — The rules that must never be violated:
- The parser never panics
- The AST schema version must be incremented when any breaking change is made to node structure
- The formatter is idempotent
- Renderers must not error on unknown node types; they must use the `fallback` child subtree
- The WASM API is the stable public API; internal Rust APIs are not stable
- All user-supplied strings are untrusted and must be sanitized before rendering

**Crate/package responsibilities** — For each crate and package, what it does, what it depends on, and what must never be added to it.

**How to add a feature** — Step-by-step checklist for adding a new node type, a new directive, or a new renderer.

**Common pitfalls** — A list of mistakes that are easy to make and how to avoid them. Include: assuming the lexer produces one token per character, forgetting to handle `Extension` nodes in a renderer, breaking idempotency in the formatter, not incrementing the AST version.

**Testing philosophy** — How tests are organized, what the fixture system is, what counts as sufficient test coverage.

**Build commands reference** — Every command needed to build, test, format, and lint the project.

### Task 19: Final validation

After all files are written:

1. Run `cargo check --workspace` and fix all errors and warnings.
2. Run `cargo test --workspace` and fix all failing tests.
3. Run `cargo fmt --all` and `cargo clippy --all -- -D warnings` and fix all issues.
4. Run `pnpm install` and `pnpm build` in the JavaScript packages and fix all TypeScript errors.
5. Verify that the fixture files all parse correctly by running them through the parser in tests.
6. Verify that the README contains no emojis and no marketing language.
7. Verify that the LICENSE file is the complete AGPLv3 text and is not truncated.

---

## Constraints and Requirements

**Do not use any of the following:**
- Markdown parsing libraries (e.g. `pulldown-cmark`). The parser is written from scratch.
- Regular expressions in the parser. Use the PEG approach with recursive descent.
- `unwrap()` or `expect()` anywhere in the parser or formatter. All error paths must be handled.
- `unsafe` Rust in `vell-core` or `vell-fmt`. WASM bindings may use `unsafe` only where `wasm-bindgen` requires it.
- Any runtime dependencies in `vell-core` beyond `serde` and `serde_json`.

**Required Rust dependencies:**
- `serde = { version = "1", features = ["derive"] }` — serialization
- `serde_json = "1"` — JSON output
- `thiserror = "1"` — error types
- `tower-lsp = "0.20"` — LSP server (vell-lsp only)
- `wasm-bindgen = "0.2"` — WASM bindings (vell-wasm only)

**Required TypeScript/JavaScript dependencies:**
- TypeScript `^5.0`
- No runtime dependencies in `vell-js` beyond the WASM binary
- `vscode-languageclient` in `vell-vscode`

**Code quality requirements:**
- Every public function, struct, enum, and module in Rust must have a doc comment (`///`).
- Every exported TypeScript function and type must have a JSDoc comment.
- No `TODO` or `FIXME` comments in committed code — either implement it or create a tracked issue reference.
- All tests must have descriptive names that describe what they test, not how.

---

## Success Criteria

The implementation is complete when:

1. `cargo test --workspace` passes with zero failures.
2. `cargo clippy --all -- -D warnings` produces zero warnings.
3. The six example `.vl` files in `spec/examples/` all parse without errors.
4. The HTML renderer produces valid HTML5 for all six example files.
5. The formatter is idempotent on all six example files (formatting twice produces the same output as formatting once).
6. The LSP server starts, connects, and provides diagnostics for a file containing a deliberate parse error.
7. The VS Code extension activates for a `.vl` file and applies syntax highlighting.
8. The `README.md`, `CONTRIBUTING.md`, and `AGENT.md` files are complete and contain no truncated sections.
9. The `LICENSE` file contains the complete AGPLv3 text.
10. All source files carry the SPDX license header.

Begin with Task 1. Proceed in order. Do not skip tasks.
