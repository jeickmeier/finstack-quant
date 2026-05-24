## finstack-wasm documentation style

`finstack-wasm` is a Rust crate exported to JavaScript/TypeScript via `wasm-bindgen`.
The primary consumer experience is **TypeScript IntelliSense**, so documentation must be written
to render well in generated `.d.ts` files.

### Where docs live

- **Source of truth**: Rust doc comments (`///`) on `#[wasm_bindgen]` exports in `finstack-wasm/src/**`.
- **Delivery mechanism**: `wasm-pack build` generates `pkg/finstack_wasm.d.ts`; this package’s `types`
  points to that file.

### Required sections for exported APIs

For every exported function/class/constructor/static factory/method:

- **Summary**: 1–2 lines describing what the API does and when to use it.
- **Parameters**: Use JSDoc tags:
  - `@param <name> - description (include units + constraints)`
  - `@returns - description (include units)`
  - `@throws - when an error is thrown`
- **Conventions** (when applicable):
  - Day count, calendar, compounding, settlement rules
  - Rate units (decimal vs bps)
  - Curve IDs expected in `MarketContext`
- **Example**: At least one `@example` block that is copy/paste runnable.

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
/// @returns - ...
/// @throws {Error} ...
///
/// @example
/// ```javascript
/// import init, { core, valuations } from "finstack-wasm";
///
/// await init();
/// const asOf = core.createDate(2024, 1, 2);
/// const isValid = valuations.instruments.validateInstrumentJson(instrumentJson);
/// const result = valuations.instruments.priceInstrument(instrumentJson, marketJson, asOf, "discounting");
/// ```
/// ```
````
