// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Samin Yeasar

import { getRuntimeScript } from "./runtime.js";

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

/** A resolved cross-reference target. */
interface LabelTarget {
  /** Anchor ID for the destination (e.g., "eq-e:mass-energy"). */
  anchorId: string;
  /** Display text to show in the reference (e.g., "(1)" or "Theorem 1"). */
  displayText: string;
}

/** Render-time context shared across all render functions. */
interface RenderContext {
  /** Footnote definitions keyed by marker. */
  footnotes: Map<string, VellNode>;
  /** Set of footnote markers that have at least one reference in the document. */
  footnoteRefs: Set<string>;
  /** Auto-incrementing equation counter for numbering. */
  equationCounter: number;
  /** Auto-incrementing counters for theorem environments (keyed by name). */
  theoremCounters: Record<string, number>;
  /** Pre-computed label → target map for cross-reference resolution. */
  labelMap: Record<string, LabelTarget>;
  /** Pre-collected TOC entries: (level, text, id). */
  tocEntries: Array<{ level: number; text: string; id: string }>;
  /** Pre-collected LOF entries: (caption, id). */
  lofEntries: Array<{ caption: string; id: string }>;
  /** Pre-collected LOT entries: (caption, id). */
  lotEntries: Array<{ caption: string; id: string }>;
}

// ---------------------------------------------------------------------------
// Top‑level API
// ---------------------------------------------------------------------------

/** Renders a document to a complete HTML string. */
export function render(doc: VellDocument, options?: { interactive?: boolean }): string {
  const ctx = buildRenderContext(doc);
  const content = (doc.children ?? []).map(n => renderNode(n, ctx)).join("\n");
  const footnotesHtml = renderFootnotesSection(ctx);
  const title = doc.metadata?.title
    ? `<title>${escapeHtml(doc.metadata.title)}</title>`
    : "";
  const langAttr = doc.metadata?.lang
    ? ` lang="${escapeAttr(doc.metadata.lang)}"`
    : "";
  const dirAttr = doc.metadata?.lang
    ? ` dir="${isRtlLanguage(doc.metadata.lang) ? "rtl" : "ltr"}"`
    : "";
  // Embed the reactive runtime for interactive documents
  const runtimeScript = options?.interactive
    ? `\n<script defer>${getRuntimeScript()}</script>`
    : "";
  return (
    `<!doctype html>\n` +
    `<html${langAttr}${dirAttr}><head><meta charset="utf-8">${title}<style>\n${VELL_CSS}\n</style>${runtimeScript}</head>` +
    `<body>${content}${footnotesHtml}</body></html>`
  );
}

/** Renders a document to a complete interactive HTML string with embedded runtime. */
export function renderInteractive(doc: VellDocument): string {
  return render(doc, { interactive: true });
}

/** Renders a document to a browser DocumentFragment. */
export function renderToFragment(doc: VellDocument): DocumentFragment {
  const ctx = buildRenderContext(doc);
  const template = document.createElement("template");
  const content = (doc.children ?? []).map(n => renderNode(n, ctx)).join("\n");
  const footnotesHtml = renderFootnotesSection(ctx);
  template.innerHTML = content + footnotesHtml;
  return template.content;
}

// ---------------------------------------------------------------------------
// Context building – collects footnote definitions in a pre‑pass
// ---------------------------------------------------------------------------

function buildRenderContext(doc: VellDocument): RenderContext {
  const footnotes = new Map<string, VellNode>();
  collectFootnotes(doc.children ?? [], footnotes);
  return {
    footnotes,
    footnoteRefs: new Set<string>(),
    equationCounter: 0,
    theoremCounters: {},
    labelMap: collectLabels(doc),
    tocEntries: collectTocEntries(doc),
    lofEntries: collectLofEntries(doc),
    lotEntries: collectLotEntries(doc),
  };
}

function collectFootnotes(
  nodes: VellNode[],
  map: Map<string, VellNode>,
): void {
  for (const node of nodes) {
    if (node.type === "FootnoteDefinition" && node.marker != null) {
      map.set(String(node.marker), node);
    }
    if (node.children && Array.isArray(node.children)) {
      collectFootnotes(node.children as VellNode[], map);
    }
  }
}

function renderFootnotesSection(ctx: RenderContext): string {
  if (ctx.footnotes.size === 0) return "";
  const items: string[] = [];
  for (const [marker, node] of ctx.footnotes) {
    const childrenHtml = (node.children as VellNode[] ?? [])
      .map(n => renderNode(n, ctx))
      .join("\n");
    const backlink =
      ctx.footnoteRefs.has(marker)
        ? ` <a href="#fnref-${escapeAttr(marker)}" aria-label="Back to reference">\u21A9</a>`
        : "";
    items.push(
      `<li id="fn-${escapeAttr(marker)}">` +
        `<span class="fn-marker">^${escapeHtml(marker)}</span> ` +
        `${childrenHtml}${backlink}</li>`,
    );
  }
  return (
    `\n<section class="footnotes" role="doc-footnotes">` +
    `<h2>Footnotes</h2><ol>${items.join("\n")}</ol></section>`
  );
}

// ---------------------------------------------------------------------------
// Block‑level node rendering
// ---------------------------------------------------------------------------

function renderNode(node: VellNode, ctx: RenderContext): string {
  switch (node.type) {
    /* ---- existing block types ---------------------------------------- */
    case "Heading": {
      const level = node.level ?? 1;
      const id = String(
        node.id ?? slug(textOf(node.children as VellInline[])),
      );
      return `<h${level} id="${escapeAttr(id)}">${renderInlineList(node.children as VellInline[], ctx)}</h${level}>`;
    }
    case "Paragraph":
      return `<p>${renderInlineList(node.children as VellInline[], ctx)}</p>`;
    case "Blockquote": {
      const admon = node.admonition_type
        ? ` class="vell-admonition vell-${escapeAttr(String(node.admonition_type))}"`
        : "";
      return `<blockquote${admon}>${(node.children as VellNode[] ?? []).map(n => renderNode(n, ctx)).join("\n")}</blockquote>`;
    }
    case "CodeBlock": {
      const lang = node.lang ? ` class="language-${escapeAttr(String(node.lang))}"` : "";
      const execAttr = node.executable ? ` data-executable="true"` : "";
      return `<pre><code${lang}${execAttr}>${escapeHtml(String(node.source ?? ""))}</code></pre>`;
    }
    case "MathBlock": {
      const src = String(node.source ?? "");
      const mathml = latexToMathml(src, true);
      const alttext = escapeAttr(mathmlToPlainText(mathml));
      return `<math display="block" alttext="${alttext}">${mathml}</math>`;
    }
    case "List": {
      const tag = node.ordered ? "ol" : "ul";
      const startAttr =
        node.ordered && node.start != null ? ` start="${escapeAttr(String(node.start))}"` : "";
      return `<${tag}${startAttr}>${(node.items as VellNode[] ?? []).map((item: VellNode) => {
        const checkedAttr =
          item.checked != null
            ? ` data-checked="${escapeAttr(String(item.checked))}"`
            : "";
        return `<li${checkedAttr}>${(item.children as VellNode[] ?? []).map(n => renderNode(n, ctx)).join("")}</li>`;
      }).join("")}</${tag}>`;
    }
    case "Table":
      return renderTable(node, ctx);
    case "HorizontalRule":
      return `<hr>`;

    /* ---- newly supported block types --------------------------------- */
    case "DefinitionList":
      return renderDefinitionList(node, ctx);
    case "ReferenceDefinition": {
      const id = escapeAttr(String(node.id ?? ""));
      const url = sanitizeUrl(String(node.url ?? ""));
      const title = node.title ? ` title="${escapeAttr(String(node.title))}"` : "";
      return `<data class="vell-ref-def" data-id="${id}" data-url="${url}"${title}></data>`;
    }
    case "FootnoteDefinition": {
      // Collected in pre‑pass; skip inline rendering.
      return "";
    }
    case "VarDeclaration": {
      const name = escapeAttr(String(node.name ?? ""));
      const value = stringifyValue(node.value);
      return `<meta itemprop="${name}" content="${escapeAttr(value)}">`;
    }
    case "ForLoop": {
      const variable = escapeAttr(String(node.variable ?? ""));
      const iterable = escapeAttr(String(node.iterable ?? ""));
      const children = (node.children as VellNode[] ?? [])
        .map(n => renderNode(n, ctx))
        .join("\n");
      return `<div class="vell-for" data-variable="${variable}" data-iterable="${iterable}">\n${children}\n</div>`;
    }
    case "IfBlock": {
      const condition = escapeAttr(String(node.condition ?? ""));
      const consequent = (node.consequent as VellNode[] ?? [])
        .map(n => renderNode(n, ctx))
        .join("\n");
      let html = `<div class="vell-if" data-condition="${condition}">\n${consequent}\n</div>`;
      const alternate = node.alternate as VellNode[] | undefined;
      if (alternate && alternate.length > 0) {
        const altHtml = alternate.map(n => renderNode(n, ctx)).join("\n");
        html += `\n<div class="vell-else" data-condition="${condition}">\n${altHtml}\n</div>`;
      }
      return html;
    }

    /* ---- directives and extensions ----------------------------------- */
    case "Directive":
      return renderDirective(node, ctx);
    case "Extension": {
      const name = escapeAttr(String(node.name ?? ""));
      const children = (node.children as VellNode[] ?? [])
        .map(n => renderNode(n, ctx))
        .join("\n");
      return `<div class="vell-extension" data-name="${name}">\n${children}\n</div>`;
    }

    /* ---- fallback – render children ----------------------------------- */
    default:
      return (node.children as VellNode[] ?? []).map(n => renderNode(n, ctx)).join("\n");
  }
}

// ---------------------------------------------------------------------------
// Directive rendering
// ---------------------------------------------------------------------------

function renderDirective(node: VellNode, ctx: RenderContext): string {
  const name = String(node.name ?? "");
  const props = node.props as Record<string, unknown> | undefined;

  switch (name) {
    /* --- existing directives --- */
    case "Figure": {
      const src = sanitizeUrl(String(props?.src ?? ""));
      const alt = escapeAttr(String(props?.alt ?? ""));
      const caption = escapeHtml(String(props?.caption ?? ""));
      return `<figure><img src="${src}" alt="${alt}"><figcaption>${caption}</figcaption></figure>`;
    }
    case "Slide":
      return `<section class="vell-slide">${(node.children as VellNode[] ?? []).map(n => renderNode(n, ctx)).join("\n")}</section>`;
    case "Layout": {
      const cols = escapeAttr(String(props?.columns ?? 1));
      return `<div class="vell-layout vell-cols-${cols}">${(node.children as VellNode[] ?? []).map(n => renderNode(n, ctx)).join("\n")}</div>`;
    }

    /* --- missing directives --- */
    case "Code": {
      const lang = props?.lang ? ` data-lang="${escapeAttr(String(props.lang))}"` : "";
      const execAttr = props?.executable ? ` data-executable="true"` : "";
      const source = escapeHtml(String(props?.source ?? ""));
      return `<div class="vell-code"${lang}${execAttr}>\n<pre><code>${source}</code></pre>\n</div>`;
    }
    case "Diagram": {
      const diagramType = props?.type ? String(props.type) : "general";
      const caption = props?.caption ? String(props.caption) : "";
      // Extract source from children text
      const source = extractTextFromChildren(node.children as VellNode[]);
      const escapedSource = escapeHtml(source);
      const captionHtml = caption
        ? `\n<div class="diagram-caption">${escapeHtml(caption)}</div>`
        : "";
      let body: string;
      switch (diagramType) {
        case "mermaid":
          body = `\n<div class="mermaid">\n${escapedSource}\n</div>`;
          break;
        case "dot":
          body = `\n<div class="graphviz">\n<pre class="dot">\n${escapedSource}\n</pre>\n</div>`;
          break;
        default:
          body = `\n<pre>\n${escapedSource}\n</pre>`;
      }
      return `<div class="vell-diagram" data-type="${escapeAttr(diagramType)}">${body}${captionHtml}\n</div>`;
    }
    case "Chart": {
      const chartType = String(props?.type ?? "bar");
      const title = props?.title ? String(props.title) : "";
      const source = extractTextFromChildren(node.children as VellNode[]);
      const data: Array<{ label: string; value: number }> = [];
      for (const line of source.split("\n")) {
        const trimmed = line.trim();
        if (!trimmed) continue;
        const commaIdx = trimmed.lastIndexOf(",");
        if (commaIdx === -1) continue;
        const label = trimmed.substring(0, commaIdx).trim();
        const valStr = trimmed.substring(commaIdx + 1).trim();
        const val = parseFloat(valStr);
        if (!isNaN(val) && label) {
          data.push({ label, value: val });
        }
      }
      if (chartType === "bar" && data.length > 0) {
        const svg = renderBarChartSvg(data, title);
        return `<div class="vell-chart vell-chart-bar">\n${svg}\n</div>`;
      }
      let html = `<div class="vell-chart vell-chart-${escapeAttr(chartType)}">\n`;
      if (title) html += `<div class="chart-title">${escapeHtml(title)}</div>\n`;
      html += `<table>\n`;
      for (const d of data) {
        html += `<tr><td>${escapeHtml(d.label)}</td><td>${d.value}</td></tr>\n`;
      }
      html += `</table>\n</div>`;
      return html;
    }
    case "Cite": {
      const key = escapeAttr(String(props?.key ?? ""));
      const text = escapeHtml(String(props?.text ?? ""));
      return `<cite class="vell-cite" data-key="${key}">${text}</cite>`;
    }
    case "Animation": {
      const url = sanitizeUrl(String(props?.url ?? ""));
      const autoplay = props?.autoplay ? " autoplay" : "";
      const loop = props?.loop ? " loop" : "";
      const caption = props?.caption
        ? `<figcaption>${escapeHtml(String(props.caption))}</figcaption>`
        : "";
      return `<figure class="vell-animation"><video src="${url}" controls${autoplay}${loop}></video>${caption}</figure>`;
    }
    case "Frame": {
      const url = sanitizeUrl(String(props?.url ?? ""));
      const title = escapeAttr(String(props?.title ?? "Embedded content"));
      return `<iframe src="${url}" title="${title}" sandbox="allow-scripts"></iframe>`;
    }
    case "Accessibility": {
      const role = escapeAttr(String(props?.role ?? "region"));
      const label = props?.label
        ? ` aria-label="${escapeAttr(String(props.label))}"`
        : "";
      const children = (node.children as VellNode[] ?? [])
        .map(n => renderNode(n, ctx))
        .join("\n");
      return `<div role="${role}"${label}>${children || ""}</div>`;
    }
    case "Theme": {
      const url = sanitizeUrl(String(props?.url ?? ""));
      return `<link rel="stylesheet" href="${url}">`;
    }
    case "Meta": {
      const nameAttr = escapeAttr(String(props?.name ?? ""));
      const content = escapeAttr(String(props?.content ?? ""));
      return `<meta name="${nameAttr}" content="${content}">`;
    }
    /* --- Phase 13: Template/Theme directives --- */
    case "Template": {
      const name = props?.name ? escapeAttr(String(props.name)) : "";
      const url = props?.url ? sanitizeUrl(String(props.url)) : "";
      const style = props?.style ? String(props.style) : "";
      let html = "";
      if (url) {
        html += `<link rel="stylesheet" href="${url}">\n`;
      }
      if (name) {
        html += `<meta name="vell-template" content="${name}">\n`;
      }
      if (style) {
        html += `<style>\n${style}\n</style>\n`;
      }
      const children = (node.children as VellNode[] ?? [])
        .map(n => renderNode(n, ctx))
        .join("\n");
      html += children;
      return html;
    }

    /* --- Phase 8: Chemical equations with subscript formatting --- */
    case "Chem": {
      const source = String(props?.source ?? extractTextFromChildren(node.children as VellNode[]));
      return `<div class="vell-chem">\n<code class="chem-formula">${formatChemFormula(source)}</code>\n</div>`;
    }

    /* --- Phase 9: Table of Contents --- */
    case "Toc": {
      let items = "";
      if (ctx.tocEntries.length === 0) {
        items = `<li class="toc-placeholder">(no headings found)</li>`;
      } else {
        items = ctx.tocEntries.map(e => {
          const indent = "  ".repeat((e.level - 1));
          return `${indent}<li><a href="#${escapeAttr(e.id)}">${escapeHtml(e.text)}</a></li>`;
        }).join("\n");
      }
      return `<nav class="vell-toc" role="toc">\n<h2>Table of Contents</h2>\n<ol class="toc-list">\n${items}\n</ol>\n</nav>`;
    }
    case "Lof": {
      let items = "";
      if (ctx.lofEntries.length === 0) {
        items = `<li class="lof-placeholder">(no figures found)</li>`;
      } else {
        items = ctx.lofEntries.map(e => {
          const href = e.id ? ` href="#${escapeAttr(e.id)}"` : "";
          return `<li><a${href}>${e.caption ? escapeHtml(e.caption) : "(unnamed figure)"}</a></li>`;
        }).join("\n");
      }
      return `<nav class="vell-lof" role="lof">\n<h2>List of Figures</h2>\n<ul class="lof-list">\n${items}\n</ul>\n</nav>`;
    }
    case "Lot": {
      let items = "";
      if (ctx.lotEntries.length === 0) {
        items = `<li class="lot-placeholder">(no tables found)</li>`;
      } else {
        items = ctx.lotEntries.map(e => {
          const href = e.id ? ` href="#${escapeAttr(e.id)}"` : "";
          return `<li><a${href}>${e.caption ? escapeHtml(e.caption) : "(unnamed table)"}</a></li>`;
        }).join("\n");
      }
      return `<nav class="vell-lot" role="lot">\n<h2>List of Tables</h2>\n<ul class="lot-list">\n${items}\n</ul>\n</nav>`;
    }

    /* --- Phase 10: Function plot --- */
    case "Plot": {
      const fnExpr = escapeAttr(String(props?.fn ?? "sin(x)"));
      const xmin = Number(props?.xmin ?? -6);
      const xmax = Number(props?.xmax ?? 6);
      const ymin = Number(props?.ymin ?? -2);
      const ymax = Number(props?.ymax ?? 2);
      const width = 500;
      const height = 250;
      const pad = 40;
      const plotW = width - 2 * pad;
      const plotH = height - 2 * pad;
      const cx = pad + ((0 - xmin) / (xmax - xmin)) * plotW;
      const cy = pad + ((ymax - 0) / (ymax - ymin)) * plotH;
      
      let svg = `<svg width="${width}px" height="${height}px" viewBox="0 0 ${width} ${height}" xmlns="http://www.w3.org/2000/svg">\n`;
      svg += `<rect x="0" y="0" width="${width}" height="${height}" fill="#f8f9fa" rx="4"/>\n`;
      // Axes
      svg += `<line x1="${pad}" y1="${cy}" x2="${width - pad}" y2="${cy}" stroke="#a0aec0" stroke-width="1"/>\n`;
      svg += `<line x1="${cx}" y1="${pad}" x2="${cx}" y2="${height - pad}" stroke="#a0aec0" stroke-width="1"/>\n`;
      svg += `<text x="${width / 2}" y="20" text-anchor="middle" font-size="12" fill="#2d3748">Plot of ${escapeHtml(fnExpr)}</text>\n`;
      svg += `<text x="${width / 2}" y="${height - 8}" text-anchor="middle" font-size="10" fill="#718096">Function evaluation requires a JavaScript runtime.</text>\n`;
      svg += `</svg>\n`;
      return `<div class="vell-plot">\n${svg}\n</div>`;
    }

    /* --- Phase 11: Interactive form directives --- */
    case "Input": {
      const bind = props?.bind ? ` data-bind="${escapeAttr(String(props.bind))}"` : "";
      const inputType = escapeAttr(String(props?.type ?? "text"));
      const placeholder = props?.placeholder ? ` placeholder="${escapeAttr(String(props.placeholder))}"` : "";
      const input = `<input type="${inputType}"${placeholder}${bind}>`;
      if (props?.label) {
        return `<label>${escapeHtml(String(props.label))} ${input}</label>`;
      }
      return input;
    }
    case "Select": {
      const bind = props?.bind ? ` data-bind="${escapeAttr(String(props.bind))}"` : "";
      let optionsHtml = "";
      if (props?.options) {
        const opts = String(props.options).split(",");
        for (const opt of opts) {
          const trimmed = opt.trim();
          if (trimmed) optionsHtml += `<option value="${escapeAttr(trimmed)}">${escapeHtml(trimmed)}</option>`;
        }
      } else {
        const source = extractTextFromChildren(node.children as VellNode[]);
        for (const line of source.split("\n")) {
          const trimmed = line.trim();
          if (trimmed) optionsHtml += `<option value="${escapeAttr(trimmed)}">${escapeHtml(trimmed)}</option>`;
        }
      }
      const select = `<select${bind}>\n${optionsHtml}\n</select>`;
      if (props?.label) {
        return `<label>${escapeHtml(String(props.label))} ${select}</label>`;
      }
      return select;
    }
    case "Checkbox": {
      const bind = props?.bind ? ` data-bind="${escapeAttr(String(props.bind))}"` : "";
      const checked = props?.checked ? " checked" : "";
      const label = props?.label ? ` ${escapeHtml(String(props.label))}` : "";
      return `<label><input type="checkbox"${bind}${checked}>${label}</label>`;
    }
    case "Data": {
      if (props?.data) {
        return `<script type="application/json" data-vell-init>${JSON.stringify(props.data)}</script>`;
      }
      if (props?.source) {
        return `<meta data-vell-data="${escapeAttr(String(props.source))}">`;
      }
      return "";
    }
    case "Slider": {
      const min = escapeAttr(String(props?.min ?? "0"));
      const max = escapeAttr(String(props?.max ?? "100"));
      const value = escapeAttr(String(props?.value ?? "0"));
      const bind = props?.bind ? ` data-bind="${escapeAttr(String(props.bind))}"` : "";
      const input = `<input type="range" min="${min}" max="${max}" value="${value}"${bind}>`;
      if (props?.label) {
        return `<label>${escapeHtml(String(props.label))} ${input}</label>`;
      }
      return input;
    }

    /* --- Phase 8: Professional Math Engine --- */
    case "Equation":
      return renderEquation(node, ctx);

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
    case "Notation":
      return renderTheoremEnvironment(node, ctx);

    case "Ref":
      return renderRef(node, ctx);

    case "Align":
    case "Align*":
    case "Matrix":
    case "PMatrix":
    case "BMatrix":
    case "VMatrix":
    case "Cases":
      return renderMathEnv(node, ctx);

    /* --- fallback --- */
    default: {
      const safeName = escapeAttr(name);
      const children = (node.children as VellNode[] ?? [])
        .map(n => renderNode(n, ctx))
        .join("\n");
      return `<div class="vell-directive" data-name="${safeName}">\n${children}\n</div>`;
    }
  }
}

// ---------------------------------------------------------------------------
// Definition list rendering
// ---------------------------------------------------------------------------

function renderDefinitionList(node: VellNode, ctx: RenderContext): string {
  const items = node.items as Array<Record<string, unknown>> ?? [];
  const parts: string[] = ["<dl>"];
  for (const item of items) {
    const term = renderInlineList(item.term as VellInline[] | undefined, ctx);
    parts.push(`<dt>${term}</dt>`);
    const defs = item.definition as VellNode[] | undefined;
    if (defs) {
      for (const def of defs) {
        parts.push(`<dd>${renderNode(def, ctx)}</dd>`);
      }
    }
  }
  parts.push("</dl>");
  return parts.join("\n");
}

// ---------------------------------------------------------------------------
// Phase 9: Label collection for cross-references
// ---------------------------------------------------------------------------

function extractTextFromChildren(children: VellNode[]): string {
  const parts: string[] = [];
  for (const child of children) {
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

function renderBarChartSvg(
  data: Array<{ label: string; value: number }>,
  title: string,
): string {
  const width = 500;
  const height = 250;
  const padLeft = 60;
  const padRight = 20;
  const padTop = title ? 40 : 20;
  const padBottom = 50;
  const chartW = width - padLeft - padRight;
  const chartH = height - padTop - padBottom;

  if (data.length === 0) {
    return `<svg width="${width}px" height="${height}px" viewBox="0 0 ${width} ${height}" xmlns="http://www.w3.org/2000/svg"></svg>`;
  }

  const maxVal = Math.max(...data.map(d => d.value), 1);
  const n = data.length;
  const barW = Math.max((chartW / n) * 0.7, 8);
  const gap = (chartW / n) * 0.3;
  const colors = ["#3182ce", "#38a169", "#d69e2e", "#e53e3e", "#805ad5", "#319795", "#dd6b20", "#2b6cb0"];

  let svg = `<svg width="${width}px" height="${height}px" viewBox="0 0 ${width} ${height}" xmlns="http://www.w3.org/2000/svg">\n`;

  if (title) {
    svg += `<text x="${width / 2}" y="22" text-anchor="middle" font-size="14" font-weight="bold" fill="#2d3748">${escapeHtml(title)}</text>\n`;
  }

  const yTicks = 5;
  for (let i = 0; i <= yTicks; i++) {
    const y = padTop + chartH - (chartH * i) / yTicks;
    const val = (maxVal * i) / yTicks;
    svg += `<line x1="${padLeft}" y1="${y}" x2="${padLeft + chartW}" y2="${y}" stroke="#e2e8f0" stroke-width="1" stroke-dasharray="4,2"/>\n`;
    svg += `<text x="${padLeft - 6}" y="${y + 3}" text-anchor="end" font-size="10" fill="#666">${val.toFixed(1)}</text>\n`;
  }

  svg += `<line x1="${padLeft}" y1="${padTop + chartH}" x2="${padLeft + chartW}" y2="${padTop + chartH}" stroke="#a0aec0" stroke-width="1"/>\n`;

  for (let i = 0; i < data.length; i++) {
    const d = data[i];
    const barH = maxVal > 0 ? (chartH * d.value) / maxVal : 0;
    const x = padLeft + i * (barW + gap) + gap / 2;
    const y = padTop + chartH - barH;
    const color = colors[i % colors.length];

    svg += `<rect x="${x}" y="${y}" width="${barW}" height="${barH}" fill="${color}" rx="2"/>\n`;
    svg += `<text x="${x + barW / 2}" y="${padTop + chartH + 16}" text-anchor="middle" font-size="9" fill="#4a5568">${escapeHtml(d.label)}</text>\n`;
    svg += `<text x="${x + barW / 2}" y="${y - 4}" text-anchor="middle" font-size="9" fill="#718096">${d.value}</text>\n`;
  }

  svg += "</svg>\n";
  return svg;
}

function formatChemFormula(source: string): string {
  return source.replace(/(\d+)/g, "<sub>$1</sub>");
}

function collectTocEntries(doc: VellDocument): Array<{ level: number; text: string; id: string }> {
  const entries: Array<{ level: number; text: string; id: string }> = [];
  for (const node of doc.children ?? []) {
    if (node.type === "Heading") {
      const level = Number(node.level ?? 1);
      const text = textOf(node.children as VellInline[]);
      const id = String(node.id ?? slug(text));
      entries.push({ level, text, id });
    }
  }
  return entries;
}

function collectLofEntries(doc: VellDocument): Array<{ caption: string; id: string }> {
  const entries: Array<{ caption: string; id: string }> = [];
  collectLofEntriesNodes(doc.children ?? [], entries);
  return entries;
}

function collectLofEntriesNodes(
  nodes: VellNode[],
  entries: Array<{ caption: string; id: string }>,
): void {
  for (const node of nodes) {
    if (node.type === "Directive" && node.name === "Figure") {
      const props = node.props as Record<string, unknown> | undefined;
      entries.push({
        caption: String(props?.caption ?? ""),
        id: String(props?.id ?? ""),
      });
    }
    if (node.children && Array.isArray(node.children)) {
      collectLofEntriesNodes(node.children as VellNode[], entries);
    }
  }
}

function collectLotEntries(doc: VellDocument): Array<{ caption: string; id: string }> {
  const entries: Array<{ caption: string; id: string }> = [];
  collectLotEntriesNodes(doc.children ?? [], entries);
  return entries;
}

function collectLotEntriesNodes(
  nodes: VellNode[],
  entries: Array<{ caption: string; id: string }>,
): void {
  for (const node of nodes) {
    if (node.type === "Table") {
      entries.push({ caption: "", id: "" });
    }
    if (node.type === "Directive" && (node.name === "Table" || node.name === "GridTable")) {
      const props = node.props as Record<string, unknown> | undefined;
      entries.push({
        caption: String(props?.caption ?? ""),
        id: String(props?.id ?? ""),
      });
    }
    if (node.children && Array.isArray(node.children)) {
      collectLotEntriesNodes(node.children as VellNode[], entries);
    }
  }
}

function collectLabels(doc: VellDocument): Record<string, LabelTarget> {
  const labels: Record<string, LabelTarget> = {};
  let eqCounter = 0;
  const thmCounters: Record<string, number> = {};
  eqCounter = collectLabelsNodes(doc.children ?? [], labels, eqCounter, thmCounters);
  return labels;
}

function collectLabelsNodes(
  nodes: VellNode[],
  labels: Record<string, LabelTarget>,
  eqCounter: number,
  thmCounters: Record<string, number>,
): number {
  for (const node of nodes) {
    if (node.type === "Directive") {
      const name = String(node.name ?? "");
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

      eqCounter = collectLabelsNodes(node.children as VellNode[] ?? [], labels, eqCounter, thmCounters);
    }
    if (node.children && Array.isArray(node.children)) {
      eqCounter = collectLabelsNodes(node.children as VellNode[], labels, eqCounter, thmCounters);
    }
  }
  return eqCounter;
}

// ---------------------------------------------------------------------------
// Phase 8: Equation & Theorem rendering
// ---------------------------------------------------------------------------

function renderEquation(node: VellNode, ctx: RenderContext): string {
  const props = node.props as Record<string, unknown> | undefined;
  const source = String(props?.source ?? "");
  const label = props?.label ? escapeAttr(String(props.label)) : "";

  ctx.equationCounter++;
  const eqNum = ctx.equationCounter;

  const mathml = latexToMathml(source, true);
  const idAttr = label ? ` id="eq-${label}"` : "";
  const labelAttr = label ? ` data-label="${label}"` : "";

  return (
    `<div class="vell-equation"${idAttr} data-number="${eqNum}"${labelAttr}>` +
    `<table class="eq-table"><tr>` +
    `<td class="eq-math"><math display="block">${mathml}</math></td>` +
    `<td class="eq-number">(${eqNum})</td>` +
    `</tr></table></div>`
  );
}

function renderTheoremEnvironment(node: VellNode, ctx: RenderContext): string {
  const name = String(node.name ?? "");
  const props = node.props as Record<string, unknown> | undefined;
  const extra = props?.name ? escapeHtml(String(props.name)) : null;
  const themeClass = name.toLowerCase();

  let number: number | null = null;
  if (name !== "Proof" && name !== "Remark" && name !== "Example" && name !== "Notation") {
    ctx.theoremCounters[name] = (ctx.theoremCounters[name] ?? 0) + 1;
    number = ctx.theoremCounters[name];
  }

  const children = (node.children as VellNode[] ?? [])
    .map(n => renderNode(n, ctx))
    .join("\n");

  const thmLabel = props?.label ? escapeAttr(String(props.label)) : "";
  const idAttr = thmLabel ? ` id="thm-${thmLabel}"` : "";

  let labelText = escapeHtml(name);
  if (number !== null) labelText += ` ${number}`;
  if (extra) labelText += ` (${extra})`;

  return (
    `<div class="vell-theorem vell-${escapeAttr(themeClass)}"${idAttr}>` +
    `<div class="theorem-label">${labelText}</div>` +
    `<div class="theorem-body">${children}</div>` +
    `</div>`
  );
}

function renderRef(node: VellNode, ctx: RenderContext): string {
  const props = node.props as Record<string, unknown> | undefined;
  const label = String(props?.label ?? "");
  const target = ctx.labelMap[label];
  if (target) {
    return `<a href="#${escapeAttr(target.anchorId)}" class="vell-ref">${escapeHtml(target.displayText)}</a>`;
  }
  return `<span class="unresolved-ref">[?${escapeHtml(label)}]</span>`;
}

function renderMathEnv(node: VellNode, ctx: RenderContext): string {
  const name = String(node.name ?? "");
  const props = node.props as Record<string, unknown> | undefined;
  const source = String(props?.source ?? "");
  const themeClass = name.toLowerCase();

  let html = `<div class="vell-math-env vell-${escapeAttr(themeClass)}">`;
  if (source) {
    const mathml = latexToMathml(source, true);
    html += `<math display="block">${mathml}</math>`;
  }
  for (const child of (node.children as VellNode[] ?? [])) {
    html += renderNode(child, ctx);
  }
  html += `</div>`;
  return html;
}

// ---------------------------------------------------------------------------
// Inline node rendering
// ---------------------------------------------------------------------------

function renderInlineList(
  nodes: VellInline[] | undefined,
  ctx: RenderContext,
): string {
  return (nodes ?? []).map(n => renderInline(n, ctx)).join("");
}

function renderInline(node: VellInline, ctx: RenderContext): string {
  switch (node.type) {
    /* --- plain / formatting --- */
    case "Text":
      return escapeHtml(String(node.value ?? ""));
    case "Bold":
      return `<strong>${renderInlineList(node.children, ctx)}</strong>`;
    case "Italic":
      return `<em>${renderInlineList(node.children, ctx)}</em>`;
    case "Underline":
      return `<u>${renderInlineList(node.children, ctx)}</u>`;
    case "Strikethrough":
      return `<del>${renderInlineList(node.children, ctx)}</del>`;
    case "Code":
      return `<code>${escapeHtml(String(node.value ?? ""))}</code>`;
    case "Superscript":
      return `<sup>${renderInlineList(node.children, ctx)}</sup>`;
    case "Subscript":
      return `<sub>${renderInlineList(node.children, ctx)}</sub>`;

    /* --- links & images --- */
    case "Link": {
      const href = sanitizeUrl(String(node.href ?? ""));
      const title = node.title
        ? ` title="${escapeAttr(String(node.title))}"`
        : "";
      return `<a href="${href}"${title}>${renderInlineList(node.children, ctx)}</a>`;
    }
    case "LinkRef": {
      const id = escapeAttr(String(node.id ?? ""));
      return `<a href="#ref-${id}" class="link-ref">${renderInlineList(node.children, ctx)}</a>`;
    }
    case "Image": {
      const src = sanitizeUrl(String(node.src ?? ""));
      const alt = escapeAttr(String(node.alt ?? ""));
      const title = node.title
        ? ` title="${escapeAttr(String(node.title))}"`
        : "";
      return `<img src="${src}" alt="${alt}"${title}>`;
    }
    case "ImageRef": {
      const id = escapeAttr(String(node.id ?? ""));
      const alt = escapeAttr(String(node.alt ?? ""));
      return `<img src="" alt="${alt}" data-ref="${id}" class="img-ref">`;
    }

    /* --- math --- */
    case "MathInline": {
      const src = String(node.source ?? "");
      const mathml = latexToMathml(src, false);
      const alttext = escapeAttr(mathmlToPlainText(mathml));
      return `<math display="inline" alttext="${alttext}">${mathml}</math>`;
    }

    /* --- variables & components --- */
    case "VarInterpolation":
      return `<span data-vell-var="${escapeAttr(String(node.name ?? ""))}"></span>`;
    case "InlineComponent":
      return `<vell-component name="${escapeAttr(String(node.name ?? ""))}"></vell-component>`;

    /* --- citations & footnotes --- */
    case "Citation":
      return `<cite data-key="${escapeAttr(String(node.key ?? ""))}">[${escapeHtml(String(node.key ?? ""))}]</cite>`;
    case "FootnoteRef": {
      const marker = String(node.marker ?? "");
      ctx.footnoteRefs.add(marker);
      return `<sup><a href="#fn-${escapeAttr(marker)}" id="fnref-${escapeAttr(marker)}">[${escapeHtml(marker)}]</a></sup>`;
    }

    /* --- breaks --- */
    case "SoftBreak":
      return "\n";
    case "HardBreak":
      return "<br>";

    default:
      return "";
  }
}

// ---------------------------------------------------------------------------
// Table rendering
// ---------------------------------------------------------------------------

function renderTable(node: VellNode, ctx: RenderContext): string {
  const headers = node.headers as VellNode[] ?? [];
  const rows = node.rows as VellNode[][] ?? [];
  const head =
    headers.length > 0
      ? `<thead><tr>${headers
          .map(c => `<th>${renderInlineList(c.children as VellInline[], ctx)}</th>`)
          .join("")}</tr></thead>`
      : "";
  const body =
    rows.length > 0
      ? `<tbody>${rows
          .map(
            row =>
              `<tr>${row
                .map(c => {
                  const colspan =
                    c.colspan && Number(c.colspan) > 1
                      ? ` colspan="${escapeAttr(String(c.colspan))}"`
                      : "";
                  const rowspan =
                    c.rowspan && Number(c.rowspan) > 1
                      ? ` rowspan="${escapeAttr(String(c.rowspan))}"`
                      : "";
                  const align = c.align
                    ? ` style="text-align:${escapeAttr(String(c.align))}"`
                    : "";
                  return `<td${colspan}${rowspan}${align}>${renderInlineList(c.children as VellInline[], ctx)}</td>`;
                })
                .join("")}</tr>`,
          )
          .join("")}</tbody>`
      : "";
  return `<table>\n${head}${body}</table>`;
}

// ---------------------------------------------------------------------------
// LaTeX to MathML converter
// ---------------------------------------------------------------------------

function latexToMathml(latex: string, _isBlock: boolean): string {
  const stack: string[] = [];
  let i = 0;

  while (i < latex.length) {
    const ch = latex[i];

    if (ch === "^") {
      i++;
      const sup = parseMathGroup(latex, i);
      i = sup.newPos;
      const base = stack.pop() ?? "";
      if (!base) {
        stack.push(`<msup><mrow/><mrow>${sup.value}</mrow></msup>`);
      } else {
        stack.push(`<msup>${base}${wrapMrow(sup.value)}</msup>`);
      }
    } else if (ch === "_") {
      i++;
      const sub = parseMathGroup(latex, i);
      i = sub.newPos;
      const base = stack.pop() ?? "";
      if (!base) {
        stack.push(`<msub><mrow/><mrow>${sub.value}</mrow></msub>`);
      } else {
        stack.push(`<msub>${base}${wrapMrow(sub.value)}</msub>`);
      }
    } else if (ch === "{") {
      i++;
      let depth = 1;
      let group = "";
      while (i < latex.length && depth > 0) {
        if (latex[i] === "{") {
          depth++;
          if (depth > 1) group += "{";
        } else if (latex[i] === "}") {
          depth--;
          if (depth > 0) group += "}";
        } else {
          group += latex[i];
        }
        if (depth > 0) i++;
      }
      i++;
      const converted = latexToMathml(group, false);
      stack.push(wrapMrow(converted));
    } else if (ch === "}") {
      i++;
    } else if (ch === "\\") {
      i++;
      if (i < latex.length && (latex[i] === " " || latex[i] === "\n")) {
        i++;
        continue;
      }
      let cmd = "";
      while (i < latex.length && /[a-zA-Z]/.test(latex[i])) {
        cmd += latex[i];
        i++;
      }
      if (!cmd) {
        if (i < latex.length && [",", ";", ":", "!"].includes(latex[i])) {
          i++;
        } else {
          stack.push("<mtext>\\</mtext>");
        }
      } else {
        const result = latexCmdToMathml(cmd, latex, i);
        i = result.newPos;
        stack.push(result.value);
      }
    } else if (ch === " " || ch === "\t" || ch === "\n" || ch === "\r") {
      i++;
    } else if (ch === "~") {
      stack.push("<mtext> </mtext>");
      i++;
    } else {
      if (/[0-9]/.test(ch)) {
        stack.push(`<mn>${escapeHtml(ch)}</mn>`);
      } else if (/[a-zA-Z]/.test(ch)) {
        stack.push(`<mi>${escapeHtml(ch)}</mi>`);
      } else {
        const op: Record<string, string> = {
          "+": "+", "-": "\u{2212}", "=": "=",
          "<": "&lt;", ">": "&gt;",
          "(": "(", ")": ")", "[": "[", "]": "]",
          ",": ",", ".": ".", "!": "!", "?": "?",
          "/": "/", "|": "|", "%": "%", ":": ":", ";": ";",
          '"': "&quot;",
        };
        if (op[ch] !== undefined) {
          stack.push(`<mo>${op[ch]}</mo>`);
        }
      }
      i++;
    }
  }

  if (stack.length === 1) {
    return stack[0];
  }
  return `<mrow>${stack.join("")}</mrow>`;
}

function wrapMrow(content: string): string {
  if (content.startsWith("<mrow>") && content.endsWith("</mrow>")) {
    return content;
  }
  return `<mrow>${content}</mrow>`;
}

interface MathGroupResult {
  value: string;
  newPos: number;
}

function parseMathGroup(latex: string, pos: number): MathGroupResult {
  let i = pos;
  while (i < latex.length && /[ \t]/.test(latex[i])) {
    i++;
  }
  if (i >= latex.length) {
    return { value: "", newPos: i };
  }
  if (latex[i] === "{") {
    i++;
    let depth = 1;
    let content = "";
    while (i < latex.length && depth > 0) {
      if (latex[i] === "{") {
        depth++;
        if (depth > 1) content += "{";
      } else if (latex[i] === "}") {
        depth--;
        if (depth > 0) content += "}";
      } else {
        content += latex[i];
      }
      if (depth > 0) i++;
    }
    i++;
    return { value: latexToMathml(content, false), newPos: i };
  }
  if (i < latex.length) {
    const c = latex[i];
    i++;
    if (/[0-9]/.test(c)) {
      return { value: `<mn>${c}</mn>`, newPos: i };
    }
    if (/[a-zA-Z]/.test(c)) {
      return { value: `<mi>${c}</mi>`, newPos: i };
    }
    const opSingle: Record<string, string> = {
      "+": "+", "-": "-", "=": "=",
      "(": "(", ")": ")", "[": "[", "]": "]",
      ",": ",", ".": ".", "!": "!", "?": "?",
      "/": "/", "|": "|",
    };
    if (opSingle[c] !== undefined) {
      return { value: `<mo>${opSingle[c]}</mo>`, newPos: i };
    }
  }
  return { value: "", newPos: i };
}

interface CmdResult {
  value: string;
  newPos: number;
}

function latexCmdToMathml(cmd: string, latex: string, pos: number): CmdResult {
  let newPos = pos;

  switch (cmd) {
    // Greek lowercase
    case "alpha": return { value: "<mi>&alpha;</mi>", newPos };
    case "beta": return { value: "<mi>&beta;</mi>", newPos };
    case "gamma": return { value: "<mi>&gamma;</mi>", newPos };
    case "delta": return { value: "<mi>&delta;</mi>", newPos };
    case "epsilon": return { value: "<mi>&epsilon;</mi>", newPos };
    case "zeta": return { value: "<mi>&zeta;</mi>", newPos };
    case "eta": return { value: "<mi>&eta;</mi>", newPos };
    case "theta": return { value: "<mi>&theta;</mi>", newPos };
    case "iota": return { value: "<mi>&iota;</mi>", newPos };
    case "kappa": return { value: "<mi>&kappa;</mi>", newPos };
    case "lambda": return { value: "<mi>&lambda;</mi>", newPos };
    case "mu": return { value: "<mi>&mu;</mi>", newPos };
    case "nu": return { value: "<mi>&nu;</mi>", newPos };
    case "xi": return { value: "<mi>&xi;</mi>", newPos };
    case "omicron": return { value: "<mi>&omicron;</mi>", newPos };
    case "pi": return { value: "<mi>&pi;</mi>", newPos };
    case "rho": return { value: "<mi>&rho;</mi>", newPos };
    case "sigma": return { value: "<mi>&sigma;</mi>", newPos };
    case "tau": return { value: "<mi>&tau;</mi>", newPos };
    case "upsilon": return { value: "<mi>&upsilon;</mi>", newPos };
    case "phi": return { value: "<mi>&phi;</mi>", newPos };
    case "chi": return { value: "<mi>&chi;</mi>", newPos };
    case "psi": return { value: "<mi>&psi;</mi>", newPos };
    case "omega": return { value: "<mi>&omega;</mi>", newPos };
    // Greek variants
    case "varepsilon": return { value: "<mi>&epsilon;</mi>", newPos };
    case "vartheta": return { value: "<mi>&theta;</mi>", newPos };
    case "varrho": return { value: "<mi>&rho;</mi>", newPos };
    case "varsigma": return { value: "<mi>&sigmaf;</mi>", newPos };
    case "varphi": return { value: "<mi>&phi;</mi>", newPos };
    // Greek uppercase
    case "Gamma": return { value: "<mi>&Gamma;</mi>", newPos };
    case "Delta": return { value: "<mi>&Delta;</mi>", newPos };
    case "Theta": return { value: "<mi>&Theta;</mi>", newPos };
    case "Lambda": return { value: "<mi>&Lambda;</mi>", newPos };
    case "Xi": return { value: "<mi>&Xi;</mi>", newPos };
    case "Pi": return { value: "<mi>&Pi;</mi>", newPos };
    case "Sigma": return { value: "<mi>&Sigma;</mi>", newPos };
    case "Phi": return { value: "<mi>&Phi;</mi>", newPos };
    case "Psi": return { value: "<mi>&Psi;</mi>", newPos };
    case "Omega": return { value: "<mi>&Omega;</mi>", newPos };
    // Operators
    case "int": return { value: "<mo>&#x222B;</mo>", newPos };
    case "iint": return { value: "<mo>&#x222C;</mo>", newPos };
    case "iiint": return { value: "<mo>&#x222D;</mo>", newPos };
    case "sum": return { value: "<mo>&#x2211;</mo>", newPos };
    case "prod": return { value: "<mo>&#x220F;</mo>", newPos };
    case "coprod": return { value: "<mo>&#x2210;</mo>", newPos };
    case "oint": return { value: "<mo>&#x222E;</mo>", newPos };
    case "nabla": return { value: "<mo>&#x2207;</mo>", newPos };
    case "partial": return { value: "<mo>&#x2202;</mo>", newPos };
    case "infty": return { value: "<mo>&#x221E;</mo>", newPos };
    case "times": return { value: "<mo>&#x00D7;</mo>", newPos };
    case "div": return { value: "<mo>&#x00F7;</mo>", newPos };
    case "pm": return { value: "<mo>&#x00B1;</mo>", newPos };
    case "mp": return { value: "<mo>&#x2213;</mo>", newPos };
    case "cdot": return { value: "<mo>&#x00B7;</mo>", newPos };
    case "circ": return { value: "<mo>&#x2218;</mo>", newPos };
    case "ast": return { value: "<mo>&#x2217;</mo>", newPos };
    case "star": return { value: "<mo>&#x22C6;</mo>", newPos };
    case "langle": return { value: "<mo>&#x27E8;</mo>", newPos };
    case "rangle": return { value: "<mo>&#x27E9;</mo>", newPos };
    case "lvert": return { value: "<mo>|</mo>", newPos };
    case "rvert": return { value: "<mo>|</mo>", newPos };
    case "cdots": return { value: "<mo>&#x22EF;</mo>", newPos };
    case "ldots": return { value: "<mo>&#x2026;</mo>", newPos };
    case "vdots": return { value: "<mo>&#x22EE;</mo>", newPos };
    case "ddots": return { value: "<mo>&#x22F1;</mo>", newPos };
    case "otimes": return { value: "<mo>&#x2297;</mo>", newPos };
    case "oplus": return { value: "<mo>&#x2295;</mo>", newPos };
    case "ominus": return { value: "<mo>&#x2296;</mo>", newPos };
    case "oslash": return { value: "<mo>&#x2298;</mo>", newPos };
    case "odot": return { value: "<mo>&#x2299;</mo>", newPos };
    // Relations
    case "equiv": return { value: "<mo>&#x2261;</mo>", newPos };
    case "approx": return { value: "<mo>&#x2248;</mo>", newPos };
    case "sim": return { value: "<mo>&#x223C;</mo>", newPos };
    case "simeq": return { value: "<mo>&#x2243;</mo>", newPos };
    case "cong": return { value: "<mo>&#x2245;</mo>", newPos };
    case "propto": return { value: "<mo>&#x221D;</mo>", newPos };
    case "neq": return { value: "<mo>&#x2260;</mo>", newPos };
    case "le": return { value: "<mo>&#x2264;</mo>", newPos };
    case "ge": return { value: "<mo>&#x2265;</mo>", newPos };
    case "ll": return { value: "<mo>&#x226A;</mo>", newPos };
    case "gg": return { value: "<mo>&#x226B;</mo>", newPos };
    case "prec": return { value: "<mo>&#x227A;</mo>", newPos };
    case "succ": return { value: "<mo>&#x227B;</mo>", newPos };
    case "preceq": return { value: "<mo>&#x227C;</mo>", newPos };
    case "succeq": return { value: "<mo>&#x227D;</mo>", newPos };
    // Set symbols
    case "subset": return { value: "<mo>&#x2282;</mo>", newPos };
    case "supset": return { value: "<mo>&#x2283;</mo>", newPos };
    case "subseteq": return { value: "<mo>&#x2286;</mo>", newPos };
    case "supseteq": return { value: "<mo>&#x2287;</mo>", newPos };
    case "cap": return { value: "<mo>&#x2229;</mo>", newPos };
    case "cup": return { value: "<mo>&#x222A;</mo>", newPos };
    case "setminus": return { value: "<mo>&#x2216;</mo>", newPos };
    case "emptyset": return { value: "<mo>&#x2205;</mo>", newPos };
    case "varnothing": return { value: "<mo>&#x2205;</mo>", newPos };
    case "in": return { value: "<mo>&#x2208;</mo>", newPos };
    case "notin": return { value: "<mo>&#x2209;</mo>", newPos };
    case "ni": return { value: "<mo>&#x220B;</mo>", newPos };
    // Arrows
    case "rightarrow": return { value: "<mo>&#x2192;</mo>", newPos };
    case "leftarrow": return { value: "<mo>&#x2190;</mo>", newPos };
    case "Rightarrow": return { value: "<mo>&#x21D2;</mo>", newPos };
    case "Leftarrow": return { value: "<mo>&#x21D0;</mo>", newPos };
    case "leftrightarrow": return { value: "<mo>&#x2194;</mo>", newPos };
    case "Leftrightarrow": return { value: "<mo>&#x21D4;</mo>", newPos };
    case "uparrow": return { value: "<mo>&#x2191;</mo>", newPos };
    case "downarrow": return { value: "<mo>&#x2193;</mo>", newPos };
    case "mapsto": return { value: "<mo>&#x21A6;</mo>", newPos };
    case "implies": return { value: "<mo>&#x21D2;</mo>", newPos };
    case "iff": return { value: "<mo>&#x21D4;</mo>", newPos };
    // Logical
    case "forall": return { value: "<mo>&#x2200;</mo>", newPos };
    case "exists": return { value: "<mo>&#x2203;</mo>", newPos };
    case "nexists": return { value: "<mo>&#x2204;</mo>", newPos };
    case "neg": return { value: "<mo>&#x00AC;</mo>", newPos };
    case "lor": return { value: "<mo>&#x2228;</mo>", newPos };
    case "land": return { value: "<mo>&#x2227;</mo>", newPos };
    case "top": return { value: "<mo>&#x22A4;</mo>", newPos };
    case "bot": return { value: "<mo>&#x22A5;</mo>", newPos };
    case "vdash": return { value: "<mo>&#x22A2;</mo>", newPos };
    case "mid": return { value: "<mo>&#x2223;</mo>", newPos };
    case "models": return { value: "<mo>&#x22A7;</mo>", newPos };
    // Functions
    case "sin": case "cos": case "tan": case "cot": case "sec": case "csc":
    case "log": case "ln":
    case "sinh": case "cosh": case "tanh":
    case "arcsin": case "arccos": case "arctan":
    case "det": case "dim": case "lim": case "max": case "min": case "sup": case "inf":
    case "exp": case "deg": case "arg": case "ker": case "hom": case "gcd": case "Pr":
      return { value: `<mi>${cmd}</mi>`, newPos };
    // Fractions
    case "frac": {
      const num = parseMathGroup(latex, newPos);
      newPos = num.newPos;
      const den = parseMathGroup(latex, newPos);
      newPos = den.newPos;
      return {
        value: `<mfrac>${wrapMrow(num.value)}${wrapMrow(den.value)}</mfrac>`,
        newPos,
      };
    }
    // Square root
    case "sqrt": {
      const content = parseMathGroup(latex, newPos);
      newPos = content.newPos;
      return { value: `<msqrt>${wrapMrow(content.value)}</msqrt>`, newPos };
    }
    // Accents
    case "hat": {
      const content = parseMathGroup(latex, newPos);
      newPos = content.newPos;
      return { value: `<mover>${wrapMrow(content.value)}<mo>^</mo></mover>`, newPos };
    }
    case "tilde": {
      const content = parseMathGroup(latex, newPos);
      newPos = content.newPos;
      return { value: `<mover>${wrapMrow(content.value)}<mo>~</mo></mover>`, newPos };
    }
    case "bar": {
      const content = parseMathGroup(latex, newPos);
      newPos = content.newPos;
      return { value: `<mover>${wrapMrow(content.value)}<mo>&#x00AF;</mo></mover>`, newPos };
    }
    case "dot": {
      const content = parseMathGroup(latex, newPos);
      newPos = content.newPos;
      return { value: `<mover>${wrapMrow(content.value)}<mo>.</mo></mover>`, newPos };
    }
    case "ddot": {
      const content = parseMathGroup(latex, newPos);
      newPos = content.newPos;
      return { value: `<mover>${wrapMrow(content.value)}<mo>..</mo></mover>`, newPos };
    }
    case "vec": {
      const content = parseMathGroup(latex, newPos);
      newPos = content.newPos;
      return { value: `<mover>${wrapMrow(content.value)}<mo>&#x2192;</mo></mover>`, newPos };
    }
    // Font commands
    case "mathbb": {
      const content = parseMathGroup(latex, newPos);
      newPos = content.newPos;
      const letter = content.value.replace(/<[^>]+>/g, "").trim().charAt(0) || "";
      return { value: `<mi mathvariant="double-struck">${letter}</mi>`, newPos };
    }
    case "mathcal": {
      const content = parseMathGroup(latex, newPos);
      newPos = content.newPos;
      const letter = content.value.replace(/<[^>]+>/g, "").trim().charAt(0) || "A";
      return { value: `<mi mathvariant="script">${letter}</mi>`, newPos };
    }
    case "mathrm": {
      const content = parseMathGroup(latex, newPos);
      newPos = content.newPos;
      return { value: `<mi mathvariant="normal">${content.value}</mi>`, newPos };
    }
    case "mathbf": {
      const content = parseMathGroup(latex, newPos);
      newPos = content.newPos;
      return { value: `<mi mathvariant="bold">${content.value}</mi>`, newPos };
    }
    case "mathit": {
      const content = parseMathGroup(latex, newPos);
      newPos = content.newPos;
      return { value: `<mi mathvariant="italic">${content.value}</mi>`, newPos };
    }
    case "mathsf": {
      const content = parseMathGroup(latex, newPos);
      newPos = content.newPos;
      return { value: `<mi mathvariant="sans-serif">${content.value}</mi>`, newPos };
    }
    case "mathtt": {
      const content = parseMathGroup(latex, newPos);
      newPos = content.newPos;
      return { value: `<mi mathvariant="monospace">${content.value}</mi>`, newPos };
    }
    // Binomial coefficient
    case "binom": {
      const num = parseMathGroup(latex, newPos);
      newPos = num.newPos;
      const den = parseMathGroup(latex, newPos);
      newPos = den.newPos;
      return {
        value: `<mrow><mo>(</mo><mfrac linethickness="0">${wrapMrow(num.value)}${wrapMrow(den.value)}</mfrac><mo>)</mo></mrow>`,
        newPos,
      };
    }
    // Named operator
    case "operatorname": {
      const content = parseMathGroup(latex, newPos);
      newPos = content.newPos;
      return { value: `<mi>${content.value}</mi>`, newPos };
    }
    // Physics: bra-ket notation
    case "bra": {
      const content = parseMathGroup(latex, newPos);
      newPos = content.newPos;
      return {
        value: `<mrow><mo>&#x27E8;</mo>${wrapMrow(content.value)}<mo>|</mo></mrow>`,
        newPos,
      };
    }
    case "ket": {
      const content = parseMathGroup(latex, newPos);
      newPos = content.newPos;
      return {
        value: `<mrow><mo>|</mo>${wrapMrow(content.value)}<mo>&#x27E9;</mo></mrow>`,
        newPos,
      };
    }
    case "braket": {
      const content = parseMathGroup(latex, newPos);
      newPos = content.newPos;
      const pipeIdx = content.value.indexOf("|");
      if (pipeIdx !== -1) {
        const left = latexToMathml(content.value.substring(0, pipeIdx).trim(), false);
        const right = latexToMathml(content.value.substring(pipeIdx + 1).trim(), false);
        return {
          value: `<mrow><mo>&#x27E8;</mo>${wrapMrow(left)}<mo>|</mo>${wrapMrow(right)}<mo>&#x27E9;</mo></mrow>`,
          newPos,
        };
      }
      return {
        value: `<mrow><mo>&#x27E8;</mo>${wrapMrow(content.value)}<mo>&#x27E9;</mo></mrow>`,
        newPos,
      };
    }
    // Text in math
    case "text": {
      const content = parseMathGroup(latex, newPos);
      newPos = content.newPos;
      return { value: `<mtext>${escapeHtml(content.value)}</mtext>`, newPos };
    }
    // Additional math commands
    case "displaystyle": {
      const content = parseMathGroup(latex, newPos);
      newPos = content.newPos;
      return { value: content.value, newPos };
    }
    case "left": {
      // \left(, \left[, \left\{, etc. — skip the delimiter as a hint
      newPos++;
      return { value: "", newPos };
    }
    case "right": {
      newPos++;
      return { value: "", newPos };
    }
    case "bigl": case "bigr": case "biggl": case "biggr":
    case "Bigl": case "Bigr": case "Biggl": case "Biggr": {
      newPos++;
      return { value: "", newPos };
    }
    case "not": {
      const content = parseMathGroup(latex, newPos);
      newPos = content.newPos;
      return { value: `<mo>&#x00AC;</mo>${wrapMrow(content.value)}`, newPos };
    }
    case "colon": return { value: "<mo>:</mo>", newPos };
    case "prime": return { value: "<mo>&#x2032;</mo>", newPos };
    case "degree": return { value: "<mo>&#x00B0;</mo>", newPos };
    case "percent": return { value: "<mo>%</mo>", newPos };
    case "angles": return { value: "<mo>&#x27E8;</mo><mo>&#x27E9;</mo>", newPos };
    case "flat": return { value: "<mo>&#x266D;</mo>", newPos };
    case "sharp": return { value: "<mo>&#x266F;</mo>", newPos };
    case "natural": return { value: "<mo>&#x266E;</mo>", newPos };
    case "Re": return { value: "<mi mathvariant=\"double-struck\">R</mi>", newPos };
    case "Im": return { value: "<mi mathvariant=\"double-struck\">I</mi>", newPos };
    // spacing
    case "quad": case "qquad":
    case "thinspace": case "medspace": case "thickspace":
    case "negthinspace": case "negmedspace": case "negthickspace":
    case " ": case ",": case ";": case ":":
      return { value: "", newPos };
    // Stub commands that are ignored safely
    case "label": case "ref": case "tag":
      return { value: "", newPos };
    // Default: render as mtext
    default:
      return { value: `<mtext>\\${cmd}</mtext>`, newPos };
  }
}

// ---------------------------------------------------------------------------
// Utility helpers
// ---------------------------------------------------------------------------

function textOf(nodes?: VellInline[]): string {
  return (nodes ?? [])
    .map(n =>
      n.type === "Text" ? String(n.value ?? "") : textOf(n.children),
    )
    .join("");
}

function slug(value: string): string {
  return value
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-|-$/g, "");
}

function escapeHtml(value: string): string {
  return value.replace(
    /[&<>"]/g,
    ch =>
      ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[ch] ?? ch,
  );
}

function escapeAttr(value: string): string {
  return escapeHtml(value);
}

function stringifyValue(value: unknown): string {
  if (value === null || value === undefined) return "";
  if (typeof value === "string") return value;
  return JSON.stringify(value);
}

/** Detect if a language code is a right-to-left (RTL) language. */
function isRtlLanguage(lang: string): boolean {
  const rtlLangs = [
    "ar", "arc", "bcc", "bqi", "ckb", "dv", "fa", "glk",
    "ha", "he", "khw", "ks", "ku", "mzn", "nqo", "pa",
    "ps", "sd", "ug", "ur", "uz", "yi",
  ];
  const base = lang.split("-")[0].toLowerCase();
  return rtlLangs.includes(base);
}

/** Strip all MathML/HTML tags from a string, returning plain text for alttext. */
function mathmlToPlainText(markup: string): string {
  return markup.replace(/<[^>]*>/g, " ").replace(/\s+/g, " ").trim();
}

/**
 * Comprehensive CSS for Vell rendered output.
 * Includes base styles, theme colors, code blocks, tables, admonitions,
 * academic theorem environments, diagrams, equations, print CSS,
 * high-contrast accessibility overrides, and CJK typography support.
 */
export const VELL_CSS = `
/* Base */
body { font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Helvetica, Arial, sans-serif; line-height: 1.6; color: #1a202c; max-width: 800px; margin: 0 auto; padding: 1em; }
code { font-family: 'SFMono-Regular', Consolas, 'Liberation Mono', Menlo, monospace; }
pre { font-family: 'SFMono-Regular', Consolas, 'Liberation Mono', Menlo, monospace; background: #f7fafc; padding: 0.8em; overflow-x: auto; border-radius: 4px; }
img { max-width: 100%; height: auto; }
.vell-equation { margin: 0.8em 0; padding: 0.2em 0; }
.eq-table { width: 100%; border: none; border-collapse: collapse; }
.eq-table td { padding: 0; vertical-align: middle; }
.eq-math { text-align: center; width: 90%; }
.eq-number { text-align: right; width: 10%; padding-left: 1em; font-size: 0.9em; color: #555; }
.vell-theorem { margin: 1em 0; padding: 0.6em 1em; border-left: 3px solid #3182ce; background: #f7fafc; }
.vell-proof { border-left-color: #718096; background: #fefefe; }
.vell-lemma { border-left-color: #38a169; background: #f0fff4; }
.vell-corollary { border-left-color: #d69e2e; background: #fffff0; }
.vell-definition { border-left-color: #805ad5; background: #faf5ff; }
.vell-remark { border-left-color: #a0aec0; background: #f5f5f5; }
.vell-example { border-left-color: #319795; background: #f0fdfa; }
.vell-conjecture { border-left-color: #e53e3e; background: #fff5f5; }
.vell-axiom { border-left-color: #dd6b20; background: #fffaf0; }
.vell-proposition { border-left-color: #2b6cb0; background: #ebf8ff; }
.theorem-label { font-weight: bold; font-style: italic; margin-bottom: 0.3em; color: #2d3748; }
.theorem-body > :first-child { margin-top: 0; }
.theorem-body > :last-child { margin-bottom: 0; }
.vell-math-env { margin: 0.8em 0; padding: 0.4em 1em; background: #fdfdfd; border: 1px solid #e2e8f0; border-radius: 4px; }
.vell-math-env math { display: block; margin: 0.4em 0; }
.vell-ref { color: #2b6cb0; text-decoration: none; }
.vell-ref:hover { text-decoration: underline; }
.unresolved-ref { color: #e53e3e; font-style: italic; }
.admonition { padding: 0.6em 1em; margin: 1em 0; border-left: 4px solid #3182ce; background: #ebf8ff; }
.admonition.warning, .admonition.warn { border-left-color: #d69e2e; background: #fffff0; }
.admonition.danger, .admonition.error { border-left-color: #e53e3e; background: #fff5f5; }
.admonition.success, .admonition.tip { border-left-color: #38a169; background: #f0fff4; }
.vell-chem { margin: 0.6em 0; padding: 0.4em 1em; background: #f0fdfa; border: 1px solid #b2f5ea; border-radius: 4px; font-family: 'Courier New', monospace; }
.vell-chem .chem-formula { font-size: 1.1em; font-weight: bold; color: #234e52; }
.vell-toc, .vell-lof, .vell-lot { margin: 1em 0; padding: 0.6em 1em; background: #f7fafc; border: 1px solid #e2e8f0; border-radius: 4px; }
.vell-toc h2, .vell-lof h2, .vell-lot h2 { font-size: 1.1em; margin: 0 0 0.5em 0; color: #2d3748; }
.vell-toc .toc-list, .vell-lof .lof-list, .vell-lot .lot-list { padding-left: 1.5em; }
.vell-toc .toc-list li, .vell-lof .lof-list li, .vell-lot .lot-list li { margin: 0.2em 0; }
.toc-placeholder, .lof-placeholder, .lot-placeholder { color: #a0aec0; font-style: italic; list-style: none; }
.vell-diagram { margin: 1em 0; padding: 1em; background: #f8f9fa; border: 1px solid #e2e8f0; border-radius: 4px; overflow-x: auto; }
.vell-diagram .diagram-caption { font-size: 0.9em; color: #666; margin-top: 0.5em; font-style: italic; }
.vell-diagram pre { margin: 0; white-space: pre; font-family: 'Courier New', monospace; font-size: 0.9em; line-height: 1.4; }
.vell-diagram[data-type="mermaid"] .mermaid { margin: 0; }
.vell-diagram[data-type="ascii"] pre { color: #333; }
.vell-diagram[data-type="dot"] .graphviz { margin: 0; }
.vell-diagram[data-type="dot"] pre.dot { color: #2b6cb0; }
.vell-chart { margin: 1em 0; padding: 0.5em; overflow-x: auto; }
.vell-chart svg { display: block; margin: 0 auto; }
.chart-title { text-align: center; font-size: 1em; font-weight: bold; margin-bottom: 0.3em; color: #2d3748; }
.vell-plot { margin: 1em 0; padding: 0.5em; overflow-x: auto; }
.vell-plot svg { display: block; margin: 0 auto; }
/* Print CSS */
@media print {
  body { font-size: 11pt; line-height: 1.5; color: #000; background: #fff; max-width: none; padding: 0; }
  @page { margin: 2.54cm; }
  @page :first { margin-top: 2.54cm; }
  h1, h2, h3, h4, h5, h6 { page-break-after: avoid; }
  h1 { page-break-before: always; }
  h1:first-of-type { page-break-before: avoid; }
  table { page-break-inside: avoid; }
  pre, blockquote { page-break-inside: avoid; }
  img { page-break-inside: avoid; }
  a { color: #000; text-decoration: none; }
  a[href^="http"]::after { content: " (" attr(href) ")"; font-size: 0.8em; color: #555; }
  .vell-slide { display: none; }
  .vell-diagram { border: 1px solid #ddd; }
  .vell-chart svg { max-width: 100%; }
  .vell-plot svg { max-width: 100%; }
  .vell-chem { border: 1px solid #b2f5ea; }
  .vell-equation { page-break-inside: avoid; }
  .footnotes { page-break-before: always; font-size: 0.85em; }
  .page-break { page-break-before: always; }
  .toc { page-break-after: always; }
}
/* Phase 19: High contrast theme */
@media (prefers-contrast: high) {
  body { color: #000; background: #fff; }
  a { color: #0056b3; text-decoration: underline; }
  a.vell-ref { color: #004080; }
  pre, code { background: #fff; border: 2px solid #000; }
  table th, table td { border: 2px solid #000; }
  th { background: #e0e0e0; }
  .vell-theorem { border-left: 4px solid #000; background: #fff; }
  .vell-proof { border-left-color: #555; }
  .vell-lemma { border-left-color: #333; }
  .vell-corollary { border-left-color: #777; }
  .vell-definition { border-left-color: #333; }
  .admonition { border-left: 4px solid #000; background: #fff; }
  .vell-chem { background: #fff; border: 2px solid #234e52; }
  .vell-toc, .vell-lof, .vell-lot { background: #fff; border: 2px solid #000; }
  .vell-diagram { background: #fff; border: 2px solid #000; }
  .vell-math-env { background: #fff; border: 2px solid #000; }
  .vell-equation { border: 1px solid #000; padding: 0.3em; }
  input, select, textarea { border: 2px solid #000; }
}
/* Phase 19: CJK typography */
:lang(zh) body { font-family: "Noto Sans SC", "PingFang SC", "Microsoft YaHei", "Hiragino Sans GB", sans-serif; line-height: 1.9; }
:lang(ja) body { font-family: "Noto Sans JP", "Hiragino Sans", "Yu Gothic", "Meiryo", sans-serif; line-height: 1.9; }
:lang(ko) body { font-family: "Noto Sans KR", "Apple SD Gothic Neo", "Malgun Gothic", sans-serif; line-height: 1.9; }
:lang(zh) pre, :lang(ja) pre, :lang(ko) pre { font-family: "Noto Sans Mono CJK SC", "Source Han Sans SC", "Noto Sans Mono", monospace; }
:lang(zh) h1, :lang(zh) h2, :lang(zh) h3 { letter-spacing: 0.05em; }
:lang(ja) h1, :lang(ja) h2, :lang(ja) h3 { letter-spacing: 0.05em; }
:lang(ko) h1, :lang(ko) h2, :lang(ko) h3 { letter-spacing: 0.03em; }
`;

function sanitizeUrl(value: string): string {
  try {
    const url = new URL(value, "https://vell.local");
    return ["http:", "https:", "mailto:"].includes(url.protocol) ||
      !/^[a-zA-Z][a-zA-Z0-9+.-]*:/.test(value)
      ? escapeAttr(value)
      : "";
  } catch {
    return "";
  }
}
