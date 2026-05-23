# finstack-attribution

Multi-period P&L attribution for individual instruments. Decomposes mark-to-market
change between two dates (T₀ → T₁) into contributions from carry, rates curves,
credit curves, inflation, correlations, FX, volatility, model parameters, and
market scalars.

Attribution is layered by cost and fidelity. The lightest entry points reprice
once per date, the heaviest perform per-factor bump-and-reprice loops. Pick the
cheapest method that answers your question — every additional tier adds
repricing cost and operational moving parts.

## Methodologies

| Tier         | Entry point                                                | Behavior                                                                                       |
|--------------|------------------------------------------------------------|------------------------------------------------------------------------------------------------|
| Minimal      | [`simple_pnl_bridge`](src/lib.rs)                          | Scalar `value(T₁) − value(T₀)` in target currency. No decomposition.                          |
| Linear       | [`attribute_pnl_metrics_based`](src/metrics_based.rs)      | Linear (and optional second-order) approximation from precomputed metrics. No extra repricing. |
| Parallel     | [`attribute_pnl_parallel`](src/parallel.rs)                | Isolate one factor at a time (T₀ for that factor, T₁ elsewhere). Residual carries cross-effects. |
| Waterfall    | [`attribute_pnl_waterfall`](src/waterfall.rs)              | Apply factors in order; per-factor P&Ls sum to total P&L up to tolerance. Order matters.       |
| Taylor       | [`attribute_pnl_taylor`](src/taylor.rs)                    | First- and optional second-order sensitivity expansion from bump-and-reprice Greeks.           |

Default waterfall order (from [`default_waterfall_order`](src/waterfall.rs)):

```
Carry → RatesCurves → CreditCurves → InflationCurves → Correlations
      → Fx → Volatility → ModelParameters → MarketScalars
```

## Factors

`AttributionFactor` in [`types.rs`](src/types.rs) enumerates the nine factor
families. Each populates a top-level field on `PnlAttribution` (`carry`,
`rates_curves_pnl`, …) and, when requested, an optional `*_detail` struct with
finer breakdowns:

- **Carry** — theta, accrual, pull-to-par, financing.
- **RatesCurves** — per-curve and optional per-tenor IR risk
  (`RatesCurvesAttribution`).
- **CreditCurves** — per-hazard-curve spread P&L, with optional generic /
  per-level / adder decomposition via a calibrated `CreditFactorModelRef`
  (`CreditFactorAttribution`).
- **InflationCurves** — real-rate and CPI curve moves.
- **Correlations** — base correlation curve changes for structured credit.
- **Fx** — spot FX revaluation in the target reporting currency.
- **Volatility** — implied-vol surface moves (`VolAttribution`).
- **ModelParameters** — prepayment, default, recovery, conversion-policy and
  other model inputs snapshotted via [`ModelParamsSnapshot`](src/model_params.rs).
- **MarketScalars** — dividends, equity/commodity spots, inflation index fixings.

## Layout

```
attribution/
├── lib.rs                  # Module docs, simple_pnl_bridge, public re-exports
├── types.rs                # AttributionFactor, PnlAttribution, AttributionMeta, *Detail structs
├── factors.rs              # MarketSnapshot, restore flags, per-factor market mutation
├── helpers.rs              # reprice_instrument, compute_pnl, compute_pnl_with_fx
├── parallel.rs             # attribute_pnl_parallel
├── waterfall.rs            # attribute_pnl_waterfall, default_waterfall_order
├── metrics_based.rs        # attribute_pnl_metrics_based (linear from metrics)
├── taylor.rs               # attribute_pnl_taylor, TaylorAttributionConfig
├── model_params.rs         # ModelParamsSnapshot, with_model_params, measure_*_shift
├── credit_factor.rs        # compute_credit_factor_attribution, model wiring
├── credit_cascade.rs       # Waterfall credit-factor cascade
├── credit_decomposition.rs # Generic / per-level / adder decomposition
├── execution.rs            # AttributionSpec::execute dispatcher
└── spec.rs                 # JSON envelope, AttributionSpec, AttributionResult
```

## Dependencies

```toml
[dependencies]
finstack-attribution = { path = "../finstack/attribution" }
finstack-core        = { path = "../finstack/core" }
finstack-valuations  = { path = "../finstack/valuations" }
```

Import path uses underscores:

```rust
use finstack_attribution::{
    attribute_pnl_parallel, attribute_pnl_waterfall, default_waterfall_order,
    AttributionFactor, PnlAttribution,
};
```

## Quick start

### Parallel attribution

```rust,ignore
use finstack_attribution::attribute_pnl_parallel;
use finstack_core::config::FinstackConfig;

let attribution = attribute_pnl_parallel(
    &instrument,
    &market_t0,
    &market_t1,
    as_of_t0,
    as_of_t1,
    &FinstackConfig::default(),
    None, // optional ModelParamsSnapshot at T₀
)?;

println!("Total P&L:  {}", attribution.total_pnl);
println!("Carry:      {}", attribution.carry);
println!("Rates:      {}", attribution.rates_curves_pnl);
println!("Credit:     {}", attribution.credit_curves_pnl);
println!("FX:         {}", attribution.fx_pnl);
println!("Residual:   {} ({:.2}%)", attribution.residual, attribution.meta.residual_pct);

assert!(attribution.residual_within_meta_tolerance());
```

### Waterfall attribution

```rust,ignore
use finstack_attribution::{attribute_pnl_waterfall, default_waterfall_order};

let attribution = attribute_pnl_waterfall(
    &instrument,
    &market_t0,
    &market_t1,
    as_of_t0,
    as_of_t1,
    &FinstackConfig::default(),
    default_waterfall_order(),
    false, // strict_validation
    None,  // optional ModelParamsSnapshot at T₀
)?;

assert!(attribution.residual_within_tolerance(0.01, 1.0));
```

### Metrics-based attribution

Requires `ValuationResult`s priced at both dates with the metrics in
[`default_attribution_metrics`](src/spec.rs) (or your own subset).

```rust,ignore
use finstack_attribution::{attribute_pnl_metrics_based, default_attribution_metrics};
use finstack_valuations::instruments::PricingOptions;

let metrics = default_attribution_metrics();
let val_t0 = instrument.price_with_metrics(&market_t0, as_of_t0, &metrics, PricingOptions::default())?;
let val_t1 = instrument.price_with_metrics(&market_t1, as_of_t1, &metrics, PricingOptions::default())?;

let attribution = attribute_pnl_metrics_based(
    &instrument, &market_t0, &market_t1, &val_t0, &val_t1, as_of_t0, as_of_t1,
)?;
```

### Per-tenor curve detail

When parallel or waterfall runs request curve detail, the optional `rates_detail`
field exposes per-`(curve_id, tenor)` P&L:

```rust,ignore
if let Some(rates) = &attribution.rates_detail {
    for ((curve_id, tenor), pnl) in &rates.by_tenor {
        println!("{curve_id} {tenor}: {pnl}");
    }
}
```

## JSON specification

[`AttributionEnvelope`](src/spec.rs) / [`AttributionSpec`](src/spec.rs) define a
schema-versioned (`finstack.attribution/1`) JSON contract used by bindings and
batch pipelines. A spec carries an `InstrumentJson` payload, two
`MarketContextState` snapshots, both `as_of` dates, the methodology, and
optional config / credit-factor-model overrides.

```rust,ignore
use finstack_attribution::{AttributionEnvelope, AttributionSpec, ATTRIBUTION_SCHEMA_V1};

let envelope: AttributionEnvelope = serde_json::from_str(&json)?;
assert_eq!(envelope.schema, ATTRIBUTION_SCHEMA_V1);

let result_envelope = envelope.execute()?;
let result = &result_envelope.result; // AttributionResult { attribution, results_meta }
```

`AttributionSpec::from_json_inputs` is the binding-friendly constructor used by
the Python and WASM layers. Schemas live under `schemas/attribution/1/`.

## Public API

| Item                                                                              | Module           | Notes                                       |
|-----------------------------------------------------------------------------------|------------------|---------------------------------------------|
| `simple_pnl_bridge`                                                               | `lib`            | Total P&L, no decomposition                 |
| `attribute_pnl_parallel`                                                          | `parallel`       | Factor isolation, residual reports cross-effects |
| `attribute_pnl_waterfall`, `default_waterfall_order`                              | `waterfall`      | Sum-preserving ordered decomposition        |
| `attribute_pnl_metrics_based`                                                     | `metrics_based`  | Linear approximation from precomputed metrics |
| `attribute_pnl_taylor`, `TaylorAttributionConfig`                                | `taylor`         | Sensitivity-based expansion mapped to `PnlAttribution` |
| `PnlAttribution`, `AttributionFactor`, `AttributionMethod`, `AttributionMeta`     | `types`          | Result envelope and factor enums            |
| `CarryDetail`, `RatesCurvesAttribution`, `CreditCurvesAttribution`, `CreditFactorAttribution`, `InflationCurvesAttribution`, `CorrelationsAttribution`, `FxAttribution`, `VolAttribution`, `ModelParamsAttribution`, `ScalarsAttribution`, `CrossFactorDetail`, `CreditCarryDecomposition`, `CreditCarryByLevel`, `LevelCarry`, `LevelPnl`, `SourceLine` | `types` | Per-factor detail structs                   |
| `MarketSnapshot`, `MarketRestoreFlags`                                             | `factors`        | T₀/T₁ snapshot and per-factor restore primitives |
| `compute_pnl`, `compute_pnl_with_fx`                                              | `helpers`        | Money/FX arithmetic for P&L computation     |
| `ModelParamsSnapshot`, `extract_model_params`, `with_model_params`, `measure_prepayment_shift`, `measure_default_shift`, `measure_recovery_shift`, `measure_conversion_shift` | `model_params` | Model-parameter snapshotting and shift attribution |
| `compute_credit_factor_attribution`, `CreditAttributionInput`, `CreditFactorDetailOptions`, `CreditFactorModelRef`, `credit_factor_model_id` | `credit_factor` | Calibrated credit-factor decomposition of `credit_curves_pnl` |
| `AttributionEnvelope`, `AttributionSpec`, `AttributionConfig`, `AttributionResult`, `AttributionResultEnvelope`, `ATTRIBUTION_SCHEMA_V1`, `default_attribution_metrics` | `spec` | JSON contract |

## Conventions

- **Sign convention**: positive `PnlAttribution.total_pnl` is a gain to the
  long-position holder. Each factor P&L follows the same sign.
- **Currency**: every P&L term is `Money` in a single reporting currency. FX
  conversion is resolved through `market_t1` (see `compute_pnl_with_fx`).
- **Carry definition**: `carry = value(T₁ market, T₁ date) − value(T₁ market, T₀ date)`
  in parallel/waterfall runs; metrics-based carry uses theta × Δt.
- **Curve moves** are applied as full-snapshot replacements, not parametric
  shocks. "Parallel"/"per-tenor" labels refer to the *reporting* granularity,
  not the shape of the underlying market move.
- **Residual interpretation**:
  - Waterfall residuals should be ≤ ~0.01% of total P&L; persistent larger
    residuals indicate a factor not represented in the chosen order.
  - Parallel residuals capture genuine cross-effects (e.g. rates × FX) and can
    legitimately reach a few percent on large multi-factor moves.
  - Metrics-based residuals scale with the size of the market move and
    instrument convexity.
- **Validation**: `PnlAttribution::residual_within_tolerance(abs, pct)` and
  `residual_within_meta_tolerance()` compare residual against `AttributionMeta`
  thresholds populated from `FinstackConfig`.

## Numerical behavior

- All four methodologies guard against missing curves/surfaces and report
  zero contribution rather than panicking when a factor is absent from both
  market snapshots.
- Per-factor bump-and-reprice paths in `parallel`, `waterfall`, and `taylor`
  reuse a single `MarketSnapshot` and apply targeted restore flags
  (`MarketRestoreFlags`) to avoid full-context cloning.
- Taylor attribution uses central differences by default; bump sizes are
  configurable via `TaylorAttributionConfig`.
- Strict mode (`AttributionConfig::strict_validation = true`) propagates per-factor
  pricing errors; otherwise they are logged via `tracing` and the factor's P&L
  is set to zero.
- Output rounding follows `FinstackConfig::rounding`; `AttributionConfig::rounding_scale`
  overrides the per-currency scale for a single run.

## Extending

Adding a new factor requires coordinated updates to:

1. `AttributionFactor` and `PnlAttribution` in [`types.rs`](src/types.rs).
2. The factor-isolation / restore logic in [`factors.rs`](src/factors.rs).
3. All four methodology modules (`parallel`, `waterfall`, `metrics_based`,
   `taylor`).
4. `default_waterfall_order` in [`waterfall.rs`](src/waterfall.rs).
5. The JSON schema under `schemas/attribution/1/` and parity tests under
   `tests/attribution/`.

Follow an existing factor (e.g. `Fx` or `Volatility`) end-to-end as a template.

## Bindings

- **Python**: `AttributionSpec`-based JSON pipeline; result types serialize via
  serde and are exposed under `finstack.attribution`. See
  `finstack-py/parity_contract.toml`.
- **WASM**: attribution is exposed as a JSON-first surface under
  `finstack-wasm/exports/attribution.js`. It intentionally mirrors the Python
  JSON/spec entry points (`attribute_pnl`, `attribute_pnl_from_spec`,
  `validate_attribution_json`, and the default-list helpers) rather than the
  full Rust type surface. The agreed WASM facade is pinned in
  `[wasm_attribution_subset]` in `finstack-py/parity_contract.toml`.

## Related

- [`finstack-valuations`](../valuations/README.md) — instrument repricing used at T₀ and T₁.
- [`finstack-cashflows`](../cashflows/README.md) — accrual and carry inputs.
- [`finstack-factor-model`](../factor-model/) — calibrated credit-factor models consumed via `CreditFactorModelRef`.
- [`finstack-portfolio`](../portfolio/README.md) — aggregates per-instrument `PnlAttribution`s into book-level views.

## References

Quantitative references: [`docs/REFERENCES.md`](../../docs/REFERENCES.md).

- Fixed-income sensitivity intuition: `docs/REFERENCES.md#tuckman-serrat-fixed-income`
- Risk decomposition and factor attribution: `docs/REFERENCES.md#meucci-risk-and-asset-allocation`

## Verification

```bash
cargo fmt -p finstack-attribution
cargo clippy -p finstack-attribution --all-features -- -D warnings
cargo test  -p finstack-attribution
cargo test  -p finstack-attribution --doc
RUSTDOCFLAGS='-D warnings' cargo doc -p finstack-attribution --no-deps --all-features
```
