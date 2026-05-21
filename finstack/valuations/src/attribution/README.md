# Attribution

P&L attribution decomposes mark-to-market change between two dates into factor contributions (carry, curves, credit, FX, vol, model parameters, scalars).

## Methods

| Method | Module | Behavior |
|--------|--------|----------|
| **Parallel** | `parallel.rs` | Isolate one factor at a time (T₀ level for that factor, T₁ elsewhere). Residual captures cross-effects. |
| **Waterfall** | `waterfall.rs` | Apply factors in order; factor P&Ls sum to total P&L up to tolerance. Order matters. |
| **Metrics-based** | `metrics_based.rs` | Linear (and optional second-order) approximation from precomputed metrics; no extra repricing. |

Default waterfall order: Carry → RatesCurves → CreditCurves → InflationCurves → Correlations → Fx → Volatility → ModelParameters → MarketScalars (`default_waterfall_order()`).

## Factors

`AttributionFactor` in `types.rs` covers the nine dimensions above. Detailed structs (per-curve, per-tenor, per surface) attach to `PnlAttribution` when the run requests them.

## JSON

`AttributionEnvelope` / `AttributionSpec` (`spec.rs`) use schema `finstack.attribution/1` with `market_t0`, `market_t1`, instrument JSON, dates, and method. Call `AttributionSpec::execute()` or use the envelope helpers.

## Usage

```rust
use finstack_valuations::attribution::attribute_pnl_parallel;
use finstack_core::config::FinstackConfig;

let attribution = attribute_pnl_parallel(
    &instrument,
    &market_t0,
    &market_t1,
    as_of_t0,
    as_of_t1,
    &FinstackConfig::default(),
)?;

assert!(attribution.residual_within_meta_tolerance());
```

Metrics-based attribution needs metrics at both dates (for example `Theta`, `Dv01`, `Cs01`, `Vega`) on the input `ValuationResult` pair.

## Model parameters

`model_params.rs` snapshots structured-credit prepayment/default/recovery and convertible conversion inputs for the ModelParameters factor.

## Validation

`PnlAttribution::residual_within_tolerance` and `residual_within_meta_tolerance` compare the residual to config in `AttributionMeta`. Waterfall runs typically stay within ~0.1%; parallel runs often show a few percent residual on large moves.

## Extending

A new factor requires updates to `types.rs`, `factors.rs`, all three methodology modules, `dataframe.rs`, and `default_waterfall_order()`. Follow an existing factor implementation.

## Related

- [`../metrics/README.md`](../metrics/README.md)
- [`../results/README.md`](../results/README.md)
