#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Builds the Vell WASM module and copies it into the npm package.
#
# Prerequisites:
#   - wasm-pack (install: cargo install wasm-pack)
#   - Rust toolchain with wasm32-unknown-unknown target
#     (install: rustup target add wasm32-unknown-unknown)
#
# Usage: bash scripts/build-wasm.sh

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PKG_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
PROJECT_ROOT="$(cd "$PKG_DIR/../.." && pwd)"
WASM_CRATE="$PROJECT_ROOT/crates/vell-wasm"

echo "==> Building vell-wasm with wasm-pack..."
cd "$WASM_CRATE"
wasm-pack build \
  --target web \
  --out-dir "$PKG_DIR/pkg" \
  --release

echo ""
echo "==> WASM build complete."
echo ""
echo "    Files written to: $PKG_DIR/pkg/"
echo "    - vell_wasm_bg.wasm  (WebAssembly binary)"
echo "    - vell_wasm.js       (JS glue code)"
echo "    - vell_wasm.d.ts     (TypeScript types)"
echo "    - package.json       (npm metadata for the WASM package)"
echo ""
echo "    To publish the npm package with WASM included, run:"
echo "      cd $PKG_DIR && npm publish"
