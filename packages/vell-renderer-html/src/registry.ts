// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Samin Yeasar

/**
 * Standalone extension registry for the vell-renderer-html package.
 * Allows third-party plugins to register custom directive and
 * inline component renderers at runtime.
 *
 * This is an independent copy that the renderer can use without
 * depending on the vell-js package.
 */

/** Render function signature for a directive or extension node. */
export type ExtensionRenderer = (
  node: any,
  renderChildren: (nodes: any[]) => string,
  renderInlines: (nodes?: any[]) => string,
  doc: any,
) => string;

/** Render function signature for an inline component. */
export type InlineComponentRenderer = (
  node: any,
  renderInlines: (nodes?: any[]) => string,
  doc: any,
) => string;

/** Plugin metadata. */
export interface ExtensionPlugin {
  name: string;
  version: string;
  description?: string;
  directives?: Record<string, ExtensionRenderer>;
  inlineComponents?: Record<string, InlineComponentRenderer>;
  beforeRender?: (doc: any) => void;
  afterRender?: (html: string, doc: any) => string;
}

// Global registry state
const directiveRenderers = new Map<string, ExtensionRenderer>();
const inlineRenderers = new Map<string, InlineComponentRenderer>();
const beforeHooks: Array<(doc: any) => void> = [];
const afterHooks: Array<(html: string, doc: any) => string> = [];
const registeredPlugins = new Map<string, ExtensionPlugin>();

/**
 * Registers an extension plugin.
 * @returns the previous plugin with this name, if any.
 */
export function registerPlugin(plugin: ExtensionPlugin): ExtensionPlugin | undefined {
  const existing = registeredPlugins.get(plugin.name);

  if (plugin.directives) {
    for (const [name, renderer] of Object.entries(plugin.directives)) {
      directiveRenderers.set(name, renderer);
    }
  }

  if (plugin.inlineComponents) {
    for (const [name, renderer] of Object.entries(plugin.inlineComponents)) {
      inlineRenderers.set(name, renderer);
    }
  }

  if (plugin.beforeRender) beforeHooks.push(plugin.beforeRender);
  if (plugin.afterRender) afterHooks.push(plugin.afterRender);

  registeredPlugins.set(plugin.name, plugin);
  return existing;
}

/** Unregisters a plugin by name. */
export function unregisterPlugin(name: string): boolean {
  const plugin = registeredPlugins.get(name);
  if (!plugin) return false;
  if (plugin.directives) for (const k of Object.keys(plugin.directives)) directiveRenderers.delete(k);
  if (plugin.inlineComponents) for (const k of Object.keys(plugin.inlineComponents)) inlineRenderers.delete(k);
  registeredPlugins.delete(name);
  // Rebuild hooks
  beforeHooks.length = 0;
  afterHooks.length = 0;
  for (const [, p] of registeredPlugins) {
    if (p.beforeRender) beforeHooks.push(p.beforeRender);
    if (p.afterRender) afterHooks.push(p.afterRender);
  }
  return true;
}

/** Returns the directive renderer for a given name, if registered. */
export function getDirectiveRenderer(name: string): ExtensionRenderer | undefined {
  return directiveRenderers.get(name);
}

/** Returns the inline component renderer for a given name, if registered. */
export function getInlineRenderer(name: string): InlineComponentRenderer | undefined {
  return inlineRenderers.get(name);
}

/** Runs all before-render hooks. */
export function runBeforeRenderHooks(doc: any): void {
  for (const hook of beforeHooks) hook(doc);
}

/** Runs all after-render hooks. */
export function runAfterRenderHooks(html: string, doc: any): string {
  let result = html;
  for (const hook of afterHooks) result = hook(result, doc);
  return result;
}
