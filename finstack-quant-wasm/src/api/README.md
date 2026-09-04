# WASM API tree

The `wasm-bindgen` layer, ~17k lines across 13 modules, one per crate domain. Every
item here converts arguments, calls a workspace crate, and converts the result back.
Nothing computes. It is the mirror of
[`../../../finstack-quant-py/src/bindings/`](../../../finstack-quant-py/src/bindings/README.md),
and several files say so in their own module docs so the Rust → PyO3 → wasm-bindgen
triplet stays file-aligned.

Package-level context — namespaces, quick start, return-shape contract, error
`kind` values, Money/Rate/date conventions — is in [`../../README.md`](../../README.md).
This file covers what that one does not: where each binding physically lives, how a
Rust module reaches the published JS namespace, and the structural rules the build
enforces.

## Layout

| Path                               | Feeds                                  | Contents                                                                                                                                                                                                                                   |
| ---------------------------------- | -------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `mod.rs`                           | —                                      | `pub mod` for each of the 13 domains. **No glob re-exports**                                                                                                                                                                               |
| `core/currency.rs`                 | `core`                                 | `Currency`                                                                                                                                                                                                                                 |
| `core/money.rs`                    | `core`                                 | `Money`                                                                                                                                                                                                                                    |
| `core/types.rs`                    | `core`                                 | `Rate`, `Bps`, `Percentage`                                                                                                                                                                                                                |
| `core/dates.rs`                    | `core`                                 | `DayCount`, `DayCountContext`, `Tenor`, `createDate`, `dateFromEpochDays`, `adjust`, `availableCalendars`                                                                                                                                  |
| `core/market_data.rs`              | `core`                                 | `DiscountCurve`, `HazardCurve`, `ForwardCurve`, `VolCube`, `FxDeltaVolSurface`, `FxMatrix`, `FxConversionPolicy`, `FxRateResult`, `FxQuoteConvention`, `FxPairConvention`, `fxMarketPair`, `fxPairConvention`, `fxPipSize`, `invertFxRate` |
| `core/math.rs`                     | `core`                                 | Cholesky, statistics, special functions, compensated summation, `longestPositiveRun`                                                                                                                                                       |
| `models/liability_management.rs`   | `models.credit`                        | `analyzeExchangeOffer`, `analyzeLme` (liability management)                                                                                                                                                                                |
| `analytics/performance.rs`         | `analytics`                            | `JsPerformance` → JS `Performance`; the only class in the analytics namespace                                                                                                                                                              |
| `analytics/regression.rs`          | `analytics`                            | `constrainedLeastSquares`                                                                                                                                                                                                                  |
| `analytics/support.rs`             | —                                      | `pub(super)` argument parsing (`parse_f64_vec`, `parse_f64_matrix`) with a `Float64Array` fast path                                                                                                                                        |
| `attribution/mod.rs`               | `attribution`                          | `attributePnl`, `attributePnlJson`, waterfall/metric defaults, schema validation                                                                                                                                                           |
| `cashflows/mod.rs`                 | `cashflows`                            | Schedule build/validate, accrual, dated flows, CPR↔SMM / CDR↔MDR                                                                                                                                                                           |
| `covenants/mod.rs`                 | `covenants`                            | Spec/report/engine validation, `evaluateEngine`, preset packages                                                                                                                                                                           |
| `models/factor/mod.rs`             | `models.factor.credit`                 | `CreditFactorModel`, `CreditCalibrator`, `decomposeLevels`, `decomposePeriod`, `FactorCovarianceForecast`                                                                                                                                  |
| `features/mod.rs`                  | `features`                             | Signal cleaning, neutralization, weighting, timeseries / cross-sectional / panel transforms                                                                                                                                                |
| `margin/mod.rs`                    | `margin`                               | CSA presets and validation, `calculateVm`, `computeBilateralXva`                                                                                                                                                                           |
| `models/monte_carlo.rs`            | `models.monteCarlo`                    | European / Asian / American / Heston pricers plus Black-Scholes helpers                                                                                                                                                                    |
| `models/rates/dtsm.rs`             | `models.rates.dtsm`                    | `nelsonSiegelYields`                                                                                                                                                                                                                       |
| `models/liquidity.rs`              | `models.liquidity`                     | Roll and Amihud estimators, days-to-liquidate tiering, Bangia LVaR, Almgren-Chriss impact, Kyle lambda                                                                                                                                     |
| `portfolio/mod.rs`                 | `portfolio`                            | `Portfolio`, spec parsing, valuation, attribution (Brinson, Campisi, grid, factor-Brinson), TWRR/MWR, optimization, replay, VaR/ES decomposition, and risk budgets                                                                         |
| `portfolio/materialization.rs`     | `portfolio`                            | Strict materialization: `InstrumentArtifactCache`, plus the `Portfolio.fromMaterialization` / `Portfolio.validateMaterialization` half of the `Portfolio` class                                                                            |
| `portfolio/sensitivity.rs`         | `portfolio`                            | `computeFactorSensitivities(WithMarket)`, `computePnlProfiles(WithMarket)`, `decomposeFactorRisk`                                                                                                                                          |
| `scenarios/mod.rs`                 | `scenarios`                            | Spec parse/compose/validate, builtin templates, `applyScenario`, `computeHorizonReturn`                                                                                                                                                    |
| `statements/mod.rs`                | `statements`                           | Model and check-suite validation, `evaluateModel`, `runMonteCarlo`, formula parsing                                                                                                                                                        |
| `statements_analytics/mod.rs`      | `statements_analytics`                 | Sensitivity, variance, scenario sets, backtesting, goal seek, DCF, LBO, WACC, check reports                                                                                                                                                |
| `statements_analytics/comps.rs`    | `statements_analytics`                 | Comparable-company analysis; re-exported through `mod.rs`                                                                                                                                                                                  |
| `valuations/pricing.rs`            | `valuations.instruments`, `valuations` | `validateInstrumentJson`, `priceInstrument(WithMarket)`, `instrumentCashflows*`, `listModels*`, `listStandardMetrics*`, `bondFromCashflowsJson`, and `validateValuationResultJson` (root)                                                  |
| `valuations/fixed_income.rs`       | `valuations.instruments`               | Typed `Bond`, `TermLoan`                                                                                                                                                                                                                   |
| `valuations/structured_credit.rs`  | `valuations.instruments`               | `structuredCreditTranche{DiscountMargin,Oas,BreakevenCdr,ScenarioTable,Metrics}`                                                                                                                                                           |
| `valuations/composite.rs`          | `valuations.composite`                 | Composite initialization, rebalancing, primitive exposures, execution trades, and history                                                                                                                                                  |
| `valuations/fx.rs`                 | `valuations.fx`                        | `FxSpot`, `FxForward`, `FxSwap`, `Ndf`, `FxOption`, `FxDigitalOption`, `FxTouchOption`, `FxBarrierOption`, `FxVarianceSwap`, `QuantoOption`                                                                                                |
| `models/credit.rs`                 | `models.credit`                        | Structural-credit factories: Merton, CreditGrades, dynamic recovery, endogenous hazard, toggle exercise                                                                                                                                    |
| `valuations/credit_derivatives.rs` | `valuations.creditDerivatives`         | CDS-family example payload factories                                                                                                                                                                                                       |
| `models/correlation/mod.rs`        | `models.correlation`                   | Copulas, recovery models, joint probabilities, `nearestCorrelation`, tranche loss statistics                                                                                                                                               |
| `models/analytic.rs`               | `models`                               | Closed forms: `bsPrice`, `bsGreeks`, `bsImpliedVol`, `black76ImpliedVol`, `barrierCall`, `asianOptionPrice`, `lookbackOptionPrice`, `quantoOptionPrice`, `vanillaExpiryPayoff`                                                             |
| `models/fourier.rs`                | `models`                               | COS-method pricers: `bsCosPrice`, `vgCosPrice`, `mertonJumpCosPrice`                                                                                                                                                                       |
| `valuations/exotic_rates.rs`       | `valuations`                           | Deterministic coupon helpers: TARN, snowball, inverse floater, CMS spread, range accrual                                                                                                                                                   |
| `models/volatility.rs`             | `models.volatility`                    | `SabrParameters`, `SabrModel`, `SabrSmile`, `SabrCalibrator`, surface/cube/FX volatility evaluation                                                                                                                                        |
| `valuations/calibration.rs`        | `valuations`                           | `calibrate`, `validateCalibrationJson`, `dryRun`                                                                                                                                                                                           |
| `valuations/market_handle.rs`      | `valuations`                           | `Market` — parse a `MarketContext` once, reuse across `*WithMarket` calls                                                                                                                                                                  |

The table lists binding-bearing files plus the root wiring and analytics
conversion helper. The remaining `mod.rs` files only declare submodules or
re-export their children.

Shared conversion helpers are one level up in [`../utils/`](../utils): `to_js_value`,
`to_js_err`, `to_js_error`, `structured_js_error`, `contract_to_js_error`,
`materialization_to_js_error`, `check_js_safe_count`, `MAX_SAFE_JS_INTEGER`, and the
date helpers `parse_iso_date`, `parse_iso_dates`, `date_to_iso`.

## Reaching JavaScript

`wasm-bindgen` produces a single flat module under `pkg/`, which is an internal
build artifact. A binding becomes public only when it is added to a facade file:

```
src/api/<domain>/*.rs   →   exports/<domain>.js   →   index.js   →   index.d.ts
```

The `Feeds` column above names the JS namespace, which is not always the filename.
`valuations` splits into `exports/valuations.js` plus five nested files under
`exports/valuations/` — `instruments.js`, `fx.js`, `credit.js`,
`creditDerivatives.js`, `correlation.js` — and `models.factor` exposes credit
engines under a nested `credit` key. The mapping is many-to-many in both directions:
`pricing.rs`, `fixed_income.rs`, and `structured_credit.rs` all feed
`instruments.js`, while `pricing.rs` alone also puts `validateValuationResultJson`
on the `valuations` root.

The facade is mostly a re-export map, but not purely: `exports/valuations.js`
`JSON.stringify`s object arguments for the calibration entry points, and
`exports/portfolio.js` rebinds `Portfolio.fromMaterialization` /
`Portfolio.validateMaterialization` to inject an ephemeral
`InstrumentArtifactCache` when the caller omits one. Check the facade before
assuming a JS signature equals the Rust one.

`index.d.ts` is hand-maintained and is the authoritative published contract. A new
export that is not in it is invisible to TypeScript users, and
`tests/dts_contract.rs` fails.

## Structural rules

- **No glob re-exports in `mod.rs`, and `../lib.rs` does not `pub use api::*`.**
  A glob would pull `api::core` into the crate root and shadow `std::core`. This is
  why no `core_ns` rename is needed anywhere. The only `pub use` lines in this tree
  are the three narrow ones in `analytics/mod.rs` and `statements_analytics/mod.rs`.
  (The "Module Initialization" snippet in
  [`../../../.agents/rules/wasm/code-standards.md`](../../../.agents/rules/wasm/code-standards.md)
  still shows `pub use api::*` — the real `lib.rs` does not, and must not.)

- **Wrapper types are named structs with an `inner` field**, `pub(crate)` when a
  sibling module borrows it:

  ```rust
  #[wasm_bindgen(js_name = Market)]
  pub struct JsMarket {
      inner: Arc<MarketContext>,
  }
  ```

  Tuple structs (`pub struct JsBond(Bond)`) are forbidden: they block safe
  extraction from `JsValue` and produce `JsCast` trait-bound errors. Prefix the Rust
  type `Js*`; the JS-visible name comes from `js_name` / `js_class` and must equal
  the canonical Rust name.

- **Serialize only through `crate::utils::to_js_value`.** Raw
  `serde_wasm_bindgen::to_value` serializes Rust maps as ES2015 `Map`s, which
  `JSON.stringify` silently drops, and the shape then disagrees with `index.d.ts`
  and with the dicts the Python bindings return. `to_js_value` uses the
  `json_compatible` serializer so maps become plain objects. `mise run wasm-lint`
  greps for offenders and fails the build; the only legal call site is inside
  `../utils/mod.rs`.

- **Errors go through `to_js_err` / `to_js_error`**, which throw a real `Error` named
  `FinstackError` with a `kind` of `not_found` / `validation` / `computation`.
  Persisted-contract paths use `contract_to_js_error` and
  `materialization_to_js_error`, which select `kind` from the Rust enum variant
  rather than by sniffing the message, and attach `error.report` for structured
  diagnostics.

- **Keep validation in a private `*_inner` helper** that returns the domain error,
  and make the `#[wasm_bindgen]` function a thin converter. Native tests cannot
  inspect a `JsValue` — `js_sys::Error` only works under `wasm32` — so this split is
  what lets `cargo nextest` assert on error content while JS still receives a
  structured object. This is stated in `mod.rs`'s own module docs.

- **`unwrap`, `expect`, and `panic` are denied at the crate root** (`../lib.rs`)
  outside `#[cfg(test)]`, alongside `#![forbid(unsafe_code)]`.

- **Integer widths.** `u64`/`i64` cross as `BigInt`. `usize` marshals as an f64, so
  any count that can plausibly grow must be guarded with
  `utils::check_js_safe_count` rather than silently rounding past
  `Number.MAX_SAFE_INTEGER`. `attribution/mod.rs` documents why it is exempt.
  For LSMC valuations, `standard_error` measures pricing-path sampling uncertainty
  under the frozen fitted exercise policy and excludes regression approximation,
  time-grid discretization, and model error. Structured valuation results follow
  this rule for Monte Carlo `seed`; use a
  BigInt-aware replacer such as
  `JSON.stringify(result, (_, value) => typeof value === "bigint" ? value.toString() : value)`
  when a portable JSON document is needed. The decimal string retains the full
  seed even though ordinary `JSON.stringify(result)` rejects `BigInt`.

- **Doc comments before the attribute.** Every JS-facing callable documents each
  caller-supplied input with a substantive `@param` in its `///` block, placed
  _above_ `#[wasm_bindgen]`. `scripts/check_wasm_api_input_docs.py` (via
  `mise run wasm-doc`) enforces this.

## Adding a binding

1. Add the item to the right `src/api/<domain>/` file; create a new file and a
   `pub mod` line if the domain warrants a split (only `core`, `analytics`,
   `portfolio`, `statements_analytics`, and `valuations` are split today).
2. Give it a `js_name` matching the canonical Rust name, `///` docs with `@param`
   per input, `to_js_value` for structured returns, and `to_js_err` for failures.
3. Add it to the matching `../../exports/*.js` namespace.
4. Declare it in `../../index.d.ts`.
5. Extend the matching `../../tests/wasm_*.rs` suite — the names track subject
   area, not domain (`wasm_math.rs`, `wasm_core_market_data.rs`,
   `wasm_credit_factor_hierarchy.rs`), so pick by content — and, where the JS
   shape matters, the Node facade tests under `../../tests/facade/`.
6. Record the Rust↔Python↔JS triplet in
   [`../../../finstack-quant-py/parity_contract.toml`](../../../finstack-quant-py/parity_contract.toml)
   — the WASM namespace listings live there, and
   `finstack-quant-py/tests/parity/test_contract_topology.py` checks the facade
   files against it.

## Tests

`../../tests/` holds four layers, described in
[`../../tests/README.md`](../../tests/README.md): `wasm_*.rs` (wasm-bindgen tests
per domain), `dts_contract.rs` and `return_shapes.rs` (native, contract pins),
`typescript/` (`tsc` compile checks under two `lib` targets), and `facade/`
(Node test runner against the built package).

```bash
mise run rust-lint     # clippy --workspace covers this crate
mise run wasm-lint     # prettier + eslint + the to_js_value serializer check
mise run wasm-doc      # @param completeness on every JS-facing callable
mise run wasm-test     # wasm-pack test --node, then build web+node, then the facade tests
cargo nextest run -p finstack-quant-wasm --lib --test dts_contract
cargo nextest run -p finstack-quant-wasm --test return_shapes   # no mise task selects this
```

`mise run rust-test` runs only the `--lib` and `dts_contract` targets for this
crate; the `wasm_*.rs` suites need a wasm runtime and come in through
`mise run wasm-test`. Never run `cargo test` directly in this workspace.

## Related

- [`../../README.md`](../../README.md) — package overview, namespaces, JS conventions
- [`../../tests/README.md`](../../tests/README.md) — the four test layers
- [`../../../finstack-quant-py/src/bindings/README.md`](../../../finstack-quant-py/src/bindings/README.md)
  — the PyO3 mirror of this tree
- [`../../../.agents/rules/wasm/code-standards.md`](../../../.agents/rules/wasm/code-standards.md)
  and [`../../../.agents/rules/wasm/javascript-usage-standards.md`](../../../.agents/rules/wasm/javascript-usage-standards.md)
