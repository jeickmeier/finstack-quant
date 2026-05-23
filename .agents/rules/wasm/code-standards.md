---
trigger: model_decision
description: Rust-Wasm Bindings
globs:
---
# WASM Bindings Code Standards for finstack-wasm

## Core Principles

1. **Rust is canonical** — module tree and type names mirror the Rust umbrella crate; the JS facade owns namespacing only.
2. **Small bundle size** — minimize generated WASM size through careful feature selection.
3. **Performance** — avoid unnecessary allocations and copies between JS and WASM.
4. **Error handling** — convert Rust errors to JavaScript-friendly error messages.
5. **Cross-platform** — support both browser and Node.js environments.
6. **Builder entrypoints** — expose `Type.builder(...)` as the only builder entrypoint.

## Project Structure

### Organization

```
finstack-wasm/
├── src/
│   ├── lib.rs            # mod api; pub use api::*;
│   ├── api/              # NEW: crate-namespaced binding tree
│   │   ├── mod.rs        # pub mod declarations for each crate domain
│   │   ├── core_ns/      # core bindings (named core_ns to avoid shadowing std::core)
│   │   ├── analytics/
│   │   ├── margin/
│   │   ├── valuations/
│   │   ├── statements/
│   │   ├── statements_analytics/
│   │   ├── portfolio/
│   │   ├── scenarios/
│   │   ├── correlation/
│   │   └── monte_carlo/
│   └── utils.rs          # panic hook, etc.
├── index.js              # hand-written JS facade (public entrypoint)
├── index.d.ts            # TypeScript declarations for facade
├── exports/              # per-crate namespace JS files
│   ├── core.js
│   ├── analytics.js
│   ├── ...
│   └── monte_carlo.js
├── pkg/                  # generated wasm-bindgen output (INTERNAL, not public)
├── package.json          # main: ./index.js, types: ./index.d.ts
└── tests/
    └── *.test.mjs        # Node test runner facade tests
```

### Key Architecture Rules

- `finstack-wasm/src/lib.rs` exports only the `api` tree. Old flat re-exports are removed.
- `finstack-wasm/src/api/mod.rs` declares `pub mod` for each crate domain. No `pub use *` glob re-exports (they are unnecessary for wasm-bindgen and `pub use core::*` shadows `std::core`).
- The `core` Rust module is named `core_ns` on disk to avoid shadowing Rust's `core` prelude.
- `pkg/finstack_wasm.js` is an internal generated artifact, NOT the public API.
- The published entrypoint is `index.js`, a hand-written facade that groups raw bindgen exports into crate namespaces.

### Naming Conventions

- Types: `PascalCase` (matching Rust)
- Functions: `camelCase` (wasm-bindgen auto-converts snake_case)
- Namespace keys in facade: `snake_case` (matching Rust crate names)
- Name exceptions allowed only for host-language collisions (e.g. `FsDate` instead of `Date` to avoid JS built-in collision); documented in `parity_contract.toml`

## Type Wrapping Patterns

### CRITICAL: Always Use Named Structs with `pub(crate) inner`

```rust
use wasm_bindgen::prelude::*;

#[wasm_bindgen(js_name = TypeName)]
#[derive(Clone, Debug)]
pub struct JsTypeName {
    pub(crate) inner: finstack_core::TypeName,
}

#[wasm_bindgen(js_class = TypeName)]
impl JsTypeName {
    #[wasm_bindgen(constructor)]
    pub fn new(param: String) -> Result<JsTypeName, JsValue> {
        let inner = param
            .parse()
            .map_err(|e| JsValue::from_str(&format!("{}", e)))?;
        Ok(JsTypeName { inner })
    }

    pub(crate) fn inner(&self) -> &finstack_core::TypeName {
        &self.inner
    }
}
```

**Do NOT use tuple structs** (e.g., `pub struct JsBond(Bond)`) — they prevent safe type extraction from `JsValue` and cause `JsCast` trait bound errors.

### Property Getters

```rust
#[wasm_bindgen]
impl JsTypeName {
    #[wasm_bindgen(getter)]
    pub fn property(&self) -> String {
        self.inner.property().to_string()
    }

    #[wasm_bindgen(getter, js_name = "numericCode")]
    pub fn numeric_code(&self) -> u32 {
        self.inner.code()
    }
}
```

## Error Handling

```rust
fn convert_error(err: finstack_core::Error) -> JsValue {
    JsValue::from_str(&format!("{}", err))
}

#[wasm_bindgen]
impl JsMoney {
    #[wasm_bindgen]
    pub fn add(&self, other: &JsMoney) -> Result<JsMoney, JsValue> {
        self.inner
            .checked_add(&other.inner)
            .map(|result| JsMoney { inner: result })
            .map_err(convert_error)
    }
}
```

## JS Facade Pattern

Each `exports/<crate>.js` groups raw bindgen exports into a namespace:

```javascript
import * as raw from "../pkg/finstack_wasm.js";

export const core = {
  Currency: raw.Currency,
  Money: raw.Money,
  dates: {
    FsDate: raw.FsDate,
    DayCount: raw.DayCount,
    buildPeriods: raw.buildPeriods,
  },
  market_data: {
    MarketContext: raw.MarketContext,
    DiscountCurve: raw.DiscountCurve,
  },
};
```

And `index.js` re-exports all namespaces:

```javascript
import init from "./pkg/finstack_wasm.js";

export { core } from "./exports/core.js";
export { analytics } from "./exports/analytics.js";
// ... one per crate domain
export default init;
```

## Module Initialization

```rust
use wasm_bindgen::prelude::*;

mod api;
mod utils;

pub use api::*;

#[wasm_bindgen(start)]
pub fn init() {
    utils::set_panic_hook();
}
```

## Performance Guidelines

- Minimize boundary crossings: batch operations where possible.
- Accept references (`&self`, `&JsMoney`) over owned values.
- Return lightweight copies (String, f64); wasm-bindgen cannot return `&str`.
- Use `serde_wasm_bindgen::to_value` for complex objects.

## Testing

- Facade tests: `finstack-wasm/tests/*.test.mjs` (Node test runner)
- Rust-side tests: `wasm_bindgen_test` in `tests/web.rs`

```javascript
import test from "node:test";
import assert from "node:assert/strict";
import init, { core, analytics } from "../index.js";

await init();

test("core namespace exposes Currency", () => {
  assert.equal(typeof core.Currency, "function");
});
```

## Review Checklist

- [ ] Wrapper types use named struct with `pub(crate) inner`.
- [ ] No `pub use *` glob re-exports in `api/mod.rs`.
- [ ] Type names match Rust; only documented exceptions (e.g. `FsDate`).
- [ ] Errors converted to `JsValue`; no `.unwrap()` on user inputs.
- [ ] Facade JS file updated with new exports.
- [ ] Tests pass: `node --test tests/*.mjs` and `cargo test -p finstack-wasm`.
