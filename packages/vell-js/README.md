# @solez-ai/vell

Typed JavaScript API for the Vell parser, validator, and formatter.

## Current setup

This package does not yet bundle the generated `vell-wasm` binary. Build `crates/vell-wasm` with `wasm-pack`, initialize the generated module, and register it before calling the API.

```ts
import init, * as wasm from '../../target/vell-wasm-pkg/vell_wasm.js';
import { parse, setWasmModule } from '@solez-ai/vell';

await init();
setWasmModule(wasm);

const result = parse('= Hello\n\nWorld.');
```

The registered module must provide `parse_to_json`, `validate`, `format_source`, and `get_version`.

See the repository documentation for the complete API and WASM build instructions.
