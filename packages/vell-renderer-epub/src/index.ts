// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Samin Yeasar

/**
 * EPUB 3 renderer for Vell documents.
 *
 * Converts a parsed VellDocument AST into a valid EPUB 3 publication
 * (ZIP archive) with XHTML content, OPF metadata, navigation document,
 * and CSS styling.
 *
 * ## EPUB 3 Structure Produced
 *
 * ```
 * mimetype                 (uncompressed, first entry)
 * META-INF/
 *   container.xml          (points to OEBPS/content.opf)
 * OEBPS/
 *   content.opf            (package document)
 *   nav.xhtml              (navigation / table of contents)
 *   styles.css             (built-in EPUB styling)
 *   chapter-001.xhtml      (chapter content, 001..N)
 *   chapter-002.xhtml
 *   ...
 * ```
 *
 * ## Usage
 *
 * ```typescript
 * import { renderEpub } from "@solez-ai/vell-renderer-epub";
 * const epubData = await renderEpub(parsedDocument);
 * // epubData is a Uint8Array of the EPUB ZIP
 * ```
 */

import JSZip from "jszip";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/** Minimal Vell AST document accepted by the renderer. */
export interface VellDocument {
  children: VellNode[];
  metadata?: {
    title?: string;
    author?: string;
    date?: string;
    lang?: string;
    variables?: Record<string, unknown>;
  };
}

/** Minimal Vell node accepted by the renderer. */
export interface VellNode {
  type: string;
  children?: VellInline[] | VellNode[];
  [key: string]: unknown;
}

/** Minimal inline node accepted by the renderer. */
export interface VellInline {
  type: string;
  children?: VellInline[];
  [key: string]: unknown;
}

/** A chapter extracted from the document. */
interface Chapter {
  id: string;
  title: string;
  nodes: VellNode[];
}

// ---------------------------------------------------------------------------
// Main entry point
// ---------------------------------------------------------------------------

/**
 * Renders a VellDocument to an EPUB 3 Uint8Array (ZIP archive).
 *
 * The document is split into chapters at top-level Heading boundaries:
 * the first heading becomes the title page, and subsequent headings
 * (level 1 or 2) start new chapters.
 */
export async function renderEpub(doc: VellDocument): Promise<Uint8Array> {
  const metadata = doc.metadata ?? {};
  const lang = metadata.lang ?? "en";
  const title = metadata.title ?? "Untitled";
  const author = metadata.author ?? "Unknown";
  const now = new Date().toISOString().replace(/[-:]/g, "").split(".")[0] + "Z";

  // Split document into chapters
  const chapters = splitIntoChapters(doc, title);

  // Generate unique identifier
  const bookId = `urn:uuid:${generateUuid()}`;

  const zip = new JSZip();

  // -----------------------------------------------------------------------
  // 1. mimetype — MUST be first, uncompressed, no extra fields
  // -----------------------------------------------------------------------
  zip.file("mimetype", "application/epub+zip", {
    compression: "STORE",
    date: new Date(0), // fixed date for reproducibility
  });

  // -----------------------------------------------------------------------
  // 2. META-INF/container.xml
  // -----------------------------------------------------------------------
  zip.file(
    "META-INF/container.xml",
    `<?xml version="1.0" encoding="UTF-8"?>
<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
  <rootfiles>
    <rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/>
  </rootfiles>
</container>`,
  );

  // -----------------------------------------------------------------------
  // 3. OEBPS/styles.css
  // -----------------------------------------------------------------------
  zip.file("OEBPS/styles.css", EPUB_CSS);

  // -----------------------------------------------------------------------
  // 4. Chapter XHTML files
  // -----------------------------------------------------------------------
  const manifestItems: string[] = [];
  const spineItems: string[] = [];

  for (let i = 0; i < chapters.length; i++) {
    const ch = chapters[i];
    const fileId = `chapter-${String(i + 1).padStart(3, "0")}`;
    const fileName = `${fileId}.xhtml`;
    const isNav = i === 0; // first chapter is the title page

    manifestItems.push(
      `    <item id="${fileId}" href="${fileName}" media-type="application/xhtml+xml"/>`,
    );
    spineItems.push(
      `    <itemref idref="${fileId}"/>`,
    );

    const xhtml = renderChapterXhtml(ch, title, lang, isNav);
    zip.file(`OEBPS/${fileName}`, xhtml);
  }

  // -----------------------------------------------------------------------
  // 5. OEBPS/nav.xhtml — EPUB navigation document
  // -----------------------------------------------------------------------
  const navXhtml = renderNavXhtml(chapters, title, lang);
  zip.file("OEBPS/nav.xhtml", navXhtml);
  manifestItems.push(
    `    <item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/>`,
  );

  // -----------------------------------------------------------------------
  // 6. OEBPS/content.opf — Package document
  // -----------------------------------------------------------------------
  const opf = renderOpf(
    bookId,
    title,
    author,
    lang,
    now,
    manifestItems,
    spineItems,
  );
  zip.file("OEBPS/content.opf", opf);

  // -----------------------------------------------------------------------
  // Generate ZIP
  // -----------------------------------------------------------------------
  const blob = await zip.generateAsync({ type: "nodebuffer", mimeType: "application/epub+zip" });
  return new Uint8Array(blob);
}

// ---------------------------------------------------------------------------
// Chapter splitting
// ---------------------------------------------------------------------------

/**
 * Splits document children into chapters.
 * - Content before the first heading becomes the title page.
 * - Each level-1 or level-2 heading starts a new chapter.
 */
function splitIntoChapters(doc: VellDocument, docTitle: string): Chapter[] {
  const chapters: Chapter[] = [];
  let currentNodes: VellNode[] = [];
  let currentTitle = docTitle;
  let chapterCount = 0;

  const flush = () => {
    if (currentNodes.length > 0) {
      chapterCount++;
      chapters.push({
        id: `ch-${String(chapterCount).padStart(3, "0")}`,
        title: currentTitle,
        nodes: currentNodes,
      });
      currentNodes = [];
    }
  };

  for (const node of doc.children ?? []) {
    if (node.type === "Heading") {
      flush();
      currentTitle = textOf(node.children as VellInline[] | undefined);
      currentNodes.push(node);
    } else {
      currentNodes.push(node);
    }
  }
  flush();

  // If no headings were found, create a single chapter
  if (chapters.length === 0) {
    chapters.push({
      id: "ch-001",
      title: docTitle,
      nodes: doc.children ?? [],
    });
  }

  return chapters;
}

// ---------------------------------------------------------------------------
// XHTML chapter rendering
// ---------------------------------------------------------------------------

function renderChapterXhtml(
  chapter: Chapter,
  bookTitle: string,
  lang: string,
  isTitlePage: boolean,
): string {
  const dir = isRtlLanguage(lang) ? "rtl" : "ltr";
  const bodyClass = isTitlePage ? ` class="title-page"` : "";
  const bodyContent = chapter.nodes.map((n) => renderNodeToXhtml(n)).join("\n");

  return `<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE html>
<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops" lang="${escapeXml(lang)}" dir="${dir}">
<head>
  <meta charset="UTF-8"/>
  <title>${escapeXml(chapter.title)}</title>
  <link rel="stylesheet" type="text/css" href="styles.css"/>
</head>
<body${bodyClass}>
  <section epub:type="bodymatter">
    ${bodyContent}
  </section>
</body>
</html>`;
}

// ---------------------------------------------------------------------------
// Navigation document (nav.xhtml)
// ---------------------------------------------------------------------------

function renderNavXhtml(
  chapters: Chapter[],
  bookTitle: string,
  lang: string,
): string {
  const tocItems = chapters
    .map(
      (ch, i) =>
        `          <li><a href="chapter-${String(i + 1).padStart(3, "0")}.xhtml">${escapeXml(ch.title)}</a></li>`,
    )
    .join("\n");

  const dir = isRtlLanguage(lang) ? "rtl" : "ltr";
  return `<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE html>
<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops" lang="${escapeXml(lang)}" dir="${dir}">
<head>
  <meta charset="UTF-8"/>
  <title>${escapeXml(bookTitle)}</title>
  <link rel="stylesheet" type="text/css" href="styles.css"/>
</head>
<body>
  <nav epub:type="toc">
    <h1>Table of Contents</h1>
    <ol>
${tocItems}
    </ol>
  </nav>
  <nav epub:type="landmarks" hidden="hidden">
    <ol>
      <li><a epub:type="bodymatter" href="chapter-001.xhtml">Start of Content</a></li>
    </ol>
  </nav>
</body>
</html>`;
}

// ---------------------------------------------------------------------------
// OPF package document
// ---------------------------------------------------------------------------

function renderOpf(
  bookId: string,
  title: string,
  author: string,
  lang: string,
  modified: string,
  manifestItems: string[],
  spineItems: string[],
): string {
  return `<?xml version="1.0" encoding="UTF-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="book-id" xml:lang="${escapeXml(lang)}">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:dcterms="http://purl.org/dc/terms/">
    <dc:identifier id="book-id">${escapeXml(bookId)}</dc:identifier>
    <dc:title>${escapeXml(title)}</dc:title>
    <dc:creator id="creator">${escapeXml(author)}</dc:creator>
    <dc:language>${escapeXml(lang)}</dc:language>
    <meta property="dcterms:modified">${modified}</meta>
    <meta property="dcterms:created">${modified}</meta>
    <meta name="generator" content="Vell ${getVersion()}"/>
  </metadata>
  <manifest>
${manifestItems.join("\n")}
    <item id="css" href="styles.css" media-type="text/css"/>
  </manifest>
  <spine>
${spineItems.join("\n")}
  </spine>
</package>`;
}

// ---------------------------------------------------------------------------
// Node-to-XHTML rendering
// ---------------------------------------------------------------------------

function renderNodeToXhtml(node: VellNode): string {
  switch (node.type) {
    case "Heading": {
      const level = Math.min(6, Math.max(1, Number(node.level ?? 1)));
      const id = node.id ? ` id="${escapeXml(String(node.id))}"` : "";
      return `<h${level}${id}>${renderInlines(node.children as VellInline[] | undefined)}</h${level}>`;
    }
    case "Paragraph":
      return `<p>${renderInlines(node.children as VellInline[] | undefined)}</p>`;
    case "Blockquote": {
      const admon =
        node.admonition_type
          ? ` class="admonition admonition-${escapeXml(String(node.admonition_type).toLowerCase())}"`
          : "";
      const children = (node.children as VellNode[] ?? [])
        .map(renderNodeToXhtml)
        .join("\n");
      return `<blockquote${admon}>${children}</blockquote>`;
    }
    case "CodeBlock": {
      const lang = node.lang
        ? ` class="language-${escapeXml(String(node.lang))}"`
        : "";
      const source = escapeXml(String(node.source ?? ""));
      return `<pre><code${lang}>${source}</code></pre>`;
    }
    case "MathBlock": {
      const src = String(node.source ?? "");
      return `<pre class="math-block">${escapeXml(src)}</pre>`;
    }
    case "List": {
      const tag = node.ordered ? "ol" : "ul";
      const startAttr =
        node.ordered && node.start != null
          ? ` start="${escapeXml(String(node.start))}"`
          : "";
      const items = (node.items as VellNode[] ?? [])
        .map((item: VellNode) => {
          const checkedAttr =
            item.checked != null
              ? ` data-checked="${escapeXml(String(item.checked))}"`
              : "";
          const children = (item.children as VellNode[] ?? [])
            .map(renderNodeToXhtml)
            .join("");
          return `<li${checkedAttr}>${children}</li>`;
        })
        .join("\n");
      return `<${tag}${startAttr}>${items}</${tag}>`;
    }
    case "Table": {
      const headers = (node.headers as VellNode[] ?? [])
        .map(
          (c) =>
            `<th>${renderInlines(c.children as VellInline[] | undefined)}</th>`,
        )
        .join("");
      const rows = (node.rows as VellNode[][] ?? [])
        .map(
          (row) =>
            `<tr>${row
              .map((c) => {
                const colspan =
                  c.colspan && Number(c.colspan) > 1
                    ? ` colspan="${escapeXml(String(c.colspan))}"`
                    : "";
                const align = c.align
                  ? ` style="text-align:${escapeXml(String(c.align))}"`
                  : "";
                return `<td${colspan}${align}>${renderInlines(c.children as VellInline[] | undefined)}</td>`;
              })
              .join("")}</tr>`,
        )
        .join("");
      const head =
        headers.length > 0 ? `<thead><tr>${headers}</tr></thead>` : "";
      const body =
        rows.length > 0 ? `<tbody>${rows}</tbody>` : "";
      return `<table>${head}${body}</table>`;
    }
    case "HorizontalRule":
      return "<hr/>";
    case "DefinitionList": {
      const items = (node.items as Array<Record<string, unknown>> ?? [])
        .map((item) => {
          const term = renderInlines(item.term as VellInline[] | undefined);
          const defs = (item.definition as VellNode[] ?? [])
            .map(renderNodeToXhtml)
            .join("");
          return `<dt>${term}</dt><dd>${defs}</dd>`;
        })
        .join("\n");
      return `<dl>${items}</dl>`;
    }
    case "Directive": {
      return renderDirectiveToXhtml(node);
    }
    // ForFor, IfBlock render as div with data attributes (informational)
    case "ForLoop":
    case "IfBlock":
    case "VarDeclaration":
    case "ReferenceDefinition":
    case "FootnoteDefinition":
      return "";
    default:
      return (node.children as VellNode[] ?? [])
        .map(renderNodeToXhtml)
        .join("\n");
  }
}

// ---------------------------------------------------------------------------
// Directive rendering for EPUB
// ---------------------------------------------------------------------------

function renderDirectiveToXhtml(node: VellNode): string {
  const name = String(node.name ?? "");
  const props = node.props as Record<string, unknown> | undefined;

  switch (name) {
    case "Figure": {
      const src = String(props?.src ?? "");
      const alt = escapeXml(String(props?.alt ?? ""));
      const caption = escapeXml(String(props?.caption ?? ""));
      return `<figure><img src="${src}" alt="${alt}"/><figcaption>${caption}</figcaption></figure>`;
    }
    case "Diagram": {
      const diagramType = String(props?.type ?? "general");
      const caption = props?.caption
        ? `\n    <figcaption>${escapeXml(String(props.caption))}</figcaption>`
        : "";
      return `<figure class="diagram diagram-${escapeXml(diagramType)}"><pre>${escapeXml(extractTextContent(node))}</pre>${caption}</figure>`;
    }
    case "Chart": {
      const title = props?.title ? escapeXml(String(props.title)) : "";
      const t = title ? `<caption>${title}</caption>` : "";
      return `<figure class="chart"><table>${t}${renderChartDataTable(node)}</table></figure>`;
    }
    case "Code": {
      const source = escapeXml(String(props?.source ?? ""));
      return `<pre><code>${source}</code></pre>`;
    }
    case "Theorem":
    case "Proof":
    case "Lemma":
    case "Corollary":
    case "Definition":
    case "Remark":
    case "Example":
    case "Conjecture":
    case "Axiom":
    case "Proposition":
    case "Notation": {
      const children = (node.children as VellNode[] ?? [])
        .map(renderNodeToXhtml)
        .join("\n");
      const extra = props?.name ? ` (${escapeXml(String(props.name))})` : "";
      return `<div class="theorem theorem-${escapeXml(name.toLowerCase())}"><span class="theorem-label">${escapeXml(name)}${extra}.</span> ${children}</div>`;
    }
    case "Equation": {
      const source = String(props?.source ?? "");
      return `<div class="equation"><pre>${escapeXml(source)}</pre></div>`;
    }
    case "Align":
    case "Matrix":
    case "PMatrix":
    case "BMatrix":
    case "VMatrix":
    case "Cases": {
      const source = String(props?.source ?? "");
      return `<div class="math-env math-${escapeXml(name.toLowerCase())}"><pre>${escapeXml(source)}</pre></div>`;
    }
    case "Chem": {
      const source = String(props?.source ?? extractTextContent(node));
      return `<code class="chem-formula">${formatChemFormula(source)}</code>`;
    }
    case "Cite": {
      const key = escapeXml(String(props?.key ?? ""));
      return `<cite>[${key}]</cite>`;
    }
    case "Slide": {
      // Slides not meaningful in EPUB — render children inline
      return (node.children as VellNode[] ?? [])
        .map(renderNodeToXhtml)
        .join("\n");
    }
    default:
      return "";
  }
}

// ---------------------------------------------------------------------------
// Inline node rendering
// ---------------------------------------------------------------------------

function renderInlines(nodes: VellInline[] | undefined): string {
  return (nodes ?? []).map(renderInlineToXhtml).join("");
}

function renderInlineToXhtml(node: VellInline): string {
  switch (node.type) {
    case "Text":
      return escapeXml(String(node.value ?? ""));
    case "Bold":
      return `<strong>${renderInlines(node.children)}</strong>`;
    case "Italic":
      return `<em>${renderInlines(node.children)}</em>`;
    case "Underline":
      return `<u>${renderInlines(node.children)}</u>`;
    case "Strikethrough":
      return `<del>${renderInlines(node.children)}</del>`;
    case "Code":
      return `<code>${escapeXml(String(node.value ?? ""))}</code>`;
    case "Superscript":
      return `<sup>${renderInlines(node.children)}</sup>`;
    case "Subscript":
      return `<sub>${renderInlines(node.children)}</sub>`;
    case "Link": {
      const href = String(node.href ?? "");
      const title = node.title
        ? ` title="${escapeXml(String(node.title))}"`
        : "";
      return `<a href="${href}"${title}>${renderInlines(node.children)}</a>`;
    }
    case "Image": {
      const src = String(node.src ?? "");
      const alt = escapeXml(String(node.alt ?? ""));
      const title = node.title
        ? ` title="${escapeXml(String(node.title))}"`
        : "";
      return `<img src="${src}" alt="${alt}"${title}/>`;
    }
    case "MathInline": {
      const src = String(node.source ?? "");
      return `<code class="math-inline">${escapeXml(src)}</code>`;
    }
    case "VarInterpolation":
      return `<span class="vell-var">@{escapeXml(String(node.name ?? ""))}</span>`;
    case "Citation":
      return `<cite>[${escapeXml(String(node.key ?? ""))}]</cite>`;
    case "FootnoteRef": {
      const marker = String(node.marker ?? "");
      return `<a class="footnote-ref" href="#fn-${escapeXml(marker)}"><sup>[${escapeXml(marker)}]</sup></a>`;
    }
    case "SoftBreak":
      return "\n";
    case "HardBreak":
      return "<br/>";
    default:
      return "";
  }
}

// ---------------------------------------------------------------------------
// Chart data rendering
// ---------------------------------------------------------------------------

function renderChartDataTable(node: VellNode): string {
  const props = node.props as Record<string, unknown> | undefined;
  const title = props?.title ? String(props.title) : "";
  const source = extractTextContent(node);
  let html = "";
  if (title) {
    html += `<caption>${escapeXml(title)}</caption>`;
  }
  html += "<thead><tr><th>Label</th><th>Value</th></tr></thead><tbody>";
  for (const line of source.split("\n")) {
    const trimmed = line.trim();
    if (!trimmed) continue;
    const commaIdx = trimmed.lastIndexOf(",");
    if (commaIdx === -1) continue;
    const label = trimmed.substring(0, commaIdx).trim();
    const val = trimmed.substring(commaIdx + 1).trim();
    html += `<tr><td>${escapeXml(label)}</td><td>${escapeXml(val)}</td></tr>`;
  }
  html += "</tbody>";
  return html;
}

// ---------------------------------------------------------------------------
// Utility functions
// ---------------------------------------------------------------------------

function textOf(nodes: VellInline[] | undefined): string {
  return (nodes ?? [])
    .map((n) =>
      n.type === "Text" ? String(n.value ?? "") : textOf(n.children),
    )
    .join("");
}

function extractTextContent(node: VellNode): string {
  const parts: string[] = [];
  for (const child of node.children as VellNode[] ?? []) {
    if (child.type === "Paragraph" && child.children) {
      for (const inline of child.children as VellInline[]) {
        if (inline.type === "Text") {
          parts.push(String(inline.value ?? ""));
        }
      }
    }
  }
  return parts.join("\n");
}

function formatChemFormula(source: string): string {
  // Wrap numbers in <sub> tags for chemical formulas
  return source.replace(/(\d+)/g, "<sub>$1</sub>");
}

function escapeXml(value: string): string {
  return value
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&apos;");
}

/** Generates a v4 UUID string for the book identifier. */
function generateUuid(): string {
  return "xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx".replace(/[xy]/g, (c) => {
    const r = (Math.random() * 16) | 0;
    return (c === "x" ? r : (r & 0x3) | 0x8).toString(16);
  });
}

function getVersion(): string {
  return "1.0";
}

/** Detect if a language code is right-to-left (Arabic, Hebrew, Persian, Urdu, etc.). */
function isRtlLanguage(lang: string): boolean {
  const rtlLangs = [
    "ar", "arc", "bcc", "bqi", "ckb", "dv", "fa", "glk",
    "ha", "he", "khw", "ks", "ku", "mzn", "nqo", "pa",
    "ps", "sd", "ug", "ur", "uz", "yi",
  ];
  const base = lang.split("-")[0].toLowerCase();
  return rtlLangs.includes(base);
}

// ---------------------------------------------------------------------------
// EPUB CSS
// ---------------------------------------------------------------------------

const EPUB_CSS = `/* Vell EPUB 3 Default Stylesheet */
@page {
  margin: 1em;
}

body {
  font-family: Georgia, "Times New Roman", serif;
  font-size: 1em;
  line-height: 1.5;
  color: #1a1a1a;
  margin: 1em 1em 0;
  padding: 0;
}

/* Title page */
body.title-page {
  text-align: center;
  padding-top: 20%;
}
body.title-page h1 {
  font-size: 2em;
  margin-bottom: 0.5em;
}

/* Headings */
h1 { font-size: 1.6em; margin-top: 1.5em; margin-bottom: 0.5em; page-break-before: always; }
h2 { font-size: 1.3em; margin-top: 1.2em; margin-bottom: 0.4em; }
h3 { font-size: 1.15em; margin-top: 1em; margin-bottom: 0.3em; }
h4, h5, h6 { font-size: 1em; margin-top: 0.8em; margin-bottom: 0.3em; }

/* Paragraphs */
p {
  margin: 0.5em 0;
  text-indent: 0;
  orphans: 2;
  widows: 2;
}

/* Blockquotes and admonitions */
blockquote {
  margin: 0.8em 1em;
  padding: 0.3em 0.8em;
  border-left: 3px solid #aaa;
  background: #f9f9f9;
}
blockquote.admonition {
  border-left-width: 4px;
  padding: 0.5em 0.8em;
}
blockquote.admonition-note { border-color: #3182ce; }
blockquote.admonition-tip { border-color: #38a169; }
blockquote.admonition-warning { border-color: #dd6b20; }
blockquote.admonition-important { border-color: #d69e2e; }
blockquote.admonition-caution { border-color: #e53e3e; }

/* Code blocks */
pre {
  font-family: "Courier New", Courier, monospace;
  font-size: 0.85em;
  background: #f5f5f5;
  border: 1px solid #ddd;
  padding: 0.5em;
  white-space: pre-wrap;
  word-wrap: break-word;
  margin: 0.5em 0;
}
code {
  font-family: "Courier New", Courier, monospace;
  font-size: 0.9em;
  background: #f5f5f5;
  padding: 0.1em 0.3em;
}
pre code {
  background: transparent;
  padding: 0;
}

/* Lists */
ul, ol { margin: 0.5em 0; padding-left: 1.5em; }
li { margin: 0.2em 0; }
li[data-checked]::before {
  content: attr(data-checked) " ";
  font-family: monospace;
}

/* Tables */
table {
  border-collapse: collapse;
  width: 100%;
  margin: 0.8em 0;
  font-size: 0.9em;
}
th, td {
  border: 1px solid #ccc;
  padding: 0.3em 0.5em;
  text-align: left;
}
th { background: #eee; font-weight: bold; }

/* Theorems and math environments */
div.theorem {
  margin: 0.8em 0;
  padding: 0.5em 0.8em;
  border-left: 3px solid #666;
  background: #f8f8f8;
}
span.theorem-label {
  font-weight: bold;
}
div.equation, div.math-env {
  margin: 0.5em 0;
  text-align: center;
}
div.equation pre, div.math-env pre {
  display: inline-block;
  text-align: left;
}

/* Figures */
figure {
  margin: 0.8em 0;
  text-align: center;
}
figcaption {
  font-style: italic;
  font-size: 0.9em;
  margin-top: 0.3em;
}

/* Footnotes */
a.footnote-ref { text-decoration: none; }
.math-inline { font-style: italic; }

/* Chemical formulas */
code.chem-formula sub { font-size: 0.75em; }

/* Inline styling */
strong { font-weight: bold; }
em { font-style: italic; }
u { text-decoration: underline; }
del { text-decoration: line-through; }
sup { font-size: 0.75em; vertical-align: super; }
sub { font-size: 0.75em; vertical-align: sub; }

/* Page break helpers */
h1 { page-break-before: always; }
h1:first-of-type { page-break-before: avoid; }
h2, h3, h4 { page-break-after: avoid; }
pre, table, figure, blockquote, div.theorem { page-break-inside: avoid; }

/* Phase 19: High contrast theme */
@media (prefers-contrast: high) {
  body { color: #000; background: #fff; }
  a { color: #0000cc; text-decoration: underline; }
  pre, code { background: #fff; border: 2px solid #000; }
  table th, table td { border: 2px solid #000; }
  th { background: #e0e0e0; }
  blockquote { border-left: 4px solid #000; background: #fff; }
  div.theorem { border-left: 4px solid #000; background: #fff; }
  figure { border: 1px solid #000; }
}

/* Phase 19: CJK typography */
:lang(zh) body { font-family: "Noto Sans SC", "PingFang SC", "Microsoft YaHei", "Hiragino Sans GB", serif; line-height: 1.9; }
:lang(ja) body { font-family: "Noto Sans JP", "Hiragino Sans", "Yu Gothic", "Meiryo", serif; line-height: 1.9; }
:lang(ko) body { font-family: "Noto Sans KR", "Apple SD Gothic Neo", "Malgun Gothic", serif; line-height: 1.9; }
:lang(zh) pre, :lang(ja) pre, :lang(ko) pre { font-family: "Noto Sans Mono CJK SC", "Source Han Sans SC", "Courier New", monospace; }
`;
