// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Samin Yeasar

/** WASM loader abstraction for generated vell-wasm packages. */
export interface VellWasmModule {
  parse_to_json(source: string): string;
  validate(source: string): string;
  get_version(): string;
}

let wasmModule: VellWasmModule | undefined;

/** Installs a WASM module implementation. */
export function setWasmModule(module: VellWasmModule): void {
  wasmModule = module;
}

/** Returns the installed WASM module or a descriptive error. */
export function getWasmModule(): VellWasmModule {
  if (!wasmModule) {
    throw new Error("Vell WASM module has not been loaded. Call setWasmModule first.");
  }
  return wasmModule;
}
