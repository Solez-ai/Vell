# Vell Language Reference

> Version 1.0 | AST Schema v1
>
> Vell is created, owned, and developed by **Samin Yeasar** as a solo developer project.

---

## Table of Contents

1. [Overview](#1-overview)
2. [File Format](#2-file-format)
3. [Comments](#3-comments)
4. [Block Elements](#4-block-elements)
   - 4.1 [Headings](#41-headings)
   - 4.2 [Paragraphs](#42-paragraphs)
   - 4.3 [Blockquotes](#43-blockquotes)
   - 4.4 [Admonitions](#44-admonitions)
   - 4.5 [Code Blocks](#45-code-blocks)
   - 4.6 [Math Blocks](#46-math-blocks)
   - 4.7 [Lists](#47-lists)
   - 4.8 [Tables](#48-tables)
   - 4.9 [Horizontal Rules](#49-horizontal-rules)
   - 4.10 [Definition Lists](#410-definition-lists)
   - 4.11 [Reference Definitions](#411-reference-definitions)
   - 4.12 [Footnote Definitions](#412-footnote-definitions)
5. [Inline Elements](#5-inline-elements)
   - 5.1 [Text](#51-text)
   - 5.2 [Bold](#52-bold)
   - 5.3 [Italic](#53-italic)
   - 5.4 [Underline](#54-underline)
   - 5.5 [Strikethrough](#55-strikethrough)
   - 5.6 [Inline Code](#56-inline-code)
   - 5.7 [Superscript and Subscript](#57-superscript-and-subscript)
   - 5.8 [Links](#58-links)
   - 5.9 [Images](#59-images)
   - 5.10 [Inline Math](#510-inline-math)
   - 5.11 [Citations](#511-citations)
   - 5.12 [Footnote References](#512-footnote-references)
   - 5.13 [Line Breaks](#513-line-breaks)
6. [Variables and Reactivity](#6-variables-and-reactivity)
   - 6.1 [Variable Declarations](#61-variable-declarations)
   - 6.2 [Variable Interpolation](#62-variable-interpolation)
   - 6.3 [For Loops](#63-for-loops)
   - 6.4 [If Blocks](#64-if-blocks)
7. [Directives](#7-directives)
   - 7.1 [Built-in Directives](#71-built-in-directives)
   - 7.2 [Directive Syntax](#72-directive-syntax)
   - 7.3 [Inline Components](#73-inline-components)
   - 7.4 [Meta Directive](#74-meta-directive)
   - 7.5 [Cross-References](#75-cross-references)
   - 7.6 [Diagrams](#76-diagrams)
   - 7.7 [Charts](#77-charts)
   - 7.8 [Include](#78-include)
   - 7.9 [Slides](#79-slides)
   - 7.10 [Table of Contents](#710-table-of-contents)
8. [Extensions](#8-extensions)
9. [Escape Sequences](#9-escape-sequences)
10. [Document Metadata](#10-document-metadata)
11. [Variable System Reference](#11-variable-system-reference)
12. [Property Values](#12-property-values)
13. [Complete Example](#13-complete-example)
14. [CLI Reference](#14-cli-reference)
   - 14.1 [vell render html](#141-vell-render-html)
   - 14.2 [vell render pdf](#142-vell-render-pdf)
   - 14.3 [vell render slides](#143-vell-render-slides)
   - 14.4 [vell parse](#144-vell-parse)
   - 14.5 [vell fmt](#145-vell-fmt)
   - 14.6 [vell validate](#146-vell-validate)

---

## 1. Overview

Vell is a human-readable markup language designed to bridge the gap between Markdown's simplicity and LaTeX's expressive power. It produces a **versioned, deterministic abstract syntax tree (AST)** that all tooling operates on, ensuring that the same source always produces the same rendered output regardless of the renderer version or configuration.

### Design Philosophy

Vell was built on the following principles:

- **Readability first.** Source files should be as readable as the rendered output. Syntax is chosen to be visually intuitive.
- **Deterministic parsing.** A formal PEG grammar guarantees that every valid `.vl` file produces exactly one AST. No flavor-dependent behavior.
- **Linear-time parsing.** The parser runs in O(n) time proportional to the input length.
- **Extensibility by design.** The directive system (`@[Name](props)`) allows custom extensions without modifying the grammar.
- **Interactive by default.** Variables, for-loops, and conditionals make Vell documents reactive rather than static.
- **Portable toolchain.** The reference parser is written in Rust and compiled to WebAssembly, making it available in any JavaScript environment.

### File Extension

Vell source files use the `.vl` file extension.

---

## 2. File Format

Vell documents are UTF-8 encoded plain text files. Line endings can be either LF (`\n`) or CRLF (`\r\n`). The parser normalizes line endings internally.

A Vell document consists of a sequence of **block elements** separated by blank lines. Each block element is identified by its first non-whitespace characters. Inline elements are parsed within the text of block elements.

### Character Encoding

All Vell documents should be UTF-8 encoded. The parser operates on byte-level spans but treats character boundaries correctly for multi-byte UTF-8 sequences. Non-ASCII characters (Unicode) are fully supported in text content.

---

## 3. Comments

Vell does not have a native comment syntax. The parser treats all text as content. To include non-rendered annotations, use the `@[Accessibility]` directive or place content in a block that renderers will skip (such as an empty reference definition).

> Future versions may introduce a comment syntax such as `//` or `%`.

---

## 4. Block Elements

Block elements form the structural backbone of a Vell document. Each block begins at the start of a line and continues until a blank line or a different block type is encountered.

### 4.1 Headings

Headings are created using one or more `=` characters followed by a space and the heading text. The number of `=` characters determines the heading level (1 through 6).

```vell
= Title              # Level 1 (document title)
== Section           # Level 2
=== Subsection       # Level 3
==== Sub-subsection  # Level 4
===== Deep           # Level 5
====== Deeper        # Level 6
```

**Rules:**
- There must be a space between the `=` markers and the heading text.
- Heading text can contain inline markup (bold, italic, code, math, etc.).
- The first level-1 heading in a document sets the document's metadata title.
- Headings automatically generate an `id` attribute (slugified from the text) for anchor linking.

**AST representation:**
```json
{
  "type": "Heading",
  "level": 1,
  "children": [{ "type": "Text", "value": "Title" }],
  "id": "title",
  "span": { "start": 0, "end": 8 }
}
```

### 4.2 Paragraphs

Paragraphs are sequences of inline content separated by blank lines. They are the default block type and require no special prefix.

```vell
This is a simple paragraph. It can contain *bold*, /italic/, and other inline elements.

This is a second paragraph, separated by a blank line.
```

**Rules:**
- Paragraphs cannot start with characters that trigger other block types (`=`, `>`, `` ` ``, `- `, `|`, `+`, `:`, `@`, `[`, digits followed by `. `).
- Paragraphs wrap implicitly; a single paragraph can span multiple lines.
- Blank lines separate paragraphs from each other and from other block types.

### 4.3 Blockquotes

Blockquotes are created by prefixing lines with `>`.

```vell
> This is a blockquote.
> It can span multiple lines.
>
> Blockquotes can contain nested **inline** markup.
```

**Rules:**
- Each line of the blockquote must start with `>`.
- An optional space after `>` is recommended but not required.
- Blockquotes can contain other block elements (paragraphs, lists, code blocks) when nested.
- The content after `>` is parsed as a nested Vell document.

**AST representation:**
```json
{
  "type": "Blockquote",
  "children": [
    {
      "type": "Paragraph",
      "children": [{ "type": "Text", "value": "This is a blockquote." }]
    }
  ],
  "admonition_type": null
}
```

### 4.4 Admonitions

Admonitions are styled blockquotes with a type indicator. They use the `> [!TYPE]` syntax.

```vell
> [!NOTE]
> This is a note to the reader.

> [!WARNING]
> This is a warning about something important.

> [!TIP]
> This is a helpful tip.

> [!IMPORTANT]
> This is crucial information.

> [!CAUTION]
> This requires careful attention.
```

**Supported admonition types:** `NOTE`, `TIP`, `IMPORTANT`, `WARNING`, `CAUTION`

**Rules:**
- The admonition declaration `> [!TYPE]` must be on its own line.
- Subsequent lines with `>` content form the admonition body.
- Renderers apply distinct visual styling for each admonition type (colors, icons).

### 4.5 Code Blocks

Fenced code blocks use triple backticks ``` ``` ``` with an optional language identifier.

```vell
```rust
fn main() {
    println!("Hello, Vell!");
}
```

```python
def hello():
    print("Hello, Vell!")
```
```

**Rules:**
- The opening fence must be at the start of a line (optional leading whitespace is allowed).
- The language identifier follows the opening fence on the same line.
- The closing fence must be on its own line with just ``` ``` ```.
- Content inside code blocks is treated as literal text (no inline parsing).

**AST representation:**
```json
{
  "type": "CodeBlock",
  "lang": "rust",
  "source": "fn main() {\n    println!(\"Hello, Vell!\");\n}",
  "executable": false
}
```

### 4.6 Math Blocks

Math blocks contain raw LaTeX mathematical expressions, delimited by `$$` on their own lines.

```vell
$$
\int_0^1 x^2 \, dx = \frac{1}{3}
$$

$$
E = mc^2
$$
```

**Rules:**
- Both opening and closing `$$` must be on their own lines.
- Content between `$$` delimiters is raw LaTeX and is not parsed as Vell inline content.
- Renderers should render math blocks using MathML or a LaTeX-to-MathML converter.

### 4.7 Lists

Vell supports both unordered and ordered lists.

**Unordered lists** use `- ` as the list marker:

```vell
- First item
- Second item
- Third item
```

**Ordered lists** use a number followed by `. `:

```vell
1. First step
2. Second step
3. Third step
```

**Nested lists** use indentation (2 spaces per level):

```vell
- Level one
  - Level two
    - Level three
- Back to level one
```

**Rules:**
- Unordered list markers are `- ` (hyphen followed by space).
- Ordered list markers are digits followed by `. ` (e.g., `1. `).
- Indentation must be a multiple of 2 spaces.
- List items can contain paragraphs and other block content.

### 4.8 Tables

Vell supports two table syntaxes: **pipe tables** (simple) and **grid tables** (advanced).

**Pipe tables** use pipe characters `|` to separate columns:

```vell
| Name    | Age | City     |
|---------|-----|----------|
| Alice   | 30  | New York |
| Bob     | 25  | London   |
```

Alignment is controlled by colons in the separator row:

```vell
| Left | Center | Right |
|:-----|:------:|------:|
| A    | B      | C     |
```

**Grid tables** use `+` and `-` for borders, supporting merged cells:

```vell
+----------+----------+
| Header 1 | Header 2 |
+==========+==========+
| Cell A   | Cell B   |
+----------+----------+
| Cell C   | Cell D   |
+----------+----------+
```

### 4.9 Horizontal Rules

A horizontal rule is created with three or more consecutive `-` characters on their own line:

```vell
---
```

**Rules:**
- Must be at least 3 `-` characters.
- Must be on its own line.
- Cannot contain other characters.

### 4.10 Definition Lists

Definition lists pair a term with its definition, using `:: ` as the term marker:

```vell
:: Term
   Definition of the term goes here.

:: Another Term
   Definition of the second term.
   It can span multiple lines.
```

### 4.11 Reference Definitions

Reference definitions create named references that can be used in link references:

```vell
[ref]: https://example.com "Optional Title"
[image]: /images/photo.png
```

### 4.12 Footnote Definitions

Footnote definitions provide content for footnote references:

```vell
[^one]: This is the content of footnote one.
[^two]: This is the content of footnote two, with *inline* markup.
```

---

## 5. Inline Elements

Inline elements appear within the text of block elements and are delimited by matching pairs of characters.

### 5.1 Text

Plain text is any character sequence that does not trigger inline markup. All text is preserved exactly as written, with backslash-escaped delimiters rendered as literal characters.

### 5.2 Bold

Bold text is delimited by `*` characters:

```vell
This is *bold text* in a sentence.
```

### 5.3 Italic

Italic text is delimited by `/` characters:

```vell
This is /italic text/ in a sentence.
```

### 5.4 Underline

Underlined text is delimited by `_` characters:

```vell
This is _underlined text_ in a sentence.
```

### 5.5 Strikethrough

Strikethrough text is delimited by `~` characters:

```vell
This is ~strikethrough text~ in a sentence.
```

### 5.6 Inline Code

Inline code is delimited by backtick `` ` `` characters:

```vell
Use the `parse_document` function to parse source text.
```

### 5.7 Superscript and Subscript

Superscript uses `^` delimiters and subscript uses `,,` delimiters:

```vell
Einstein's E^mc^2^ is famous.
Water is H,,2,,O.
```

### 5.8 Links

Vell supports inline links and reference-style links:

**Inline links:**
```vell
[Visit our site](https://example.com)
[A link with title](https://example.com "Homepage")
```

**Reference-style links:**
```vell
[Reference link][ref-id]
[Reference link][]
```

### 5.9 Images

Images use `![alt](src)` syntax:

```vell
![Vell Logo](/images/logo.png "Vell Logo")
![Reference image][image-ref]
```

### 5.10 Inline Math

Inline math uses `$` delimiters:

```vell
The equation $E = mc^2$ is famous.
In a right triangle, $a^2 + b^2 = c^2$.
```

### 5.11 Citations

Citations use `[[key]]` syntax:

```vell
This result was established by [[smith2023]].
```

### 5.12 Footnote References

Footnote references use `[^marker]` syntax:

```vell
This statement has a footnote[^one] attached.
```

### 5.13 Line Breaks

- **Soft breaks** occur naturally at line boundaries in the source and render as a space in output.
- **Hard breaks** are not currently supported with a specific syntax; paragraphs are automatically joined.

---

## 6. Variables and Reactivity

Vell includes a built-in variable system that enables dynamic document content.

### 6.1 Variable Declarations

Variables are declared using `@var`:

```vell
@var name = "Vell"
@var count = 42
@var items = [1, 2, 3]
@var enabled = true
@var pi = 3.14159
```

**Supported value types:**
- Strings: `"hello"` or bare words like `hello`
- Numbers: `42`, `3.14`, `-1`
- Booleans: `true`, `false`
- Null: `null`
- Arrays: `[1, 2, 3]`
- Objects: `{"key": "value"}`

### 6.2 Variable Interpolation

Variables are referenced using `@{name}`:

```vell
The value of count is @{count}.
Welcome, @{name}! You have @{count} items.
```

If a variable is referenced before it is declared, the parser emits a warning (not an error).

### 6.3 For Loops

For loops iterate over array variables:

```vell
@var fruits = ["apple", "banana", "cherry"]

@for fruit in @{fruits} {
  - @{fruit}
}
```

### 6.4 If Blocks

If blocks conditionally render content based on variable values:

```vell
@if @{count} > 0 {
  You have @{count} items.
} else {
  You have no items.
}
```

---

## 7. Directives

Directives are the extension mechanism of Vell. They use `@[Name](props)` syntax and can have optional block bodies.

### 7.1 Built-in Directives

Vell includes 13 built-in directives:

| Directive | Purpose | Example |
|-----------|---------|---------|
| `@[Meta]` | Sets document metadata | `@[Meta](title="My Doc" author="Me")` |
| `@[Include]` | Includes content from another Vell file | `@[Include](path="chapter2.vl")` |
| `@[Slide]` | Defines a presentation slide | `@[Slide]` ... |
| `@[Figure]` | Embeds an image with caption | `@[Figure](src="chart.png" caption="Data")` |
| `@[Code]` | Displays a code snippet | `@[Code](lang=rust)` ... |
| `@[Diagram]` | Renders a diagram with type-specific formatting | `@[Diagram](type=mermaid caption="Flowchart")` ... |
| `@[Chart]` | Renders a data visualization (bar chart) | `@[Chart](type=bar title="Sales")` ... |
| `@[Cite]` | Formatted citation | `@[Cite](key=smith2023)` |
| `@[Animation]` | Animated content | `@[Animation]` ... |
| `@[Frame]` | Bordered box | `@[Frame]` ... |
| `@[Layout]` | Multi-column layout | `@[Layout](columns=2)` ... |
| `@[Column]` | Column definition | `@[Column]` ... |
| `@[Accessibility]` | Alt text / ARIA | `@[Accessibility](role=banner)` |
| `@[Theme]` | Visual theme | `@[Theme](name=dark)` |
| `@[Slider]` | Interactive slider | `@[Slider](min=0 max=100)` |
| `@[Equation]` | Numbered equation with optional label | `@[Equation](source="E=mc^2" label=e:mass-energy)` |
| `@[Ref]` | Cross-reference to a labeled equation or theorem | `@[Ref](label=e:mass-energy)` |
| `@[Theorem]` | Theorem environment (auto-numbered) | `@[Theorem](name="Pythagoras" label=thm:test)` ... |
| `@[Proof]` | Proof environment (no number) | `@[Proof]` ... |
| `@[Lemma]` | Lemma environment | `@[Lemma](name="Triangle Inequality")` ... |
| `@[Corollary]` | Corollary environment | `@[Corollary](name="Sum of Angles")` ... |
| `@[Definition]` | Definition environment | `@[Definition](name="Prime Number")` ... |
| `@[Remark]` | Remark environment (no number) | `@[Remark]` ... |
| `@[Example]` | Example environment (no number) | `@[Example]` ... |
| `@[Conjecture]` | Conjecture environment | `@[Conjecture](name="Goldbach's")` ... |
| `@[Axiom]` | Axiom environment | `@[Axiom](name="Euclid's Fifth")` ... |
| `@[Proposition]` | Proposition environment | `@[Proposition](name="Sum of n")` ... |
| `@[Align]` | Multi-line equation alignment | `@[Align](source="a &= b")` ... |
| `@[PMatrix]` | Matrix with parentheses | `@[PMatrix](source="...")` ... |
| `@[Cases]` | Piecewise function cases | `@[Cases](source="...")` ... |

### 7.2 Directive Syntax

Directives can be self-closing or have a body:

```vell
@[Figure](src="chart.png" caption="Quarterly Data")

@[Frame](title="Important") {
  Content inside the frame goes here.
}
```

### 7.3 Inline Components

Inline components appear within paragraph text:

```vell
The @[Slider](min=0 max=100) component lets users select a range.
```

### 7.4 Meta Directive

The `@[Meta]` directive sets document-level metadata:

```vell
@[Meta](title="My Document" author="Jane Doe" date="2026-01-15" lang=en)
```

**Supported properties:** `title`, `author`, `date`, `lang`

### 7.5 Cross-References

The `@[Ref]` directive creates a clickable cross-reference to a labeled equation or theorem environment.
Labels are attached via the `label` property on `@[Equation]` or theorem directives.

**Syntax:**

```vell
@[Equation](source="E = mc^2" label=e:mass-energy)

@[Theorem](name="Pythagoras" label=thm:pythagoras) {
  ...
}

See @[Ref](label=e:mass-energy) and @[Ref](label=thm:pythagoras) for details.
```

**How it works:**

1. A label is assigned to an equation or theorem using the `label` property (e.g., `label=e:mass-energy`).
2. The `@[Ref]` directive references that label elsewhere in the document.
3. Renderers resolve the reference by pre-computing equation and theorem numbers in a first pass.
4. The resolved output displays the target's auto-numbered label (e.g., "(1)" for equations, "Theorem 1" for theorems).
5. In HTML output, the reference is rendered as a clickable hyperlink (`<a>` tag) that jumps to the target.

**Properties:**

| Property | Type | Required | Description |
|----------|------|----------|-------------|
| `label`  | string | Yes | The label to resolve. Must match a `label` on an `@[Equation]` or theorem directive. |

**Label scoping:**

Labels must be unique within a document. If a referenced label does not exist, renderers display an error indicator such as `[?label]` with distinct styling (red italic text).

**Resolved display text:**

| Target Type | Display Text | Example |
|-------------|--------------|---------|
| `@[Equation]` with `label=e:mass-energy` | `(1)` — the auto-incremented equation number | link shows "(1)" |
| `@[Theorem]` with `label=thm:pythagoras` | `Theorem 1 (Pythagoras)` — name + number + optional extra name | link shows "Theorem 1 (Pythagoras)" |
| `@[Lemma]` with `label=lem:triangle` | `Lemma 1` — no extra name | link shows "Lemma 1" |
| `@[Proof]` with `label=proof:main` | `Proof` — unnumbered environments display just the name | link shows "Proof" |

**Complete example:**

```vell
== Numbered Equations

@[Equation](source="E = mc^2" label=e:mass-energy)

@[Equation](source="a^2 + b^2 = c^2" label=e:pythagoras)

== Theorem Environments

@[Theorem](name="Pythagoras" label=thm:pythagoras) {
  For any right triangle with legs *a*, *b* and hypotenuse *c*:

  $$
  a^2 + b^2 = c^2
  $$
}

== Cross-References

Reference equation @[Ref](label=e:mass-energy) (the mass-energy equivalence)
and equation @[Ref](label=e:pythagoras) (the Pythagorean theorem).

Theorem @[Ref](label=thm:pythagoras) proves the same relationship.
```

In the rendered output, `@[Ref](label=e:mass-energy)` displays as "(1)" (clickable),
`@[Ref](label=thm:pythagoras)` displays as "Theorem 1 (Pythagoras)" (clickable),
and clicking either link scrolls to the corresponding target.

**Implementation notes for renderers:**

Renderers must perform a two-pass rendering pipeline to support cross-references:

1. **First pass — label collection:** Walk the AST and collect all labels from `@[Equation]` and theorem directives, computing their auto-incremented numbers. Store the mapping `label → (anchor_id, display_text)`.
2. **Second pass — rendering:** When encountering a `@[Ref]` directive, look up the label in the collected map and render the appropriate link or display text.

The Rust, HTML, and PDF renderers all implement this pre-pass pattern.

### 7.6 Diagrams

The `@[Diagram]` directive renders visual diagrams with support for multiple diagram types.

**Syntax:**

```vell
@[Diagram](type=mermaid caption="Sequence diagram of a request") {
  sequenceDiagram
    participant Client
    participant Server
    Client->>Server: Request
    Server-->>Client: Response
}
```

**Properties:**

| Property | Type | Required | Default | Description |
|----------|------|----------|---------|-------------|
| `type` | string | No | `general` | Diagram type: `mermaid`, `ascii`, or `general` |
| `caption` | string | No | — | Optional caption displayed below the diagram |

**Diagram types:**

| Type | Description | Rendering |
|------|-------------|-----------|
| `mermaid` | [Mermaid.js](https://mermaid.js.org/) flowchart, sequence, or other diagram defined in source | Wrapped in `<div class="mermaid">` for client-side rendering by Mermaid.js |
| `ascii` | Plain ASCII/Unicode art | Rendered in a `<pre>` block with monospace font |
| `general` | Catch-all for any diagram format | Rendered in a `<pre>` block with monospace font |

**Body content:**

The body contains the raw diagram source text. The parser preserves the text content and passes it to the renderer without modification. For Mermaid diagrams, the body should contain valid Mermaid syntax. For ASCII art, the body should contain pre-formatted text.

**HTML output:**

- **Mermaid:** `<div class="vell-diagram" data-type="mermaid"><div class="mermaid">\n{source}\n</div></div>`
- **ASCII/General:** `<div class="vell-diagram" data-type="ascii"><pre>\n{source}\n</pre></div>`
- Both include an optional `<div class="diagram-caption">{caption}</div>` when a caption is provided.

**CSS classes:**

- `.vell-diagram` — outer container with border, background, and rounded corners
- `.vell-diagram[data-type="mermaid"]` — targets Mermaid-specific styling
- `.vell-diagram[data-type="ascii"]` — targets ASCII-specific styling
- `.mermaid` — Mermaid.js container (used by the Mermaid library for rendering)
- `.diagram-caption` — italic caption text below the diagram

**Example — Mermaid sequence diagram:**

```vell
== API Request Flow

@[Diagram](type=mermaid caption="Client-server request flow") {
  sequenceDiagram
    participant Client
    participant Server
    participant Database
    Client->>Server: POST /api/data
    Server->>Database: INSERT query
    Database-->>Server: Success
    Server-->>Client: 200 OK
}
```

**Example — ASCII art diagram:**

```vell
== Network Topology

@[Diagram](type=ascii caption="Simple network topology") {
  ----------            ----------
  | Client |---------->| Server |
  ----------            ----------
       |                     |
       v                     v
  ----------            ----------
  | Router |            | Switch |
  ----------            ----------
}
```

**Example — General diagram:**

```vell
@[Diagram](caption="Custom diagram format") {
  ################
  #  Start Node  #
  ################
        |
        v
  ################
  #  Process Node #
  ################
}
```

**Notes:**
- Body content should avoid characters that trigger Vell block-level parsing (e.g., avoid lines of three or more `-` characters alone, which are parsed as horizontal rules). Use indentation within the directive body where needed.
- Mermaid diagrams require the Mermaid.js library to be loaded separately in the HTML page for client-side rendering.
- The diagram source is HTML-escaped for safe insertion into the output.

### 7.7 Charts

The `@[Chart]` directive renders data visualizations as inline SVG charts.

**Syntax:**

```vell
@[Chart](type=bar title="Quarterly Revenue") {
  Q1, 45
  Q2, 62
  Q3, 38
  Q4, 71
}
```

**Properties:**

| Property | Type | Required | Default | Description |
|----------|------|----------|---------|-------------|
| `type` | string | No | `bar` | Chart type. Currently only `bar` is supported. |
| `title` | string | No | — | Title displayed above the chart |

**Data format:**

The body content contains one data point per line, with each line in the format:

```
{label}, {value}
```

Where:
- `{label}` — The category label (displayed below each bar)
- `{value}` — A numeric value (determines the bar height)

Lines are comma-separated. The last comma on each line is used as the delimiter,
so labels may contain commas (e.g., "New York, NY").

**Example — Simple bar chart:**

```vell
== Revenue by Quarter

@[Chart](type=bar title="Quarterly Revenue (USD)") {
  Q1, 45000
  Q2, 62000
  Q3, 38000
  Q4, 71000
}
```

**Example — Product comparison:**

```vell
== Product Sales

@[Chart](type=bar title="Sales by Category") {
  Electronics, 120
  Clothing, 85
  Food, 200
  Books, 45
  Sports, 68
}
```

**HTML output:**

When data is present and the type is `bar`:

```html
<div class="vell-chart vell-chart-bar">
  <svg width="500px" height="250px" viewBox="0 0 500 250" xmlns="http://www.w3.org/2000/svg">
    <text x="250" y="22" text-anchor="middle" font-size="14" font-weight="bold">Quarterly Revenue</text>
    <!-- Y-axis gridlines, tick labels -->
    <!-- X-axis line -->
    <!-- Colored bars with value labels on top and category labels below -->
  </svg>
</div>
```

When no data is present or the type is not `bar`, a fallback table is rendered:

```html
<div class="vell-chart">
  <div class="chart-title">Title</div>
  <table>
    <tr><td>Q1</td><td>45</td></tr>
    ...
  </table>
</div>
```

**SVG chart features:**

- Auto-scales to data values (y-axis with 5 evenly-spaced gridlines)
- Up to 8 distinct bar colors (cycling through a palette of blue, green, yellow, red, purple, teal, orange, dark blue)
- Rounded bar corners (`rx="2"`)
- Numeric value labels above each bar
- Category labels below each bar
- Optional title centered above the chart
- Responsive SVG viewBox with fixed aspect ratio

**CSS classes:**

- `.vell-chart` — outer container with overflow scroll for wide charts
- `.vell-chart-bar` — targets bar chart-specific styling
- `.chart-title` — centered bold title text
- SVG elements use inline attributes for colors, sizes, and positioning

**Empty data:**

If no valid data lines are found in the body, the chart renders as an empty SVG (no bars, no text). This is safe but produces no visual output.

### 7.8 Include

The `@[Include]` directive merges content from external Vell files into the current document.
It enables modular document authoring where large documents are split across multiple files.

**Syntax:**

```vell
@[Include](path="chapter2.vl")
@[Include](path="sections/introduction.vl")
@[Include](path="/absolute/path/to/appendix.vl")
```

**Properties:**

| Property | Type | Required | Description |
|----------|------|----------|-------------|
| `path` | string | Yes | Path to the Vell file to include. Can be relative to the including file or absolute. |

**How it works:**

The `@[Include]` directive is resolved **before parsing**, not during rendering. When you run
`vell render html`, `vell render pdf`, or `vell render slides`, the CLI:

1. Scans the source file line-by-line for `@[Include](path="...")` directives.
2. Reads the referenced file from disk.
3. Recursively resolves any `@[Include]` directives within that file.
4. Splices the resolved content into the source in place of the `@[Include]` line.
5. Parses the complete merged source as a single document.

**Path resolution:**

- **Relative paths** are resolved relative to the directory of the including file.
- **Absolute paths** (starting with `/` or a drive letter like `C:\`) are used as-is.
- If no base directory is available (e.g., reading from stdin), paths are resolved relative
to the current working directory.

**Circular include detection:**

The resolver detects circular includes (file A includes B which includes A) and returns an
error message identifying the cycle. This prevents infinite recursion.

**Example:**

Given a file `main.vl`:

```vell
= My Book

@[Include](path="chapter1.vl")
@[Include](path="chapter2.vl")

== Conclusion

Thanks for reading!
```

And a file `chapter1.vl`:

```vell
== Chapter 1: Beginnings

This is the first chapter of the book.
```

The rendered output will contain content from both files as if they were a single document,
with headings, references, and structure unified.

**Notes:**

- Included files are not separately parsed; they are merged at the text level before parsing.
- This means document-level metadata (`@[Meta]`) and variable declarations (`@var`) from
  included files affect the entire merged document.
- Include resolution is a CLI-level feature; the parser itself treats `@[Include]` as a
  generic directive if includes are not pre-resolved.
- The path value must be quoted with double quotes (`path="file.vl"`). Single quotes or
  bare paths are not supported.

### 7.9 Slides

The `@[Slide]` directive defines a single slide in a presentation deck. Multiple `@[Slide]`
directives together form a complete slide show.

**Syntax:**

```vell
@[Slide] {
  == Slide Title

  Content for this slide.
  - Bullet point
  - Another point
}

@[Slide] {
  === Code Slide

  ```python
  print("Hello, Vell!")
  ```
}
```

**Properties:**

The `@[Slide]` directive has no required properties. It uses only its body content for
the slide contents.

| Property | Type | Required | Description |
|----------|------|----------|-------------|
| `id` | string | No | Optional identifier for the slide, used for anchor linking |

**Rendering targets:**

| Target | Output |
|--------|--------|
| `vell render html` | Each `@[Slide]` becomes a `<section class="vell-slide">` element. In print mode, slides are hidden. |
| `vell render pdf` | Slides are hidden in the PDF-friendly output (slide content is redundant in print). |
| `vell render slides` | Each `@[Slide]` becomes a `<section>` within a reveal.js slide deck. Content before the first `@[Slide]` becomes the title slide. |

**Slide deck rendering (reveal.js):**

When using `vell render slides`, the output is a self-contained HTML file that loads
reveal.js from CDN. The slide deck includes:

- Slide navigation (arrow keys or on-screen controls)
- Slide number indicator
- Progress bar
- Hash-based slide URLs for direct linking
- Smooth slide transitions

Content outside any `@[Slide]` directive (such as the document title and introductory
paragraphs) is automatically wrapped into a title slide. This makes it seamless to
convert any document into a presentation.

**Example — Complete presentation:**

```vell
= My Presentation

A short overview of our project.

@[Slide] {
  == Agenda

  1. Introduction
  2. Methodology
  3. Results
  4. Conclusion
}

@[Slide] {
  == Results

  The data shows a significant improvement.

  @[Chart](type=bar title="Key Metrics") {
    Before, 45
    After, 92
  }
}

@[Slide] {
  == Conclusion

  Thank you! Questions?
}
```

### 7.10 Table of Contents

Vell generates a Table of Contents (TOC) automatically from document headings when using
the PDF render target. The TOC is generated during rendering, not from a directive.

**How it works:**

When rendering with `vell render pdf`, the renderer:

1. Scans all headings in the document.
2. Collects each heading's level, text content, and anchor ID.
3. Generates a navigable TOC at the beginning of the document, before the main content.
4. Inserts a page break after the TOC so it appears on its own page(s).

**TOC structure:**

The TOC is an indented list where indentation reflects heading level:

```html
<nav class="toc" role="toc">
  <h1>Table of Contents</h1>
  <a href="#introduction">Introduction</a><br>
    <a href="#background">  Background</a><br>
    <a href="#methodology">  Methodology</a><br>
  <a href="#results">Results</a><br>
    <a href="#analysis">  Analysis</a><br>
</nav>
```

**TOC features:**

- **Auto-generated** — No manual TOC maintenance needed. The TOC always reflects the
  current heading structure.
- **Clickable links** — Each TOC entry links to its corresponding heading anchor in
  the document, enabling navigation in PDF viewers that support hyperlinks.
- **Level-aware indentation** — Level-2 headings are indented under level-1, level-3
  under level-2, and so on.
- **Page-break separation** — The TOC always starts on its own page, followed by a
  page break before the main content.

**TOC in other formats:**

| Format | TOC Support |
|--------|-------------|
| `vell render html` | Standard HTML rendering includes inline heading anchors. TOC generation is available in the PDF variant. |
| `vell render pdf` | Full TOC at document start with indentation and links. |
| `vell render slides` | No TOC; slides use the reveal.js slide overview feature instead. |

---

## 8. Extensions

Extensions are namespaced directives that follow the `@[org/Name]` pattern. They are preserved as `Extension` nodes in the AST so that unsupported renderers can still provide safe output.

```vell
@[myorg/Widget](data=42) {
  Custom widget content.
}
```

**Rules:**
- Extension names must contain a `/` separator.
- Unknown extensions are never rejected by the parser.
- Extensions with braced bodies have their content parsed as nested Vell documents.

---

## 9. Escape Sequences

A backslash before an inline delimiter character produces the literal character instead of starting markup:

```vell
This \*is not bold\* and this \/is not italic\/.
This \_is not underlined\_ and this \~is not struck\~.
```

**Supported escape characters:**
- `\*` — literal asterisk (not bold)
- `\/` — literal slash (not italic)
- `\_` — literal underscore (not underline)
- `\~` — literal tilde (not strikethrough)
- `\^` — literal caret (not superscript)
- `\,` — literal comma (not subscript start)
- `` \` `` — literal backtick (not inline code)
- `\$` — literal dollar sign (not inline math)
- `\\` — literal backslash

---

## 10. Document Metadata

Document metadata is extracted from the first level-1 heading (title) and `@[Meta]` directives. The metadata is stored in the `metadata` field of the AST:

```json
{
  "metadata": {
    "title": "My Document",
    "author": "Jane Doe",
    "date": "2026-01-15",
    "lang": "en",
    "variables": {}
  }
}
```

---

## 11. Variable System Reference

### Declaration Rules

- Variable names must start with a letter or underscore and contain only alphanumeric characters and underscores.
- Values are parsed as JSON when possible; strings without quotes are accepted.
- Variables are ordered by declaration and can be shadowed by later declarations.
- The parser collects all variables into the document's metadata for runtime use.

### Scope

Variables declared with `@var` have document-level scope. They are available for interpolation after their declaration point. For-loops create a loop variable scoped to the loop body.

### Type Support

| JSON Type | Vell Declaration | Example |
|-----------|-----------------|---------|
| String | `@var x = "hello"` or `@var x = hello` | `@var name = "Vell"` |
| Number | `@var x = 42` or `@var x = 3.14` | `@var pi = 3.14159` |
| Boolean | `@var x = true` | `@var debug = true` |
| Null | `@var x = null` | `@var data = null` |
| Array | `@var x = [1, 2, 3]` | `@var items = ["a", "b"]` |
| Object | `@var x = {"k": "v"}` | `@var meta = {"version": 1}` |

---

## 12. Property Values

Directive properties support multiple value types:

- **Strings:** `name="value"` or `name=bareword`
- **Numbers:** `count=42`, `pi=3.14`
- **Booleans:** `enabled=true`, `visible=false`
- **Variables:** `data=@{var_name}`
- **Null:** `optional=null`

Multiple properties are space-separated:

```vell
@[Figure](src="chart.png" width=800 height=600 caption="Data")
```

---

## 13. Complete Example

The following example demonstrates most of Vell's features in a single document:

```vell
@[Meta](title="Research Note" author="Samin Yeasar" date="2026-01-15" lang=en)

= A Complete Vell Document

@var sample = 42
@var items = ["alpha", "beta", "gamma"]

== Introduction

This document showcases Vell's capabilities. It has *bold* text,
/italic/ text, `inline code`, and $E = mc^2$ inline math.

=== Features

- Deterministic parsing via PEG grammar
- Versioned, portable AST
- Variables and reactivity: @{sample}
- Extensible directive system

=== Data Table

| Metric | Value | Status |
|--------|-------|--------|
| Alpha  | 100   | ✅     |
| Beta   | 200   | ✅     |
| Gamma  | 300   | ❌     |

=== Math

$$
\int_{-\infty}^{\infty} e^{-x^2} \, dx = \sqrt{\pi}
$$

=== Code Example

```rust
fn greet(name: &str) -> String {
    format!("Hello, {name}!")
}
```

=== Blockquote and Footnotes

> This is an important observation that deserves
> to be highlighted for the reader[^note].

[^note]: This footnote provides additional context.

=== Dynamic Content

@for item in @{items} {
  - Current item: @{item}
}

@if @{sample} > 0 {
  The sample value @{sample} is positive.
}

> [!TIP]
> Vell combines the readability of Markdown with the
> expressiveness of LaTeX — and adds reactivity on top.
```

---

## 14. CLI Reference

The Vell CLI provides several subcommands for parsing, formatting, rendering, and validating
Vell documents. All commands accept input from a file or stdin.

### 14.1 `vell render html`

Renders a Vell document to standard HTML5 output. Includes CSS styling, MathML for math,
cross-reference resolution, footnotes, and diagram/chart rendering.

**Usage:**

```bash
vell render html input.vl                # prints to stdout
vell render html input.vl -o output.html  # writes to file
vell render html < input.vl               # reads from stdin
```

**Features:**

- Full document structure (headings, paragraphs, lists, tables, blockquotes)
- Math rendering via MathML (LaTeX input → MathML output)
- Cross-reference resolution for labeled equations and theorems
- Diagram rendering (Mermaid, ASCII, general)
- SVG bar chart rendering from chart data
- Footnote collection and end-of-document section
- URL sanitization for safe links
- Variable interpolation with reactive `data-vell-var` attributes
- Interactive form directives (Input, Select, Checkbox, Slider)
- `@[Include]` directive resolution (pre-processing file merging)

### 14.2 `vell render pdf`

Renders a Vell document to PDF-friendly HTML optimized for printing or PDF conversion.
The output includes print-specific CSS features.

**Usage:**

```bash
vell render pdf input.vl                  # prints to stdout
vell render pdf input.vl -o output.html   # writes PDF-friendly HTML file
vell render pdf < input.vl                # reads from stdin
```

**PDF-specific features:**

- **Auto-generated Table of Contents** — Scans all headings in the document and generates
  an indented, hyperlinked TOC at the document start, followed by a page break.
- **Print-optimized CSS** — The `@media print` rules include:
  - A4 page margins (2.54cm / 1 inch)
  - Page breaks before each `h1` and `h2` heading
  - `page-break-inside: avoid` on tables, code blocks, blockquotes, images, and equations
  - Running page header with document title (`@top-center`)
  - Running page footer with page numbers (`@bottom-center`)
  - URL display after external links (`content: attr(href)`)
  - Hidden slide elements (`.vell-slide` set to `display: none`)
- **Running page header** — Document title displayed at the top of every printed page
- **Page numbering** — Automatic page numbers at the bottom of every page
- **TOC page-break** — Separate page for the table of contents before document body

**Converting to PDF:**

The output HTML can be converted to PDF using any browser's "Print to PDF" feature or
command-line tools like `wkhtmltopdf` or headless Chromium:

```bash
vell render pdf input.vl -o output.html
# Then open output.html in a browser and use Print → Save as PDF
# Or use headless Chrome:
# google-chrome --headless --print-to-pdf=output.pdf output.html
```

### 14.3 `vell render slides`

Renders a Vell document to a self-contained reveal.js slide deck HTML file.

**Usage:**

```bash
vell render slides input.vl                  # prints to stdout
vell render slides input.vl -o output.html   # writes slide deck HTML
vell render slides < input.vl                # reads from stdin
```

**Slide deck features:**

- **reveal.js integration** — Outputs a standalone HTML file that loads reveal.js from CDN
  (no local dependencies required). Each `@[Slide]` directive becomes a `<section>` element.
- **Title slide** — Content before the first `@[Slide]` directive is automatically wrapped
  into the initial slide, making it easy to add introductory material.
- **Navigation controls** — Arrow keys, on-screen navigation buttons, and swipe gestures
- **Slide number indicator** — Current slide / total slides displayed in the corner
- **Progress bar** — Visual progress indicator at the bottom of the screen
- **Hash-based URLs** — Each slide gets a unique URL hash for direct linking
- **Smooth transitions** — Default slide transition with configurable effects
- **Obscured by default** — In standard HTML output, slides render as `<section class="vell-slide">`
  which is hidden by print CSS (`.vell-slide { display: none; }` in `@media print`)

**If no `@[Slide]` directives exist:**

The entire document content is wrapped in a single slide, making any Vell document
immediately presentable.

**Customization:**

The generated HTML links to the default reveal.js white theme. To customize the appearance,
modify the theme URL in the output or apply custom CSS via `@[Theme]` directives.

### 14.4 `vell parse`

Parses a Vell document and outputs the AST as JSON. Useful for debugging and for tools
that need to inspect the document structure.

**Usage:**

```bash
vell parse input.vl       # prints AST JSON to stdout
vell parse < input.vl     # reads from stdin
```

### 14.5 `vell fmt`

Formats a Vell document to canonical style. The formatter is idempotent (formatting
twice produces the same output as formatting once).

**Usage:**

```bash
vell fmt input.vl              # prints formatted source to stdout
vell fmt input.vl --check       # checks if file is correctly formatted (exit code)
vell fmt < input.vl             # reads from stdin
```

**Formatting rules:**

- Consistent `=` markers for headings with single space separator
- Normalized list markers (`-` for unordered, `1.` for ordered)
- Canonical property ordering in directives
- No trailing whitespace
- Single blank line between top-level blocks
- Maximum line length of 100 characters for prose (soft wrap)

### 14.6 `vell validate`

Validates a Vell document and prints diagnostics (errors and warnings) to stderr.
Returns a non-zero exit code if any errors are found.

**Usage:**

```bash
vell validate input.vl       # validates file, prints diagnostics
vell validate < input.vl     # reads from stdin
```

**Diagnostics include:**

- Parse errors (malformed syntax, unterminated delimiters, invalid indentation)
- Warnings for undefined variable references (variables may be provided at runtime)
- Malformed table and directive structure warnings

---

> **License:** AGPL-3.0-or-later
> **Author:** Samin Yeasar
> **Version:** 1.0
> **AST Schema:** v1
