## finstack-quant-wasm documentation style

`finstack-quant-wasm` is a Rust crate exported to JavaScript/TypeScript via `wasm-bindgen`.
The primary consumer experience is **TypeScript IntelliSense**, so documentation must be written
to render well in generated `.d.ts` files.

### Where docs live

- **Binding source**: Rust doc comments (`///`) on `#[wasm_bindgen]` exports in
  `finstack-quant-wasm/src/**`. Place them before the `#[wasm_bindgen]`
  attribute so wasm-bindgen includes them in its raw declarations.
- **Published source**: `index.d.ts` is the package's namespaced TypeScript
  facade and the `types` entry point. `wasm-pack build` generates the flat
  `pkg/finstack_quant_wasm.d.ts` input, not the published facade.
- **Synchronization**: after `mise run wasm-pkg`, run
  `node scripts/sync-facade-jsdoc.mjs --write` followed by
  `node scripts/complete-facade-jsdoc.mjs --write`. The first command carries
  Rust/wasm-bindgen documentation into the matching facade member; the second
  fills only missing TypeScript-facing contract sections from its exact
  signature. Review financial conventions and generated fallback wording when
  adding or changing an API.

### Required sections for exported APIs

For every exported function/class/constructor/static factory/method/property:

- **Summary**: 1–2 lines describing what the API does and when to use it.
- **Parameters**: Use JSDoc tags, or a Rustdoc `# Arguments` section when the
  API already follows the canonical Rust documentation style. Both forms must:
  - Name every caller-supplied parameter exactly (Rust or generated camelCase).
  - Give a substantive description including units and constraints.
  - JSDoc form: `@param <name> - description`.
  - Rustdoc form: `*`<name>`- description`.
  - `@returns - description (include units)`
  - `@throws - when an error is thrown`
- **Conventions** (when applicable):
  - Day count, calendar, compounding, settlement rules
  - Rate units (decimal vs bps)
  - Curve IDs expected in `MarketContext`
- **Examples**: Each namespace and constructor interface needs a copy/paste
  runnable `@example`. A class-level example may cover routine accessors; add a
  method-specific example when its calculation, convention, or input shape is
  not already clear from that class example.

Properties need a substantive hover summary. State units, representation, and
read-only or ownership behavior whenever it affects TypeScript callers.

### Financial documentation rules (non-negotiable)

- **Rates**: always state whether inputs are **decimal** (e.g. `0.05`) or **bps** (e.g. `120.0`).
- **Dates**: clarify the role of each date (`asOf` valuation date vs `issue`/`start` vs `maturity`).
- **Curves**: document expected IDs and required market data (what must exist in `MarketContext`).
- **Prices**: clarify quote convention (clean vs dirty, percent-of-par vs absolute).

### Template: constructor / factory

````rust
/// One-line summary of the API.
///
/// Conventions:
/// - Rates: ...
/// - Day count: ...
/// - Calendar/BDC: ...
///
/// @param instrument_id - ...
/// @param ... - ...
/// @returns ...
/// @throws {Error} ...
///
/// @example
/// ```javascript
/// import init, { core, valuations } from "finstack-quant-wasm";
///
/// await init();
/// const asOf = core.createDate(2024, 1, 2);
/// const isValid = valuations.instruments.validateInstrumentJson(instrumentJson);
/// const result = valuations.instruments.priceInstrument(instrumentJson, marketJson, asOf, "discounting");
/// ```
/// ```
````
