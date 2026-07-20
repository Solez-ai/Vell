// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Samin Yeasar

/**
 * RSS 2.0 and Atom feed generator for Vell documents.
 * Walks the Vell AST and produces RSS or Atom XML feeds from
 * document metadata and section/heading content.
 */

import type { VellDocument, Node, InlineNode } from "./index.js";

/** Configuration for feed generation. */
export interface FeedOptions {
  /** Feed type: "rss" or "atom". Default: "rss". */
  type?: "rss" | "atom";
  /** Site URL (required for Atom). */
  siteUrl?: string;
  /** Feed description. */
  description?: string;
  /** Language tag (e.g. "en", "fr"). */
  language?: string;
  /** Maximum number of entries to include. Default: all. */
  maxEntries?: number;
  /** Author name (overrides document metadata). */
  author?: string;
  /** Copyright / license string. */
  copyright?: string;
}

/** A single feed entry. */
interface FeedEntry {
  title: string;
  url: string;
  description: string;
  date: string;
  author?: string;
}

/**
 * Generates an RSS 2.0 XML feed from a Vell document.
 * Uses headings as feed items with their text content as descriptions.
 */
export function generateRssFeed(doc: VellDocument, options?: FeedOptions): string {
  const opts: FeedOptions = { type: "rss", ...options };
  const title = doc.metadata?.title ?? "Vell Feed";
  const description = opts.description ?? `Feed generated from ${title}`;
  const language = opts.language ?? doc.metadata?.lang ?? "en";
  const siteUrl = opts.siteUrl ?? "https://vell-lang.dev";
  const author = opts.author ?? doc.metadata?.author ?? "Unknown";
  const copyright = opts.copyright ?? `Copyright ${new Date().getFullYear()}, ${author}`;

  const entries = collectEntries(doc, opts.maxEntries);

  let xml = `<?xml version="1.0" encoding="UTF-8"?>\n`;
  xml += `<rss version="2.0" xmlns:atom="http://www.w3.org/2005/Atom">\n`;
  xml += `<channel>\n`;
  xml += `  <title>${escapeXml(title)}</title>\n`;
  xml += `  <link>${escapeXml(siteUrl)}</link>\n`;
  xml += `  <description>${escapeXml(description)}</description>\n`;
  xml += `  <language>${escapeXml(language)}</language>\n`;
  xml += `  <copyright>${escapeXml(copyright)}</copyright>\n`;
  xml += `  <lastBuildDate>${toRssDate(new Date())}</lastBuildDate>\n`;
  xml += `  <atom:link href="${escapeXml(siteUrl)}/feed.xml" rel="self" type="application/rss+xml"/>\n`;
  xml += `  <generator>Vell Feed Generator</generator>\n`;

  for (const entry of entries) {
    xml += `  <item>\n`;
    xml += `    <title>${escapeXml(entry.title)}</title>\n`;
    xml += `    <link>${escapeXml(entry.url)}</link>\n`;
    xml += `    <guid isPermaLink="true">${escapeXml(entry.url)}</guid>\n`;
    xml += `    <description><![CDATA[${entry.description}]]></description>\n`;
    if (entry.author) xml += `    <author>${escapeXml(entry.author)}</author>\n`;
    xml += `    <pubDate>${toRssDate(new Date(entry.date))}</pubDate>\n`;
    xml += `  </item>\n`;
  }

  xml += `</channel>\n</rss>\n`;
  return xml;
}

/**
 * Generates an Atom 1.0 XML feed from a Vell document.
 */
export function generateAtomFeed(doc: VellDocument, options?: FeedOptions): string {
  const opts: FeedOptions = { type: "atom", ...options };
  const title = doc.metadata?.title ?? "Vell Feed";
  const description = opts.description ?? `Feed generated from ${title}`;
  const siteUrl = opts.siteUrl ?? "https://vell-lang.dev";
  const author = opts.author ?? doc.metadata?.author ?? "Unknown";
  const copyright = opts.copyright ?? `Copyright ${new Date().getFullYear()}, ${author}`;
  const feedUrl = `${siteUrl}/feed.atom`;

  const entries = collectEntries(doc, opts.maxEntries);

  let xml = `<?xml version="1.0" encoding="UTF-8"?>\n`;
  xml += `<feed xmlns="http://www.w3.org/2005/Atom">\n`;
  xml += `  <title>${escapeXml(title)}</title>\n`;
  xml += `  <subtitle>${escapeXml(description)}</subtitle>\n`;
  xml += `  <link href="${escapeXml(siteUrl)}" />\n`;
  xml += `  <link rel="self" href="${escapeXml(feedUrl)}" />\n`;
  xml += `  <id>${escapeXml(siteUrl)}</id>\n`;
  xml += `  <updated>${toAtomDate(new Date())}</updated>\n`;
  xml += `  <author><name>${escapeXml(author)}</name></author>\n`;
  xml += `  <rights>${escapeXml(copyright)}</rights>\n`;
  xml += `  <generator>Vell Feed Generator</generator>\n`;

  for (const entry of entries) {
    xml += `  <entry>\n`;
    xml += `    <title>${escapeXml(entry.title)}</title>\n`;
    xml += `    <link href="${escapeXml(entry.url)}" />\n`;
    xml += `    <id>${escapeXml(entry.url)}</id>\n`;
    xml += `    <published>${toAtomDate(new Date(entry.date))}</published>\n`;
    xml += `    <updated>${toAtomDate(new Date(entry.date))}</updated>\n`;
    xml += `    <summary type="html"><![CDATA[${entry.description}]]></summary>\n`;
    if (entry.author) xml += `    <author><name>${escapeXml(entry.author)}</name></author>\n`;
    xml += `  </entry>\n`;
  }

  xml += `</feed>\n`;
  return xml;
}

/** Collects feed entries from document headings and their paragraph content. */
function collectEntries(doc: VellDocument, maxEntries?: number): FeedEntry[] {
  const entries: FeedEntry[] = [];
  const siteUrl = "https://vell-lang.dev";
  const date = doc.metadata?.date ?? new Date().toISOString().split("T")[0];

  for (const node of doc.children ?? []) {
    if (node.type === "Heading") {
      const level = Number(node.level ?? 1);
      if (level > 2) continue; // Only top-level sections for feed
      const title = textOf(node.children as InlineNode[]);
      const id = String(node.id ?? slugify(title));
      const url = `${siteUrl}#${encodeURIComponent(id)}`;

      // Gather subsequent paragraph content as description
      let description = title;
      const idx = (doc.children ?? []).indexOf(node);
      if (idx >= 0 && idx + 1 < (doc.children ?? []).length) {
        const next = doc.children![idx + 1];
        if (next.type === "Paragraph") {
          const paraText = textOf(next.children as InlineNode[]).trim();
          if (paraText) description = `${title}: ${paraText.slice(0, 300)}`;
        }
      }

      entries.push({
        title,
        url,
        description,
        date,
        author: doc.metadata?.author,
      });

      if (maxEntries && entries.length >= maxEntries) break;
    }
  }

  return entries;
}

function textOf(nodes: InlineNode[] | undefined): string {
  if (!nodes) return "";
  let result = "";
  for (const node of nodes) {
    switch (node.type) {
      case "Text": result += String(node.value ?? ""); break;
      case "Code": result += String(node.value ?? ""); break;
      case "Bold": case "Italic": case "Underline": case "Strikethrough":
      case "Superscript": case "Subscript":
      case "Link": case "LinkRef":
        result += textOf(node.children as InlineNode[]); break;
      case "MathInline": result += String(node.source ?? ""); break;
      case "VarInterpolation": result += String(node.name ?? ""); break;
      case "SoftBreak": case "HardBreak": result += " "; break;
    }
  }
  return result;
}

function slugify(text: string): string {
  return text.normalize("NFKC").toLowerCase().replace(/[^\p{L}\p{N}]+/gu, "-").replace(/^-|-$/g, "") || "entry";
}

function escapeXml(s: string): string {
  return s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;").replace(/'/g, "&apos;");
}

function toRssDate(date: Date): string {
  return date.toUTCString();
}

function toAtomDate(date: Date): string {
  return date.toISOString();
}
