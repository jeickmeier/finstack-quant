# finstack-quant-wasm

WebAssembly bindings for the Finstack Quant workspace. The Rust crate compiles the
domain crates to `wasm32-unknown-unknown` with `wasm-bindgen`; the npm package wraps
the generated output in a hand-written facade so browser and Node.js callers see the
same namespace tree as the Rust umbrella crate.

Nothing in this package computes. Every export converts arguments, delegates to a
workspace crate, and converts the result back, so behavior questions are answered by
the Rust crate — not here.

## Where it sits

The crate depends on the 13 domain crates under [`../finstack-quant/`](../finstack-quant)
(`core`, `analytics`, `attribution`, `cashflows`, `covenants`,
`features`, `margin`, `models`, `valuations`, `statements`,
`statements-analytics`, `portfolio`, `scenarios`). No Rust crate depends on this one.

[`finstack-quant-py`](../finstack-quant-py/README.md) is the sibling binding layer;
the two are held to the same result-return contract, and their return-shape tests are
line-for-line mirrors.

The package ships no worker or thread-pool setup, so every call runs on the calling
thread and blocks it. Calibration, Monte Carlo, portfolio revaluation, and factor
sensitivities are CPU-heavy; run them in a Web Worker and behind an
application-level timeout. The facade files flag the specific entry points where
this matters.

## Package layout

| Path                                           | Role                                                                          |
| ---------------------------------------------- | ----------------------------------------------------------------------------- |
| [`index.js`](index.js)                         | published entrypoint; default export is the wasm initializer                  |
| [`index.d.ts`](index.d.ts)                     | hand-maintained TypeScript contract for the facade (the IntelliSense surface) |
| [`exports/`](exports)                          | one namespace shim per crate domain, re-exporting raw bindgen names           |
| [`src/api/`](src/api)                          | Rust bindings, one module per crate domain                                    |
| [`src/utils/`](src/utils)                      | shared error, serialization, and date conversion helpers                      |
| [`types/generated/`](types/generated)          | Rust-owned TypeScript types for JSON envelopes (`ts-rs` output)               |
| [`tests/`](tests)                              | four test layers — see [`tests/README.md`](tests/README.md)                   |
| [`benchmarks/bench.mjs`](benchmarks/bench.mjs) | Node micro-benchmarks against the `pkg-node/` build                           |
| [`scripts/`](scripts)                          | JSDoc / TypeScript documentation checkers and sync tooling                    |
| `pkg/`, `pkg-node/`                            | gitignored `wasm-pack` output (web and Node targets) — never edit             |

`pkg/finstack_quant_wasm.js` is an internal build artifact, not the public API.
Its `README.md` and `.d.ts` are wasm-pack copies; treat nothing in `pkg/` or
`pkg-node/` as source.

## Namespaces

`index.js` exports the initializer plus these 14 namespaces, assembled from
`exports/*.js`:

| Namespace              | Contents                                                                                                                                                                                                                                                        |
| ---------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `core`                 | `Currency`, `Money`, `Rate`/`Bps`/`Percentage`, `DayCount`, `Tenor`, date helpers, `DiscountCurve`/`HazardCurve`/`ForwardCurve`, `VolCube`, `FxDeltaVolSurface`, `FxMatrix`, and the `math` helpers (Cholesky, statistics, special functions, stable summation) |
| `analytics`            | `Performance` panel engine, `constrainedLeastSquares`                                                                                                                                                                                                           |
| `attribution`          | `attributePnl`, `attributePnlJson`, waterfall/metric defaults, schema validation                                                                                                                                                                                |
| `calibration`          | quote ingestion, market construction, plan validation, dependency graphs, and explicit Bermudan LMM base-vol fitting                                                                                                                                            |
| `cashflows`            | schedule build/validate, `accruedInterest`, dated flows, CPR↔SMM and CDR↔MDR conversions                                                                                                                                                                        |
| `covenants`            | spec/report/engine validation, `evaluateEngine`, preset covenant packages                                                                                                                                                                                       |
| `features`             | signal cleaning, neutralization, weighting, and timeseries / cross-sectional / panel transforms                                                                                                                                                                 |
| `margin`               | CSA presets and validation, `calculateVm`, `computeBilateralXva`                                                                                                                                                                                                |
| `models`               | analytical/Fourier/SABR exports plus nested `monteCarlo`, `credit`, `correlation`, `factor`, `rates`, `volatility`, and `liquidity` model engines                                                                                                               |
| `portfolio`            | `Portfolio` and `InstrumentArtifactCache`, materialization, Brinson / Campisi / grid attribution, TWRR and MWR, valuation and scenario revaluation, VaR and ES decomposition, factor sensitivities                                                              |
| `scenarios`            | spec parse/compose/validate, builtin templates and components, `applyScenario`, `computeHorizonReturn`                                                                                                                                                          |
| `statements`           | model and check-suite validation, `evaluateModel`, `runMonteCarlo`, formula parsing                                                                                                                                                                             |
| `statements_analytics` | sensitivity, variance, scenario sets, backtesting, goal seek, DCF, LBO, WACC, check reports, comps                                                                                                                                                              |
| `valuations`           | nested `instruments`, `fx`, `creditDerivatives`, `composite`, and `market`; product-specific coupon helpers; and the reusable `Market` handle                                                                                                                   |

Hover any namespace member in a TypeScript IDE for its arguments, result shape,
error behavior, and conventions. `index.d.ts` is the authoritative surface; use its
camelCase parameter names.

## Quick start

```javascript
import init, { analytics, calibration, core, models, valuations } from 'finstack-quant-wasm';

await init();

// Currency-tagged amounts. `amount` is the f64 view; `amountDecimal()` is exact.
const usd = new core.Currency('USD');
const notional = new core.Money(1_000_000, usd);
notional.amount; // 1000000
notional.amountDecimal(); // '1000000'

// Rates are decimals internally; use the factories for quoted units.
core.Rate.fromBp(250).asDecimal; // 0.025

// Panel analytics. Per-ticker results come back as Float64Array.
const perf = analytics.Performance.fromReturns(
  ['2024-01-31', '2024-02-29', '2024-03-31'],
  [[0.01, -0.02, 0.03]],
  ['FUND'],
  null,
  'monthly'
);
perf.sharpe(0.0); // Float64Array [ 0.917662935482247 ]
perf.free();

// Typed instruments round-trip through the canonical `finstack_quant.instrument/1`
// envelope.
const bond = valuations.instruments.Bond.fixed(
  'BOND-1',
  new core.Money(1_000_000, usd),
  new core.Rate(0.05),
  '2024-01-01',
  '2034-01-01',
  'none',
  'USD-OIS'
);
JSON.parse(bond.toJson()).schema; // 'finstack_quant.instrument/1'
```

### Pricing against a market

`calibration.calibrate` turns a quote envelope into a materialized market; the result's
`result.final_market` is the MarketContext every pricing entry point accepts. When
pricing many instruments, parse it once into a `Market` handle.

```javascript
const calibrated = calibration.calibrate(envelope); // CalibrationResultEnvelope
const market = new valuations.Market(JSON.stringify(calibrated.result.final_market));

for (const instrumentJson of instruments) {
  const result = valuations.instruments.priceInstrumentWithMarket(
    instrumentJson,
    market,
    '2025-06-15',
    'default'
  );
  console.log(result.instrument_id, result.value);
}
```

Always inspect `calibrated.result.step_reports` and `calibrated.result.report` before
using a calibrated market downstream. `calibration.validateCalibrationJson` is the
fast pre-flight that canonicalizes an envelope without solving.

### Initialization: web vs Node

`index.js` re-exports the **web** target from `pkg/`. In a browser or bundler,
`await init()` resolves the `.wasm` itself. Node has no fetchable URL, so read the
bytes and hand them to the initializer:

```javascript
import { readFileSync } from 'node:fs';
import init, { core } from 'finstack-quant-wasm';

await init({
  module_or_path: readFileSync('node_modules/finstack-quant-wasm/pkg/finstack_quant_wasm_bg.wasm'),
});
```

The `pkg-node/` build (`wasm-pack --target nodejs`) is also published and
self-initializes on import, so it needs no `init` call. The package `exports` map
declares no `./pkg-node` subpath, so reach it by file path
(`node_modules/finstack-quant-wasm/pkg-node/finstack_quant_wasm.js`) rather than by
package specifier, and note that it exposes the flat bindgen names, not the
namespaces.

## Conventions

Read [`../INVARIANTS.md`](../INVARIANTS.md) for the workspace rules. The ones that
bite JavaScript callers specifically:

**Return shapes.** The intended contract is that computation entry points return
structured JavaScript values, `*Json`-suffixed exports return JSON strings,
`*Text`-suffixed exports return prose, and numeric vectors cross as `Float64Array`.
The declaration surface pins each return shape explicitly. Wire entry points use
the `*Json` suffix, including `attributePnlEnvelopeJson`, `transformPanelJson`, and
`instrumentCashflowsWithMarketJson`; read `index.d.ts` for the authoritative type.

Map-shaped results are plain objects, never ES2015 `Map`s — bindings must serialize
through `crate::utils::to_js_value`, never `serde_wasm_bindgen::to_value`, whose
`Map` output `JSON.stringify` silently drops. The facade itself never calls
`JSON.parse`. `mise run wasm-lint` enforces the serializer rule; the allowlisted
shapes are pinned by `tests/return_shapes.rs`, which no `mise` task selects — run it
explicitly with `cargo nextest run -p finstack-quant-wasm --test return_shapes`.

For LSMC valuations, `standard_error` measures pricing-path sampling uncertainty
under the frozen fitted exercise policy and excludes regression approximation,
time-grid discretization, and model error. Monte Carlo valuation details expose
their full-width deterministic `seed` as a JavaScript `bigint`. Preserve it when producing application JSON with a replacer,
for example
`JSON.stringify(result, (_, value) => typeof value === "bigint" ? value.toString() : value)`.
The resulting decimal string is lossless; ordinary `JSON.stringify(result)` throws
when Monte Carlo details are present.

**Errors.** Bindings that route through `crate::utils::to_js_err` — the large
majority — throw a real `Error` whose `name` is `FinstackError` and whose `kind` is
`not_found`, `validation`, or `computation`. The persisted-contract entry points
(`portfolio.Portfolio.fromMaterialization` and `validateMaterialization`) instead
throw `ContractValidationError`: `kind` is `report` when structured diagnostics are
available, and `error.report` then carries the serialized `ValidationReport`; a
breached load limit gives `kind` `limit_exceeded`. Their remaining failures stay
`FinstackError` with a domain `kind` (`unknown_entity`, `fx_conversion`,
`valuation`, `missing_market_data`, and so on). A handful of low-level argument
guards still reject with a bare string, so match on `err.message` defensively
rather than assuming `Error` everywhere.

**Money.** `new core.Money(amount, currency)` takes a finite JavaScript `number`,
converts it to a Rust `Decimal`, and does **not** round to the currency's minor
units. Precision already lost in the `number` cannot be recovered. `toString()` and
other formatting do not mutate the stored amount; `amountDecimal()` renders the exact
stored `Decimal` as a string. `add` and `sub` refuse to mix currencies.

**Rates.** Rates are decimals: `0.05` is 5%. Use `Rate.fromPercent` / `Rate.fromBp`
for quoted units. `Rate.fromBp` is integer-backed and rejects fractional basis points
rather than rounding them.

**Dates.** Two conventions coexist, both matching the Rust API. The `core` date
functions (`createDate`, `dateFromEpochDays`, `adjust`) speak signed epoch-day
integers. Instrument, market, and pricing entry points take ISO-8601 date strings
(`'2025-06-15'`). Calendar codes come from `core.availableCalendars()`.

**Integer widths.** `u64`/`i64` arguments and returns cross as `BigInt`
(Monte Carlo seeds, `DayCount.calendarDays`). `usize` counts marshal as IEEE-754
doubles; where a count can plausibly get large, the binding calls
`utils::check_js_safe_count` and throws above `Number.MAX_SAFE_INTEGER` rather than
returning a silently rounded value.

**Determinism.** Simulation entry points take an explicit seed; the same seed and
path count reproduce the same estimate.

**Serde strictness.** JSON envelopes deny unknown fields and are versioned
(`finstack_quant.instrument/1`, `finstack_quant.calibration/1`, …). See
[`../docs/CONTRACTS.md`](../docs/CONTRACTS.md) and
[`../docs/SERDE_STABILITY.md`](../docs/SERDE_STABILITY.md).

## Type declarations

`wasm-bindgen` emits declarations under `pkg/`, but they describe a flat module,
not the namespaced facade. The published root contract is therefore the
hand-maintained [`index.d.ts`](index.d.ts), policed by `tests/dts_contract.rs` and
`tsc` compile checks under two `lib` targets — both wired into `mise` tasks — plus
the manually-run `tests/return_shapes.rs`.

`types/generated/*` holds only the Rust-owned JSON envelope shapes, exported from the
Rust types with `ts-rs`. Regenerate with `mise run wasm-gen-bindings`; verify without
mutating the tree with `mise run wasm-check-bindings` (also run by `mise run gen-check`).

## WASM Object Disposal

Most functions return plain JavaScript values and need no manual cleanup.
Every class generated by `wasm-bindgen` owns WebAssembly heap memory and
exposes `free()`. Call it when a handle is no longer needed; do not use the
object afterward. When the JavaScript runtime defines `Symbol.dispose`,
`wasm-bindgen` also installs `[Symbol.dispose]` as the same function, so
explicit-resource-management syntax is supported. The base declarations keep
`free()` strongly typed through `WasmOwned` but intentionally omit the
conditional computed member so ES2020 consumers do not need TypeScript's
`esnext.disposable` library. Consumers that enable that library may locally
augment `WasmOwned` with `Disposable`.

## Building

From the repository root:

```bash
mise run wasm-build   # web target only -> pkg/
mise run wasm-pkg     # web + node targets, release -> pkg/ and pkg-node/
mise run wasm-clean   # remove wasm-pack output and wasm32 target artifacts
```

Package-local equivalents:

```bash
npm run build         # wasm-pack build --target web    --out-dir pkg
npm run build:node    # wasm-pack build --target nodejs  --out-dir pkg-node
npm run bench         # Node micro-benchmarks (requires build:node)
```

`wasm-pack`, the `wasm32-unknown-unknown` target, and Node are pinned in
[`../mise.toml`](../mise.toml); `mise install` provisions them.

## Verification

```bash
mise run wasm-test    # wasm-bindgen tests, then web + node builds, then facade tests
mise run wasm-lint    # prettier --check, eslint, and the to_js_value serializer check
mise run wasm-doc     # JSDoc/TypeScript documentation gates + tsc declaration checks
mise run rust-test    # workspace nextest, including this crate's dts_contract suite
```

`mise run all-lint` includes `wasm-lint`, `wasm-doc`, and `gen-check`. The four test
layers, what each one catches, and how to run them individually are documented in
[`tests/README.md`](tests/README.md).

Do not run `cargo test` directly in this workspace; use `mise run rust-test` or
`cargo nextest`.

## Adding or changing a binding

Follow [`../.agents/rules/wasm/code-standards.md`](../.agents/rules/wasm/code-standards.md).
The rules that the gates actually enforce:

- Wrapper types are **named** structs with a `pub(crate) inner` field
  (`pub struct JsBond { pub(crate) inner: Bond }`), never tuple structs — tuple
  structs block safe extraction from `JsValue`.
- `src/api/mod.rs` declares `pub mod` per domain and adds no glob re-exports;
  `src/lib.rs` does not `pub use api::*`, which is what keeps the `core` module from
  shadowing `std::core`.
- Serialize with `crate::utils::to_js_value`; map errors with
  `crate::utils::to_js_err` / `to_js_error`. No `unwrap`, `expect`, or `panic`
  (denied at the crate root).
- Keep validation logic in a private `*_inner` helper returning the domain error and
  make the `#[wasm_bindgen]` function a thin converter, so native tests can assert on
  the error while JS still receives a structured one.
- Every JS-facing callable documents each caller-supplied input with a substantive
  `@param` in the Rust doc comment, placed **before** the `#[wasm_bindgen]`
  attribute (`mise run wasm-doc`).
- Add the new name to the matching `exports/*.js` namespace and to `index.d.ts`,
  then extend the relevant suite under `tests/`.

Cross-language parity is a separate contract; see
[`../finstack-quant-py/README.md`](../finstack-quant-py/README.md) and
[`../.agents/rules/wasm/javascript-usage-standards.md`](../.agents/rules/wasm/javascript-usage-standards.md).

## License

MIT OR Apache-2.0
