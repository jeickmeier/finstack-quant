# Attribution

P&L attribution decomposes mark-to-market change between two dates into factor
contributions: carry, rates curves, credit curves, inflation, correlations, FX,
volatility, model parameters, and market scalars.

## Methods

| Method | Module | Behavior |
|--------|--------|----------|
| **Parallel** | `parallel.rs` | Isolate one factor at a time (T₀ level for that factor, T₁ elsewhere). Residual captures cross-effects. |
| **Waterfall** | `waterfall.rs` | Apply factors in order; factor P&Ls sum to total P&L up to tolerance. Order matters. |
| **Metrics-based** | `metrics_based.rs` | Linear (and optional second-order) approximation from precomputed metrics; no extra repricing. |
| **Taylor** | `taylor.rs` | Sensitivity-based Taylor expansion from bump-and-reprice Greeks; optional second-order terms. |

Default waterfall order: Carry → RatesCurves → CreditCurves → InflationCurves
→ Correlations → Fx → Volatility → ModelParameters → MarketScalars
(`default_waterfall_order()`).

## Factors

`AttributionFactor` in `types.rs` covers the nine dimensions above. Detailed
structs (per-curve, per-tenor, per surface) attach to `PnlAttribution` when the
run requests them.

## JSON

`AttributionEnvelope` / `AttributionSpec` (`spec.rs`) use schema
`finstack_quant.attribution/1` with `market_t0`, `market_t1`, instrument JSON, dates,
and method. Call `AttributionSpec::execute()` or use the envelope helpers.

## Usage

```rust,ignore
use finstack_quant_attribution::{attribute_pnl_parallel, ExecutionPolicy};
use finstack_quant_core::config::FinstackConfig;

let attribution = attribute_pnl_parallel(
    &instrument,
    &market_t0,
    &market_t1,
    as_of_t0,
    as_of_t1,
    &FinstackConfig::default(),
    ExecutionPolicy::Parallel,
)?;

assert!(attribution.residual_within_meta_tolerance());
```

Metrics-based attribution needs metrics at both dates on the input
`ValuationResult` pair. `default_attribution_metrics()` returns the default set
used by the JSON/spec pipeline.

## Model parameters

`model_params.rs` snapshots structured-credit prepayment/default/recovery and
convertible conversion inputs for the ModelParameters factor.

## Validation

`PnlAttribution::residual_within_tolerance` and
`residual_within_meta_tolerance` compare the residual to config in
`AttributionMeta`. Waterfall runs typically stay within ~0.1%; parallel runs
often show a few percent residual on large moves.

## Extending

A new factor requires updates to `types.rs`, `factors.rs`, the methodology
modules (`parallel`, `waterfall`, `metrics_based`, `taylor`),
`default_waterfall_order()`, the JSON schemas under `schemas/attribution/1/`,
and the attribution tests. Follow an existing factor implementation end to end.

## Related

- [Crate overview](../README.md)
- [Valuation metrics](../../valuations/src/metrics/README.md)
- [Valuation results](../../valuations/src/results/README.md)
