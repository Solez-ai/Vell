# Vell Extension Authoring Guide

> Create custom directives, plugins, and templates for Vell documents
>
> Vell is created and maintained by **Samin Yeasar**

---

## Table of Contents

1. [Introduction](#1-introduction)
2. [Extension Architecture](#2-extension-architecture)
3. [Creating an Extension](#3-creating-an-extension)
   - 3.1 [Naming Conventions](#31-naming-conventions)
   - 3.2 [Properties Schema](#32-properties-schema)
   - 3.3 [Block Bodies](#33-block-bodies)
4. [Extension AST Representation](#4-extension-ast-representation)
5. [Adding Extension Support to Renderers](#5-adding-extension-support-to-renderers)
   - 5.1 [HTML Renderer Adapter](#51-html-renderer-adapter)
   - 5.2 [PDF Renderer Adapter](#52-pdf-renderer-adapter)
   - 5.3 [Custom Renderers](#53-custom-renderers)
6. [Plugin Architecture](#6-plugin-architecture)
   - 6.1 [Extension Registry API](#61-extension-registry-api)
   - 6.2 [Built-in Extension Library](#62-built-in-extension-library)
   - 6.3 [Template System](#63-template-system)
7. [LSP Integration](#7-lsp-integration)
   - 7.1 [Hover Documentation](#71-hover-documentation)
   - 7.2 [Completions](#72-completions)
   - 7.3 [Diagnostics](#73-diagnostics)
8. [Best Practices](#8-best-practices)
9. [Example Extensions](#9-example-extensions)
   - 9.1 [YouTube Embed Extension](#91-youtube-embed-extension)
   - 9.2 [Chart Extension](#92-chart-extension)
   - 9.3 [Mermaid Diagram Extension](#93-mermaid-diagram-extension)
   - 9.4 [Callout Extension](#94-callout-extension)
   - 9.5 [Map Embed Extension](#95-map-embed-extension)
10. [Extension Distribution](#10-extension-distribution)
11. [Versioning and Compatibility](#11-versioning-and-compatibility)
12. [Plugin Hooks Reference](#12-plugin-hooks-reference)
13. [Template System Reference](#13-template-system-reference)
14. [Testing Extensions](#14-testing-extensions)
15. [Debugging Tips](#15-debugging-tips)
16. [Counter Extension Example](#16-counter-extension-example)
17. [Progress Bar Extension Example](#17-progress-bar-extension-example)

---

## 1. Introduction

Vell's extension system allows developers to add custom directives that go beyond the built-in set. Extensions are namespaced directives that are preserved as `Extension` nodes in the AST, making them portable across different renderers and tooling versions.

### Why Extensions?

- **Custom functionality** — Add features not covered by built-in directives
- **Domain-specific content** — Create specialized blocks for your field (scientific diagrams, interactive widgets, custom visualizations)
- **Tooling integration** — Hook into the parser, formatter, LSP, and renderers
- **Forward compatibility** — Extensions are preserved even when renderers don't support them

### How Extensions Work

1. A user writes `@[myorg/Widget](data=42)` in their `.vl` file
2. The parser recognizes the namespace pattern (`org/Name`) and preserves it as an `Extension` node
3. Supported renderers detect the extension name and apply custom rendering logic
4. Unsupported renderers provide safe fallback output (child content or a placeholder label)

---

## 2. Extension Architecture

The extension system has four layers:

```
┌─────────────────────────────────────────────┐
│  Vell Source                                │
│  @[myorg/Widget](data=42) { body }          │
└─────────────────┬───────────────────────────┘
                  │ parsing
                  ▼
┌─────────────────────────────────────────────┐
│  AST Extension Node                         │
│  { type: "Extension", name: "myorg/Widget", │
│    props: { data: 42 }, children: [...] }   │
└─────────────────┬───────────────────────────┘
                  │ rendering
                  ▼
┌─────────────────────────────────────────────┐
│  Extension Registry                         │
│  (loaded adapters: YouTube, Chart, etc.)    │
└─────────────────┬───────────────────────────┘
                  │ dispatch
                  ▼
┌─────────────────────────────────────────────┐
│  Renderer Adapter                           │
│  if (name === "myorg/Widget") {             │
│    renderCustomWidget(props, children)      │
│  }                                          │
└─────────────────────────────────────────────┘
```

### Key Design Decisions

1. **Parser is agnostic.** The parser does not validate or reject any extension. It simply preserves the structure.
2. **Renderers opt in.** Each renderer decides which extensions to support. Unknown extensions get fallback rendering.
3. **Properties are typed.** The `PropValue` type supports strings, numbers, booleans, null, and variable references.
4. **Bodies are nested documents.** Extension block bodies are parsed as full Vell documents, supporting arbitrary nesting.

### Extension Lifecycle

An extension goes through these stages during a document's lifecycle:

1. **Authoring:** User writes `@[org/Name](props) { body }` in a `.vl` file.
2. **Parsing:** The parser creates an `Extension` node with `name`, `props`, `children`, and `raw_source`.
3. **Pre-processing (optional):** Some extensions may transform the AST before rendering (e.g., resolving external data).
4. **Rendering:** A renderer adapter transforms the `Extension` node into output format.
5. **Post-processing (future):** Extensions may modify rendered output (e.g., injecting scripts).

---

## 3. Creating an Extension

### 3.1 Naming Conventions

Extension names follow the pattern `namespace/Name`:

```vell
@[org/Widget](data=42)
@[npm/chart](type=bar data=[1,2,3])
@[company/diagram](engine=mermaid)
```

**Rules:**
- The name must contain a `/` separator
- The namespace (`org`) should be unique to avoid conflicts
- The name (`Name`) should be PascalCase for readability
- Names without `/` are treated as built-in directives and must be registered in the parser

**Recommended namespace patterns:**
| Namespace | Usage |
|-----------|-------|
| `embed/` | Media embeds (YouTube, Vimeo, Maps) |
| `npm/` | Open-source packages published on npm |
| `gh/` | GitHub-hosted extensions |
| `org/` | Organization-specific extensions (e.g., `acmecorp/`) |

### 3.2 Properties Schema

Extension properties are defined in the `props` field. Supported value types:

| Type | Syntax | Example |
|------|--------|---------|
| String | `name="value"` or `name=bareword` | `title="My Chart"` |
| Number | `name=42` | `width=800` |
| Boolean | `name=true` | `interactive=true` |
| Null | `name=null` | `optional=null` |
| Variable | `name=@{var}` | `data=@{dataset}` |

Properties are accessed via the `props` object in the AST:

```json
{
  "type": "Extension",
  "name": "npm/chart",
  "props": {
    "type": "bar",
    "data": [1, 2, 3],
    "title": "Sales Data",
    "interactive": true
  }
}
```

### 3.3 Block Bodies

Extensions can include braced block bodies that contain nested Vell content:

```vell
@[npm/callout](type=warning) {
  This is a warning message with **bold** text.

  It can contain multiple paragraphs.
}
```

The body content is parsed as a full Vell document and available as `children` in the AST:

```json
{
  "type": "Extension",
  "name": "npm/callout",
  "props": { "type": "warning" },
  "children": [
    {
      "type": "Paragraph",
      "children": [
        { "type": "Text", "value": "This is a warning message with " },
        { "type": "Bold", "children": [{ "type": "Text", "value": "bold" }] },
        { "type": "Text", "value": " text." }
      ]
    }
  ]
}
```

---

## 4. Extension AST Representation

Every extension is stored as an `Extension` node in the AST:

```json
{
  "type": "Extension",
  "name": "myorg/Widget",
  "props": {
    "key": "value"
  },
  "children": [
    /* nested Vell document nodes */
  ],
  "raw_source": "@[myorg/Widget](key=value) { ... }",
  "span": {
    "start": 0,
    "end": 45
  }
}
```

| Field | Type | Description |
|-------|------|-------------|
| `name` | `string` | Full extension name including namespace |
| `props` | `object` | Key-value properties (string → PropValue) |
| `children` | `Node[]` | Parsed block body (empty if no body) |
| `raw_source` | `string` | Original source text of the extension directive |
| `span` | `Span` | Byte offsets in the original source |

**Important:** Renderers must not assume the `raw_source` field is present or accurate. It may be omitted in AST versions after serialization/deserialization.

---

## 5. Adding Extension Support to Renderers

### 5.1 HTML Renderer Adapter

To add extension support to the HTML renderer, create a handler function and register it:

```typescript
// extensions/my-chart.ts
import { ExtensionAdapter, ExtensionContext, PropValue, VellNode } from "@vell-lang/extensions";

function renderChart(node: VellNode, ctx: ExtensionContext): string {
  const props = node.props ?? {};
  const type = String(props.type ?? "bar");
  const data = String(props.data ?? "[]");

  // Generate chart HTML
  return `<div class="chart chart-${ctx.escapeHtml(type)}" data-data="${ctx.escapeHtml(data)}"></div>`;
}

export const Chart: ExtensionAdapter = {
  name: "myorg/Chart",
  description: "Renders a chart from data",
  schema: {
    type: { type: "string", required: true },
    data: { type: "string", required: true },
  },
  render: renderChart,
};
```

### 5.2 PDF Renderer Adapter

For the PDF renderer, extension handlers receive the `pdfkit` document instance:

```typescript
// In PdfRenderer class:
private extensionHandlers: Record<string, (props: Record<string, PropValue>, children: Node[]) => void> = {};

registerExtension(name: string, handler: (props: Record<string, PropValue>, children: Node[]) => void): void {
  this.extensionHandlers[name] = handler;
}

private renderExtension(node: Node): void {
  const handler = this.extensionHandlers[node.name ?? ""];
  if (handler) {
    handler(node.props ?? {}, node.children ?? []);
    return;
  }
  // Built-in fallback
  this.renderFallbackExtension(node);
}
```

### 5.3 Custom Renderers

For custom renderers (slides, e-books, CLI tools), follow the same adapter pattern:

```typescript
interface ExtensionAdapter {
  name: string;
  render(props: Record<string, PropValue>, children: Node[], context: RenderContext): void;
}

class Renderer {
  private adapters: Map<string, ExtensionAdapter> = new Map();

  use(adapter: ExtensionAdapter): void {
    this.adapters.set(adapter.name, adapter);
  }

  private renderExtension(node: Node): void {
    const adapter = this.adapters.get(node.name ?? "");
    if (adapter) {
      adapter.render(node.props ?? {}, node.children ?? [], this.context);
    } else {
      this.renderFallback(node);
    }
  }
}
```

---

## 6. Plugin Architecture

Phase 13 introduces a formal extension registry and plugin architecture for Vell.

### 6.1 Extension Registry API

The `@vell-lang/extensions` package provides a centralized registry for extension adapters:

```typescript
import {
  register,
  getExtension,
  getRegisteredExtensions,
  renderExtension,
  createExtensionContext,
  ExtensionAdapter,
} from "@vell-lang/extensions";

// Register built-in extensions
register(YouTube, Chart, Mermaid, Callout, Map);

// Look up an extension by name
const chartAdapter = getExtension("npm/chart");

// Render an extension node
const html = renderExtension(extensionNode, extensionContext);

// List all registered extensions
const names = getRegisteredExtensions();
// → ["embed/YouTube", "embed/Vimeo", "npm/chart", "npm/mermaid", "npm/callout", "npm/map"]
```

### ExtensionAdapter Interface

```typescript
interface ExtensionAdapter {
  name: string;
  description: string;
  schema: Record<string, { type: string; required?: boolean; default?: PropValue }>;
  render(node: VellNode, ctx: ExtensionContext): string;
  fallback?(node: VellNode, ctx: ExtensionContext): string;
}
```

### ExtensionContext Interface

```typescript
interface ExtensionContext {
  escapeHtml(value: string): string;
  sanitizeUrl(value: string): string;
}
```

### 6.2 Built-in Extension Library

The `@vell-lang/extensions` package ships with a curated set of built-in extensions:

| Extension | Name | Description |
|-----------|------|-------------|
| YouTube | `embed/YouTube` | Embed a YouTube video with iframe |
| Vimeo | `embed/Vimeo` | Embed a Vimeo video with iframe |
| Chart | `npm/chart` | Render charts (bar, pie, doughnut) with inline SVG |
| Mermaid | `npm/mermaid` | Render Mermaid.js diagrams |
| Callout | `npm/callout` | Styled admonition/callout boxes |
| Map | `npm/map` | Embed an OpenStreetMap location |

These extensions are auto-registered when the package is imported:

```typescript
import { YouTube, Chart } from "@vell-lang/extensions";

// Already registered — ready to use via renderExtension()
```

You can also render extensions directly from the HTML renderer:

```typescript
import { renderExtension, createExtensionContext } from "@vell-lang/extensions";

function renderNode(node: VellNode): string {
  if (node.type === "Extension") {
    return renderExtension(node, createExtensionContext());
  }
  // ... handle other node types
}
```

### 6.3 Template System

The `@[Template]` directive provides document-level styling and theming:

```vell
@[Template](name=scientific-paper style=
  body { font-family: 'Georgia', serif; font-size: 11pt; line-height: 1.8; }
  h1, h2, h3 { font-family: 'Helvetica Neue', sans-serif; color: #1a365d; }
  pre { background: #f7fafc; border: 1px solid #e2e8f0; padding: 0.8em; }
)

@[Template](name=presentation url="https://cdn.example.com/themes/dark.css")
```

**Properties:**

| Property | Type | Description |
|----------|------|-------------|
| `name` | string | Template name identifier |
| `url` | string | URL to external CSS stylesheet |
| `style` | string | Inline CSS (overrides URL-based styles) |

Templates are rendered as:
- `<link rel="stylesheet">` for URL-based templates
- `<style>` blocks for inline CSS
- `<meta name="vell-template">` for name metadata

---

## 7. LSP Integration

Providing good LSP support for extensions improves the developer experience for users of your extension.

### 7.1 Hover Documentation

Register hover text for your extension:

```typescript
// In the LSP server's hover handler:
const extensionDocs: Record<string, string> = {
  "npm/chart": "**npm/chart**\n\nRenders a chart from data.\n\nProperties:\n- `type`: Chart type (bar, line, pie)\n- `data`: Array of numeric values",
};

// When hovering over an Extension node:
if (node.type === "Extension" && extensionDocs[node.name!]) {
  return {
    contents: HoverContents::Scalar(MarkedString::String(extensionDocs[node.name!])),
    range: None,
  };
}
```

### 7.2 Completions

Register completion items for your extension:

```typescript
// In the LSP server's completion handler:
const extensionCompletions: CompletionItem[] = [
  CompletionItem {
    label: "@[npm/chart]".to_string(),
    detail: Some("Render a chart from data".to_string()),
    kind: Some(CompletionItemKind::KEYWORD),
    ..
  },
];
```

### 7.3 Diagnostics

If your extension has specific validation rules, you can add custom diagnostics:

```typescript
// After parsing, validate extension properties:
for (const node of doc.children) {
  if (node.type === "Extension" && node.name === "npm/chart") {
    if (!node.props?.type) {
      diagnostics.push(Diagnostic {
        range: range_from_span(source, &node.span),
        severity: Some(DiagnosticSeverity::ERROR),
        message: "npm/chart requires a 'type' property (bar, line, or pie)".to_string(),
        ..Diagnostic::default()
      });
    }
  }
}
```

---

## 8. Best Practices

### Do's

- **Use namespaced names** (`myorg/Name`) to avoid conflicts with other extensions and future built-in directives.
- **Provide meaningful fallback content.** If your extension has a block body, the children will be rendered as fallback. Make the fallback useful.
- **Document your extension's schema.** Specify which properties are required, optional, and what types they accept.
- **Handle missing properties gracefully.** Use defaults for optional properties and provide clear error messages for missing required ones.
- **Register LSP support.** Hover docs and completions make your extension discoverable and usable.
- **Test against all renderers.** If your extension is used in both HTML and PDF output, ensure both renderers handle it.
- **Use the ExtensionAdapter interface.** Create adapters that implement the `ExtensionAdapter` type for consistent behavior across renderers.

### Don'ts

- **Don't assume every renderer supports your extension.** Always provide sensible fallback output.
- **Don't use built-in directive names.** Names without `/` are reserved for future built-in directives.
- **Don't modify the AST.** Extensions are read-only consumers of the AST.
- **Don't embed executable code** in extension properties that could be a security risk.

### Fallback Strategy

Design your extension so that unsupported renderers still produce useful output:

```vell
@[npm/chart](type=bar data=[1, 3, 2, 4]) {
  Data: 1, 3, 2, 4 (bar chart)
}
```

In this example, renderers that don't support `npm/chart` will still render the child text: "Data: 1, 3, 2, 4 (bar chart)".

---

## 9. Example Extensions

### 9.1 YouTube Embed Extension

```vell
@[embed/YouTube](video="dQw4w9WgXcQ" width=560)

@[embed/YouTube](video="dQw4w9WgXcQ") {
  A very informative video.
}
```

**HTML renderer output:**
```html
<div class="vell-extension" data-name="embed/YouTube">
  <iframe width="560" height="315" src="https://www.youtube-nocookie.com/embed/dQw4w9WgXcQ"
    frameborder="0" allowfullscreen title="YouTube video"></iframe>
  <div class="vell-extension-fallback">A very informative video.</div>
</div>
```

**Schema:**
```json
{
  "name": "embed/YouTube",
  "props": {
    "video": { "type": "string", "required": true },
    "width": { "type": "number", "default": 560 },
    "height": { "type": "number", "default": 315 }
  }
}
```

### 9.2 Chart Extension

```vell
@[npm/chart](type=bar data=[10, 25, 15, 30, 20] labels=Q1-Q5)

@[npm/chart](type=line data=[5, 10, 8, 15] color=blue) {
  Quarterly trend: Q1=5, Q2=10, Q3=8, Q4=15
}
```

**HTML renderer output:**
```html
<div class="vell-chart vell-chart-bar">
  <svg width="500px" height="250px" viewBox="0 0 500 250" ...>
    <!-- SVG bar chart -->
  </svg>
</div>
```

**Schema:**
```json
{
  "name": "npm/chart",
  "props": {
    "type": { "type": "string", "required": true, "values": ["bar", "line", "pie"] },
    "data": { "type": "string", "required": true },
    "labels": { "type": "string", "optional": true }
  }
}
```

### 9.3 Mermaid Diagram Extension

```vell
@[npm/mermaid](theme=default) {
  graph TD
    A[Start] --> B{Decision}
    B -->|Yes| C[Continue]
    B -->|No| D[Stop]
}
```

**HTML renderer output:**
```html
<div class="vell-extension" data-name="npm/mermaid" data-theme="default">
  <pre class="mermaid">graph TD
    A[Start] --> B{Decision}
    B -->|Yes| C[Continue]
    B -->|No| D[Stop]</pre>
</div>
```

**Schema:**
```json
{
  "name": "npm/mermaid",
  "props": {
    "theme": { "type": "string", "default": "default", "values": ["default", "dark", "neutral"] }
  },
  "body": "Mermaid diagram definition text"
}
```

### 9.4 Callout Extension

```vell
@[npm/callout](type=warning title="Danger Zone") {
  This is a warning callout box.
}
```

**HTML renderer output:**
```html
<div class="vell-callout vell-callout-warning">
  <span class="callout-icon">⚠</span>
  <div class="callout-title">Danger Zone</div>
  <div class="callout-body">
    <p>This is a warning callout box.</p>
  </div>
</div>
```

### 9.5 Map Embed Extension

```vell
@[npm/map](lat=51.5074 lng=-0.1278 zoom=12 title="London") {
  London, UK
}
```

---

## 10. Extension Distribution

### Package Structure

```
my-vell-extension/
├── package.json
├── src/
│   ├── index.ts           # Main entry point (exports ExtensionAdapter)
│   ├── html-adapter.ts    # HTML renderer support
│   ├── pdf-adapter.ts     # PDF renderer support
│   └── lsp.ts             # LSP integration
├── docs/
│   └── README.md          # Extension documentation
└── tests/
    └── fixtures.vl        # Test fixtures
```

### Registration

Extensions are registered at the application level:

```typescript
import { register } from "@vell-lang/extensions";
import { MyExtension } from "my-vell-extension";

register(MyExtension);
```

### npm Package

Publish your extension as an npm package following the naming convention:

```
vell-extension-{name}
```

Include in your `package.json`:
```json
{
  "name": "vell-extension-npm-chart",
  "version": "1.0.0",
  "description": "Chart extension for Vell documents",
  "main": "dist/index.js",
  "types": "dist/index.d.ts",
  "peerDependencies": {
    "@vell-lang/renderer-html": "^0.1.0",
    "@vell-lang/extensions": "^0.1.0"
  }
}
```

---

## 11. Versioning and Compatibility

### AST Compatibility

Extensions are stored as `Extension` nodes in the AST. The AST schema guarantees:
- The `Extension` node type will not be removed
- The `name`, `props`, and `children` fields will not be renamed
- New fields may be added in future versions

### Renderer Compatibility

Extension authors should:
1. **Document minimum renderer versions** required for full support
2. **Test against multiple renderer versions** to ensure compatibility
3. **Use semantic versioning** for your extension package
4. **Provide fallback content** for unsupported renderers

### Breaking Changes

When making breaking changes to your extension:
1. Increment the major version of your package
2. Update the extension name if necessary (e.g., `myorg/Widget-v2`)
3. Document migration paths for users

---

## 12. Plugin Hooks Reference

The extension registry supports the following plugin hooks in the render pipeline:

| Hook | Description | Phase |
|------|-------------|-------|
| `render` | Transform an Extension node into output | Render |
| `fallback` | Generate fallback content for unsupported renderers | Render |
| `schema` | Define property schema for validation and LSP | Registration |
| `description` | Human-readable description for tooling | Registration |

### Pipeline Integration

```
Source → Parser → AST → [ Pre-process Hooks ] → Renderer → [ Post-process Hooks ] → Output
                                │                                    │
                         Extension Nodes                       Processed Output
```

**Pre-process hooks** (future): Transform or annotate Extension nodes before rendering.
**Post-process hooks** (future): Transform rendered output (e.g., minify, inject scripts).

---

## 13. Template System Reference

### Directive Syntax

```vell
@[Template](name=template-name style=
  /* CSS rules */
)

@[Template](name=template-name url="https://example.com/theme.css")
```

### Properties

| Property | Type | Required | Description |
|----------|------|----------|-------------|
| `name` | string | No | Template identifier (e.g., "scientific-paper") |
| `url` | string | No | URL to external CSS stylesheet |
| `style` | string | No | Inline CSS content |

### Rendering Behavior

- `url` produces `<link rel="stylesheet" href="...">` in the HTML `<head>`
- `style` produces `<style>...</style>` in the document body
- `name` produces `<meta name="vell-template" content="...">` for metadata

### Multiple Templates

Multiple `@[Template]` directives can be used in a single document. They are applied in order:

```vell
@[Template](url="https://cdn.example.com/base.css")
@[Template](style=h1 { color: #2b6cb0; } p { font-size: 11pt; })
```

### Best Practices

1. Use `url` for reusable templates hosted externally
2. Use `style` for document-specific overrides
3. Combine `@[Template]` with `@[Theme]` for complex theming
4. Templates affect only the HTML output; PDF/slides respect their own styling

---

## 14. Testing Extensions

Thorough testing ensures your extension works across different renderers and edge cases.

### Unit Testing

Test individual extension components in isolation:

```typescript
import { describe, it, expect } from "vitest";
import { MyExtension } from "./my-extension";
import { createExtensionContext } from "@vell-lang/extensions";

describe("MyExtension", () => {
  it("renders with required properties", () => {
    const node = {
      type: "Extension",
      name: "myorg/Widget",
      props: { data: 42 },
      children: [],
    };
    const ctx = createExtensionContext();
    const result = MyExtension.render(node, ctx);
    expect(result).toContain("data=\"42\"");
  });

  it("uses default values for optional properties", () => {
    const node = {
      type: "Extension",
      name: "myorg/Widget",
      props: {},
      children: [],
    };
    const ctx = createExtensionContext();
    const result = MyExtension.render(node, ctx);
    expect(result).toContain("default");
  });

  it("escapes HTML in user-supplied property values", () => {
    const node = {
      type: "Extension",
      name: "myorg/Widget",
      props: { title: "<script>alert('xss')</script>" },
      children: [],
    };
    const ctx = createExtensionContext();
    const result = MyExtension.render(node, ctx);
    expect(result).not.toContain("<script>");
    expect(result).toContain("&lt;script&gt;");
  });
});
```

### Integration Testing

Test extension rendering within a full document:

```typescript
import { parse } from "vell-js";
import { render } from "@vell-lang/renderer-html";
import { register, renderExtension } from "@vell-lang/extensions";
import { MyExtension } from "./my-extension";

describe("MyExtension integration", () => {
  it("renders correctly in a complete document", () => {
    register(MyExtension);
    const source = `= Test\n\n@[myorg/Widget](data=42) {\n  Fallback content\n}`;
    const ast = parse(source);
    const html = render(ast);
    expect(html).toContain("myorg/Widget");
    expect(html).toContain("data=\"42\"");
  });
});
```

### Test Fixtures

Create `.vl` fixture files for manual and automated testing:

```vell
# tests/fixtures/widget-basic.vl
= Widget Test

@[myorg/Widget](data=42) {
  This is fallback content.
}
```

Run all fixtures through your renderer and verify output:

```bash
# For each fixture, parse and render
vell parse tests/fixtures/widget-basic.vl > /tmp/ast.json
# Then pipe AST to your custom renderer
```

### Test Checklist

- [ ] Extension renders with all required properties
- [ ] Extension uses default values for missing optional properties
- [ ] Extension sanitizes URLs and escapes HTML in user input
- [ ] Fallback content is rendered when the extension is not supported
- [ ] Extension handles null/undefined property values gracefully
- [ ] Extension produces valid HTML5 output
- [ ] Body content with nested Vell markup renders correctly
- [ ] Multiple instances of the extension in one document work independently
- [ ] Extension works with both self-closing and body syntax

---

## 15. Debugging Tips

### Inspect the AST

Use `vell parse` to see how your extension is represented in the AST:

```bash
vell parse my-document.vl | jq '.children[] | select(.type == "Extension")'
```

This will show the full `Extension` node with `name`, `props`, `children`, and `span`.

### Check Property Values

Verify that property values are parsed correctly:

```bash
vell parse my-document.vl | jq '.children[] | select(.type == "Extension") | .props'
```

### Validate HTML Output

Use the W3C HTML validator to check your extension's output:

```bash
vell render html my-document.vl > output.html
# Paste output.html content into https://validator.w3.org/
```

### Log Extension Dispatch

Add logging to your extension adapter to trace when it's called:

```typescript
const Chart: ExtensionAdapter = {
  name: "npm/chart",
  render(node, ctx) {
    console.debug(`[Chart] Rendering with props:`, node.props);
    // ... render logic ...
  },
};
```

### Common Issues

| Issue | Likely Cause | Solution |
|-------|-------------|----------|
| Extension not rendering | Extension not registered | Call `register(MyExtension)` before rendering |
| Wrong property value | Property name mismatch | Check exact property name in source vs. adapter |
| HTML not escaping | Missing `escapeHtml` call | Wrap all user-supplied strings with `ctx.escapeHtml()` |
| Fallback not showing | No body content provided | Add descriptive body content to the extension |
| LSP not showing hover docs | Extension docs not registered | Add entry to the extension docs map in LSP handler |

---

## 16. Counter Extension Example

A complete, runnable extension that renders an auto-incrementing counter, useful for numbered steps or sections.

### Source Syntax

```vell
@[npm/counter](start=10 step=5) {
  Step content here.
}
```

### Properties

| Property | Type | Required | Default | Description |
|----------|------|----------|---------|-------------|
| `start` | number | No | `1` | Starting counter value |
| `step` | number | No | `1` | Increment between counters |
| `format` | string | No | `"decimal"` | Output format: `decimal`, `roman` (lowercase), `ROMAN` (uppercase), `alpha` (a, b, c...) |

### HTML Adapter

```typescript
import { ExtensionAdapter, ExtensionContext, VellNode } from "@vell-lang/extensions";

let counterState = new Map<string, number>();

function renderCounter(node: VellNode, ctx: ExtensionContext): string {
  const props = node.props ?? {};
  const start = Number(props.start ?? 1);
  const step = Number(props.step ?? 1);
  const format = String(props.format ?? "decimal");

  // Get or initialize counter for this document
  const key = node.span ? `${node.span.start}` : Math.random().toString();
  if (!counterState.has(key)) {
    counterState.set(key, start);
  }
  const current = counterState.get(key)!;
  counterState.set(key, current + step);

  // Format the number
  let display: string;
  switch (format) {
    case "roman":
      display = toRoman(current).toLowerCase();
      break;
    case "ROMAN":
      display = toRoman(current);
      break;
    case "alpha":
      display = String.fromCharCode(96 + current); // a, b, c, ...
      break;
    default:
      display = String(current);
  }

  // Render children as fallback
  const childrenHtml = (node.children ?? [])
    .map((child) => renderNode(child, ctx))
    .join("");

  return `<div class="vell-counter" data-value="${ctx.escapeHtml(display)}">
    <span class="counter-number">${ctx.escapeHtml(display)}.</span>
    <span class="counter-body">${childrenHtml}</span>
  </div>`;
}

export const Counter: ExtensionAdapter = {
  name: "npm/counter",
  description: "Auto-incrementing step counter",
  schema: {
    start: { type: "number", default: 1 },
    step: { type: "number", default: 1 },
    format: { type: "string", default: "decimal" },
  },
  render: renderCounter,
};
```

### Usage Example

```vell
= Project Plan

== Phase 1

@[npm/counter] {
  Research and gather requirements.
}

@[npm/counter] {
  Design the system architecture.
}

== Phase 2

@[npm/counter](start=10) {
  Implement core features.
}

@[npm/counter](step=2) {
  Write documentation. (Counter: 12)
}
```

---

## 17. Progress Bar Extension Example

A visual progress bar extension that renders a colored, percentage-based progress bar.

### Source Syntax

```vell
@[npm/progress](value=75 max=100 color=green)
```

### Properties

| Property | Type | Required | Default | Description |
|----------|------|----------|---------|-------------|
| `value` | number | Yes | — | Current progress value |
| `max` | number | No | `100` | Maximum progress value |
| `color` | string | No | `blue` | Bar color: `blue`, `green`, `red`, `yellow`, `purple` |
| `showLabel` | boolean | No | `true` | Whether to show percentage text |

### HTML Adapter

```typescript
import { ExtensionAdapter, ExtensionContext, VellNode } from "@vell-lang/extensions";

function renderProgressBar(node: VellNode, ctx: ExtensionContext): string {
  const props = node.props ?? {};
  const value = Number(props.value ?? 0);
  const max = Number(props.max ?? 100);
  const color = String(props.color ?? "blue");
  const showLabel = props.showLabel !== false;

  const percent = Math.min(100, Math.max(0, (value / max) * 100));
  const colorMap: Record<string, string> = {
    blue: "#3182ce",
    green: "#38a169",
    red: "#e53e3e",
    yellow: "#d69e2e",
    purple: "#805ad5",
  };
  const barColor = colorMap[color] ?? colorMap.blue;

  const labelHtml = showLabel
    ? `<span class="progress-label">${Math.round(percent)}%</span>`
    : "";

  return `<div class="vell-progress-bar" role="progressbar" aria-valuenow="${value}" aria-valuemin="0" aria-valuemax="${max}">
    <div class="progress-track">
      <div class="progress-fill" style="width: ${percent}%; background-color: ${barColor};"></div>
    </div>
    ${labelHtml}
  </div>`;
}

export const ProgressBar: ExtensionAdapter = {
  name: "npm/progress",
  description: "Visual progress bar",
  schema: {
    value: { type: "number", required: true },
    max: { type: "number", default: 100 },
    color: { type: "string", default: "blue" },
    showLabel: { type: "boolean", default: true },
  },
  render: renderProgressBar,
};
```

### Usage Example

```vell
== Project Status

- Research: @[npm/progress](value=100 color=green)
- Design: @[npm/progress](value=80 color=blue)
- Implementation: @[npm/progress](value=45 color=yellow)
- Testing: @[npm/progress](value=20 color=red showLabel=false)
```

**Renderers that don't support the extension** will display the inline component text with a fallback label:
```html
<span class="vell-component">[npm/progress]</span>
```

---

> **License:** AGPL-3.0-or-later
> **Author:** Samin Yeasar
> **Version:** 1.0
