# Vell AST Reference

> Version 1 | Schema v1
>
> Reference implementation authored by **Samin Yeasar**

---

## Table of Contents

1. [Overview](#1-overview)
2. [Document Root](#2-document-root)
3. [Span](#3-span)
4. [Document Metadata](#4-document-metadata)
5. [Block Nodes](#5-block-nodes)
   - 5.1 [Heading](#51-heading)
   - 5.2 [Paragraph](#52-paragraph)
   - 5.3 [Blockquote](#53-blockquote)
   - 5.4 [CodeBlock](#54-codeblock)
   - 5.5 [MathBlock](#55-mathblock)
   - 5.6 [List](#56-list)
   - 5.7 [Table](#57-table)
   - 5.8 [HorizontalRule](#58-horizontalrule)
   - 5.9 [DefinitionList](#59-definitionlist)
   - 5.10 [ReferenceDefinition](#510-referencedefinition)
   - 5.11 [FootnoteDefinition](#511-footnotedefinition)
   - 5.12 [VarDeclaration](#512-vardeclaration)
   - 5.13 [ForLoop](#513-forloop)
   - 5.14 [IfBlock](#514-ifblock)
   - 5.15 [Directive](#515-directive)
   - 5.16 [Extension](#516-extension)
6. [Inline Nodes](#6-inline-nodes)
   - 6.1 [Text](#61-text)
   - 6.2 [Bold](#62-bold)
   - 6.3 [Italic](#63-italic)
   - 6.4 [Underline](#64-underline)
   - 6.5 [Strikethrough](#65-strikethrough)
   - 6.6 [Code](#66-code)
   - 6.7 [Superscript](#67-superscript)
   - 6.8 [Subscript](#68-subscript)
   - 6.9 [Link](#69-link)
   - 6.10 [LinkRef](#610-linkref)
   - 6.11 [Image](#611-image)
   - 6.12 [ImageRef](#612-imageref)
   - 6.13 [MathInline](#613-mathinline)
   - 6.14 [VarInterpolation](#614-varinterpolation)
   - 6.15 [InlineComponent](#615-inlinecomponent)
   - 6.16 [Citation](#616-citation)
   - 6.17 [FootnoteRef](#617-footnoteref)
   - 6.18 [SoftBreak](#618-softbreak)
   - 6.19 [HardBreak](#619-hardbreak)
7. [Supporting Types](#7-supporting-types)
   - 7.1 [ListItem](#71-listitem)
   - 7.2 [TableCell](#72-tablecell)
   - 7.3 [DefinitionItem](#73-definitionitem)
   - 7.4 [PropValue](#74-propvalue)
   - 7.5 [Alignment](#75-alignment)
   - 7.6 [NodeKind](#76-nodekind)
8. [Schema Validation](#8-schema-validation)
9. [Versioning Policy](#9-versioning-policy)
10. [Common Traversal Patterns](#10-common-traversal-patterns)

---

## 1. Overview

The Vell AST (Abstract Syntax Tree) is a versioned, deterministic representation of a parsed Vell document. It is designed to be:

- **Portable** — serialized as JSON, consumed by renderers in any language
- **Stable** — the schema version is incremented only on breaking changes
- **Complete** — every source construct maps to a unique AST node
- **Lossless** — source spans are preserved for diagnostics and tooling

The AST is defined by the JSON Schema in `spec/ast-schema.json` and implemented by the Rust types in `crates/vell-core/src/ast.rs`.

### JSON Schema

The formal schema is at `spec/ast-schema.json` (draft-07). All nodes share a common structure with `type` (string discriminator) and `span` (byte-offset source range). Additional properties vary by node type.

### Schema Validation Rules

1. Every node MUST have a `type` field.
2. Every node MUST have a `span` field with `start` and `end` byte offsets.
3. The `version` field on the document root MUST be `1`.
4. Renderers MUST NOT reject documents with unknown node types (see Extension handling).

---

## 2. Document Root

The root of the AST is a `Document` object:

```json
{
  "version": 1,
  "children": [ /* Node[] */ ],
  "metadata": { /* DocumentMetadata */ },
  "span": { "start": 0, "end": 1024 }
}
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `version` | `integer` (const: 1) | Yes | AST schema version; incremented on breaking changes |
| `children` | `Node[]` | Yes | Top-level block nodes in document order |
| `metadata` | `DocumentMetadata` | Yes | Extracted document metadata |
| `span` | `Span` | Yes | Byte span covering the entire document |

---

## 3. Span

Every node includes a `span` that records its byte offsets in the original source text. This enables precise error reporting, diagnostics, and editor tooling.

```json
{
  "start": 0,
  "end": 15
}
```

| Field | Type | Description |
|-------|------|-------------|
| `start` | `integer` | Inclusive byte offset where the node starts |
| `end` | `integer` | Exclusive byte offset where the node ends |

**Note:** Spans reference positions in the original, unmodified source text. Formatters that rewrite source text produce new spans after re-parsing.

**Span usage guidelines:**
- A renderer can use `span` to highlight source locations in diagnostics.
- Formatters use spans to map formatted output back to source positions.
- LSP tooling uses spans to compute text ranges for hover, completion, and diagnostics.

---

## 4. Document Metadata

The `metadata` object carries document-level information extracted during parsing:

```json
{
  "title": "My Document",
  "author": "Jane Doe",
  "date": "2026-01-15",
  "lang": "en",
  "variables": {
    "count": 42,
    "name": "Vell"
  }
}
```

| Field | Type | Description |
|-------|------|-------------|
| `title` | `string` or `null` | Set from the first level-1 heading or `@[Meta](title=...)` |
| `author` | `string` or `null` | Set from `@[Meta](author=...)` |
| `date` | `string` or `null` | Set from `@[Meta](date=...)` |
| `lang` | `string` or `null` | BCP-47 language tag, set from `@[Meta](lang=...)` |
| `variables` | `object` | Map of declared variable names to their JSON values |

---

## 5. Block Nodes

### 5.1 Heading

Represents a section heading.

```json
{
  "type": "Heading",
  "level": 1,
  "children": [
    { "type": "Text", "value": "Introduction" }
  ],
  "id": "introduction",
  "span": { "start": 0, "end": 16 }
}
```

| Field | Type | Description |
|-------|------|-------------|
| `level` | `integer` (1-6) | Heading depth (1 = highest) |
| `children` | `InlineNode[]` | Heading text with inline markup |
| `id` | `string` or `null` | Slugified anchor identifier |

**Example with inline markup:**
Source: `== The *Fast* /Algorithm/`
```json
{
  "type": "Heading",
  "level": 2,
  "children": [
    { "type": "Text", "value": "The " },
    { "type": "Bold", "children": [{ "type": "Text", "value": "Fast" }] },
    { "type": "Text", "value": " " },
    { "type": "Italic", "children": [{ "type": "Text", "value": "Algorithm" }] }
  ],
  "id": "the-fast-algorithm"
}
```

### 5.2 Paragraph

Represents a text paragraph.

```json
{
  "type": "Paragraph",
  "children": [
    { "type": "Text", "value": "This is a paragraph with " },
    { "type": "Bold", "children": [{ "type": "Text", "value": "bold" }] },
    { "type": "Text", "value": " text." }
  ],
  "span": { "start": 0, "end": 35 }
}
```

| Field | Type | Description |
|-------|------|-------------|
| `children` | `InlineNode[]` | Paragraph content with inline markup |

### 5.3 Blockquote

Represents a quoted block, optionally with an admonition type.

```json
{
  "type": "Blockquote",
  "children": [
    {
      "type": "Paragraph",
      "children": [{ "type": "Text", "value": "Quoted content." }]
    }
  ],
  "admonition_type": null,
  "span": { "start": 0, "end": 35 }
}
```

| Field | Type | Description |
|-------|------|-------------|
| `children` | `Node[]` | Parsed content inside the blockquote |
| `admonition_type` | `string` or `null` | Admonition type (`NOTE`, `WARNING`, etc.) when `> [!TYPE]` syntax is used |

**Admonition example:**
Source: `> [!WARNING]\n> Careful!`
```json
{
  "type": "Blockquote",
  "admonition_type": "WARNING",
  "children": [
    { "type": "Paragraph", "children": [{ "type": "Text", "value": "Careful!" }] }
  ]
}
```

### 5.4 CodeBlock

Represents a fenced code block.

```json
{
  "type": "CodeBlock",
  "lang": "rust",
  "source": "fn main() {\n    println!(\"Hello\");\n}",
  "executable": false,
  "span": { "start": 0, "end": 55 }
}
```

| Field | Type | Description |
|-------|------|-------------|
| `lang` | `string` or `null` | Language identifier from the opening fence |
| `source` | `string` | Raw code content (no inline parsing) |
| `executable` | `boolean` | Whether the code block is marked as executable |

### 5.5 MathBlock

Represents a display math block with raw LaTeX.

```json
{
  "type": "MathBlock",
  "source": "\\int_0^1 x^2 \\, dx = \\frac{1}{3}",
  "span": { "start": 0, "end": 42 }
}
```

| Field | Type | Description |
|-------|------|-------------|
| `source` | `string` | Raw LaTeX content between `$$` delimiters |

### 5.6 List

Represents an ordered or unordered list.

```json
{
  "type": "List",
  "ordered": false,
  "start": null,
  "items": [
    {
      "children": [
        {
          "type": "Paragraph",
          "children": [{ "type": "Text", "value": "First item" }]
        }
      ],
      "checked": null
    },
    {
      "children": [
        {
          "type": "Paragraph",
          "children": [{ "type": "Text", "value": "Second item" }]
        }
      ],
      "checked": null
    }
  ],
  "span": { "start": 0, "end": 24 }
}
```

| Field | Type | Description |
|-------|------|-------------|
| `ordered` | `boolean` | `true` for ordered lists, `false` for unordered |
| `start` | `integer` or `null` | Starting number for ordered lists |
| `items` | `ListItem[]` | List items, each containing block children |

**Task list example:**
Source: `- [x] Done\n- [ ] Pending`
```json
{
  "type": "List",
  "ordered": false,
  "items": [
    {
      "children": [{ "type": "Paragraph", "children": [{ "type": "Text", "value": "Done" }] }],
      "checked": true
    },
    {
      "children": [{ "type": "Paragraph", "children": [{ "type": "Text", "value": "Pending" }] }],
      "checked": false
    }
  ]
}
```

### 5.7 Table

Represents a pipe table or grid table.

```json
{
  "type": "Table",
  "headers": [
    {
      "children": [{ "type": "Text", "value": "Name" }],
      "colspan": 1,
      "rowspan": 1,
      "align": null
    },
    {
      "children": [{ "type": "Text", "value": "Value" }],
      "colspan": 1,
      "rowspan": 1,
      "align": null
    }
  ],
  "rows": [
    [
      {
        "children": [{ "type": "Text", "value": "Alpha" }],
        "colspan": 1,
        "rowspan": 1,
        "align": null
      },
      {
        "children": [{ "type": "Text", "value": "100" }],
        "colspan": 1,
        "rowspan": 1,
        "align": null
      }
    ]
  ],
  "span": { "start": 0, "end": 48 }
}
```

| Field | Type | Description |
|-------|------|-------------|
| `headers` | `TableCell[]` | Header row cells |
| `rows` | `TableCell[][]` | Body rows, each an array of cells |

### 5.8 HorizontalRule

Represents a thematic break.

```json
{
  "type": "HorizontalRule",
  "span": { "start": 0, "end": 3 }
}
```

No additional fields beyond `type` and `span`.

### 5.9 DefinitionList

Represents a list of term-definition pairs.

```json
{
  "type": "DefinitionList",
  "items": [
    {
      "term": [{ "type": "Text", "value": "Term" }],
      "definition": [
        {
          "type": "Paragraph",
          "children": [{ "type": "Text", "value": "Definition text." }]
        }
      ]
    }
  ],
  "span": { "start": 0, "end": 35 }
}
```

| Field | Type | Description |
|-------|------|-------------|
| `items` | `DefinitionItem[]` | Term-definition pairs |

### 5.10 ReferenceDefinition

Represents a link reference definition.

```json
{
  "type": "ReferenceDefinition",
  "id": "ref",
  "url": "https://example.com",
  "title": "Example Site",
  "span": { "start": 0, "end": 42 }
}
```

| Field | Type | Description |
|-------|------|-------------|
| `id` | `string` | Reference identifier (used in `[text][id]` links) |
| `url` | `string` | Target URL |
| `title` | `string` or `null` | Optional title text |

### 5.11 FootnoteDefinition

Represents a footnote body definition.

```json
{
  "type": "FootnoteDefinition",
  "marker": "one",
  "children": [
    {
      "type": "Paragraph",
      "children": [{ "type": "Text", "value": "Footnote content." }]
    }
  ],
  "span": { "start": 0, "end": 25 }
}
```

| Field | Type | Description |
|-------|------|-------------|
| `marker` | `string` | Footnote identifier (matched with `[^marker]` references) |
| `children` | `Node[]` | Block content of the footnote |

### 5.12 VarDeclaration

Represents a variable declaration.

```json
{
  "type": "VarDeclaration",
  "name": "count",
  "value": 42,
  "span": { "start": 0, "end": 14 }
}
```

| Field | Type | Description |
|-------|------|-------------|
| `name` | `string` | Variable name (identifier) |
| `value` | any JSON value | Variable value (string, number, boolean, null, array, or object) |

### 5.13 ForLoop

Represents a `@for` loop block.

```json
{
  "type": "ForLoop",
  "variable": "item",
  "iterable": "items",
  "children": [
    {
      "type": "Paragraph",
      "children": [
        { "type": "Text", "value": "- " },
        { "type": "VarInterpolation", "name": "item" }
      ]
    }
  ],
  "span": { "start": 0, "end": 45 }
}
```

| Field | Type | Description |
|-------|------|-------------|
| `variable` | `string` | Loop variable name |
| `iterable` | `string` | Name of the iterable variable |
| `children` | `Node[]` | Repeated block content |

### 5.14 IfBlock

Represents a conditional block.

```json
{
  "type": "IfBlock",
  "condition": "@{count} > 0",
  "consequent": [
    {
      "type": "Paragraph",
      "children": [{ "type": "Text", "value": "Positive!" }]
    }
  ],
  "alternate": [
    {
      "type": "Paragraph",
      "children": [{ "type": "Text", "value": "Not positive." }]
    }
  ],
  "span": { "start": 0, "end": 55 }
}
```

| Field | Type | Description |
|-------|------|-------------|
| `condition` | `string` | Raw condition expression |
| `consequent` | `Node[]` | Content rendered when condition is true |
| `alternate` | `Node[]` or `null` | Optional else-branch content |

### 5.15 Directive

Represents a recognized built-in directive.

```json
{
  "type": "Directive",
  "name": "Figure",
  "props": {
    "src": "chart.png",
    "caption": "Quarterly Data"
  },
  "children": [],
  "span": { "start": 0, "end": 55 }
}
```

| Field | Type | Description |
|-------|------|-------------|
| `name` | `string` | Directive name (e.g., `Figure`, `Code`, `Meta`) |
| `props` | `object` (string → PropValue) | Key-value properties |
| `children` | `Node[]` | Optional braced body content |

### 5.16 Extension

Represents a namespaced or unknown directive preserved for external processing.

```json
{
  "type": "Extension",
  "name": "myorg/Widget",
  "props": {
    "data": 42
  },
  "children": [],
  "raw_source": "@[myorg/Widget](data=42)",
  "span": { "start": 0, "end": 26 }
}
```

| Field | Type | Description |
|-------|------|-------------|
| `name` | `string` | Full extension name (including namespace) |
| `props` | `object` (string → PropValue) | Key-value properties |
| `children` | `Node[]` | Optional braced body content |
| `raw_source` | `string` | Raw source text of the extension directive |

---

## 6. Inline Nodes

### 6.1 Text

Plain text content.

```json
{ "type": "Text", "value": "Hello, world!", "span": { "start": 0, "end": 13 } }
```

| Field | Type | Description |
|-------|------|-------------|
| `value` | `string` | The text content |

### 6.2 Bold

Strong emphasis.

```json
{ "type": "Bold", "children": [{ "type": "Text", "value": "important" }], "span": { "start": 0, "end": 13 } }
```

### 6.3 Italic

Emphasis.

```json
{ "type": "Italic", "children": [{ "type": "Text", "value": "emphasis" }], "span": { "start": 0, "end": 10 } }
```

### 6.4 Underline

Underlined content.

```json
{ "type": "Underline", "children": [{ "type": "Text", "value": "underline" }], "span": { "start": 0, "end": 12 } }
```

### 6.5 Strikethrough

Struck-through content.

```json
{ "type": "Strikethrough", "children": [{ "type": "Text", "value": "removed" }], "span": { "start": 0, "end": 11 } }
```

### 6.6 Code

Inline code.

```json
{ "type": "Code", "value": "fn main()", "span": { "start": 0, "end": 10 } }
```

| Field | Type | Description |
|-------|------|-------------|
| `value` | `string` | Raw code content (no inline parsing) |

### 6.7 Superscript

Superscript text.

```json
{ "type": "Superscript", "children": [{ "type": "Text", "value": "2" }], "span": { "start": 0, "end": 5 } }
```

### 6.8 Subscript

Subscript text.

```json
{ "type": "Subscript", "children": [{ "type": "Text", "value": "2" }], "span": { "start": 0, "end": 7 } }
```

### 6.9 Link

Inline hyperlink.

```json
{
  "type": "Link",
  "href": "https://example.com",
  "title": "Example",
  "children": [{ "type": "Text", "value": "Click here" }],
  "span": { "start": 0, "end": 35 }
}
```

| Field | Type | Description |
|-------|------|-------------|
| `href` | `string` | Target URL |
| `title` | `string` or `null` | Optional hover title |
| `children` | `InlineNode[]` | Link text with inline markup |

**Complex link example (nested bold in link text):**
```json
{
  "type": "Link",
  "href": "https://example.com",
  "title": null,
  "children": [
    { "type": "Bold", "children": [{ "type": "Text", "value": "Click" }] },
    { "type": "Text", "value": " here" }
  ]
}
```

### 6.10 LinkRef

Reference-style link.

```json
{
  "type": "LinkRef",
  "id": "ref",
  "children": [{ "type": "Text", "value": "Reference" }],
  "span": { "start": 0, "end": 18 }
}
```

| Field | Type | Description |
|-------|------|-------------|
| `id` | `string` | Reference definition identifier |
| `children` | `InlineNode[]` | Link text with inline markup |

### 6.11 Image

Inline image.

```json
{
  "type": "Image",
  "src": "/images/logo.png",
  "alt": "Logo",
  "title": "Vell Logo",
  "span": { "start": 0, "end": 28 }
}
```

| Field | Type | Description |
|-------|------|-------------|
| `src` | `string` | Image source URL |
| `alt` | `string` | Alt text |
| `title` | `string` or `null` | Optional hover title |

### 6.12 ImageRef

Reference-style image.

```json
{
  "type": "ImageRef",
  "id": "logo",
  "alt": "Logo",
  "span": { "start": 0, "end": 18 }
}
```

### 6.13 MathInline

Inline math with raw LaTeX.

```json
{ "type": "MathInline", "source": "E = mc^2", "span": { "start": 0, "end": 10 } }
```

| Field | Type | Description |
|-------|------|-------------|
| `source` | `string` | Raw LaTeX content between `$` delimiters |

### 6.14 VarInterpolation

Variable reference.

```json
{ "type": "VarInterpolation", "name": "count", "span": { "start": 0, "end": 8 } }
```

| Field | Type | Description |
|-------|------|-------------|
| `name` | `string` | Variable name to interpolate |

### 6.15 InlineComponent

Inline directive component.

```json
{
  "type": "InlineComponent",
  "name": "Slider",
  "props": { "min": 0, "max": 100 },
  "span": { "start": 0, "end": 25 }
}
```

| Field | Type | Description |
|-------|------|-------------|
| `name` | `string` | Component name |
| `props` | `object` (string → PropValue) | Component properties |

### 6.16 Citation

Citation reference.

```json
{ "type": "Citation", "key": "smith2023", "span": { "start": 0, "end": 14 } }
```

| Field | Type | Description |
|-------|------|-------------|
| `key` | `string` | Citation key |

### 6.17 FootnoteRef

Footnote reference.

```json
{ "type": "FootnoteRef", "marker": "one", "span": { "start": 0, "end": 7 } }
```

| Field | Type | Description |
|-------|------|-------------|
| `marker` | `string` | Footnote marker (matches `[^marker]:` definition) |

### 6.18 SoftBreak

Soft line break within inline content.

```json
{ "type": "SoftBreak", "span": { "start": 10, "end": 11 } }
```

Renders as a space in output.

### 6.19 HardBreak

Hard line break.

```json
{ "type": "HardBreak", "span": { "start": 10, "end": 12 } }
```

Renders as a newline in output.

---

## 7. Supporting Types

### 7.1 ListItem

```json
{
  "children": [ /* Node[] */ ],
  "checked": null,
  "span": { "start": 0, "end": 12 }
}
```

| Field | Type | Description |
|-------|------|-------------|
| `children` | `Node[]` | Block content of the list item |
| `checked` | `boolean` or `null` | Task-list checkbox state (`true` for `[x]`, `false` for `[ ]`, `null` for plain items) |

### 7.2 TableCell

```json
{
  "children": [ /* InlineNode[] */ ],
  "colspan": 1,
  "rowspan": 1,
  "align": "left",
  "span": { "start": 0, "end": 8 }
}
```

### 7.3 DefinitionItem

```json
{
  "term": [ /* InlineNode[] */ ],
  "definition": [ /* Node[] */ ],
  "span": { "start": 0, "end": 25 }
}
```

### 7.4 PropValue

Property values used by directives and inline components. Represented as untagged JSON:

| JSON Representation | Vell Type |
|---------------------|-----------|
| `"string"` | String |
| `42` / `3.14` | Number |
| `true` / `false` | Bool |
| `null` | Null |
| `"@{name}"` (in props) | Variable reference |

### 7.5 Alignment

```json
"left" | "center" | "right"
```

Table cell alignment indicator.

### 7.6 NodeKind

Enum-like discriminator without associated data.

```json
"Heading" | "Paragraph" | "Blockquote" | "CodeBlock" | "MathBlock" | "List" | "Table" | "HorizontalRule" | "DefinitionList" | "ReferenceDefinition" | "FootnoteDefinition" | "VarDeclaration" | "ForLoop" | "IfBlock" | "Directive" | "Extension"
```

---

## 8. Schema Validation

The AST schema is defined in `spec/ast-schema.json` (JSON Schema draft-07). Key validation rules:

1. **Document root** requires `version` (must be 1), `children`, `metadata`, and `span`.
2. **Every node** requires `type` and `span`.
3. **Block nodes** have `children: Node[]` when they contain nested blocks.
4. **Inline nodes** have `children: InlineNode[]` when they contain nested inline content.
5. **Additional properties** are allowed on all node types for forward compatibility.

Renderers should use the schema to validate AST input and provide meaningful errors for malformed documents.

---

## 9. Versioning Policy

The AST schema follows semantic versioning:

- **Major version** (e.g., v2): Breaking changes to node structure or semantics. Renderers must update.
- **Minor version** (e.g., v1.1): New node types added. Existing renderers continue to work with fallback behavior.
- **Patch version** (e.g., v1.0.1): Bug fixes or documentation changes. No impact on AST structure.

The current schema version is **1** (v1). The version is incremented in the `version` field of the `Document` root node and reflected in the `AST_VERSION` constant in `crates/vell-core/src/ast.rs`.

---

## 10. Common Traversal Patterns

When working with the AST programmatically (in renderers, formatters, or linters), the following traversal patterns are commonly used.

### Recursive Walk (All Nodes)

Walk every node in the tree depth-first:

```typescript
function walk(node: Node, visit: (node: Node) => void): void {
  visit(node);
  for (const child of node.children ?? []) {
    walk(child, visit);
  }
}
```

### Find Nodes by Type

Collect all nodes of a specific type:

```typescript
function findNodesByType(root: Document, type: string): Node[] {
  const results: Node[] = [];
  walk(root, (node) => {
    if (node.type === type) results.push(node);
  });
  return results;
}

// Usage: find all headings
const headings = findNodesByType(doc, "Heading");
```

### Collect Cross-Reference Labels

Traverse only `Directive` nodes to build the label map:

```typescript
function collectLabels(root: Document): Map<string, LabelInfo> {
  const labels = new Map();
  let eqCounter = 0;
  const thmCounters: Record<string, number> = {};

  for (const child of root.children) {
    collectLabelsRecursive(child, labels, eqCounter, thmCounters);
  }
  return labels;
}
```

### Render All Inline Children

Convert inline node arrays to output strings:

```typescript
function renderInlines(inlines: InlineNode[]): string {
  return inlines.map(renderInline).join("");
}
```

### Extract All Text Content

Get plain text content from a node subtree (useful for search indexing):

```typescript
function extractText(node: Node): string {
  if (node.type === "Text") return node.value;
  if (node.type === "Code" || node.type === "CodeBlock") return node.source ?? node.value ?? "";
  let text = "";
  for (const child of node.children ?? []) {
    text += extractText(child);
  }
  return text;
}
```

### Span-to-Range Conversion (LSP)

Convert byte spans to LSP line/column ranges for editor tooling:

```typescript
function spanToRange(source: string, span: Span): Range {
  const startLines = source.substring(0, span.start).split("\n");
  const endLines = source.substring(0, span.end).split("\n");
  return {
    start: { line: startLines.length - 1, character: startLines[startLines.length - 1].length },
    end: { line: endLines.length - 1, character: endLines[endLines.length - 1].length },
  };
}
```

---

> **License:** AGPL-3.0-or-later
> **Author:** Samin Yeasar
> **Schema Version:** 1
