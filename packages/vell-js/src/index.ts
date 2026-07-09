// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Samin Yeasar

import { getWasmModule } from "./wasm";

/** Source span in byte offsets. */
export interface Span { start: number; end: number }
/** Directive property value. */
export type PropValue = string | number | boolean | null;
/** Inline node variants. */
export type InlineNode = { type: string; span: Span; [key: string]: unknown };
/** Block node variants. */
export type Node = { type: string; span: Span; [key: string]: unknown };
/** Document metadata. */
export interface DocumentMetadata { title?: string; author?: string; date?: string; lang?: string; variables: Record<string, unknown> }
/** Vell document AST. */
export interface VellDocument { version: number; children: Node[]; metadata: DocumentMetadata; span: Span }
/** Parser diagnostic. */
export interface ParseError { kind: string; span: Span; message: string; suggestion?: string }
/** Parser result. */
export interface ParseResult { document: VellDocument | null; errors: ParseError[]; warnings: ParseError[] }

/** Parses source with the installed WASM parser. */
export function parse(source: string): ParseResult { return JSON.parse(getWasmModule().parse_to_json(source)) as ParseResult; }
/** Parses source or throws the first parse error. */
export function parseOrThrow(source: string): VellDocument { const result = parse(source); if (!result.document) { throw new Error(result.errors[0]?.message ?? "Unable to parse Vell source"); } return result.document; }
/** Returns validation diagnostics. */
export function validate(source: string): ParseError[] { return JSON.parse(getWasmModule().validate(source)) as ParseError[]; }
/** Formats source with the currently installed formatter integration. */
export function format(source: string): string { return source.endsWith("\n") ? source : `${source}\n`; }
/** Returns parser version. */
export function getVersion(): string { return getWasmModule().get_version(); }
