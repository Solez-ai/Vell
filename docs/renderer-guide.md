# Vell Renderer Guide

> A comprehensive guide to building renderers for the Vell AST
>
> Vell is created and maintained by **Samin Yeasar**

---

## Table of Contents

1. [Introduction](#1-introduction)
2. [Renderer Architecture](#2-renderer-architecture)
3. [Node Dispatch Strategy](#3-node-dispatch-strategy)
4. [Block Node Rendering](#4-block-node-rendering)
   - 4.1 [Document](#41-document)
   - 4.2 [Heading](#42-heading)
   - 4.3 [Paragraph](#43-paragraph)
   - 4.4 [Blockquote and Admonitions](#44-blockquote-and-admonitions)
   - 4.5 [CodeBlock](#45-codeblock)
   - 4.6 [MathBlock](#46-mathblock)
   - 4.7 [List](#47-list)
   - 4.8 [Table](#48-table)
   - 4.9 [HorizontalRule](#49-horizontalrule)
   - 4.10 [DefinitionList](#410-definitionlist)
   - 4.11 [ReferenceDefinition](#411-referencedefinition)
   - 4.12 [FootnoteDefinition](#412-footnotedefinition)
   - 4.13 [VarDeclaration](#413-vardeclaration)
   - 4.14 [ForLoop and IfBlock](#414-forloop-and-ifblock)
   - 4.15 [Directive](#415-directive)
   - 4.16 [Extension](#416-extension)
5. [Inline Node Rendering](#5-inline-node-rendering)
   - 5.1 [Text](#51-text)
   - 5.2 [Bold](#52-bold)
   - 5.3 [Italic](#53-italic)
   - 5.4 [Underline](#54-underline)
   - 5.5 [Strikethrough](#55-strikethrough)
   - 5.6 [Code](#56-code)
   - 5.7 [Superscript and Subscript](#57-superscript-and-subscript)
   - 5.8 [Link and LinkRef](#58-link-and-linkref)
   - 5.9 [Image and ImageRef](#59-image-and-imageref)
   - 5.10 [MathInline](#510-mathinline)
   - 5.11 [VarInterpolation](#511-varinterpolation)
   - 5.12 [InlineComponent](#512-inlinecomponent)
   - 5.13 [Citation](#513-citation)
   - 5.14 [FootnoteRef](#514-footnoteref)
   - 5.15 [SoftBreak and HardBreak](#515-softbreak-and-hardbreak)
6. [Safety Requirements](#6-safety-requirements)
   - 6.1 [HTML Escaping](#61-html-escaping)
   - 6.2 [URL Sanitization](#62-url-sanitization)
   - 6.3 [Cross-Site Scripting Prevention](#63-cross-site-scripting-prevention)
7. [Footnote Handling](#7-footnote-handling)
8. [Extension Fallback](#8-extension-fallback)
9. [HTML Renderer Reference](#9-html-renderer-reference)
10. [PDF Renderer Reference](#10-pdf-renderer-reference)
11. [Testing Your Renderer](#11-testing-your-renderer)
12. [Accessibility](#12-accessibility)
13. [Performance Optimization](#13-performance-optimization)
14. [CSS Examples](#14-css-examples)

---

## 1. Introduction

A Vell renderer consumes the AST produced by the Vell parser and produces output in a target format (HTML, PDF, slides, etc.). This guide covers everything you need to build a conformant Vell renderer.

### Core Principles

1. **Never parse source text.** Always work from the AST. The parser's job is to produce the AST; the renderer's job is to consume it.
2. **Always escape user-supplied strings.** Every `Text`, `value`, `source`, `href`, `src`, `alt`, `title`, and other user-provided string must be escaped for the target output format.
3. **Always provide fallback behavior.** Unknown or unsupported node types should produce safe, neutral output — never crash.
4. **Be forward-compatible.** New node types may be added in future AST versions. Renderers should tolerate unknown node types by rendering their children or a placeholder.

### The Renderer Contract

```
Input:  VellDocument (parsed AST JSON)
Output: Target format (HTML string, PDF bytes, etc.)
```

A renderer:
- Receives a complete, validated AST
- Walks the `children` arrays recursively
- Dispatches on the `type` field of each node
- Produces format-specific output
- Never modifies the AST

---

## 2. Renderer Architecture

All Vell renderers follow the same architectural pattern:

```
render(document: VellDocument) -> Output
  render_block(node: Node) -> void
  render_inline(nodes: InlineNode[]) -> string
  render_inline_node(node: InlineNode) -> string
```

### Two-Pass Rendering

Some renderers benefit from a two-pass approach:

1. **First pass:** Collect metadata, footnotes, reference definitions, and cross-reference targets.
2. **Second pass:** Render all blocks with the collected data available.

#### Cross-Reference Resolution

The `@[Ref]` directive creates clickable links to labeled equations and theorem environments. Renderers must implement the following pre-pass to resolve cross-references:

**Pre-pass — label collection:**

1. Walk every `Directive` node in the AST before rendering.
2. For each `@[Equation]` directive, increment an equation counter. If the directive has a `label` property, store `label → { anchor_id: "eq-{label}", display_text: "({N})" }` where `N` is the equation number.
3. For each theorem directive (`Theorem`, `Lemma`, `Corollary`, `Definition`, `Axiom`, `Conjecture`, `Proposition`, `Proof`, `Remark`, `Example`, `Notation`), increment a per-type counter for numbered environments. Skip auto-numbering for `Proof`, `Remark`, `Example`, and `Notation`. If the directive has a `label` property, store `label → { anchor_id: "thm-{label}", display_text: "{Name} {N} ({Extra})" }` where `{N}` is the counter and `{Extra}` is the optional `name` property.

**Render pass:**

When rendering a `@[Ref]` directive (either as a block-level `Directive` node or an inline `InlineComponent` with `name="Ref"`):

1. Read the `label` property.
2. Look up the label in the pre-collected map.
3. If found, render an anchor link:
   - **HTML:** `<a href="#eq-{label}" class="vell-ref">{display_text}</a>`
   - **PDF:** Blue underlined text with the display string
4. If not found, render an error indicator:
   - **HTML:** `<span class="unresolved-ref">[?{label}]</span>`
   - **PDF:** Red italic text `[?{label}]`

**Anchor ID format:**
| Target | Format | Example |
|--------|--------|--------|
| `@[Equation]` with `label=e:mass-energy` | `#eq-{label}` | `#eq-e:mass-energy` |
| `@[Theorem]` with `label=thm:pythagoras` | `#thm-{label}` | `#thm-thm:pythagoras` |

**Display text format:**
| Target | Display Text |
|--------|-------------|
| Equation (first in document) | `(1)` |
| Theorem (numbered, no extra name) | `Theorem 1` |
| Theorem (numbered, with `name="Pythagoras"`) | `Theorem 1 (Pythagoras)` |
| Proof (unnumbered, no extra name) | `Proof` |
| Lemma (numbered, with `name="Triangle Inequality"`) | `Lemma 1 (Triangle Inequality)` |

**CSS classes for HTML renderers:**
- `.vell-ref` — resolved cross-reference link (color: `#2b6cb0`, no underline, underline on hover)
- `.unresolved-ref` — unresolved label indicator (color: `#e53e3e`, italic)

**Implementation example (pseudocode):**

```typescript
// First pass: collect labels
const labels = new Map<string, { anchorId: string; displayText: string }>();
let eqCounter = 0;
const thmCounters: Record<string, number> = {};

function collectLabels(node: Node): void {
  if (node.type === "Directive") {
    if (node.name === "Equation") {
      eqCounter++;
      if (node.props?.label) {
        labels.set(node.props.label, {
          anchorId: `eq-${node.props.label}`,
          displayText: `(${eqCounter})`,
        });
      }
    }
    if (THEOREM_NAMES.includes(node.name)) {
      // ... per-type counter logic ...
      if (node.props?.label) {
        labels.set(node.props.label, {
          anchorId: `thm-${node.props.label}`,
          displayText: `${node.name} ${num}${extra ? ` (${extra})` : ""}`,
        });
      }
    }
  }
  for (const child of node.children ?? []) {
    collectLabels(child);
  }
}

// Second pass: render Ref directive
function renderRef(props: Record<string, unknown>): string {
  const label = String(props.label ?? "");
  const target = labels.get(label);
  if (target) {
    return `<a href="#${target.anchorId}" class="vell-ref">${escapeHtml(target.displayText)}</a>`;
  }
  return `<span class="unresolved-ref">[?${escapeHtml(label)}]</span>`;
}
```

**Important notes:**
- The equation counter used during label collection must match the one used during rendering. Implementations should use a mutable counter passed by reference (Rust's `&mut u32`) or return the updated counter from recursive calls (TypeScript returning `number`).
- Labels can reference elements declared later in the document — order independence is guaranteed by the pre-pass.
- The `@[Ref]` directive works both as a block-level directive (`@[Ref](label=...)` on its own line) and as an inline component within paragraph text.

### State Management

Renderers typically maintain state for:
- **Cursor position** (in page-based renderers like PDF)
- **Footnote accumulator** (collected inline refs, rendered at end)
- **Reference table** (link references resolved to URLs)

---

## 3. Node Dispatch Strategy

The standard dispatch pattern switches on the `type` field:

```typescript
function renderBlock(node: Node): void {
  switch (node.type) {
    case "Heading":
      return renderHeading(node);
    case "Paragraph":
      return renderParagraph(node);
    case "Blockquote":
      return renderBlockquote(node);
    case "CodeBlock":
      return renderCodeBlock(node);
    // ... all other types ...
    default:
      return renderFallback(node);
  }
}
```

### Fallback Behavior

For unknown node types:
1. If the node has `children`, render them recursively.
2. If the node has a `name`, render a neutral placeholder `[type name]`.
3. Otherwise, skip the node silently.

---

## 4. Block Node Rendering

### 4.1 Document

The document root wraps all children. In HTML:

```html
<!DOCTYPE html>
<html lang="en">
<head><meta charset="UTF-8"><title>{title}</title></head>
<body>
  {rendered children}
</body>
</html>
```

In PDF, the document root sets up the page layout (A4, margins, fonts).

### 4.2 Heading

Headings render with appropriate heading level tags.

- **HTML:** `<h1>` through `<h6>` with `id` attribute
- **PDF:** Bold font with size depending on level (22pt for h1, 16pt for h2, 13pt for h3, 11pt for h4)

The `id` field should be used as the anchor identifier for linking.

### 4.3 Paragraph

Paragraphs render inline children as a prose block.

- **HTML:** `<p>` tag
- **PDF:** Body font (10pt Helvetica) with inline styling

Paragraphs should be separated from adjacent blocks by appropriate vertical spacing.

### 4.4 Blockquote and Admonitions

Blockquotes render with visual indentation or a left border.

- **HTML:** `<blockquote>` tag, or `<div class="admonition">` for admonitions
- **PDF:** Left vertical bar (2px) with 12pt indent offset

Admonition types (`NOTE`, `TIP`, `WARNING`, `IMPORTANT`, `CAUTION`) should receive distinct visual treatment (colors, icons, or labels).

**Admonition CSS styling example:**
```css
.admonition { border-left: 4px solid; padding: 1em; margin: 1em 0; border-radius: 4px; }
.admonition[data-type="NOTE"] { background: #ebf8ff; border-color: #3182ce; }
.admonition[data-type="WARNING"] { background: #fffaf0; border-color: #dd6b20; }
.admonition[data-type="TIP"] { background: #f0fff4; border-color: #38a169; }
.admonition[data-type="IMPORTANT"] { background: #fefcbf; border-color: #d69e2e; }
.admonition[data-type="CAUTION"] { background: #fff5f5; border-color: #e53e3e; }
```

### 4.5 CodeBlock

Code blocks render with monospace font and syntax highlighting support.

- **HTML:** `<pre><code class="language-{lang}">` with escaped content
- **PDF:** Courier font, 8pt, gray background rectangle

The `lang` field should be used for syntax highlighting language selection. The `source` field contains the raw code content and must be escaped.

### 4.6 MathBlock

Math blocks contain raw LaTeX. Renderers should convert to the output format's math representation.

- **HTML:** `<math display="block">` with MathML conversion
- **PDF:** Courier font with light gray background, or MathML-to-PDF rendering

### 4.7 List

Lists render with appropriate list markers.

- **HTML:** `<ul>` for unordered, `<ol>` for ordered (with `start` attribute)
- **PDF:** "• " bullet for unordered, numbered for ordered, with 14pt indent

Task list items with `checked` set should render checkbox indicators.

```html
<!-- Task list rendering -->
<ul class="task-list">
  <li class="task-list-item">
    <input type="checkbox" checked disabled> Completed task
  </li>
  <li class="task-list-item">
    <input type="checkbox" disabled> Pending task
  </li>
</ul>
```

### 4.8 Table

Tables render with header rows and body rows.

- **HTML:** `<table><thead><tr>` for headers, `<tbody><tr>` for rows
- **PDF:** Manual cell drawing with `colspan`, `rowspan`, and `align`

Cell `align` property controls text alignment (left, center, right).

### 4.9 HorizontalRule

Renders as a visual divider.

- **HTML:** `<hr>`
- **PDF:** Horizontal line across content width

### 4.10 DefinitionList

Renders term-definition pairs.

- **HTML:** `<dl><dt>` for term, `<dd>` for definition
- **PDF:** Bold term followed by indented definition text

### 4.11 ReferenceDefinition

**Not rendered in output.** These are metadata used to resolve reference-style links. Collected in the first pass and applied when rendering `LinkRef` and `ImageRef` nodes.

### 4.12 FootnoteDefinition

**Not rendered inline.** Collected during the first pass and rendered in a special footnotes section at the end of the document.

### 4.13 VarDeclaration

**Not rendered in output.** Variables are metadata processed by the runtime system.

### 4.14 ForLoop and IfBlock

For loops and if blocks are interactive constructs that require runtime evaluation of variables.

- **Current renderers:** Render a fallback placeholder `[type: see interactive viewer]`
- **Future:** Runtime environments can evaluate these blocks when variable values are provided

### 4.15 Directive

Built-in directives should render according to their purpose:

| Directive | HTML Rendering |
|-----------|---------------|
| `@[Figure]` | `<figure><img><figcaption>` |
| `@[Code]` | `<pre><code>` |
| `@[Diagram]` | Diagram with type-specific rendering (mermaid, ascii, general) |
| `@[Chart]` | Bar chart rendered as inline SVG with auto-scaled axes |
| `@[Cite]` | Formatted citation markup |
| `@[Slide]` | Presentation slide wrapper |
| `@[Frame]` | `<div class="frame">` with border |
| `@[Layout]` | CSS grid or multi-column layout |
| `@[Column]` | Grid column within a Layout |
| `@[Accessibility]` | ARIA attributes on parent element |
| `@[Theme]` | CSS class or stylesheet selection |
| `@[Slider]` | Interactive range input |
| `@[Meta]` | Metadata only, not rendered |
| `@[Equation]` | Numbered equation with MathML and auto-incrementing counter |
| `@[Ref]` | Cross-reference link to labeled equation or theorem |
| `@[Theorem]` | Theorem block with auto-numbering and colored left border |
| `@[Proof]` | Proof block (no auto-number) |
| `@[Lemma]` | Lemma block with auto-numbering |
| `@[Corollary]` | Corollary block with auto-numbering |
| `@[Definition]` | Definition block with auto-numbering |
| `@[Remark]` | Remark block (no auto-number) |
| `@[Example]` | Example block (no auto-number) |
| `@[Conjecture]` | Conjecture block with auto-numbering |
| `@[Axiom]` | Axiom block with auto-numbering |
| `@[Proposition]` | Proposition block with auto-numbering |
| `@[Align]` | Multi-line equation alignment via MathML |
| `@[PMatrix]` | Matrix with parentheses via MathML |
| `@[Cases]` | Piecewise function cases via MathML |
| `@[Template]` | CSS stylesheet link or inline style block |

### 4.16 Extension

Namespaced directives that are not recognized as built-in. Renderers should:

1. Render the extension name as a muted label (e.g., `[Extension: org/Widget]`)
2. Recursively render any child content
3. Never reject or crash on unknown extensions

```html
<div class="vell-extension" data-extension-name="org/Widget">
  <span class="vell-extension-label">[Widget]</span>
  <!-- rendered children -->
</div>
```

---

## 5. Inline Node Rendering

### 5.1 Text

| Node | HTML | PDF |
|------|------|-----|
| `Text { value }` | `escapeHtml(value)` | `Helvetica.font(value)` |

### 5.2 Bold

| Node | HTML | PDF |
|------|------|-----|
| `Bold { children }` | `<strong>{children}</strong>` | `Helvetica-Bold.font(children)` |

### 5.3 Italic

| Node | HTML | PDF |
|------|------|-----|
| `Italic { children }` | `<em>{children}</em>` | `Helvetica-Oblique.font(children)` |

### 5.4 Underline

| Node | HTML | PDF |
|------|------|-----|
| `Underline { children }` | `<u>{children}</u>` | `text(children, { underline: true })` |

### 5.5 Strikethrough

| Node | HTML | PDF |
|------|------|-----|
| `Strikethrough { children }` | `<s>{children}</s>` | `text(children, { strike: true })` |

### 5.6 Code

| Node | HTML | PDF |
|------|------|-----|
| `Code { value }` | `<code>{escapeHtml(value)}</code>` | `Courier.font(value)` |

### 5.7 Superscript and Subscript

| Node | HTML | PDF |
|------|------|-----|
| `Superscript { children }` | `<sup>{children}</sup>` | `^text^` marker or true superscript |
| `Subscript { children }` | `<sub>{children}</sub>` | `,text,` marker or true subscript |

### 5.8 Link and LinkRef

| Node | HTML | PDF |
|------|------|-----|
| `Link { href, title, children }` | `<a href="{sanitizeUrl(href)}">{children}</a>` | `children (href)` |
| `LinkRef { id, children }` | Resolve `id` against reference definitions | Same as Link |

**URL sanitization must reject:**
- `javascript:` URLs
- `data:` URLs (except safe image types)
- Any URL that could execute code

### 5.9 Image and ImageRef

| Node | HTML | PDF |
|------|------|-----|
| `Image { src, alt, title }` | `<img src="{sanitizeUrl(src)}" alt="{escapeHtml(alt)}">` | `[Image: alt]` text |
| `ImageRef { id, alt }` | Resolve `id` against reference definitions | Same as Image |

### 5.10 MathInline

| Node | HTML | PDF |
|------|------|-----|
| `MathInline { source }` | `<math display="inline">{latexToMathml(source)}</math>` | `$source$` text |

### 5.11 VarInterpolation

| Node | HTML | PDF |
|------|------|-----|
| `VarInterpolation { name }` | `<span class="vell-var">@{name}</span>` | `@{name}` text |

### 5.12 InlineComponent

| Node | HTML | PDF |
|------|------|-----|
| `InlineComponent { name, props }` | `<span class="vell-component">{name}</span>` | `@[name]` text |

### 5.13 Citation

| Node | HTML | PDF |
|------|------|-----|
| `Citation { key }` | `<span class="vell-citation">[[key]]</span>` | `[[key]]` text |

### 5.14 FootnoteRef

| Node | HTML | PDF |
|------|------|-----|
| `FootnoteRef { marker }` | `<sup><a href="#fn-{marker}">{marker}</a></sup>` | `[^marker]` text |

### 5.15 SoftBreak and HardBreak

| Node | HTML | PDF |
|------|------|-----|
| `SoftBreak` | `\n` (renders as space) | ` ` space |
| `HardBreak` | `<br>` | `\n` newline |

---

## 6. Safety Requirements

### 6.1 HTML Escaping

All user-supplied string content must be HTML-escaped:

```typescript
function escapeHtml(text: string): string {
  return text
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#39;");
}
```

**Fields that MUST be escaped:**
- `Text.value`
- `Code.value`
- `CodeBlock.source`
- `MathBlock.source`
- `MathInline.source`
- `VarInterpolation.name`
- All `Image.alt`, `Link.title`
- Directive property string values

### 6.2 URL Sanitization

URLs from the AST must be sanitized before being used in HTML attributes:

```typescript
function sanitizeUrl(url: string): string {
  const allowed = ["http:", "https:", "mailto:", "/", "#"];
  for (const prefix of allowed) {
    if (url.toLowerCase().startsWith(prefix)) return url;
  }
  return ""; // Reject unsafe URLs
}
```

**URLs that MUST be rejected:**
- `javascript:` — XSS vector
- `data:text/html` — XSS vector
- `vbscript:` — IE-specific XSS vector
- `file:` — Local file access

### 6.3 Cross-Site Scripting Prevention

Additional safety measures:

1. Never inject raw HTML from the AST into the output
2. Always use safe DOM APIs (not `innerHTML` with untrusted content)
3. Set `Content-Security-Policy` headers when serving rendered HTML
4. Use `rel="noopener noreferrer"` on external links

---

## 7. Footnote Handling

Footnotes require a two-pass approach:

**First pass:** Scan the document for `FootnoteDefinition` nodes. Store them in a lookup table keyed by `marker`.

**Second pass:** When rendering a `FootnoteRef` inline node, emit a numbered reference link. At the end of the document, render all collected footnote definitions in a dedicated footnotes section.

```html
<!-- Footnote reference in body text -->
<sup><a href="#fn-1" id="fnref-1">1</a></sup>

<!-- Footnotes section at end -->
<section class="footnotes">
  <hr>
  <ol>
    <li id="fn-1">Footnote content. <a href="#fnref-1">↩</a></li>
  </ol>
</section>
```

---

## 8. Extension Fallback

When a renderer encounters an `Extension` node:

1. **Render the label.** Show the extension name in muted styling so users know the content exists but may not be rendered fully.
2. **Render children.** If the extension has child content, render it recursively.
3. **Never crash.** Unsupported extensions should never cause the renderer to fail.

```html
<div class="vell-extension" data-extension="org/Widget">
  <span class="vell-extension-label">[org/Widget extension]</span>
  <!-- Fallback: render children if available -->
</div>
```

---

## 9. HTML Renderer Reference

The reference HTML renderer is at `packages/vell-renderer-html/src/index.ts`.

### Key implementation details:

- **Doctype:** `<!DOCTYPE html>` at document start
- **Language:** `<html lang="{metadata.lang}">` when available
- **Title:** `<title>{metadata.title}</title>` when available
- **Body structure:** All block nodes rendered as HTML5 semantic elements
- **Math:** LaTeX-to-MathML converter with support for common commands
- **Footnotes:** Collected first, rendered as ordered list at document end
- **URL safety:** Rejects `javascript:` and unsafe `data:` URLs
- **Escaping:** All user strings through `escapeHtml`

### MathML Converter Support

The HTML renderer includes a LaTeX-to-MathML converter that handles:
- Superscripts (`^`)
- Subscripts (`_`)
- Fractions (`\frac`)
- Square roots (`\sqrt`)
- Greek letters: `\alpha`, `\beta`, `\gamma`, `\delta`, `\epsilon`, `\theta`, `\lambda`, `\mu`, `\pi`, `\sigma`, `\omega`
- Integrals (`\int`), sums (`\sum`), products (`\prod`)
- Infinity (`\infty`), partial derivatives (`\partial`)
- Arrows: `\rightarrow`, `\Rightarrow`
- Logical symbols: `\forall`, `\exists`
- Set symbols: `\in`, `\subset`, `\subseteq`

---

## 10. PDF Renderer Reference

The PDF renderer is at `packages/vell-renderer-pdf/src/index.ts`.

### Key implementation details:

- **PDF library:** Uses `pdfkit` for direct PDF generation (no HTML intermediary)
- **Page size:** A4 (595.28 × 841.89 points)
- **Margins:** 56.69 points (≈20mm) on all sides
- **Fonts:** Built-in PDF fonts (Helvetica, Helvetica-Bold, Helvetica-Oblique, Helvetica-BoldOblique, Courier)
- **Pagination:** Automatic page breaks when content exceeds page height
- **Footnote scanning:** Pre-scans document for `FootnoteDefinition` nodes
- **Fallback:** Unknown extensions render as muted labels with child content

### Page Layout Model

| Element | Font | Size | Spacing |
|---------|------|------|---------|
| Body text | Helvetica | 10pt | 1.35 line height |
| Heading 1 | Helvetica-Bold | 22pt | 24pt before |
| Heading 2 | Helvetica-Bold | 16pt | 18pt before |
| Heading 3 | Helvetica-Bold | 13pt | 14pt before |
| Code | Courier | 8pt | Gray background |
| Math | Courier | 10pt | Light background |
| Footnotes | Helvetica | 8pt | After divider |

## 11. Testing Your Renderer

### Test Data

All renderers should be tested against the files in `spec/examples/`:

| File | Content |
|------|---------|
| `01-basic.vl` | Headings, paragraphs, bold/italic, links, images, lists, blockquotes, code |
| `02-math.vl` | Inline math, block math, math in headings |
| `03-tables.vl` | Pipe tables, alignment, grid tables |
| `04-interactive.vl` | Variables, for loops, if blocks |
| `05-extensions.vl` | Directives, extensions, inline components |
| `06-full-document.vl` | Complete document with all features |
| `07-math-advanced.vl` | Advanced math: Align, Matrix, Cases environments |
| `08-theorems-equations.vl` | Equation numbering, theorem environments, cross-references |
| `09-diagrams.vl` | Mermaid diagrams, ASCII art, bar charts |

### Test Checklist

- [ ] All 9 spec examples render without errors
- [ ] Equations auto-number correctly across the document
- [ ] Theorem environments auto-number correctly (skipping Proof, Remark, Example, Notation)
- [ ] Cross-references (`@[Ref]`) resolve to correct equation/theorem numbers
- [ ] Unresolved cross-references render error indicators
- [ ] User-supplied strings are escaped/sanitized
- [ ] Unsafe URLs (`javascript:` etc.) are rejected
- [ ] Unknown node types produce fallback output
- [ ] Footnotes are collected and rendered at end
- [ ] Output is deterministic (same AST → same output)
- [ ] Empty documents render without crashing
- [ ] Documents with only metadata render without crashing
- [ ] Rendering is idempotent for format-then-render pipelines
- [ ] Diagrams render with correct type-specific wrappers (mermaid div, ascii pre)
- [ ] Chart data is parsed and rendered as SVG with proper bar scaling
- [ ] Empty chart data produces safe fallback output (empty SVG or table)

---

## 12. Accessibility

Rendered Vell documents should be accessible to users with disabilities. Follow these guidelines to ensure your renderer produces accessible output.

### Semantic HTML

Use HTML5 semantic elements for better screen reader navigation:

```html
<!-- Use <nav> for table of contents -->
<nav aria-label="Table of Contents">...</nav>

<!-- Use <main> for document body -->
<main>...</main>

<!-- Use <section> for document sections -->
<section aria-labelledby="heading-id">...</section>
```

### Image Alt Text

Always render the `alt` attribute from Image and ImageRef nodes. If alt text is missing, use an empty string `alt=""` (decorative image) rather than omitting the attribute.

```html
<!-- Good: explicit alt text -->
<img src="chart.png" alt="Quarterly revenue bar chart">

<!-- Good: decorative image -->
<img src="decoration.png" alt="">
```

### Link Titles

Use the `title` field from Link nodes as the link's title attribute for additional context:

```html
<a href="https://example.com" title="Visit Example website">Example</a>
```

### ARIA Attributes

The `@[Accessibility]` directive can inject ARIA attributes into parent elements:

```vell
@[Accessibility](role=banner aria-label="Site header")
```

Renderers should apply these attributes to the nearest block-level parent element:

```html
<div role="banner" aria-label="Site header">...</div>
```

### Color Contrast

Ensure that admonition types, diagram captions, and link colors meet WCAG 2.1 AA contrast ratios (4.5:1 for normal text, 3:1 for large text). The reference CSS uses these color combinations:
- **NOTE:** Blue (#3182ce) on light blue (#ebf8ff) — 4.7:1
- **WARNING:** Orange (#dd6b20) on light orange (#fffaf0) — 5.2:1
- **TIP:** Green (#38a169) on light green (#f0fff4) — 4.8:1

### Keyboard Navigation

- Ensure links, footnotes, and cross-references are keyboard-focusable
- Provide visible focus indicators (`:focus-visible` outlines)
- Slide decks should support keyboard navigation (arrow keys)

### Print Accessibility

PDF-friendly output should include:
- Running page headers with document titles
- Page numbers for navigation
- Visible URL text after links (`content: attr(href)` in `@media print`)

---

## 13. Performance Optimization

For renderers handling large documents, the following optimization strategies are recommended.

### Memoization

Cache frequently computed results:

```typescript
class Renderer {
  private escapedCache = new Map<string, string>();

  escapeHtml(text: string): string {
    let cached = this.escapedCache.get(text);
    if (cached === undefined) {
      cached = text
        .replace(/&/g, "&amp;")
        .replace(/</g, "&lt;")
        .replace(/>/g, "&gt;")
        .replace(/"/g, "&quot;");
      // Cache only strings shorter than 1KB
      if (text.length < 1024) this.escapedCache.set(text, cached);
    }
    return cached;
  }
}
```

### String Builder Pattern

Avoid repeated string concatenation for large documents:

```typescript
// Prefer array join over += concatenation
const parts: string[] = [];
for (const node of children) {
  parts.push(renderNode(node));
}
return parts.join("");
```

### Pre-compute Label Maps

For cross-reference resolution, compute the label map once during the pre-pass and reuse it for all `@[Ref]` directives:

```typescript
// Single pre-pass
const labelMap = collectLabels(document);

// Reuse during render pass
function renderRef(props: Record<string, unknown>): string {
  const label = String(props.label ?? "");
  const target = labelMap.get(label);
  // ...
}
```

### Stream Output

For very large documents (> 10 MB rendered output), consider streaming the output rather than building it all in memory:

```typescript
function renderStream(document: Document, writable: WritableStream): void {
  writable.write("<!DOCTYPE html>\n<html>\n<head>\n");
  // ... write incrementally ...
}
```

### Profile Before Optimizing

Always profile your renderer on real document sizes before optimizing. The parser is O(n) and renders typically spend:
- 70% of time on string escaping/encoding
- 20% of time on MathML conversion
- 10% of time on tree traversal

---

## 14. CSS Examples

The following CSS styles are used by the reference HTML renderer. You can include or customize them for your own renderer output.

### Document Layout

```css
body {
  font-family: Georgia, "Times New Roman", serif;
  font-size: 12pt;
  line-height: 1.6;
  color: #1a202c;
  max-width: 42em;
  margin: 2em auto;
  padding: 0 1em;
}
```

### Admonitions

```css
.admonition {
  border-left: 4px solid;
  padding: 0.8em 1em;
  margin: 1em 0;
  border-radius: 4px;
}
.admonition[data-type="NOTE"] { background: #ebf8ff; border-color: #3182ce; }
.admonition[data-type="TIP"]   { background: #f0fff4; border-color: #38a169; }
.admonition[data-type="WARNING"] { background: #fffaf0; border-color: #dd6b20; }
.admonition[data-type="IMPORTANT"] { background: #fefcbf; border-color: #d69e2e; }
.admonition[data-type="CAUTION"] { background: #fff5f5; border-color: #e53e3e; }
.admonition-title {
  font-weight: bold;
  margin-bottom: 0.5em;
  text-transform: uppercase;
  font-size: 0.85em;
}
```

### Code Blocks

```css
pre {
  background: #f7fafc;
  border: 1px solid #e2e8f0;
  border-radius: 4px;
  padding: 1em;
  overflow-x: auto;
  font-family: "SF Mono", "Fira Code", "Fira Mono", monospace;
  font-size: 0.9em;
  line-height: 1.4;
}
code {
  background: #f7fafc;
  padding: 0.2em 0.4em;
  border-radius: 3px;
  font-family: "SF Mono", monospace;
  font-size: 0.9em;
}
```

### Tables

```css
table {
  border-collapse: collapse;
  width: 100%;
  margin: 1em 0;
}
th, td {
  border: 1px solid #e2e8f0;
  padding: 0.5em 0.75em;
  text-align: left;
}
th {
  background: #f7fafc;
  font-weight: 600;
}
```

### Footnotes

```css
.footnotes {
  margin-top: 2em;
  padding-top: 1em;
  border-top: 1px solid #e2e8f0;
  font-size: 0.9em;
}
.footnotes ol {
  padding-left: 1.5em;
}
.footnotes li:target {
  background: #fffff0;
}
```

### Cross-References

```css
.vell-ref {
  color: #2b6cb0;
  text-decoration: none;
}
.vell-ref:hover {
  text-decoration: underline;
}
.unresolved-ref {
  color: #e53e3e;
  font-style: italic;
}
```

### Diagrams

```css
.vell-diagram {
  border: 1px solid #e2e8f0;
  background: #f7fafc;
  border-radius: 6px;
  padding: 1em;
  margin: 1em 0;
  overflow-x: auto;
}
.diagram-caption {
  text-align: center;
  font-style: italic;
  margin-top: 0.5em;
  color: #718096;
}
```

### Print Styles (PDF)

```css
@media print {
  @page {
    size: A4;
    margin: 2.54cm;
    @top-center { content: attr(data-document-title); font-size: 9pt; color: #666; }
    @bottom-center { content: counter(page); font-size: 9pt; color: #666; }
  }
  h1, h2 { page-break-before: always; }
  h3, h4 { page-break-after: avoid; }
  pre, table, blockquote, img, .vell-diagram { page-break-inside: avoid; }
  a[href^="http"]::after { content: " (" attr(href) ")"; font-size: 0.8em; color: #666; }
  .vell-slide { display: none; }
  nav.toc { page-break-after: always; }
}
```

---

> **License:** AGPL-3.0-or-later
> **Author:** Samin Yeasar
> **Version:** 1.0
