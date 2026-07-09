// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Samin Yeasar

/**
 * Vell Built-in Extension Library
 *
 * A curated set of high-quality extensions that renderers can opt into.
 * Each extension provides its own schema, HTML adapter, and LSP metadata.
 *
 * Usage:
 *   import { YouTube, Chart, Mermaid, Callout } from "@vell-lang/extensions";
 *   import { render } from "@vell-lang/renderer-html";
 *   const html = render(doc, { extensions: [YouTube, Chart] });
 */

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/** PropValue union type matching the Vell AST. */
export type PropValue = string | number | boolean | null;

/** Minimal Vell node for extension rendering. */
export interface VellNode {
  type: string;
  name?: string;
  props?: Record<string, PropValue>;
  children?: VellNode[];
  [key: string]: unknown;
}

/** Context provided to extension renderers. */
export interface ExtensionContext {
  escapeHtml(value: string): string;
  sanitizeUrl(value: string): string;
}

interface SchemaProp {
  type: string;
  required?: boolean;
  optional?: boolean;
  default?: PropValue;
}

/** An extension adapter that knows how to render a namespaced Vell extension. */
export interface ExtensionAdapter {
  /** The extension name (e.g. "embed/YouTube", "npm/chart"). */
  name: string;
  /** Description shown in LSP hover and docs. */
  description: string;
  /** JSON Schema-like property definitions for LSP completions and validation. */
  schema: Record<string, SchemaProp>;
  /** Renders the extension node to HTML. */
  render(node: VellNode, ctx: ExtensionContext): string;
  /** Fallback content for unsupported renderers (optional). */
  fallback?(node: VellNode, ctx: ExtensionContext): string;
}

// ---------------------------------------------------------------------------
// Extension Registry
// ---------------------------------------------------------------------------

/** Global registry of extension adapters. */
const registry = new Map<string, ExtensionAdapter>();

/** Register one or more extension adapters. */
export function register(...adapters: ExtensionAdapter[]): void {
  for (const adp of adapters) {
    registry.set(adp.name, adp);
  }
}

/** Get a registered extension by name. */
export function getExtension(name: string): ExtensionAdapter | undefined {
  return registry.get(name);
}

/** Get all registered extension names. */
export function getRegisteredExtensions(): string[] {
  return Array.from(registry.keys());
}

/** Create an ExtensionContext with default escape/sanitize helpers. */
export function createExtensionContext(): ExtensionContext {
  return {
    escapeHtml(value: string): string {
      return value.replace(/[&<>"]/g, (ch) =>
        ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[ch] ?? ch,
      );
    },
    sanitizeUrl(value: string): string {
      try {
        const url = new URL(value, "https://vell.local");
        return ["http:", "https:", "mailto:"].includes(url.protocol) ||
          !/^[a-zA-Z][a-zA-Z0-9+.-]*:/.test(value)
          ? value
          : "";
      } catch {
        return "";
      }
    },
  };
}

/** Render a Vell Extension node using the registered adapter, or fallback. */
export function renderExtension(node: VellNode, ctx?: ExtensionContext): string {
  const name = node.name ?? "";
  const adapter = registry.get(name);
  const ec = ctx ?? createExtensionContext();

  if (adapter) {
    return adapter.render(node, ec);
  }

  // Default fallback: render children as text
  const childrenHtml = renderFallbackChildren(node, ec);
  return `<div class="vell-extension" data-name="${ec.escapeHtml(name)}">\n${childrenHtml}\n</div>`;
}

/** Render children of a node as plain text fallback. */
function renderFallbackChildren(node: VellNode, ctx: ExtensionContext): string {
  if (!node.children || node.children.length === 0) return "";
  const parts: string[] = [];
  for (const child of node.children) {
    if (child.type === "Paragraph" && child.children) {
      for (const inline of child.children as Array<Record<string, unknown>>) {
        if (inline.type === "Text") {
          parts.push(String(inline.value ?? ""));
        }
      }
    }
  }
  return ctx.escapeHtml(parts.join("\n"));
}

// ---------------------------------------------------------------------------
// Built-in Extension: embed/YouTube
// ---------------------------------------------------------------------------

export const YouTube: ExtensionAdapter = {
  name: "embed/YouTube",
  description: "Embed a YouTube video with an iframe",
  schema: {
    video: { type: "string", required: true },
    width: { type: "number", default: 560 },
    height: { type: "number", default: 315 },
  },
  render(node: VellNode, ctx: ExtensionContext): string {
    const props = node.props ?? {};
    const videoId = String(props.video ?? "");
    const width = Number(props.width ?? 560);
    const height = Number(props.height ?? 315);
    const fallbackContent = renderFallbackChildren(node, ctx);
    if (!videoId) {
      return `<div class="vell-extension" data-name="embed/YouTube">${fallbackContent}</div>`;
    }
    return (
      `<div class="vell-extension" data-name="embed/YouTube">` +
      `<iframe width="${width}" height="${height}" src="https://www.youtube-nocookie.com/embed/${ctx.escapeHtml(videoId)}" ` +
      `frameborder="0" allowfullscreen title="YouTube video"></iframe>` +
      (fallbackContent ? `\n<div class="vell-extension-fallback">${fallbackContent}</div>` : "") +
      `</div>`
    );
  },
};

// ---------------------------------------------------------------------------
// Built-in Extension: embed/Vimeo
// ---------------------------------------------------------------------------

export const Vimeo: ExtensionAdapter = {
  name: "embed/Vimeo",
  description: "Embed a Vimeo video with an iframe",
  schema: {
    video: { type: "string", required: true },
    width: { type: "number", default: 560 },
    height: { type: "number", default: 315 },
  },
  render(node: VellNode, ctx: ExtensionContext): string {
    const props = node.props ?? {};
    const videoId = String(props.video ?? "");
    const width = Number(props.width ?? 560);
    const height = Number(props.height ?? 315);
    const fallbackContent = renderFallbackChildren(node, ctx);
    if (!videoId) {
      return `<div class="vell-extension" data-name="embed/Vimeo">${fallbackContent}</div>`;
    }
    return (
      `<div class="vell-extension" data-name="embed/Vimeo">` +
      `<iframe width="${width}" height="${height}" src="https://player.vimeo.com/video/${ctx.escapeHtml(videoId)}" ` +
      `frameborder="0" allowfullscreen title="Vimeo video"></iframe>` +
      (fallbackContent ? `\n<div class="vell-extension-fallback">${fallbackContent}</div>` : "") +
      `</div>`
    );
  },
};

// ---------------------------------------------------------------------------
// Built-in Extension: npm/chart (client-side rendered via Chart.js)
// ---------------------------------------------------------------------------

export const Chart: ExtensionAdapter = {
  name: "npm/chart",
  description: "Render an interactive chart using Chart.js",
  schema: {
    type: { type: "string", required: true, default: "bar" },
    data: { type: "string", required: true },
    labels: { type: "string", optional: true },
    title: { type: "string", optional: true },
  },
  render(node: VellNode, ctx: ExtensionContext): string {
    const props = node.props ?? {};
    const chartType = String(props.type ?? "bar");
    const dataStr = String(props.data ?? "[]");
    const labels = props.labels ? String(props.labels) : "";
    const title = props.title ? String(props.title) : "";
    const chartId = `vell-chart-${Math.random().toString(36).slice(2, 9)}`;

    let data: number[] = [];
    try {
      data = JSON.parse(dataStr);
    } catch {
      data = dataStr.split(",").map((s) => parseFloat(s.trim())).filter((n) => !isNaN(n));
    }
    if (!Array.isArray(data)) data = [];

    const fallbackContent = renderFallbackChildren(node, ctx);
    if (data.length === 0) {
      return `<div class="vell-extension" data-name="npm/chart">${fallbackContent}</div>`;
    }

    const safeTitle = title ? ctx.escapeHtml(title) : "";

    // Generate inline SVG fallback (no Chart.js dependency)
    return renderInlineChartSvg(chartId, chartType, data, labels, safeTitle, ctx);
  },
};

function renderInlineChartSvg(
  _id: string,
  chartType: string,
  data: number[],
  labels: string,
  title: string,
  ctx: ExtensionContext,
): string {
  const width = 500;
  const height = 250;
  const padLeft = 60;
  const padRight = 20;
  const padTop = title ? 40 : 20;
  const padBottom = 50;
  const chartW = width - padLeft - padRight;
  const chartH = height - padTop - padBottom;
  const maxVal = Math.max(...data, 1);
  const n = data.length;
  const barW = Math.max((chartW / n) * 0.7, 8);
  const gap = (chartW / n) * 0.3;
  const colors = ["#3182ce", "#38a169", "#d69e2e", "#e53e3e", "#805ad5", "#319795", "#dd6b20", "#2b6cb0"];

  let svg = `<svg width="${width}px" height="${height}px" viewBox="0 0 ${width} ${height}" xmlns="http://www.w3.org/2000/svg">\n`;

  if (title) {
    svg += `<text x="${width / 2}" y="22" text-anchor="middle" font-size="14" font-weight="bold" fill="#2d3748">${ctx.escapeHtml(title)}</text>\n`;
  }

  // Y-axis
  svg += `<line x1="${padLeft}" y1="${padTop}" x2="${padLeft}" y2="${padTop + chartH}" stroke="#a0aec0" stroke-width="1"/>\n`;
  const yTicks = 5;
  for (let i = 0; i <= yTicks; i++) {
    const y = padTop + chartH - (chartH * i) / yTicks;
    const val = (maxVal * i) / yTicks;
    svg += `<line x1="${padLeft}" y1="${y}" x2="${padLeft + chartW}" y2="${y}" stroke="#e2e8f0" stroke-width="1" stroke-dasharray="4,2"/>\n`;
    svg += `<text x="${padLeft - 6}" y="${y + 3}" text-anchor="end" font-size="10" fill="#666">${val.toFixed(1)}</text>\n`;
  }

  // X-axis
  svg += `<line x1="${padLeft}" y1="${padTop + chartH}" x2="${padLeft + chartW}" y2="${padTop + chartH}" stroke="#a0aec0" stroke-width="1"/>\n`;

  const labelArr = labels ? labels.split(",").map((s) => s.trim()) : data.map((_, i) => `#${i + 1}`);

  if (chartType === "pie" || chartType === "doughnut") {
    // Simple pie representation
    const total = data.reduce((a, b) => a + b, 0) || 1;
    const cx = (padLeft + chartW / 2);
    const cy = padTop + chartH / 2;
    const r = Math.min(chartW, chartH) / 2 - 10;
    let startAngle = -Math.PI / 2;
    for (let i = 0; i < data.length; i++) {
      const sliceAngle = (data[i] / total) * 2 * Math.PI;
      const endAngle = startAngle + sliceAngle;
      const x1 = cx + r * Math.cos(startAngle);
      const y1 = cy + r * Math.sin(startAngle);
      const x2 = cx + r * Math.cos(endAngle);
      const y2 = cy + r * Math.sin(endAngle);
      const largeArc = sliceAngle > Math.PI ? 1 : 0;
      const color = colors[i % colors.length];
      svg += `<path d="M${cx},${cy} L${x1},${y1} A${r},${r} 0 ${largeArc},1 ${x2},${y2} Z" fill="${color}" stroke="#fff" stroke-width="1"/>\n`;
      startAngle = endAngle;
    }
    // Legend
    const legendY = padTop + chartH + 20;
    for (let i = 0; i < Math.min(data.length, labelArr.length); i++) {
      const lx = padLeft + (i % 5) * 100;
      const ly = legendY + Math.floor(i / 5) * 18;
      svg += `<rect x="${lx}" y="${ly - 8}" width="10" height="10" fill="${colors[i % colors.length]}" rx="1"/>\n`;
      svg += `<text x="${lx + 14}" y="${ly}" font-size="9" fill="#4a5568">${ctx.escapeHtml(labelArr[i])}: ${data[i]}</text>\n`;
    }
  } else {
    // Bar chart
    for (let i = 0; i < data.length; i++) {
      const barH = (chartH * data[i]) / maxVal;
      const x = padLeft + i * (barW + gap) + gap / 2;
      const y = padTop + chartH - barH;
      const color = colors[i % colors.length];
      svg += `<rect x="${x}" y="${y}" width="${barW}" height="${barH}" fill="${color}" rx="2"/>\n`;
      const label = i < labelArr.length ? labelArr[i] : `#${i + 1}`;
      svg += `<text x="${x + barW / 2}" y="${padTop + chartH + 16}" text-anchor="middle" font-size="9" fill="#4a5568">${ctx.escapeHtml(label)}</text>\n`;
      svg += `<text x="${x + barW / 2}" y="${y - 4}" text-anchor="middle" font-size="9" fill="#718096">${data[i]}</text>\n`;
    }
  }

  svg += "</svg>\n";
  return `<div class="vell-chart vell-chart-${ctx.escapeHtml(chartType)}">\n${svg}\n</div>`;
}

// ---------------------------------------------------------------------------
// Built-in Extension: npm/mermaid (Mermaid.js Diagram)
// ---------------------------------------------------------------------------

export const Mermaid: ExtensionAdapter = {
  name: "npm/mermaid",
  description: "Render a Mermaid.js diagram from text definition",
  schema: {
    theme: { type: "string", default: "default" },
  },
  render(node: VellNode, ctx: ExtensionContext): string {
    const props = node.props ?? {};
    const theme = String(props.theme ?? "default");
    const source = extractTextFromChildren(node, ctx);
    if (!source) {
      return `<div class="vell-extension" data-name="npm/mermaid">${renderFallbackChildren(node, ctx)}</div>`;
    }
    return (
      `<div class="vell-extension" data-name="npm/mermaid" data-theme="${ctx.escapeHtml(theme)}">` +
      `<pre class="mermaid">${ctx.escapeHtml(source)}</pre>` +
      `</div>`
    );
  },
};

// ---------------------------------------------------------------------------
// Built-in Extension: npm/callout (Admonition-style callout boxes)
// ---------------------------------------------------------------------------

export const Callout: ExtensionAdapter = {
  name: "npm/callout",
  description: "Render a styled callout/admonition box with icon",
  schema: {
    type: { type: "string", required: true, default: "note" },
    title: { type: "string", optional: true },
  },
  render(node: VellNode, ctx: ExtensionContext): string {
    const props = node.props ?? {};
    const type = String(props.type ?? "note");
    const title = props.title ? String(props.title) : "";
    const safeType = ctx.escapeHtml(type.toLowerCase());
    const safeTitle = title ? ctx.escapeHtml(title) : "";

    // Render children as HTML paragraphs
    const childrenHtml = renderExtensionsChildren(node, ctx);
    const titleHtml = safeTitle ? `<div class="callout-title">${safeTitle}</div>` : "";

    const iconMap: Record<string, string> = {
      note: "&#x1F4DD;",
      tip: "&#x1F4A1;",
      warning: "&#x26A0;",
      danger: "&#x1F6A8;",
      info: "&#x2139;",
      success: "&#x2705;",
      question: "&#x2753;",
    };
    const icon = iconMap[safeType] || "";

    return (
      `<div class="vell-callout vell-callout-${safeType}">` +
      `${icon ? `<span class="callout-icon">${icon}</span>` : ""}` +
      titleHtml +
      `<div class="callout-body">${childrenHtml}</div>` +
      `</div>`
    );
  },
  fallback(node: VellNode, ctx: ExtensionContext): string {
    const props = node.props ?? {};
    const type = String(props.type ?? "note");
    const childrenText = renderFallbackChildren(node, ctx);
    return `[${type.toUpperCase()}] ${childrenText}`;
  },
};

// ---------------------------------------------------------------------------
// Built-in Extension: npm/map (OpenStreetMap embed)
// ---------------------------------------------------------------------------

export const MapEmbed: ExtensionAdapter = {
  name: "npm/map",
  description: "Embed an interactive OpenStreetMap",
  schema: {
    lat: { type: "number", required: true },
    lng: { type: "number", required: true },
    zoom: { type: "number", default: 12 },
    title: { type: "string", optional: true },
  },
  render(node: VellNode, ctx: ExtensionContext): string {
    const props = node.props ?? {};
    const lat = Number(props.lat ?? 0);
    const lng = Number(props.lng ?? 0);
    const zoom = Number(props.zoom ?? 12);
    const title = props.title ? ctx.escapeHtml(String(props.title)) : "Map location";

    if (!lat && !lng) {
      return renderFallbackChildren(node, ctx);
    }

    const mapUrl = `https://www.openstreetmap.org/export/embed.html?bbox=${lng - 0.01},${lat - 0.01},${lng + 0.01},${lat + 0.01}&layer=mapnik&marker=${lat},${lng}`;
    return (
      `<div class="vell-extension" data-name="npm/map">` +
      `<iframe width="100%" height="350" src="${ctx.sanitizeUrl(mapUrl)}" ` +
      `title="${title}" frameborder="0" style="border:0" allowfullscreen></iframe>` +
      `<br><small><a href="https://www.openstreetmap.org/#map=${zoom}/${lat}/${lng}" target="_blank">${title}</a></small>` +
      (node.children ? `\n<div class="vell-extension-fallback">${renderFallbackChildren(node, ctx)}</div>` : "") +
      `</div>`
    );
  },
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** Extract text from children (same as TS renderer's extractTextFromChildren). */
function extractTextFromChildren(node: VellNode, _ctx: ExtensionContext): string {
  const parts: string[] = [];
  for (const child of node.children ?? []) {
    if (child.type === "Paragraph" && child.children) {
      for (const inline of child.children as Array<Record<string, unknown>>) {
        if (inline.type === "Text") {
          parts.push(String(inline.value ?? ""));
        }
      }
    }
  }
  return parts.join("\n");
}

/** Render extension children as simple HTML paragraphs. */
function renderExtensionsChildren(node: VellNode, ctx: ExtensionContext): string {
  const parts: string[] = [];
  for (const child of node.children ?? []) {
    if (child.type === "Paragraph" && child.children) {
      const text = extractTextFromChildren(child as VellNode, ctx);
      if (text) {
        parts.push(`<p>${ctx.escapeHtml(text)}</p>`);
      }
    }
  }
  return parts.join("\n");
}

// ---------------------------------------------------------------------------
// Auto-register all built-in extensions
// ---------------------------------------------------------------------------

register(YouTube, Vimeo, Chart, Mermaid, Callout, MapEmbed);
