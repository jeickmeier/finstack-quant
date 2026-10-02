---
trigger: model_decision
description: Rust-Wasm Bindings
globs:
---

# WASM Bindings Code Standards for finstack-quant-wasm

The snippets below are taken from the crate. The longer contributor guide —
the file-to-namespace table, the conversion helpers and the step-by-step
"adding a binding" recipe — is `finstack-quant-wasm/src/api/README.md`; the test
layers are described in `finstack-quant-wasm/tests/README.md`.

## Core Principles

1. **Rust is canonical** — module tree and type names mirror the Rust umbrella crate; the JS facade owns namespacing only.
2. **Bindings convert, Rust computes** — a binding converts arguments, calls one Rust function, and maps the error. Validation, defaults and result shapes live in the Rust crate.
3. **Strict boundary** — every host argument other than `f64` is a `JsValue` converted by `crate::utils::input` / `crate::utils::wire`; nothing is coerced.
4. **Structured errors** — every error goes through `crate::utils::to_js_err`; the `kind` comes from the Rust error type.
5. **Cross-platform** — support both browser and Node.js environments.
6. **Builder entrypoints** — expose `Type.builder(...)` as the only builder entrypoint.

## Project Structure

### Organization

```
finstack-quant-wasm/
├── src/
│   ├── lib.rs            # pub mod api; pub mod utils; the start() panic hook
│   ├── api/              # crate-namespaced binding tree (see api/README.md)
│   │   ├── mod.rs        # pub mod declarations for each crate domain
│   │   ├── core/         # core bindings (no glob re-export, so no std::core shadowing)
│   │   ├── analytics/
│   │   ├── attribution/
│   │   ├── calibration/
│   │   ├── cashflows/
│   │   ├── covenants/
│   │   ├── features/
│   │   ├── margin/
│   │   ├── models/       # monte_carlo, factor, volatility, ... submodules
│   │   ├── portfolio/
│   │   ├── scenarios/
│   │   ├── statements/
│   │   ├── statements_analytics/
│   │   └── valuations/
│   └── utils/            # mod.rs (to_js_value, to_js_err), input.rs, wire.rs, date.rs
├── index.js              # hand-written JS facade (public entrypoint)
├── index.d.ts            # TypeScript declarations for the facade
├── exports/              # per-crate namespace JS files
│   ├── core.js
│   ├── analytics.js
│   ├── ...
│   ├── models.js         # + exports/models/{monteCarlo,factor,...}.js
│   └── valuations.js     # + exports/valuations/{instruments,fx,...}.js
├── facade-surface.json   # checked-in export surface (tests/facade/surface.test.mjs)
├── types/                # TypeScript types generated from the Rust JSON Schemas
├── pkg/, pkg-node/       # generated wasm-bindgen output (INTERNAL, not public)
├── scripts/              # doc sync / doc checks / contract-type generation
├── package.json          # main: ./index.js, types: ./index.d.ts
└── tests/
    ├── wasm.rs + wasm_*.rs   # wasm-bindgen-test suites (wasm32)
    ├── dts_contract.rs, return_shapes.rs, boundary_signatures.rs   # host tests
    ├── facade/*.test.mjs     # Node test runner, against the built package
    └── typescript/, scripts/ # tsc declaration checks, doc-tooling tests
```

### Key Architecture Rules

- `finstack-quant-wasm/src/lib.rs` exports only the `api` and `utils` trees. There are no flat re-exports.
- `finstack-quant-wasm/src/api/mod.rs` declares `pub mod` for each crate domain. No `pub use *` glob re-exports (they are unnecessary for wasm-bindgen and `pub use core::*` shadows `std::core`).
- The `core` Rust module is named `core` (`src/api/mod.rs`). `src/lib.rs` declares
  `pub mod api;` and deliberately does **not** `pub use api::*` — without the glob
  there is no `std::core` shadowing, so no `core_ns` rename is needed.
- `pkg/finstack_quant_wasm.js` is an internal generated artifact, NOT the public API.
- The published entrypoint is `index.js`, a hand-written facade that groups raw bindgen exports into crate namespaces.

### Naming Conventions

- Types: `PascalCase` (matching Rust), exported with `#[wasm_bindgen(js_name = TypeName)]` on a `JsTypeName` struct.
- Functions and members: the camelCase form of the exact Rust name, set with `js_name` (`from_numeric` → `fromNumeric`). A one-word name needs no change (`numeric` stays `numeric`).
- Namespace keys in the facade: the Rust crate name (`statements_analytics`), with camelCase sub-namespaces (`models.monteCarlo`).
- Name exceptions are allowed only for documented host-language collisions.

## Type Wrapping Patterns

### CRITICAL: Always Use Named Structs with `pub(crate) inner`

From `src/api/core/currency.rs`:

```rust
use crate::utils::to_js_err;
use finstack_quant_core::currency::Currency as RustCurrency;
use std::str::FromStr;
use wasm_bindgen::prelude::*;

#[wasm_bindgen(js_name = Currency)]
pub struct JsCurrency {
    pub(crate) inner: RustCurrency,
}

#[wasm_bindgen(js_class = Currency)]
impl JsCurrency {
    #[wasm_bindgen(constructor)]
    pub fn new(code: JsValue) -> Result<JsCurrency, JsValue> {
        let code = crate::utils::input::js_string(&code, "code")?;
        RustCurrency::from_str(&code)
            .map(|inner| JsCurrency { inner })
            .map_err(to_js_err)
    }
}
```

**Do NOT use tuple structs** (e.g., `pub struct JsBond(Bond)`) — they prevent safe type extraction from `JsValue` and cause `JsCast` trait bound errors.

### Arguments

Exported functions never declare `&str`, `String`, `bool`, integer or
`Vec<String>` parameters: wasm-bindgen's glue for those traps on a wrong type or
coerces it silently. Declare `JsValue` (`Option<JsValue>` when optional) and
convert; `tests/boundary_signatures.rs` enforces this.

| Argument | Helper |
| --- | --- |
| string, boolean, integer | `input::js_string`, `js_bool`, `js_uint` / `js_int` (and the `js_opt_*` twins) |
| number sequence / matrix | `input::js_f64_seq`, `js_f64_matrix` |
| ISO date string | `wire::js_date` |
| struct-shaped spec / config / result (object or JSON text) | `input::from_js_json::<T>` |
| value whose wire form can be a bare string — a unit enum variant (`"parallel"`), a day count, a currency | `wire::js_wire::<T>` / `js_opt_wire` |

`from_js_json` parses a JavaScript string as JSON text, so it rejects the bare
label `"parallel"`; any type with a string-valued wire form must use `js_wire`.
Never call `serde_wasm_bindgen::from_value`: its struct visitor ignores
`deny_unknown_fields`.

### Property Getters

```rust
#[wasm_bindgen(js_class = Currency)]
impl JsCurrency {
    /// ISO-4217 numeric code.
    #[wasm_bindgen(getter, js_name = numeric)]
    pub fn numeric(&self) -> u16 {
        self.inner.numeric()
    }
}
```

`js_name` is the camelCase form of the exact Rust accessor name
(`rounding_mode` → `roundingMode`); it never renames (`numeric` is not
`numericCode`).

## Error Handling

From `src/api/core/money.rs`:

```rust
use crate::utils::to_js_err;

#[wasm_bindgen(js_class = Money)]
impl JsMoney {
    #[wasm_bindgen(js_name = checkedAdd)]
    pub fn checked_add(&self, other: &JsMoney) -> Result<JsMoney, JsValue> {
        self.inner
            .checked_add(other.inner)
            .map(|inner| JsMoney { inner })
            .map_err(to_js_err)
    }
}
```

`to_js_err` throws a real `Error` named `FinstackError` whose `kind`
(`validation`, `not_found`, `computation`) comes from the Rust error type via
`IntoJsError`, never from the message text. A wrong argument type is a
`TypeError` with `kind: "invalid_type"`, thrown by the `utils::input` helpers.
Never build an error from a string (`JsValue::from_str`, `js_sys::Error::new`):
it has no `name` or `kind`. `mise run wasm-check-errors` (part of `wasm-lint`)
rejects that outside `src/utils`.

## Returning Values

- Handles (`JsCurrency`, `JsMoney`) for stateful Rust types; plain objects for data types (specs, configs, results).
- Serialize plain objects only with `crate::utils::to_js_value` (or `to_js_value_with_bigints` for payloads that embed a `ValuationResult`). `serde_wasm_bindgen::to_value` emits ES `Map`s that `JSON.stringify` drops silently; `mise run wasm-check-serializer` (part of `wasm-lint`) rejects it.
- Only `*Json`-suffixed exports return JSON text, and each has a typed twin.

## JS Facade Pattern

Each `exports/<crate>.js` groups raw bindgen exports into a namespace. From
`exports/valuations/market.js`:

```javascript
import * as wasm from '../../pkg/finstack_quant_wasm.js';

export const market = {
  listedProductCatalog: wasm.listedProductCatalog,
  ConventionRegistry: wasm.ConventionRegistry,
};
```

And `index.js` re-exports the initializer and all namespaces:

```javascript
export { default } from './pkg/finstack_quant_wasm.js';

export { core } from './exports/core.js';
export { analytics } from './exports/analytics.js';
// ... one per crate domain
```

The facade is a re-export map. JS logic is allowed there only where
wasm-bindgen cannot express the Rust signature (an optional borrowed handle,
for example), with a comment saying so. After adding, removing or renaming an
export, regenerate `facade-surface.json`
(`UPDATE_FACADE_SURFACE=1 node --test tests/facade/surface.test.mjs`) and
update `index.d.ts`.

## Module Initialization

`src/lib.rs`:

```rust
use wasm_bindgen::prelude::*;

pub mod api;
pub mod utils;

/// Module initializer: installs the panic hook.
#[wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();
}
```

wasm-bindgen runs `start` when the module is instantiated; the facade does not
export it.

## Documentation

Rustdoc above `#[wasm_bindgen]` is the source of the published JSDoc: document
every parameter (`@param`), return (`@returns`) and thrown kind (`@throws`), then
run `npm --prefix finstack-quant-wasm run docs:sync`. Every `@example` must be
a complete program: `mise run wasm-doc` type-checks each one against
`index.d.ts`, and `tests/facade/doc_examples.test.mjs` executes it.

## Performance Guidelines

- Minimize boundary crossings: batch operations where possible.
- Accept handle references (`&self`, `&JsMoney`) over owned handles: a by-value handle argument consumes the caller's object.
- Return numeric vectors as `Float64Array` (`Vec<f64>`), not as serialized arrays.

## Testing

- Facade tests: `finstack-quant-wasm/tests/facade/*.test.mjs` (Node test runner, against the built package).
- wasm32 binding tests: `wasm_bindgen_test` in `tests/wasm_*.rs`.
- Host tests of the declarations: `tests/dts_contract.rs`, `tests/return_shapes.rs`, `tests/boundary_signatures.rs`.

A facade test reads the `.wasm` bytes, because Node cannot fetch a file URL:

```javascript
import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import init, { core } from '../../index.js';

await init({
  module_or_path: readFileSync(new URL('../../pkg/finstack_quant_wasm_bg.wasm', import.meta.url)),
});

test('core namespace exposes Currency', () => {
  assert.equal(typeof core.Currency, 'function');
});
```

## Review Checklist

- [ ] Wrapper types use named struct with `pub(crate) inner`.
- [ ] No `pub use *` glob re-exports in `api/mod.rs`.
- [ ] Type and member names match Rust; any exception is explicitly documented.
- [ ] Arguments are `JsValue` converted by `utils::input` / `utils::wire`; string-valued wire types use `js_wire`.
- [ ] Errors go through `to_js_err`; results through `to_js_value`; no `.unwrap()` / `.expect()`.
- [ ] Facade JS file, `index.d.ts` and `facade-surface.json` updated with new exports.
- [ ] Gates pass: `mise run rust-lint-crate -- finstack-quant-wasm`, `mise run wasm-test`, `mise run wasm-doc`, `mise run wasm-lint`.
