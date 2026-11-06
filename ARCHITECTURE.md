# Architecture

Finstack Quant is a deterministic financial computation engine: a Rust
workspace with Python (PyO3) and WebAssembly (wasm-bindgen) bindings. All
financial logic lives in Rust. The bindings convert types, build wrappers and
map errors; the WASM package adds a namespaced JavaScript facade.

The numerical, determinism, sign, date and error rules every crate follows are
in [`INVARIANTS.md`](INVARIANTS.md). This document says where code lives,
which way dependencies point, and the invariants that cut across crates.

## Crate map

Paths are under `finstack-quant/` unless absolute. Packages are named
`finstack-quant-<name>`; the umbrella crate `finstack-quant` re-exports every
domain crate as a module (`finstack_quant::core`, `finstack_quant::valuations`,
…) and mounts `factor` at `finstack_quant::models::factor`.

| Crate | Owns | Depends on (normal dependencies) |
| --- | --- | --- |
| [`core`](finstack-quant/core/README.md) | `Money`, `Currency`, `Rate`; FX (`FxProvider`, `FxMatrix`); dates, calendars, day counts, schedules; market data (curves, surfaces, `MarketContext`); math (solvers, interpolation, statistics, linear algebra, RNG); expression engine; config and rounding; errors; `table` columnar envelope; persisted-contract and schema primitives | — |
| [`analytics`](finstack-quant/analytics/README.md) | Performance and risk statistics (`Performance`, `beta`, regression) | core |
| [`cashflows`](finstack-quant/cashflows/README.md) | Schedule construction, accrual, currency-safe dated flows | core |
| [`covenants`](finstack-quant/covenants/README.md) | Covenant definitions, evaluation, breach forecasting | core |
| [`features`](finstack-quant/features/README.md) | Vectorized panel feature transforms | core |
| [`margin`](finstack-quant/margin/README.md) | CSA specs, VM/IM (SIMM, schedule, CCP), FRTB-SBA, SA-CCR, XVA. Exposure generation is out of scope; the `Marginable` trait is the seam `valuations` implements | core |
| [`arrow-interchange`](finstack-quant/arrow-interchange/README.md) (package `finstack-quant-arrow`) | `TableEnvelope` → Arrow `RecordBatch`; not re-exported by the umbrella | core |
| [`models`](finstack-quant/models/README.md) | Model kernels: closed form, Monte Carlo (`models::monte_carlo`), PDE, trees, Fourier, volatility, short rate, credit, copulas and correlation, liquidity | core, cashflows |
| [`factor`](finstack-quant/factor/README.md) | Multi-asset factor models: definitions, matching, covariance, risk decomposition, credit-factor calibration | core, analytics |
| [`valuations/macros`](finstack-quant/valuations/macros/README.md) | `FinancialBuilder` derive | — |
| [`valuations/core`](finstack-quant/valuations/core/README.md) | Pricing framework: `Instrument` trait, `Pricer` / `PricerRegistry`, `Family`, `MetricId` and metric registry, `ValuationResult`, market conventions. No instrument types | core, cashflows, covenants, margin, models, valuations/macros |
| [`valuations/rates`](finstack-quant/valuations/rates/README.md) | Rates and inflation instruments | valuations/core, core, cashflows, margin, models, valuations/macros |
| [`valuations/fixed-income`](finstack-quant/valuations/fixed-income/README.md) | Bonds, loans, agency MBS/TBA, dollar rolls, FI TRS, bond futures | valuations/core, core, cashflows, covenants, margin, models, valuations/macros |
| [`valuations/credit`](finstack-quant/valuations/credit/README.md) | CDS, CDS indices, tranches, CDS options | valuations/core, core, cashflows, margin, models, valuations/macros |
| [`valuations/equity`](finstack-quant/valuations/equity/README.md) | Equity spot, options, futures, TRS, variance, DCF, PE funds | valuations/core, core, cashflows, margin, models, valuations/macros |
| [`valuations/fx`](finstack-quant/valuations/fx/README.md) | FX spot, forwards, swaps, NDFs, futures, vanilla and exotic options | valuations/core, core, cashflows, models, valuations/macros |
| [`valuations/exotics`](finstack-quant/valuations/exotics/README.md) | Asian, barrier, lookback, range accrual, TARN, snowball, commodities | valuations/core, core, cashflows, models, valuations/macros |
| [`valuations/structured-credit`](finstack-quant/valuations/structured-credit/README.md) | Structured credit deals, agency CMOs, asset-backed facilities | valuations/core, valuations/fixed-income, valuations/rates, core, cashflows, models, valuations/macros |
| [`valuations`](finstack-quant/valuations/README.md) | Umbrella over the framework and the seven family crates under one path tree; instruments that hold other families' legs (basket, composite, levered real estate); the `STANDARD` registry; the JSON loader (the closed world of instrument types); schemas | valuations/core, the seven family crates, core, cashflows, covenants, margin, models, valuations/macros |
| [`calibration`](finstack-quant/calibration/README.md) | Curve, hazard, inflation and vol bootstraps; SABR fits; base correlation; global calibration | valuations/core, valuations/rates, valuations/credit, core, cashflows, models |
| [`attribution`](finstack-quant/attribution/README.md) | Multi-period P&L attribution (waterfall, Taylor, metrics-based) | calibration, valuations, factor, models, core, cashflows |
| [`statements`](finstack-quant/statements/README.md) | Statement model graph and period evaluation (Value > Forecast > Formula) | valuations, core, cashflows |
| [`statements-analytics`](finstack-quant/statements-analytics/README.md) | DCF, scenario sets, sensitivity, ECL, backtesting, covenant forecasting | statements, valuations, covenants, models, core |
| [`scenarios`](finstack-quant/scenarios/README.md) | Deterministic shock and roll DSL and engine | attribution, calibration, statements, valuations, models, core, cashflows |
| [`portfolio`](finstack-quant/portfolio/README.md) | Positions and books, base-currency rollups | attribution, calibration, factor, margin, scenarios, valuations, core, cashflows |
| [`valuations/test-support`](finstack-quant/valuations/test-support/README.md) | Dev-only market builders, fixtures, tolerances | core, test-utils |
| [`test-utils`](finstack-quant/test-utils/README.md) | Golden-test framework (dev-dependency only) | — |

Bindings: `finstack-quant-py` depends on every domain crate plus
`arrow-interchange`; `finstack-quant-wasm` on every domain crate, the
umbrella and its proc-macro crate `finstack-quant-wasm/macros` (package
`finstack-quant-wasm-macros`). No Rust crate depends on a binding.

## Dependency direction

```text
core → analytics, arrow-interchange, cashflows, covenants, features, margin
core + cashflows → models
core + analytics → factor
core + cashflows + covenants + margin + models + macros → valuations/core
valuations/core (+ core, cashflows, models, macros) → rates, fixed-income, credit, equity, fx, exotics
fixed-income + rates → structured-credit
valuations/core + the seven family crates → valuations
valuations/{core, rates, credit} → calibration
calibration + valuations + factor + models → attribution
valuations → statements → statements-analytics
attribution + calibration + statements + valuations + models → scenarios
attribution + calibration + factor + margin + scenarios + valuations → portfolio
```

Rules the manifests keep:

- The six family crates depend on `valuations/core` and never on each other;
  only `structured-credit` builds on two of them.
- `models` never depends on `factor`. Every valuations crate depends on
  `models`, so a factor edit would otherwise rebuild the pricing stack. Factor's
  consumers (attribution, portfolio, the bindings) depend on it directly.
- No valuations crate depends on `calibration`; `valuations` uses it only as a
  dev-dependency for its tests.
- `margin` depends only on `core`.
- Platform differences are gated on the target, not on features: Rayon is
  compiled out for `wasm32` with `cfg(not(target_arch = "wasm32"))`. The only
  features are `json-schema` and `jsonschema-validate` (see
  [`.agents/rules/rust/code-standards.md`](.agents/rules/rust/code-standards.md)).

## Cross-cutting invariants

- **Determinism**: Decimal mode; stable ordering; parallel ≡ serial.
- **Currency safety**: arithmetic on `Money` requires one currency; FX
  conversion is always explicit.
- **Rounding and scale**: one global policy; the active `RoundingContext` is
  stamped into result metadata.
- **FX policy visibility**: the applied conversion strategy is recorded per
  layer (valuations, statements, portfolio).
- **Serde stability**: strict field names; inbound types deny unknown fields.
- **Columnar data**: `core::table` is the serializable columnar surface. There
  is no Polars dependency; `valuations::results::dataframe` emits flat JSON for
  pandas or Polars consumers.

## How a price is computed

A `MarketContext` (core) holds curves and surfaces. An instrument type in a
family crate implements `Instrument` (valuations/core). Its family's
`Family` static (`<family>/src/family.rs`) registers pricers keyed by
`InstrumentType` and `ModelKey`, and metric calculators keyed by `MetricId`.
The umbrella's `STANDARD` registry (`valuations/src/pricer/mod.rs`) joins the
seven families. Pricing returns a `ValuationResult` carrying the value, the
requested metrics and policy metadata (rounding context, FX policy).

The JSON path the bindings use, parse → validate → price, is in
`valuations/src/pricer/json.rs`; the closed list of instrument types with their
wire tags and examples is `with_instrument_json_registry!` in
`valuations/src/instruments/json_loader.rs`.

## Bindings and generated artifacts

- Python: Rust wrappers in `finstack-quant-py/src/bindings/` (one directory per
  crate); types and docstrings in the `.pyi` stubs under
  `finstack-quant-py/finstack_quant/`, compiled into the runtime `__doc__`.
- WASM: Rust exports in `finstack-quant-wasm/src/api/`; the published facade is
  `index.js` plus `exports/**/*.js`. The exports' `#[wasm_bindgen]` attributes
  and Rustdoc are the TypeScript types and JSDoc wasm-bindgen writes to
  `pkg/finstack_quant_wasm.d.ts`; the facade's `index.d.ts` is generated over it.
- Python ↔ WASM parity is tracked per namespace in
  `finstack-quant-py/parity_contract.toml`.
- JSON Schemas (`finstack-quant/*/schemas/`), the TypeScript contract types
  (`finstack-quant-wasm/types/generated/`), the UI's generated files, the WASM
  facade surface and the registry snapshot are generated from the Rust types.
  `mise run gen-write` regenerates all of them; `mise run gen-check` fails when
  one is stale.

## Where to start

- [`finstack-quant/valuations/core/src/pricer/README.md`](finstack-quant/valuations/core/src/pricer/README.md):
  instrument types, model keys, registries, the pricing pipeline.
- [`finstack-quant/valuations/core/src/instruments/README.md`](finstack-quant/valuations/core/src/instruments/README.md):
  the `Instrument` trait.
- [`finstack-quant/valuations/core/src/metrics/README.md`](finstack-quant/valuations/core/src/metrics/README.md)
  and [`results/README.md`](finstack-quant/valuations/core/src/results/README.md):
  metrics and `ValuationResult`.
- [`finstack-quant/valuations/ADDING_AN_INSTRUMENT.md`](finstack-quant/valuations/ADDING_AN_INSTRUMENT.md):
  every site a new instrument touches.
- [`CONTRIBUTING.md`](CONTRIBUTING.md): setup, the one gate (`mise run all`),
  pull requests and releases.
- [`scripts/README.md`](scripts/README.md): every checker, which convention it
  enforces, and which task runs it.
