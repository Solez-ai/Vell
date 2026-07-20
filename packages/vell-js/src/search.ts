// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Samin Yeasar

/**
 * Search index generator for Vell documents.
 * Walks the Vell AST and produces a JSON search index of headings,
 * text content, metadata, and code blocks.
 */

import type { VellDocument, Node, InlineNode } from "./index.js";

/** A single searchable entry in the index. */
export interface SearchEntry {
  /** Section or context title (e.g. heading text). */
  title: string;
  /** Plain-text content snippet. */
  snippet: string;
  /** URL fragment anchor (e.g. "#my-heading"). */
  url: string;
  /** Content type: "heading", "paragraph", "code", "figure", "table", etc. */
  type: string;
  /** Optional heading level for hierarchical sorting. */
  level?: number;
  /** Byte offset in source (from AST spans). */
  offset?: number;
}

/** Complete search index for a document. */
export interface SearchIndex {
  /** Document title from metadata. */
  title: string;
  /** ISO date string from metadata. */
  date?: string;
  /** Author name from metadata. */
  author?: string;
  /** Language tag. */
  lang?: string;
  /** All searchable entries. */
  entries: SearchEntry[];
}

/**
 * Generates a search index from a parsed Vell document.
 * Walks the AST and collects headings, paragraphs, code blocks,
 * figures, and table content into searchable entries.
 */
export function generateSearchIndex(doc: VellDocument): SearchIndex {
  const entries: SearchEntry[] = [];
  let currentHeading = "Document";

  walkNodes(doc.children ?? [], entries, () => currentHeading, (title) => { currentHeading = title; });

  return {
    title: doc.metadata?.title ?? "Vell Document",
    date: doc.metadata?.date,
    author: doc.metadata?.author,
    lang: doc.metadata?.lang,
    entries,
  };
}

function walkNodes(
  nodes: Node[],
  entries: SearchEntry[],
  getHeading: () => string,
  setHeading: (title: string) => void,
): void {
  for (const node of nodes) {
    switch (node.type) {
      case "Heading": {
        const level = Number(node.level ?? 1);
        const text = textOf(node.children as InlineNode[]);
        const id = String(node.id ?? slugify(text));
        setHeading(text);
        entries.push({
          title: text,
          snippet: text,
          url: `#${id}`,
          type: "heading",
          level,
          offset: (node.span as { start?: number })?.start,
        });
        break;
      }
      case "Paragraph": {
        const text = textOf(node.children as InlineNode[]);
        const trimmed = text.trim();
        if (trimmed) {
          entries.push({
            title: getHeading(),
            snippet: trimmed.slice(0, 200),
            url: "",
            type: "paragraph",
            offset: (node.span as { start?: number })?.start,
          });
        }
        break;
      }
      case "CodeBlock": {
        const lang = String(node.lang ?? "");
        const source = String(node.source ?? "");
        const firstLine = source.split("\n")[0]?.trim() ?? "";
        const snippet = firstLine ? `${lang ? `[${lang}] ` : ""}${firstLine}` : `Code block${lang ? ` (${lang})` : ""}`;
        entries.push({
          title: getHeading(),
          snippet: snippet.slice(0, 200),
          url: "",
          type: "code",
          offset: (node.span as { start?: number })?.start,
        });
        break;
      }
      case "Directive": {
        const name = String(node.name ?? "");
        const props = node.props as Record<string, unknown> | undefined;
        if (name === "Figure") {
          const caption = String(props?.caption ?? "");
          if (caption) {
            entries.push({
              title: getHeading(),
              snippet: caption,
              url: "",
              type: "figure",
              offset: (node.span as { start?: number })?.start,
            });
          }
        }
        if (node.children && Array.isArray(node.children)) {
          walkNodes(node.children as Node[], entries, getHeading, setHeading);
        }
        break;
      }
      case "Table": {
        const headers = node.headers as Node[] ?? [];
        const headerText = headers
          .map((h: Node) => textOf(h.children as InlineNode[]))
          .join(" | ");
        if (headerText) {
          entries.push({
            title: getHeading(),
            snippet: `Table: ${headerText}`,
            url: "",
            type: "table",
            offset: (node.span as { start?: number })?.start,
          });
        }
        break;
      }
      case "Blockquote": {
        if (node.children && Array.isArray(node.children)) {
          walkNodes(node.children as Node[], entries, getHeading, setHeading);
        }
        break;
      }
      case "List": {
        const items = node.items as Node[] ?? [];
        for (const item of items) {
          const itemText = extractPlainText(item.children as Node[]);
          if (itemText.trim()) {
            entries.push({
              title: getHeading(),
              snippet: itemText.trim().slice(0, 200),
              url: "",
              type: "list-item",
              offset: (item.span as { start?: number })?.start,
            });
          }
        }
        break;
      }
      case "ForLoop":
      case "IfBlock":
      case "Extension":
      case "FootnoteDefinition": {
        if (node.children && Array.isArray(node.children)) {
          walkNodes(node.children as Node[], entries, getHeading, setHeading);
        }
        if (node.type === "IfBlock" && node.alternate && Array.isArray(node.alternate)) {
          walkNodes(node.alternate as Node[], entries, getHeading, setHeading);
        }
        break;
      }
    }
  }
}

/** Extracts plain text from inline nodes. */
function textOf(nodes: InlineNode[] | undefined): string {
  if (!nodes) return "";
  let result = "";
  for (const node of nodes) {
    switch (node.type) {
      case "Text":
        result += String(node.value ?? "");
        break;
      case "Code":
        result += String(node.value ?? "");
        break;
      case "Bold":
      case "Italic":
      case "Underline":
      case "Strikethrough":
      case "Superscript":
      case "Subscript":
      case "Link":
      case "LinkRef":
        result += textOf(node.children as InlineNode[]);
        break;
      case "MathInline":
        result += String(node.source ?? "");
        break;
      case "VarInterpolation":
        result += String(node.name ?? "");
        break;
      case "Citation":
        result += String(node.key ?? "");
        break;
      case "SoftBreak":
      case "HardBreak":
        result += " ";
        break;
    }
  }
  return result;
}

/** Extracts plain text from block children. */
function extractPlainText(nodes: Node[]): string {
  const parts: string[] = [];
  for (const node of nodes) {
    if (node.type === "Paragraph" || node.type === "Heading") {
      parts.push(textOf(node.children as InlineNode[]));
    }
  }
  return parts.join(" ");
}

/** Slugifies text for URL fragments. */
function slugify(text: string): string {
  return text
    .normalize("NFKC")
    .toLowerCase()
    .replace(/[^\p{L}\p{N}]+/gu, "-")
    .replace(/^-|-$/g, "") || "entry";
}
