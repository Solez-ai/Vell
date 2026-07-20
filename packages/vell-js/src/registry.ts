// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Samin Yeasar

/**
 * Extension registry for Vell — allows third-party plugins to register
 * custom directive and inline component renderers at runtime.
 *
 * Plugins extend the HTML renderer by providing custom rendering functions
 * for `Directive` and `Extension` nodes by name.
 */

import type { VellDocument, Node, InlineNode } from "./index.js";

/** Render function signature for a directive or extension node. */
export type ExtensionRenderer = (
  node: Node,
  renderChildren: (nodes: Node[]) => string,
  renderInlines: (nodes?: InlineNode[]) => string,
  doc: VellDocument,
) => string;

/** Render function signature for an inline component. */
export type InlineComponentRenderer = (
  node: InlineNode,
  renderInlines: (nodes?: InlineNode[]) => string,
  doc: VellDocument,
) => string;

/** Plugin metadata. */
export interface ExtensionPlugin {
  name: string;
  version: string;
  description?: string;
  /** Custom renderers for block-level directives/extensions keyed by name. */
  directives?: Record<string, ExtensionRenderer>;
  /** Custom renderers for inline components keyed by name. */
  inlineComponents?: Record<string, InlineComponentRenderer>;
  /** Hooks called before the document is rendered. */
  beforeRender?: (doc: VellDocument) => void;
  /** Hooks called after the document is rendered (receives final HTML). */
  afterRender?: (html: string, doc: VellDocument) => string;
}

// ---------------------------------------------------------------------------
// Registry
// ---------------------------------------------------------------------------

/** Global extension registry state. */
const directiveRenderers = new Map<string, ExtensionRenderer>();
const inlineRenderers = new Map<string, InlineComponentRenderer>();
const beforeHooks: Array<(doc: VellDocument) => void> = [];
const afterHooks: Array<(html: string, doc: VellDocument) => string> = [];
const registeredPlugins = new Map<string, ExtensionPlugin>();

/**
 * Registers an extension plugin. Overwrites any existing plugin with the
 * same name.
 *
 * @returns the previous plugin with this name, if any.
 */
export function registerPlugin(plugin: ExtensionPlugin): ExtensionPlugin | undefined {
  const existing = registeredPlugins.get(plugin.name);

  // Register directive renderers
  if (plugin.directives) {
    for (const [name, renderer] of Object.entries(plugin.directives)) {
      directiveRenderers.set(name, renderer);
    }
  }

  // Register inline component renderers
  if (plugin.inlineComponents) {
    for (const [name, renderer] of Object.entries(plugin.inlineComponents)) {
      inlineRenderers.set(name, renderer);
    }
  }

  // Register hooks
  if (plugin.beforeRender) {
    beforeHooks.push(plugin.beforeRender);
  }
  if (plugin.afterRender) {
    afterHooks.push(plugin.afterRender);
  }

  registeredPlugins.set(plugin.name, plugin);
  return existing;
}

/**
 * Unregisters a plugin by name. Removes all its directive renderers,
 * inline component renderers, and hooks.
 */
export function unregisterPlugin(name: string): boolean {
  const plugin = registeredPlugins.get(name);
  if (!plugin) return false;

  if (plugin.directives) {
    for (const directiveName of Object.keys(plugin.directives)) {
      directiveRenderers.delete(directiveName);
    }
  }
  if (plugin.inlineComponents) {
    for (const compName of Object.keys(plugin.inlineComponents)) {
      inlineRenderers.delete(compName);
    }
  }
  // Hooks can't be individually removed without tracking, so we rebuild
  beforeHooks.length = 0;
  afterHooks.length = 0;
  registeredPlugins.delete(name);

  // Re-register all remaining plugins' hooks
  for (const [, p] of registeredPlugins) {
    if (p.beforeRender) beforeHooks.push(p.beforeRender);
    if (p.afterRender) afterHooks.push(p.afterRender);
  }
  return true;
}

/** Returns all registered plugins. */
export function getPlugins(): ExtensionPlugin[] {
  return [...registeredPlugins.values()];
}

/** Returns the directive renderer for a given name, if one is registered. */
export function getDirectiveRenderer(name: string): ExtensionRenderer | undefined {
  return directiveRenderers.get(name);
}

/** Returns the inline component renderer for a given name, if one is registered. */
export function getInlineRenderer(name: string): InlineComponentRenderer | undefined {
  return inlineRenderers.get(name);
}

/** Runs all before-render hooks. */
export function runBeforeRenderHooks(doc: VellDocument): void {
  for (const hook of beforeHooks) {
    hook(doc);
  }
}

/** Runs all after-render hooks. */
export function runAfterRenderHooks(html: string, doc: VellDocument): string {
  let result = html;
  for (const hook of afterHooks) {
    result = hook(result, doc);
  }
  return result;
}
