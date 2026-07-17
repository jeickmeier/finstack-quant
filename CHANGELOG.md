# Changelog

All notable changes to this workspace are recorded here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this workspace follows a pre-1.0 cadence: breaking changes between minor
versions are possible but must be announced in this file.

See [`docs/SERDE_STABILITY.md`](docs/SERDE_STABILITY.md) for the per-wire-type
stability contract and schema-version policy.

## [Unreleased]

### Changed (breaking, wire format)

- `FinstackConfig`, `RoundingPolicy` and `CurrencyScalePolicy` now reject unknown
  fields during deserialization, and so does the `tolerances` object. Misspelled
  keys previously fell back silently; config JSON/dicts with stray keys now
  raise `ValueError` in Python and throw in JavaScript. Remove unrecognized keys
  or move extension data under a namespaced `{crate}.{domain}.v{N}` entry.

### Fixed
- **`finstack-quant-statements` — swap net interest receipts were booked as
  interest expense.** A two-leg instrument whose legs net to a receipt (a
  pay-fixed swap is in the money whenever the floating leg exceeds the fixed
  leg) had that receipt booked into `interest_expense_cash` via `net.abs()`,
  overstating interest expense by twice the receipt and, under a waterfall,
  consuming real cash to service a payment that was never made. Receipts are
  now reported separately (see the additive `interest_income_cash` field
  below). Single-leg debt is unaffected: bonds and loans emit positive coupons
  meaning "issuer pays", so their behaviour is byte-for-byte unchanged.
  Expense-vs-income is resolved from the schedule's leg structure rather than
  from a flow's sign, because the sign convention differs by instrument
  (INVARIANTS.md §3).
- **`finstack-quant-statements` — waterfall inputs are validated as finite.**
  A non-finite `available_cash_node` (e.g. a division by zero, which the DSL
  produces by design) silently read as zero cash and shorted every creditor,
  and a non-finite PIK liquidity metric silently read as "not triggered".
  Amounts beyond `Decimal`'s range reached `Money::new` and panicked. All now
  error, naming the offending expression.
- **`finstack-quant-statements` — LogNormal forecasts reject invalid bases
  consistently.** The independent path let `NaN` through and the correlated
  path silently switched regime on a negative base, emitting plausible but
  meaningless Monte Carlo paths. Both now error.
- **`finstack-quant-statements` — duplicate IDs are rejected.** Duplicate
  metric IDs inside one registry document were silently coalesced last-wins,
  and duplicate adjustment IDs arriving via JSON bypassed the builder's guard
  and double-counted an add-back into adjusted EBITDA.

### Security
- **`finstack-quant-statements` — formula parsing is bounded (stack-overflow
  DoS).** A flat operator chain (`1+1+1+…`) bypassed the parser's nesting
  limit, which only guards recursion, and produced an AST whose depth equalled
  the operator count; compiling it overflowed the stack. That aborts the
  process (SIGABRT) and cannot be caught by the Python/WASM bindings, and was
  reachable from ~20 KB of untrusted registry or model-spec JSON. Formulas are
  now capped at 256 terms; oversized ones return a parse error pointing at
  intermediate model nodes.

### Added
- **`CashflowBreakdown::interest_income_cash`** (`Option<Money>`, additive per
  [`docs/SERDE_STABILITY.md`](docs/SERDE_STABILITY.md)): net cash interest
  received in a period, non-zero only for two-leg instruments that net to a
  receipt. Exposed to the DSL as `cs.interest_income.{instrument|total}` and
  reachable from Python/WASM through the existing JSON surface — no binding
  signature changes. Payloads written before this field carry `None`, which
  `interest_income_cash_or_zero()` resolves to a zero in the breakdown's own
  currency; that is semantically identical to the previous behaviour (no
  income was tracked), so no `schema_version` bump is required. The field is
  omitted from output when absent. `CashflowBreakdown::net_interest_expense_cash()`
  returns expense net of income, which may be negative when a hedge is in the
  money.

### Changed
- **`CapFloor` compounded-RFR contract (intentional pre-1.0 source break):**
  public `spread` and `overnight_coupon` fields now encode spread compounding,
  observation conventions, and payment delay. Serde and builders default these
  fields for legacy payloads/construction, but exhaustive Rust struct literals
  must add them. Overnight accrual fractions are recomputed from adjusted
  business-day boundaries before coupon, spread, and delayed-payment valuation.
- **Hull-White cap/floor volatility risk:** `vega` now bumps market normal-vol
  quotes and recalibrates short-rate sigma. Direct model-parameter sensitivity
  remains available separately as `hw_sigma_vega`; a mean-reversion-only
  override holds kappa fixed during surface calibration.
- **Hull-White cap/floor calibration:** scalar surface calibration now matches
  contractual-schedule prices rather than averaging implied short-rate sigmas.
  A backwards-compatible piecewise `hw1f_sigma_schedule` and fixed-kappa
  expiry bootstrap are available for cap/floor calibration; scalar overrides
  and market scalars continue to work unchanged.

## [0.6.0] - 2026-06-16

### Removed
- **`finstack_quant_valuations::models::closed_form::heston::HestonParams::from_market` removed.**
  Use `HestonParams::from_market_strict` and provide all five `HESTON_*`
  unitless market scalars explicitly; missing or mistyped Heston parameters now
  fail loudly instead of falling back to representative defaults.

## [0.5.0] - 2026-05-23

### `finstack-quant-wasm` — JS facade simplification (breaking)

Several wasm-bindgen JS names were trimmed/aligned for clarity. The Rust internals
keep the same behaviour; only the JS-facing names change.

- **`core.adjustBusinessDay` → `core.adjust`** — the function lives in the
  `core` namespace under date helpers; the `BusinessDay` suffix was redundant.
- **`covenants.*Covenants` suffixes dropped** — `evaluateCovenantEngine` →
  `evaluateEngine`, `lboStandardCovenants` → `lboStandard`,
  `covLiteCovenants` → `covLite`, `realEstateCovenants` → `realEstate`,
  `projectFinanceCovenants` → `projectFinance`. The namespace already conveys
  "covenants".
- **FX instruments: `toJSON` / `fromJSON` → `toJson` / `fromJson`** —
  camelCase JSON aligns with the rest of the JS facade (`fromJson` /
  `toJson` everywhere else).
- **`valuations.WasmMarket` → `valuations.Market`** and the
  `WasmMarket.fromJson(json)` static factory was removed. Use
  `new valuations.Market(json)` instead — the constructor already accepts the
  same JSON payload.
- **`FxConversionPolicy.getName()` removed.** Use `toString()` (which already
  returned the same canonical label).
- **`core.validateCorrelationMatrix(nested)` removed.** Use
  `core.validateCorrelationMatrixFlat(flat, n)` — pass a `Float64Array` /
  `number[]` of `n * n` row-major entries plus the dimension `n`. The
  nested-array overload incurred a JS-side copy for every call.

JavaScript/TypeScript consumers must update call sites accordingly. The
`finstack-quant-wasm/index.d.ts` declarations and `finstack-quant-py/parity_contract.toml`
WASM-only export list were updated to match.

### Added — Credit factor hierarchy decomposition (opt-in, non-breaking)

A hierarchical credit factor model that decomposes every issuer's spread into a sequence of common factors (user-designated generic + configurable bucket levels) plus an issuer-specific adder residual. All consumers take `Option<&CreditFactorModel>` and fall back to today's behavior when absent.

**New types:** `CreditFactorModel`, `CreditCalibrator`, `CreditCalibrationInputs`, `CreditCalibrationConfig`, `CreditHierarchySpec`, `HierarchyDimension`, `IssuerTags`, `IssuerBetaPolicy`, `IssuerBetaMode`, `IssuerBetaRow`, `LevelsAtAnchor`, `VolState`, `FactorHistories`, `CalibrationDiagnostics`, `FactorCovarianceForecast`, `VolHorizon`, `CreditVolReport`, `CreditFactorAttribution`, `LevelPnl`, `CreditCarryDecomposition`, `LevelCarry`, `SourceLine`.

**New free functions:** `decompose_levels`, `decompose_period`.

**Extended types:**
- `PnlAttribution`: adds `credit_factor_detail`, `credit_carry_decomposition` (both `Option`, default `None`).
- `CarryDetail`: `coupon_income` and `roll_down` are now `Option<SourceLine>` (custom Deserialize accepts both legacy `Money` and new `SourceLine` shapes).
- `RiskDecomposition`: adds `position_residual_contributions` (default empty).
- `MarketMapping`: new `CreditHierarchical` matcher variant.
- `AttributionSpec`: adds `credit_factor_model`, `credit_factor_detail_options`.

**New schemas:** `factor_model/credit_factor_model.schema.json` (`finstack_quant.credit_factor_model/1`), `factor_model/credit_calibration_inputs.schema.json`, `factor_model/credit_calibration_config.schema.json`. Attribution schemas extended additively — old payloads still validate.

**`CreditFactorModelRef` semantics:** Currently only the `Inline(Box<CreditFactorModel>)` variant is implemented. A path-based variant (`FilePath(PathBuf)`) is documented as a future v2 enhancement; callers that need to avoid embedding large artifacts in the spec should pre-load and pass the inline form.

**Deferred to v2:** term-structure level factors, PCA-derived generic, multivariate / DCC GARCH, online covariance updating, Ledoit-Wolf shrinkage, FRTB regulatory adapters, `CreditFactorModelRef::FilePath` variant.

**PR-12 final hardening (this entry):**
- Jupyter notebook `05_portfolio/credit_factor_hierarchy.ipynb`: end-to-end synthetic demo covering calibration, artifact save/reload, period decomposition, portfolio attribution, and vol forecast.
- Benchmarks: `finstack-quant/valuations/benches/credit_factor_calibration.rs` (500 issuers × 60 months × 3 levels); `attribution_scale.rs` extended with a 200-position parallel-attribution-with-credit-model group.
- Compatibility sweep: `no_model_compatibility.rs` tests that (1) pre-PR-7 `AttributionEnvelope` JSON deserializes with `credit_factor_model = None`, (2) pre-PR-7 `PnlAttribution` JSON deserializes with new fields defaulting to `None`, and (3) all four attribution methods (MetricsBased, Taylor, Parallel, Waterfall) produce finite, non-NaN totals when no credit model is supplied. `factor_model_serialization.rs` in `finstack-quant-portfolio` adds a test confirming pre-PR-6 `RiskDecomposition` JSON (no `position_residual_contributions` key) deserializes with the field defaulting to empty.

**v1 shipped API surface:**
- `CreditFactorModel` / `CreditCalibrator` — artifact type and offline calibration pipeline.
- `decompose_levels` / `decompose_period` — snapshot and period-over-period factor decomposition.
- `FactorCovarianceForecast` — horizon-scaled covariance and idiosyncratic vol forecasting.
- `compute_credit_factor_attribution` — P&L attribution with credit hierarchy detail.
- `CreditCarryDecomposition` / `LevelCarry` — carry split by generic + level + adder.
- Python bindings: `finstack_quant.valuations.{CreditCalibrator, CreditFactorModel, FactorCovarianceForecast, decompose_levels, decompose_period}`.

**v2 deferred items:**
- Term-structure level factors (separate beta per tenor bucket).
- PCA-derived generic factor (data-driven, not user-supplied series).
- Multivariate GARCH / DCC GARCH for time-varying factor covariances.
- Online covariance updating (incremental calibration on streaming data).
- Ledoit-Wolf shrinkage estimator for large factor cross-correlation matrices.
- FRTB P&L attribution regulatory adapter.
- `CreditFactorModelRef::FilePath` variant for artifact-path-based dispatch.

### `finstack_quant_scenarios` — production-readiness audit follow-ups

**Breaking changes:**

- **`OperationSpec::CurveParallelBp`**, `CurveNodeBp`, and the `Hierarchy*`
  curve variants now type `curve_id` and `discount_curve_id` as
  `finstack_quant_core::types::CurveId` instead of `String`. JSON wire format is
  unchanged (`CurveId` is `#[serde(transparent)]`), but Rust callers
  constructing these literals must use `"USD-OIS".into()` rather than
  `"USD-OIS".to_string()`. Same change applies to `surface_id` in
  `BaseCorrParallelPts` / `BaseCorrBucketPts` /
  `VolSurfaceParallelPct` / `VolSurfaceBucketPct` and to
  `RateBindingSpec::curve_id`.
- **`CurveKind::VolIndex` removed.** Volatility-index curves now have
  dedicated variants — `OperationSpec::VolIndexParallelPts { curve_id, points }`
  and `OperationSpec::VolIndexNodePts { curve_id, nodes, match_mode }` — that
  use absolute index points (e.g. `+1.0` lifts every knot of a VIX curve by
  1.0 vol points). The old `bp/100` rescaling shim that overloaded
  `CurveParallelBp` for vol-index curves is gone; migrate callers to the new
  variants.
- **`ApplicationReport::warnings`** is now `Vec<finstack_quant_scenarios::Warning>`
  rather than `Vec<String>`. The structured enum lets ops alerting pipelines
  pattern-match on warning categories (`HazardRecalibrationFallback`,
  `FxTriangulationInconsistent`, `VolSurfaceArbitrage`, …) without parsing
  free text. The `Display` impl preserves the previous human-readable form;
  callers that only need the string view can `.iter().map(ToString::to_string)`.
- **Equity, instrument-price, vol-surface, and instrument-by-type shocks now
  accept any finite `pct`** (including `<= -100`) — these are legitimate
  tail-risk stresses. FX shocks retain the strict `<= -100%` rejection because
  driving a spot rate to zero would propagate NaNs through triangulation.
- **`ScenarioEngine::compose` has been removed** in favor of `try_compose`.
  The permissive variant could produce specs with multiple `TimeRollForward`
  ops that the apply phase rejects; `try_compose` catches that statically.

**Correctness fixes:**

- **Hierarchy-targeted operations now fail fast** when the market context has
  no hierarchy attached. Previously the engine silently returned
  `operations_applied = 0` and a "not supported" warning, masking the
  configuration mistake. The new behaviour is a typed
  `Error::Validation(...)`.
- **Hierarchy operations that resolve to zero curves now emit
  `Warning::HierarchyNoMatch { target_path, op_kind }`** rather than being
  silently dropped.
- **Phase 1 of `ScenarioEngine::apply` flushes pending market bumps before
  generating effects for each operation.** Previously, queued bumps from a
  prior op were not visible to the next op's adapter, so cross-curve
  calibrations (e.g. ParCDS recalibration consulting FX) could read stale
  market state. The fix preserves sequential semantics — every adapter sees
  the fully-applied prior-op market — while still batching multi-effect
  outputs from a single op.
- **`MarketContext` clones reduced.** The `bump_observed` path now applies
  multiple effects in a single batched call, and the new
  `MarketContext::iter_discount_curves` accessor lets adapters scan discount
  curves without materialising a full `MarketContextState`.
- **Rate-binding `node_id` mismatch is now a hard error** rather than a
  silent rewrite. The map key is authoritative for routing.
- **Typed errors propagate end-to-end.** Adapter functions that previously
  wrapped underlying errors as `Error::Internal(format!("…: {e}"))` now use
  `?` propagation through the new `Error::Valuations(#[from]
  finstack_quant_valuations::Error)` variant.

**Performance:**

- `MarketContext::iter_discount_curves` accessor (no more
  `MarketContextState` materialisation in `resolve_discount_curve_id`).
- Hierarchy-expansion fast path: returns `Cow::Borrowed` when no
  `Hierarchy*` variants are present, avoiding an unnecessary clone of the
  operation list.

**Observability:**

- `tracing::instrument` on `apply` with a `scenario_id` field, plus per-phase
  `info_span!` (`phase_0_time_roll`, `phase_1_market`, `phase_2_rate_bindings`,
  `phase_3_statements`, `phase_4_reevaluate`).

**Refactor — internal:**

- Replaced the `ScenarioAdapter` trait + 8 trait impls with a centralised
  `match` in `engine::generate_effects`. Adapters are now free functions; the
  exhaustive match catches new `OperationSpec` variants at compile time and
  removes the silent "Operation not supported" warning fallback.
- Folded `templates/loader.rs` into `templates/json.rs`.

**FFI:**

- `finstack-quant-py` `apply_scenario` / `apply_scenario_to_market` results gain a
  `warnings_json: str` key (JSON-encoded list of structured `Warning`
  records, mirroring the WASM binding); `warnings: list[str]` is preserved
  for backwards compatibility.
- `finstack_quant.scenarios.HorizonResult` gains a `warnings_json` property with
  the same semantics.
- `finstack-quant-wasm` already returned structured warnings; consistency between
  the two FFIs is now explicit.

### Added
- `schema_version: u32` field on the following persisted result types, with
  `#[serde(default)]` for backward-compatible deserialization of pre-versioning
  payloads:
  - `finstack_quant_valuations::results::ValuationResult`
    (`VALUATION_RESULT_SCHEMA_VERSION = 1`)
  - `finstack_quant_statements::evaluator::StatementResult`
    (`STATEMENT_RESULT_SCHEMA_VERSION = 1`)
  - `finstack_quant_portfolio::results::PortfolioResult`
    (`PORTFOLIO_RESULT_SCHEMA_VERSION = 1`)
  - `finstack_quant_portfolio::optimization::PortfolioOptimizationResult`
    (`PORTFOLIO_OPTIMIZATION_RESULT_SCHEMA_VERSION = 1`)
- `finstack_quant` binding-layer error helpers now flatten the full
  `std::error::Error::source()` chain into the Python exception message, so
  calibration / solver failures retain inner context that was previously
  dropped at the FFI boundary.
- `finstack_quant::errors::display_to_py` helper for bindings that want to pass a
  `&dyn std::error::Error` and get the full source chain preserved.

### Changed
- PyO3 entry points that hold the GIL across long-running Rust work now
  release it via `py.detach(...)` (PyO3 0.28 API):
  - `finstack_quant.valuations.calibrate`
  - `finstack_quant.monte_carlo.McEngine.price_european_call` / `price_european_put`
  - `finstack_quant.monte_carlo.price_european_call` / `price_european_put`
    (module-level)
  - `finstack_quant.scenarios.apply_scenario` / `apply_scenario_to_market`
- WASM `Money.mulScalar` now routes through `Money::checked_mul_f64` for
  consistency with `divScalar`; previously it did a manual finiteness check
  and then unchecked multiplication, which could diverge in edge cases.
- WASM `SwaptionVolCube`/`VolCube` constructors and queries now use the
  structured `to_js_err()` error constructor; previously three sites used raw
  `JsValue::from_str(&e.to_string())`, breaking `err.name === "FinstackError"`
  pattern-matching for JS callers.
- Binding-layer `.map_err(|e| PyValueError::new_err(e.to_string()))` patterns
  have been replaced with centralized `crate::errors::display_to_py` /
  `core_to_py` helpers across 13 files in `finstack-quant-py/src/bindings/`.
- `eprintln!` fallback logging in production code has been migrated to
  `tracing` (structured fields, standard subscriber routing):
  - `finstack-quant-core/src/dates/calendar/types.rs` (bitset-range fallback)
  - `finstack-quant-core/src/golden/loader.rs` (non-certified suite skip)
  - `finstack-quant-valuations/src/instruments/.../revolving_credit/.../path_generator.rs`
    (CIR Feller-condition violation)
  - `finstack-quant-valuations/src/instruments/common/models/trees/binomial_tree.rs`
    (Leisen-Reimer even-step warning)
- Python binding `finstack_quant.core.money.Money` is now `frozen`; `__iadd__` /
  `__isub__` / `__imul__` / `__itruediv__` have been removed. Python `+=` etc.
  now fall back to the non-mutating dunders (`__add__` etc.) and rebind the
  variable. This matches the Rust `Money: Copy` semantics.
- Python binding `finstack_quant.core.dates.CalendarMetadata.weekend_rule` now
  returns the stable snake_case serde name (`"saturday_sunday"`,
  `"friday_saturday"`, `"friday_only"`, `"none"`) instead of the `Debug` repr.

### Removed
- Unused wall-clock `last_access: std::time::Instant` field on the expression
  cache entry (`finstack_quant_core::expr::cache`). LRU ordering is handled by the
  `lru` crate's insertion/access order — no wall-clock read was ever consumed,
  but removing it also removes a cross-run non-determinism risk.

### Fixed
- `finstack_quant_core::credit::migration::scale::RatingScale` and
  `finstack_quant_core::market_data::arbitrage` now use the deterministic
  `rustc_hash::FxHashMap` (via the `crate::HashMap` alias) instead of the
  hash-randomized `std::collections::HashMap`. The former sites would otherwise
  produce non-reproducible serialized iteration order across runs.

### Notes
- No public Rust signature was removed or renamed in this release. All
  breaking changes are guarded by new fields with `#[serde(default)]`. Existing
  persisted JSON payloads continue to deserialize under the new types.
- Schema versions start at `1` for each type above. Bumps must follow the
  policy in `docs/SERDE_STABILITY.md` and be recorded here.

## [0.4.1]

Baseline for this changelog.
