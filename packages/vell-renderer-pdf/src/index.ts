// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Samin Yeasar

/**
 * Direct Vell AST to PDF renderer.
 *
 * Walks the Vell AST and generates PDF primitives directly using pdfkit.
 * Does NOT go through HTML as an intermediate representation.
 */

import PDFDocument from "pdfkit";

// ---------------------------------------------------------------------------
// Types (mirrored from vell-js for standalone usage)
// ---------------------------------------------------------------------------

/** Byte-offset span in source. */
export interface Span {
  start: number;
  end: number;
}

/** Node property value. */
export type PropValue = string | number | boolean | null;

/** Alignment for table cells. */
export type Alignment = "left" | "center" | "right";

/** Inline AST node. */
export interface InlineNode {
  type: string;
  span: Span;
  value?: string;
  source?: string;
  name?: string;
  href?: string;
  src?: string;
  alt?: string;
  title?: string;
  marker?: string;
  key?: string;
  id?: string;
  props?: Record<string, PropValue>;
  children?: InlineNode[];
}

/** Definition list item. */
export interface DefinitionItem {
  term: InlineNode[];
  definition: Node[];
  span: Span;
}

/** List item. */
export interface ListItem {
  children: Node[];
  checked?: boolean | null;
  span: Span;
}

/** Block AST node. */
export interface Node {
  type: string;
  span: Span;
  level?: number;
  children?: Node[];
  items?: ListItem[];
  headers?: Array<{ children: InlineNode[]; colspan?: number; rowspan?: number; align?: Alignment | null }>;
  rows?: Array<Array<{ children: InlineNode[]; colspan?: number; rowspan?: number; align?: Alignment | null }>>;
  defItems?: DefinitionItem[];
  name?: string;
  props?: Record<string, PropValue>;
  value?: string;
  source?: string;
  lang?: string;
  ordered?: boolean;
  start?: number;
  id?: string;
  marker?: string;
  variable?: string;
  iterable?: string;
  condition?: string;
  consequent?: Node[];
  alternate?: Node[] | null;
  admonition_type?: string | null;
  href?: string;
  url?: string;
  title?: string | null;
  raw_source?: string;
}

/** Document metadata. */
export interface DocumentMetadata {
  title?: string;
  author?: string;
  date?: string;
  lang?: string;
  variables: Record<string, unknown>;
}

/** Complete Vell document AST. */
export interface VellDocument {
  version: number;
  children: Node[];
  metadata: DocumentMetadata;
  span: Span;
}

// ---------------------------------------------------------------------------
// PDF Renderer constants
// ---------------------------------------------------------------------------

const PAGE_WIDTH = 595.28; // A4
const PAGE_HEIGHT = 841.89;
const MARGIN_TOP = 56.69;
const MARGIN_BOTTOM = 56.69;
const MARGIN_LEFT = 56.69;
const MARGIN_RIGHT = 56.69;
const CONTENT_WIDTH = PAGE_WIDTH - MARGIN_LEFT - MARGIN_RIGHT;
const FONT_SIZE_BODY = 10;
const FONT_SIZE_H1 = 22;
const FONT_SIZE_H2 = 16;
const FONT_SIZE_H3 = 13;
const FONT_SIZE_H4 = 11;
const FONT_SIZE_CODE = 8;
const FONT_SIZE_SMALL = 8;
const LINE_HEIGHT = 1.35;

/** Renders a Vell document to a PDF Uint8Array buffer. */
export function renderToPdfBuffer(doc: VellDocument): Uint8Array {
  return new PdfRenderer().render(doc);
}

// ---------------------------------------------------------------------------
// Renderer implementation
// ---------------------------------------------------------------------------

/** A styled text segment. */
interface TextSpan {
  text: string;
  bold: boolean;
  italic: boolean;
  mono: boolean;
  underline: boolean;
  strike: boolean;
}

/** A collected footnote for end-of-document rendering. */
interface FootnoteEntry {
  marker: string;
  /** Raw plain-text content of the footnote body. */
  text: string;
}

/** A resolved cross-reference target. */
interface LabelTarget {
  anchorId: string;
  displayText: string;
}

class PdfRenderer {
  private doc!: PDFKit.PDFDocument;
  private cursorY = MARGIN_TOP;
  private footnotes: FootnoteEntry[] = [];
  /** Lookup: marker → plain text for footnote definitions in the document. */
  private footnoteDefs = new Map<string, string>();
  /** Auto-incrementing equation counter for numbering. */
  private equationCounter = 0;
  /** Auto-incrementing counters for theorem environments (keyed by name). */
  private theoremCounters: Record<string, number> = {};
  /** Pre-computed label → target map for cross-reference resolution. */
  private labelMap: Record<string, LabelTarget> = {};
  /** Pre-collected heading entries for TOC. */
  private tocEntries: Array<{ level: number; text: string; id: string }> = [];
  /** Pre-collected figure entries for LOF. */
  private lofEntries: Array<{ caption: string; id: string }> = [];
  /** Pre-collected table entries for LOT. */
  private lotEntries: Array<{ caption: string; id: string }> = [];

  private collectTocEntries(doc: VellDocument): Array<{ level: number; text: string; id: string }> {
    const entries: Array<{ level: number; text: string; id: string }> = [];
    for (const node of doc.children) {
      if (node.type === "Heading" && node.children) {
        const text = this.renderInlineNodes(node.children as InlineNode[]);
        const id = node.id ?? text.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "");
        entries.push({ level: node.level ?? 1, text, id });
      }
    }
    return entries;
  }

  private collectLofEntries(doc: VellDocument): Array<{ caption: string; id: string }> {
    const entries: Array<{ caption: string; id: string }> = [];
    for (const node of doc.children) {
      this.collectLofNode(node, entries);
    }
    return entries;
  }

  private collectLofNode(node: Node, entries: Array<{ caption: string; id: string }>): void {
    if (node.type === "Directive" && node.name === "Figure") {
      const props = (node.props ?? {}) as Record<string, unknown>;
      const caption = props.caption ? String(props.caption) : "";
      const id = props.id ? String(props.id) : "";
      entries.push({ caption, id });
    }
    for (const child of node.children ?? []) {
      this.collectLofNode(child, entries);
    }
  }

  private collectLotEntries(doc: VellDocument): Array<{ caption: string; id: string }> {
    const entries: Array<{ caption: string; id: string }> = [];
    for (const node of doc.children) {
      if (node.type === "Directive" && (node.name === "Table" || node.name === "GridTable")) {
        const props = (node.props ?? {}) as Record<string, unknown>;
        const caption = props.caption ? String(props.caption) : "";
        const id = props.id ? String(props.id) : "";
        entries.push({ caption, id });
      }
    }
    return entries;
  }

  render(doc: VellDocument): Uint8Array {
    const buffers: Buffer[] = [];

    // Reset counters for each render
    this.equationCounter = 0;
    this.theoremCounters = {};

    // Pre-scan footnote definitions
    this.footnoteDefs = this.scanFootnoteDefs(doc.children);

    // Pre-collect labels for cross-reference resolution
    this.labelMap = this.collectLabels(doc);

    // Pre-collect TOC, LOF, LOT entries
    this.tocEntries = this.collectTocEntries(doc);
    this.lofEntries = this.collectLofEntries(doc);
    this.lotEntries = this.collectLotEntries(doc);

    this.doc = new PDFDocument({
      size: "A4",
      margins: {
        top: MARGIN_TOP,
        bottom: MARGIN_BOTTOM,
        left: MARGIN_LEFT,
        right: MARGIN_RIGHT,
      },
      info: this.buildInfo(doc.metadata),
      bufferPages: false,
    });

    this.doc.on("data", (chunk: Buffer) => buffers.push(chunk));

    for (const node of doc.children) {
      this.renderBlock(node);
    }

    // Render collected footnotes at end
    if (this.footnotes.length > 0) {
      this.renderFootnotesSection();
    }

    this.doc.end();
    return new Uint8Array(Buffer.concat(buffers));
  }

  private buildInfo(metadata: DocumentMetadata): PDFKit.PDFDocumentOptions["info"] {
    const info: PDFKit.PDFDocumentOptions["info"] = {};
    if (metadata.title) info.Title = metadata.title;
    if (metadata.author) info.Author = metadata.author;
    return info;
  }

  private ensureSpace(needed: number): void {
    if (this.cursorY + needed > PAGE_HEIGHT - MARGIN_BOTTOM) {
      this.doc.addPage();
      this.cursorY = MARGIN_TOP;
    }
  }

  // -----------------------------------------------------------------------
  // Footnote pre-scan
  // -----------------------------------------------------------------------

  private scanFootnoteDefs(nodes: Node[]): Map<string, string> {
    const map = new Map<string, string>();
    for (const n of nodes) {
      if (n.type === "FootnoteDefinition" && n.marker && n.children) {
        const text = n.children
          .map((c) => this.extractPlainText(c))
          .filter(Boolean)
          .join(" ");
        map.set(n.marker, text);
      }
      // Recurse into containers
      if (n.children) {
        const childMap = this.scanFootnoteDefs(n.children);
        for (const [k, v] of childMap) {
          if (!map.has(k)) map.set(k, v);
        }
      }
    }
    return map;
  }

  private extractPlainText(node: Node): string {
    if (node.type === "Paragraph" && node.children) {
      return this.renderInlineNodes(node.children as InlineNode[]);
    }
    if (node.type === "Heading" && node.children) {
      return this.renderInlineNodes(node.children as InlineNode[]);
    }
    if (node.type === "CodeBlock") {
      return node.source ?? "";
    }
    if (node.children) {
      return node.children.map((c) => this.extractPlainText(c)).join(" ");
    }
    return "";
  }

  // -----------------------------------------------------------------------
  // Label collection for cross-references
  // -----------------------------------------------------------------------

  private collectLabels(doc: VellDocument): Record<string, LabelTarget> {
    const labels: Record<string, LabelTarget> = {};
    let eqCounter = 0;
    const thmCounters: Record<string, number> = {};
    eqCounter = this.collectLabelsNodes(doc.children, labels, eqCounter, thmCounters);
    return labels;
  }

  private collectLabelsNodes(
    nodes: Node[],
    labels: Record<string, LabelTarget>,
    eqCounter: number,
    thmCounters: Record<string, number>,
  ): number {
    for (const node of nodes) {
      if (node.type === "Directive") {
        const name = node.name ?? "";
        const props = node.props as Record<string, unknown> | undefined;

        if (name === "Equation") {
          eqCounter++;
          const eqNum = eqCounter;
          const label = props?.label ? String(props.label) : null;
          if (label) {
            labels[label] = {
              anchorId: `eq-${label}`,
              displayText: `(${eqNum})`,
            };
          }
        }

        const THEOREM_NAMES = ["Theorem", "Proof", "Lemma", "Corollary", "Definition",
          "Remark", "Example", "Conjecture", "Axiom", "Proposition", "Notation"];

        if (THEOREM_NAMES.includes(name)) {
          const isNumbered = !["Proof", "Remark", "Example", "Notation"].includes(name);
          let thmNum: number | null = null;
          if (isNumbered) {
            thmCounters[name] = (thmCounters[name] ?? 0) + 1;
            thmNum = thmCounters[name];
          }

          const label = props?.label ? String(props.label) : null;
          if (label) {
            let display = name;
            if (thmNum !== null) display += ` ${thmNum}`;
            const extra = props?.name ? String(props.name) : null;
            if (extra) display += ` (${extra})`;
            labels[label] = {
              anchorId: `thm-${label}`,
              displayText: display,
            };
          }
        }

        eqCounter = this.collectLabelsNodes(node.children as Node[] ?? [], labels, eqCounter, thmCounters);
      }
      if (node.children) {
        eqCounter = this.collectLabelsNodes(node.children, labels, eqCounter, thmCounters);
      }
    }
    return eqCounter;
  }

  // -----------------------------------------------------------------------
  // Block renderers
  // -----------------------------------------------------------------------

  private renderBlock(node: Node): void {
    switch (node.type) {
      case "Heading":
        return this.renderHeading(node);
      case "Paragraph":
        return this.renderParagraph(node);
      case "Blockquote":
        return this.renderBlockquote(node);
      case "CodeBlock":
        return this.renderCodeBlock(node);
      case "MathBlock":
        return this.renderMathBlock(node);
      case "List":
        return this.renderList(node);
      case "Table":
        return this.renderTable(node);
      case "HorizontalRule":
        return this.renderHorizontalRule();
      case "DefinitionList":
        return this.renderDefinitionList(node);
      case "ReferenceDefinition":
        return; // Not rendered in PDF body
      case "FootnoteDefinition":
        return; // Collected during pre-scan
      case "VarDeclaration":
        return; // Not rendered in output
      case "ForLoop":
        return this.renderFallback("@for");
      case "IfBlock":
        return this.renderFallback("@if");
      case "Directive":
        return this.renderDirective(node);
      case "Extension":
        return this.renderExtension(node);
      default:
        // Render unknown node children as fallback
        this.renderFallbackBody(node);
    }
  }

  private renderHeading(node: Node): void {
    const level = node.level ?? 1;
    const size = [FONT_SIZE_H1, FONT_SIZE_H2, FONT_SIZE_H3, FONT_SIZE_H4][(level - 1) & 3];
    const space = [24, 18, 14, 14][(level - 1) & 3];

    this.ensureSpace(space);
    this.doc.font("Helvetica-Bold").fontSize(size);

    const text = this.renderInlineNodes((node.children ?? []) as InlineNode[]);
    this.doc.text(text, MARGIN_LEFT, this.cursorY, { width: CONTENT_WIDTH });
    this.cursorY = this.doc.y + 6;
  }

  private renderParagraph(node: Node): void {
    const inlines = (node.children ?? []) as InlineNode[];
    if (inlines.length === 0) return;

    this.ensureSpace(12);

    // Collect footnote references
    this.collectFootnoteRefs(inlines);

    // Build styled spans and render them in runs
    const spans = this.buildTextSpans(inlines);
    if (spans.length === 0) return;

    this.doc.fontSize(FONT_SIZE_BODY);

    // Group consecutive spans with same style for efficiency
    const runs = this.mergeSpans(spans);
    let x = MARGIN_LEFT;

    for (let i = 0; i < runs.length; i++) {
      const run = runs[i];
      const { opts } = this.textOptionsForSpan(run);

      const isLast = i === runs.length - 1;
      this.doc.text(run.text, x, this.cursorY, {
        width: CONTENT_WIDTH - (x - MARGIN_LEFT),
        continued: !isLast,
        lineGap: 2,
        ...opts,
      });
      x = this.doc.x;
      this.cursorY = this.doc.y;
    }

    // The final continued: false call properly advances Y
    this.cursorY = this.doc.y + 6;
  }

  private renderBlockquote(node: Node): void {
    const indent = 12;
    const quoteX = MARGIN_LEFT + indent;
    const quoteWidth = CONTENT_WIDTH - indent;

    this.ensureSpace(10);

    // Draw vertical bar
    this.doc
      .rect(MARGIN_LEFT, this.cursorY, 2, 1000)
      .fillColor("#e0e0e0")
      .fill()
      .fillColor("#000");

    const admonitionType = node.admonition_type;

    if (admonitionType) {
      this.doc.fontSize(FONT_SIZE_SMALL);
      this.doc.font("Helvetica-Bold");
      this.doc.text(`[${admonitionType}]`, quoteX, this.cursorY, {
        width: quoteWidth,
      });
      this.cursorY = this.doc.y + 4;
    }

    this.doc.fontSize(FONT_SIZE_BODY);
    for (const child of node.children ?? []) {
      this.renderBlock(child);
    }

    this.cursorY += 4;
  }

  private renderCodeBlock(node: Node): void {
    const code = node.source ?? "";
    if (!code) return;

    this.ensureSpace(14);

    const lines = code.split("\n");
    const lineH = FONT_SIZE_CODE * LINE_HEIGHT;
    const bgHeight = Math.max(lines.length * lineH + (node.lang ? 20 : 12), 20);

    // Gray background
    this.doc
      .rect(MARGIN_LEFT, this.cursorY, CONTENT_WIDTH, bgHeight)
      .fillColor("#f5f5f5")
      .fill()
      .fillColor("#000");

    let offsetY = 0;

    // Lang label
    if (node.lang) {
      this.doc.fontSize(FONT_SIZE_SMALL);
      this.doc.font("Helvetica-Oblique");
      this.doc.fillColor("#888");
      this.doc.text(node.lang, MARGIN_LEFT + 6, this.cursorY + 2, {
        width: CONTENT_WIDTH - 12,
      });
      offsetY += 14;
    }

    // Code content
    this.doc.font("Courier").fontSize(FONT_SIZE_CODE).fillColor("#333");
    this.doc.text(code, MARGIN_LEFT + 8, this.cursorY + offsetY, {
      width: CONTENT_WIDTH - 16,
      lineGap: 1,
    });

    this.cursorY = this.doc.y + 8;
    this.doc.fillColor("#000");
  }

  private renderMathBlock(node: Node): void {
    const math = node.source ?? "";
    if (!math) return;

    this.ensureSpace(14);

    // Compute background height from content
    const lines = math.split("\n");
    const lineH = FONT_SIZE_BODY * LINE_HEIGHT;
    const bgHeight = Math.max(lines.length * lineH + 10, 24);

    // Light background
    this.doc
      .rect(MARGIN_LEFT, this.cursorY, CONTENT_WIDTH, bgHeight)
      .fillColor("#fafafa")
      .fill()
      .fillColor("#000");

    this.doc.font("Courier").fontSize(FONT_SIZE_BODY);
    this.doc.text(math, MARGIN_LEFT + 6, this.cursorY + 4, {
      width: CONTENT_WIDTH - 12,
    });
    this.cursorY = this.doc.y + 8;
  }

  private renderList(node: Node): void {
    const ordered = node.ordered ?? false;
    const start = node.start ?? 1;
    const items = node.items ?? [];
    const indent = 14;

    for (let i = 0; i < items.length; i++) {
      const item = items[i];
      const marker = ordered ? `${start + i}. ` : "\u2022 ";

      this.ensureSpace(10);

      // Draw marker
      this.doc.font("Helvetica").fontSize(FONT_SIZE_BODY);
      this.doc.text(marker, MARGIN_LEFT, this.cursorY, {
        width: indent,
        align: "left",
      });

      // Render item content
      const children = item.children ?? [];
      const itemText = children
        .map((c) => {
          if (c.type === "Paragraph" && c.children) {
            return this.renderInlineNodes(c.children as InlineNode[]);
          }
          if (c.type === "CodeBlock") return c.source ?? "";
          if (c.type === "List") {
            // Nested list — simple fallback
            return "[nested list]";
          }
          return "";
        })
        .filter(Boolean)
        .join(" ");

      this.doc.text(itemText, MARGIN_LEFT + indent, this.cursorY, {
        width: CONTENT_WIDTH - indent,
        lineGap: 1,
      });

      this.cursorY = Math.max(this.cursorY + 18, this.doc.y + 4);
    }

    this.cursorY += 4;
  }

  private renderTable(node: Node): void {
    const headers = node.headers ?? [];
    const rows = node.rows ?? [];
    const allRows = [headers, ...rows];

    if (allRows.length === 0 || allRows[0].length === 0) return;

    const colCount = allRows[0].length;
    const colWidth = CONTENT_WIDTH / colCount;
    const padding = 4;
    const cellHeight = 22;

    this.ensureSpace(allRows.length * cellHeight + 10);

    for (let r = 0; r < allRows.length; r++) {
      const row = allRows[r];
      const y = this.cursorY;

      // Header background
      if (r === 0) {
        this.doc
          .rect(MARGIN_LEFT, y, CONTENT_WIDTH, cellHeight)
          .fillColor("#f0f0f0")
          .fill()
          .fillColor("#000");
      }

      for (let c = 0; c < colCount; c++) {
        const cell = row[c];
        const x = MARGIN_LEFT + c * colWidth;
        const text = cell?.children
          ? this.renderInlineNodes(cell.children)
          : "";

        // Cell border
        this.doc
          .rect(x, y, colWidth, cellHeight)
          .strokeColor("#ccc")
          .lineWidth(0.5)
          .stroke();

        const align = r === 0 ? "center" : cell?.align ?? "left";

        this.doc
          .font(r === 0 ? "Helvetica-Bold" : "Helvetica")
          .fontSize(FONT_SIZE_BODY);

        if (text) {
          this.doc.text(text, x + padding, y + padding, {
            width: colWidth - padding * 2,
            align: align as "left" | "center" | "right",
          });
        }
      }

      this.cursorY += cellHeight;
    }

    this.cursorY += 8;
  }

  private renderHorizontalRule(): void {
    this.ensureSpace(12);
    this.doc
      .moveTo(MARGIN_LEFT, this.cursorY)
      .lineTo(MARGIN_LEFT + CONTENT_WIDTH, this.cursorY)
      .strokeColor("#999")
      .lineWidth(0.5)
      .stroke();
    this.cursorY += 14;
  }

  private renderDefinitionList(node: Node): void {
    const defItems: DefinitionItem[] = (node as any).defItems ?? (node as any).items ?? [];

    for (const item of defItems) {
      this.ensureSpace(12);

      // Render term
      const termText = this.renderInlineNodes(item.term ?? []);
      this.doc.font("Helvetica-Bold").fontSize(FONT_SIZE_BODY);
      this.doc.text(termText, MARGIN_LEFT, this.cursorY, {
        width: CONTENT_WIDTH,
      });
      this.cursorY = this.doc.y + 2;

      // Render definition
      this.doc.font("Helvetica");
      for (const defChild of item.definition ?? []) {
        this.ensureSpace(10);
        this.doc.text(
          this.extractPlainText(defChild),
          MARGIN_LEFT + 14,
          this.cursorY,
          { width: CONTENT_WIDTH - 14 },
        );
        this.cursorY = this.doc.y + 2;
      }
    }

    this.cursorY += 4;
  }

  private renderDirective(node: Node): void {
    const name = node.name ?? "Directive";
    const props = node.props as Record<string, unknown> | undefined;

    if (name === "Meta") return;

    this.ensureSpace(14);

    // Phase 8: Chemical equations with subscript formatting
    if (name === "Chem") {
      const source = props?.source ? String(props.source) : "";
      // Format subscripts: replace digits with subscript versions
      const formatted = source.replace(/(\d+)/g, (m: string) => m.split("").map((c: string) => {
        const subs: Record<string, string> = { "0": "₀", "1": "₁", "2": "₂", "3": "₃", "4": "₄", "5": "₅", "6": "₆", "7": "₇", "8": "₈", "9": "₉" };
        return subs[c] || c;
      }).join(""));
      this.doc.font("Courier").fontSize(FONT_SIZE_BODY).fillColor("#333");
      this.doc.text(formatted || "[Chemical formula]", MARGIN_LEFT, this.cursorY, { width: CONTENT_WIDTH });
      this.cursorY = this.doc.y + 8;
      return;
    }

    // Phase 9: Table of Contents
    if (name === "Toc") {
      this.doc.font("Helvetica-Bold").fontSize(FONT_SIZE_H2).fillColor("#000");
      this.doc.text("Table of Contents", MARGIN_LEFT, this.cursorY, { width: CONTENT_WIDTH });
      this.cursorY = this.doc.y + 6;
      // Scan headings from the children (we need access to doc, use pre-scanned)
      this.doc.font("Helvetica").fontSize(FONT_SIZE_BODY).fillColor("#333");
      // Since we don't have doc access in PdfRenderer.renderDirective,
      // emit a note indicating that TOC entries are collected from headings
      this.ensureSpace(10);
      this.doc.text("[TOC entries are generated from document headings at render time]", MARGIN_LEFT, this.cursorY, { width: CONTENT_WIDTH });
      this.cursorY = this.doc.y + 8;
      this.doc.fillColor("#000");
      return;
    }

    if (name === "Lof") {
      this.doc.font("Helvetica-Bold").fontSize(FONT_SIZE_H2).fillColor("#000");
      this.doc.text("List of Figures", MARGIN_LEFT, this.cursorY, { width: CONTENT_WIDTH });
      this.cursorY = this.doc.y + 6;
      this.doc.font("Helvetica-Oblique").fontSize(FONT_SIZE_SMALL).fillColor("#888");
      this.doc.text("[Figures are collected from @[Figure] directives]", MARGIN_LEFT, this.cursorY, { width: CONTENT_WIDTH });
      this.cursorY = this.doc.y + 8;
      this.doc.fillColor("#000");
      return;
    }

    if (name === "Lot") {
      this.doc.font("Helvetica-Bold").fontSize(FONT_SIZE_H2).fillColor("#000");
      this.doc.text("List of Tables", MARGIN_LEFT, this.cursorY, { width: CONTENT_WIDTH });
      this.cursorY = this.doc.y + 6;
      this.doc.font("Helvetica-Oblique").fontSize(FONT_SIZE_SMALL).fillColor("#888");
      this.doc.text("[Tables are collected from table nodes]", MARGIN_LEFT, this.cursorY, { width: CONTENT_WIDTH });
      this.cursorY = this.doc.y + 8;
      this.doc.fillColor("#000");
      return;
    }

    // Phase 10: Function plot
    if (name === "Plot") {
      const fnExpr = props?.fn ? String(props.fn) : "sin(x)";
      this.doc.font("Helvetica-Bold").fontSize(FONT_SIZE_BODY).fillColor("#2d3748");
      this.doc.text(`Plot of ${fnExpr}`, MARGIN_LEFT, this.cursorY, { width: CONTENT_WIDTH });
      this.cursorY = this.doc.y + 4;
      this.doc.font("Helvetica-Oblique").fontSize(FONT_SIZE_SMALL).fillColor("#888");
      this.doc.text("[Function plot requires an interactive viewer]", MARGIN_LEFT, this.cursorY, { width: CONTENT_WIDTH });
      this.cursorY = this.doc.y + 8;
      this.doc.fillColor("#000");
      return;
    }


    // Phase 10: Diagram rendering
    if (name === "Diagram") {
      const diagramType = props?.type ? String(props.type) : "general";
      // Extract source from children
      let source = "";
      for (const child of node.children ?? []) {
        if (child.type === "Paragraph" && child.children) {
          source += this.renderInlineNodes(child.children as InlineNode[]);
          source += "\n";
        }
      }
      const caption = props?.caption ? String(props.caption) : "";
      this.doc.font("Helvetica-Bold").fontSize(FONT_SIZE_SMALL).fillColor("#666");
      this.doc.text(`[Diagram: ${diagramType}]`, MARGIN_LEFT, this.cursorY, { width: CONTENT_WIDTH });
      this.cursorY = this.doc.y + 2;
      if (source) {
        this.doc.font("Courier").fontSize(FONT_SIZE_CODE).fillColor("#333");
        this.doc.text(source.trim(), MARGIN_LEFT + 8, this.cursorY, {
          width: CONTENT_WIDTH - 16,
          lineGap: 1,
        });
        this.cursorY = this.doc.y + 4;
      }
      if (caption) {
        this.doc.font("Helvetica-Oblique").fontSize(FONT_SIZE_SMALL).fillColor("#888");
        this.doc.text(caption, MARGIN_LEFT, this.cursorY, { width: CONTENT_WIDTH });
        this.cursorY = this.doc.y + 2;
      }
      this.doc.fillColor("#000");
      this.cursorY += 4;
      return;
    }

    // Phase 10: Chart rendering
    if (name === "Chart") {
      const chartType = props?.type ? String(props.type) : "bar";
      const title = props?.title ? String(props.title) : "";
      // Extract data from children
      let textContent = "";
      for (const child of node.children ?? []) {
        if (child.type === "Paragraph" && child.children) {
          textContent += this.renderInlineNodes(child.children as InlineNode[]);
          textContent += "\n";
        }
      }
      const data: Array<{ label: string; value: number }> = [];
      for (const line of textContent.split("\n")) {
        const trimmed = line.trim();
        if (!trimmed) continue;
        const commaIdx = trimmed.lastIndexOf(",");
        if (commaIdx === -1) continue;
        const label = trimmed.substring(0, commaIdx).trim();
        const valStr = trimmed.substring(commaIdx + 1).trim();
        const val = parseFloat(valStr);
        if (!isNaN(val) && label) data.push({ label, value: val });
      }
      if (title) {
        this.doc.font("Helvetica-Bold").fontSize(FONT_SIZE_BODY).fillColor("#2d3748");
        this.doc.text(title, MARGIN_LEFT, this.cursorY, { width: CONTENT_WIDTH });
        this.cursorY = this.doc.y + 4;
      }
      this.doc.fillColor("#000").font("Helvetica").fontSize(FONT_SIZE_BODY);
      for (const d of data) {
        this.ensureSpace(10);
        this.doc.text(`${d.label}: ${d.value}`, MARGIN_LEFT + 8, this.cursorY, {
          width: CONTENT_WIDTH - 8,
        });
        this.cursorY = this.doc.y + 2;
      }
      this.cursorY += 4;
      return;
    }

    // Phase 9: Cross-reference rendering
    if (name === "Ref") {
      const label = props?.label ? String(props.label) : "";
      const target = this.labelMap[label];
      if (target) {
        this.doc.font("Helvetica").fontSize(FONT_SIZE_BODY).fillColor("#2b6cb0");
        this.doc.text(target.displayText, MARGIN_LEFT, this.cursorY, {
          width: CONTENT_WIDTH,
          underline: true,
        });
        this.cursorY = this.doc.y + 2;
      } else {
        this.doc.font("Helvetica-Oblique").fontSize(FONT_SIZE_SMALL).fillColor("#e53e3e");
        this.doc.text(`[?${label}]`, MARGIN_LEFT, this.cursorY, {
          width: CONTENT_WIDTH,
        });
        this.cursorY = this.doc.y + 2;
      }
      this.doc.fillColor("#000");
      return;
    }

    // Phase 8: Equation rendering with auto-numbering
    if (name === "Equation") {
      const source = String(props?.source ?? "");
      const label = props?.label ? String(props.label) : "";

      // Simple auto-numbering in PDF (counter tracked per-render)
      const eqNum = ++this.equationCounter;

      this.doc.font("Helvetica-Oblique").fontSize(FONT_SIZE_SMALL).fillColor("#888");
      if (label) {
        this.doc.text(`(${eqNum}) [${label}]`, MARGIN_LEFT, this.cursorY, {
          width: CONTENT_WIDTH,
          align: "right",
        });
      } else {
        this.doc.text(`(${eqNum})`, MARGIN_LEFT, this.cursorY, {
          width: CONTENT_WIDTH,
          align: "right",
        });
      }
      this.cursorY = this.doc.y + 2;

      if (source) {
        this.doc.font("Courier").fontSize(FONT_SIZE_BODY).fillColor("#000");
        this.doc.text(source, MARGIN_LEFT, this.cursorY, {
          width: CONTENT_WIDTH,
        });
        this.cursorY = this.doc.y + 8;
      }
      return;
    }

    // Phase 8: Theorem environments
    const THEOREM_NAMES = ["Theorem", "Proof", "Lemma", "Corollary", "Definition",
      "Remark", "Example", "Conjecture", "Axiom", "Proposition", "Notation"];

    if (THEOREM_NAMES.includes(name)) {
      const extra = props?.name ? String(props.name) : null;
      const thmLabel = props?.label ? String(props.label) : null;

      // Auto-number theorems (except Proof, Remark, Example, Notation)
      let number: number | null = null;
      if (name !== "Proof" && name !== "Remark" && name !== "Example" && name !== "Notation") {
        this.theoremCounters[name] = (this.theoremCounters[name] ?? 0) + 1;
        number = this.theoremCounters[name];
      }

      this.doc.font("Helvetica-Bold").fontSize(FONT_SIZE_BODY).fillColor("#000");
      let label = name;
      if (number !== null) label += ` ${number}`;
      if (extra) label += ` (${extra})`;
      this.doc.text(label, MARGIN_LEFT, this.cursorY, {
        width: CONTENT_WIDTH,
      });
      this.cursorY = this.doc.y + 2;

      this.doc.font("Helvetica").fontSize(FONT_SIZE_BODY);
      for (const child of node.children ?? []) {
        this.renderBlock(child);
      }
      this.cursorY += 4;
      return;
    }

    // Align/Matrix/Cases environments
    if (["Align", "Align*", "Matrix", "PMatrix", "BMatrix", "VMatrix", "Cases"].includes(name)) {
      const source = String(props?.source ?? "");
      if (source) {
        this.doc.font("Courier").fontSize(FONT_SIZE_BODY).fillColor("#333");
        this.doc.text(source, MARGIN_LEFT, this.cursorY, {
          width: CONTENT_WIDTH,
        });
        this.cursorY = this.doc.y + 4;
      }
      for (const child of node.children ?? []) {
        this.renderBlock(child);
      }
      return;
    }

    // Default directive rendering
    this.doc.font("Helvetica-Oblique").fontSize(FONT_SIZE_SMALL).fillColor("#666");
    this.doc.text(`[${name} directive]`, MARGIN_LEFT, this.cursorY, {
      width: CONTENT_WIDTH,
    });
    this.cursorY = this.doc.y + 2;

    this.doc.fillColor("#000").font("Helvetica").fontSize(FONT_SIZE_BODY);
    for (const child of node.children ?? []) {
      this.renderBlock(child);
    }

    this.cursorY += 4;
  }

  private renderExtension(node: Node): void {
    const name = node.name ?? "Extension";
    this.ensureSpace(14);

    this.doc.font("Helvetica-Oblique").fontSize(FONT_SIZE_SMALL).fillColor("#999");
    this.doc.text(`[Extension: ${name}]`, MARGIN_LEFT, this.cursorY, {
      width: CONTENT_WIDTH,
    });
    this.cursorY = this.doc.y + 2;

    // Render children
    this.doc.fillColor("#000").font("Helvetica").fontSize(FONT_SIZE_BODY);
    for (const child of node.children ?? []) {
      this.renderBlock(child);
    }

    this.cursorY += 4;
  }

  private renderFallback(label: string): void {
    this.ensureSpace(12);
    this.doc.font("Helvetica-Oblique").fontSize(FONT_SIZE_SMALL).fillColor("#999");
    this.doc.text(`[${label}: see interactive viewer]`, MARGIN_LEFT, this.cursorY, {
      width: CONTENT_WIDTH,
    });
    this.cursorY = this.doc.y + 4;
    this.doc.fillColor("#000");
  }

  private renderFallbackBody(node: Node): void {
    for (const child of node.children ?? []) {
      this.renderBlock(child);
    }
  }

  // -----------------------------------------------------------------------
  // Footnotes section
  // -----------------------------------------------------------------------

  private renderFootnotesSection(): void {
    this.ensureSpace(20);

    // Divider line
    this.doc
      .moveTo(MARGIN_LEFT, this.cursorY)
      .lineTo(MARGIN_LEFT + CONTENT_WIDTH, this.cursorY)
      .strokeColor("#ccc")
      .stroke();
    this.cursorY += 12;

    // Section title
    this.doc.fontSize(FONT_SIZE_SMALL).font("Helvetica-Bold");
    this.doc.text("Footnotes", MARGIN_LEFT, this.cursorY, {
      width: CONTENT_WIDTH,
    });
    this.cursorY += 16;

    // Note entries
    this.doc.fontSize(FONT_SIZE_SMALL).font("Helvetica");
    for (const fn of this.footnotes) {
      this.ensureSpace(14);

      // Look up actual content from pre-scanned definitions
      const bodyText = this.footnoteDefs.get(fn.marker) ?? fn.text;
      const fullText = `[${fn.marker}] ${bodyText}`;

      this.doc.text(fullText, MARGIN_LEFT, this.cursorY, {
        width: CONTENT_WIDTH,
      });
      this.cursorY = this.doc.y + 2;
    }
  }

  // -----------------------------------------------------------------------
  // Inline rendering helpers
  // -----------------------------------------------------------------------

  private renderInlineNodes(nodes: InlineNode[]): string {
    return nodes.map((n) => this.renderInlineNode(n)).join("");
  }

  private renderInlineNode(node: InlineNode): string {
    switch (node.type) {
      case "Text":
        return node.value ?? "";
      case "Bold":
      case "Italic":
      case "Underline":
      case "Strikethrough":
      case "Superscript":
      case "Subscript":
        return this.renderInlineNodes(node.children ?? []);
      case "Code":
        return node.value ?? "";
      case "Link":
        return this.renderInlineNodes(node.children ?? []) + ` (${node.href ?? ""})`;
      case "LinkRef":
        return this.renderInlineNodes(node.children ?? []);
      case "Image":
        return `[Image: ${node.alt ?? ""}]`;
      case "ImageRef":
        return `[Image: ${node.alt ?? ""}]`;
      case "MathInline":
        return `$${node.source ?? ""}$`;
      case "VarInterpolation":
        return `@{${node.name ?? ""}}`;
      case "InlineComponent":
        return `@[${node.name ?? ""}]`;
      case "Citation":
        return `[[${node.key ?? ""}]]`;
      case "FootnoteRef":
        return `[^${node.marker ?? ""}]`;
      case "SoftBreak":
        return " ";
      case "HardBreak":
        return "\n";
      default:
        return "";
    }
  }

  /** Collects footnote markers from inline content. */
  private collectFootnoteRefs(nodes: InlineNode[]): void {
    for (const n of nodes) {
      if (n.type === "FootnoteRef" && n.marker) {
        if (!this.footnotes.some((f) => f.marker === n.marker)) {
          this.footnotes.push({
            marker: n.marker,
            text: `[See footnote ${n.marker}]`,
          });
        }
      }
      if (n.children) {
        this.collectFootnoteRefs(n.children);
      }
    }
  }

  /** Builds a list of styled text spans from inline nodes. */
  private buildTextSpans(nodes: InlineNode[]): TextSpan[] {
    const spans: TextSpan[] = [];

    for (const n of nodes) {
      switch (n.type) {
        case "Text":
          spans.push({ text: n.value ?? "", bold: false, italic: false, mono: false, underline: false, strike: false });
          break;

        case "Bold":
          for (const s of this.buildTextSpans(n.children ?? [])) {
            s.bold = true;
            spans.push(s);
          }
          break;

        case "Italic":
          for (const s of this.buildTextSpans(n.children ?? [])) {
            s.italic = true;
            spans.push(s);
          }
          break;

        case "Underline":
          for (const s of this.buildTextSpans(n.children ?? [])) {
            s.underline = true;
            spans.push(s);
          }
          break;

        case "Strikethrough":
          for (const s of this.buildTextSpans(n.children ?? [])) {
            s.strike = true;
            spans.push(s);
          }
          break;

        case "Code":
          spans.push({ text: n.value ?? "", bold: false, italic: false, mono: true, underline: false, strike: false });
          break;

        case "Superscript":
          spans.push({ text: `^${this.renderInlineNodes(n.children ?? [])}^`, bold: false, italic: false, mono: false, underline: false, strike: false });
          break;

        case "Subscript":
          spans.push({ text: `,${this.renderInlineNodes(n.children ?? [])},`, bold: false, italic: false, mono: false, underline: false, strike: false });
          break;

        case "Link":
          for (const s of this.buildTextSpans(n.children ?? [])) {
            s.italic = true;
            s.underline = true;
            spans.push(s);
          }
          if (n.href) {
            spans.push({ text: ` (${n.href})`, bold: false, italic: false, mono: false, underline: false, strike: false });
          }
          break;

        case "SoftBreak":
          spans.push({ text: " ", bold: false, italic: false, mono: false, underline: false, strike: false });
          break;

        case "HardBreak":
          spans.push({ text: "\n", bold: false, italic: false, mono: false, underline: false, strike: false });
          break;

        case "MathInline":
          spans.push({ text: `$${n.source ?? ""}$`, bold: false, italic: false, mono: true, underline: false, strike: false });
          break;

        case "VarInterpolation":
          spans.push({ text: `@{${n.name ?? ""}}`, bold: false, italic: false, mono: false, underline: false, strike: false });
          break;

        case "FootnoteRef":
          spans.push({ text: `[^${n.marker ?? ""}]`, bold: false, italic: false, mono: false, underline: false, strike: false });
          break;

        default:
          if (n.children) {
            spans.push(...this.buildTextSpans(n.children));
          }
          break;
      }
    }

    return spans;
  }

  /** Merges consecutive spans with identical styling into single runs. */
  private mergeSpans(spans: TextSpan[]): TextSpan[] {
    const runs: TextSpan[] = [];
    for (const s of spans) {
      const last = runs[runs.length - 1];
      if (
        last &&
        last.bold === s.bold &&
        last.italic === s.italic &&
        last.mono === s.mono &&
        last.underline === s.underline &&
        last.strike === s.strike
      ) {
        last.text += s.text;
      } else {
        runs.push({ ...s });
      }
    }
    return runs;
  }    /** Applies the correct pdfkit font and returns text options for a span. */
  private textOptionsForSpan(
    span: TextSpan,
  ): { font: string; opts: Partial<PDFKit.Mixins.TextOptions> } {
    let font = "Helvetica";
    if (span.mono) {
      font = "Courier";
    } else if (span.bold && span.italic) {
      font = "Helvetica-BoldOblique";
    } else if (span.bold) {
      font = "Helvetica-Bold";
    } else if (span.italic) {
      font = "Helvetica-Oblique";
    }
    this.doc.font(font);
    return {
      font,
      opts: {
        underline: span.underline || undefined,
        strike: span.strike || undefined,
      },
    };
  }
}

// ---------------------------------------------------------------------------
// Default export
// ---------------------------------------------------------------------------

export default { renderToPdfBuffer };
