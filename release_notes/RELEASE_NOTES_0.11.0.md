# Finstack Quant 0.11.0

**Release Date**: 2026-10-18

**Bump type**: minor (pre-1.0; intentional breaking Rust, Python, WASM, and JSON changes)
**Status**: Prepared, not yet tagged (`v0.7.0` is the latest git tag; `v0.8.0`, `v0.9.0`, and `v0.10.0` were never created)

## Executive Summary

0.11.0 splits the monolithic `finstack-quant-valuations` and
`finstack-quant-models` crates along their natural dependency boundaries so incremental
edits no longer rebuild the entire downstream graph, simplifies the `core`,
`scenarios`, and `attribution` APIs, and surfaces provenance and calculation
intermediates across valuation, cashflow, calibration, margin, attribution,
and statement results.

Callers on `v0.7.0` must apply
[`RELEASE_NOTES_0.8.0.md`](RELEASE_NOTES_0.8.0.md),
[`RELEASE_NOTES_0.9.0.md`](RELEASE_NOTES_0.9.0.md), and
[`RELEASE_NOTES_0.10.0.md`](RELEASE_NOTES_0.10.0.md) before this one. There is
no `v0.8.0`, `v0.9.0`, or `v0.10.0` tag.

The itemized history is [`CHANGELOG.md`](../CHANGELOG.md) under `[0.11.0]`.

## Who Should Upgrade

- Rust callers importing directly from `finstack-quant-models::factor`,
  `finstack-quant-valuations::instruments::rates::hw1f`, or the moved
  `cashflows` / `core` / `models` kernels, or implementing `Instrument` outside
  the workspace.
- Rust, Python, and WASM callers of `core` (`Percentage`, `Rate`, `Bps`,
  `Money`, `HolidayCalendar`, `DiscountCurve`, `FxMatrix`), `margin`,
  `scenarios`, `attribution`, or `calibration` touched by the breaking API and
  wire changes below.
- Callers consuming persisted `VmResult`, `CashflowRow`, or `QuoteQuality`
  payloads, which gain required fields or replace placeholder target/fitted
  columns with `quote_value`.

## Breaking Changes

### 1. Valuations and factor crate split

- **Factor crate (`finstack-quant-factor`)**: `finstack_quant_models::factor`
  is now the standalone `finstack-quant-factor` crate. The umbrella path
  `finstack_quant::models::factor` and the Python/WASM `models.factor`
  namespaces are unchanged. `finstack_quant_analytics::correlation` is removed;
  `nearest_correlation` and `NearestCorrelationOpts` live in
  `finstack_quant_core::math::linalg` (and remain re-exported at
  `finstack_quant_models::correlation`).
- **Valuations split (`finstack-quant-valuations-{core,rates,fixed-income,structured-credit,credit,equity,fx,exotics}`)**:
  `finstack-quant-valuations-core` holds the pricing framework (`Instrument`,
  `PricingOptions`, `PricerRegistry`, `Family`, `MetricId`, `ValuationResult`,
  conventions, and cashflow export). Each instrument family owns its
  registrations via a `Family` static (`RATES`, `FIXED_INCOME`,
  `STRUCTURED_CREDIT`, `CREDIT`, `EQUITY`, `FX`, `EXOTICS`), and
  `finstack-quant-calibration` depends only on `valuations-{core,rates,credit}`.
  The umbrella `finstack-quant-valuations` re-exports the framework and families
  at their previous paths.
- **Moved kernels**:
  - Hull-White 1F Monte Carlo and LSMC engines move from
    `finstack_quant_valuations::instruments::rates::hw1f` to
    `finstack_quant_models::rates::hw1f` (`Hw1fMcPricer`, `Hw1fLsmcPricer`,
    `Hw1fMcConfig`, `ShortRateBasis`).
  - `RateSpec` and `FloatingLegCompounding` live in
    `finstack_quant_cashflows::builder`.
  - Carr-Madan variance replication, the three-factor credit-line process
    (`CreditLineProcess`), `PiecewiseGbmProcess`, and the range-accrual /
    cliquet / autocallable payoffs move to `finstack-quant-models`.
  - Prepayment/default rate conversions (`clamped_cpr_to_smm`, `psa_to_cpr`, …)
    move to `finstack_quant_cashflows::builder`.
  - Curve-time, z-spread discount factor, compensated statistics, basis-point
    unit constants, FX spot-date helpers, and input validators move to
    `finstack-quant-core`.
- **Registry dispatch**: `PricingDispatch::from_model(model, registry)` takes
  the registry explicitly. Pricing with `PricingOptions::registry` unset
  resolves the instrument's `Instrument::family()` registry.

```rust
// Before (0.10.0)
use finstack_quant_models::factor::FactorModelConfig;
use finstack_quant_valuations::instruments::rates::hw1f::RateExoticMcConfig;

// After (0.11.0)
use finstack_quant_factor::FactorModelConfig;
use finstack_quant_models::rates::hw1f::Hw1fMcConfig;
```

### 2. Core, Margin, Scenarios, and Attribution API simplification

- **`Percentage` removed**: Use `Rate::from_percent` / `Rate::as_percent`.
  Python `Rate.as_percentage`, `Bps.as_percentage`, and
  `CreditRating.from_name` (use `CreditRating(name)`), and WASM `asPercentage` /
  `CreditRating.fromName`, are removed.
- **Exact decimal percent conversion and checked operators**:
  `Rate::from_percent`, `Rate::as_percent`, and `"%"` rate strings move the
  decimal point of the shortest decimal form (`from_percent(1.1)` is exactly
  110 bp). `Bps` and `Rate` arithmetic operators panic on overflow, zero
  divisor, or non-finite input in every build profile; Python and WASM
  `Bps.abs` raise on `i32::MIN`.
- **`Money` and `Id` cleanup**: `Money` removes `into_amount`, `into_parts`,
  `f64` `*`/`/` operators, `TryFrom<(f64, Currency)>`, and the `money!` macro.
  `Id::into_string` and `FromStr for Id` are removed.
- **`Discountable::npv` and `poisson_inverse_cdf`**: Both return `Result`
  (`PVOutput` associated type and the `λ > 30` normal approximation are
  removed).
- **Calendars and FX spot dates**: `HolidayCalendar::is_business_day` has no
  default implementation; `DateExt::is_business_day` is removed.
  `DiscountCurve` carries an optional `calendar_id` (required for `Bus/252`).
  `fx_spot_date` takes a `u32` contractual lag (0..=31) and requires a USD
  calendar for non-USD crosses. `FxMatrix::clear_cache` → `clear_quotes`,
  `cache_stats` → `stored_quote_count`.
- **Margin fallibility**: `ClearingHouseImCalculator::calculate_conservative`,
  `MarginFundingCost::calculate` / `cost_for_period`, and
  `Haircut01::calculate` return `Result` (raising `ValueError` in Python and
  throwing in WASM instead of returning `NaN`/`±∞`).
- **Scenarios and Attribution**:
  - `ScenarioSpec::with_hazard_bump_mode` is removed on every host; pass
    `hazard_bump_mode` to the constructor or set the field.
  - Python `ApplicationReport.warnings_json` and `HorizonResult.warnings_json`
    are removed (`warnings` returns the same list).
  - Rust `ScenarioSpec::parallel_bp_many` moves to
    `OperationSpec::parallel_bp_many`; `ScenarioEngine::recalibration_provider`
    → `get_recalibration_provider`.
  - Rust long-row attribution projections are imported from
    `finstack_quant_attribution::long_rows`.

### 3. Persisted wire and result changes

- **`QuoteQuality.quote_value`**: Replaces `target_value` and `fitted_value` on
  calibration quote reports and DataFrames (`quote_value` column replaces
  `target` and `fitted`). `CalibrationDiagnostics.singular_values` and
  `r_squared` are removed.
- **`VmResult`**: Gains required fields `threshold`, `independent_amount`,
  `collateral_balance`, `unrounded_call`, `mta`, and `rounding_increment`.
- **`CashflowRow`**: Requires `native_pv` on input and adds coupon-input and FX
  columns (`accrual_start`, `accrual_end`, `accrual_day_count`,
  `accrual_notional`, `index_rate`, `principal_delta`, `native_pv`, `fx_rate`).

## New Features and Improvements

- **Valuation provenance and explain traces**: Every `ValuationResult` carries
  `provenance` (`ValuationProvenance`: model, `requested_as_of`, declared
  `market_dependencies`, `scenario_price_shock_decimal`, and effective
  `sensitivity_bumps`). `PricingOptions::with_explain(ExplainOpts)` (Python
  `price_instrument(..., explain=True)`, WASM trailing `explain` boolean)
  attaches a per-cashflow PV trace under discounting and hazard-rate models.
- **Calibration explain traces and curve provenance**: `CalibrationConfig.explain`
  is now a wire setting (`{"explain": {"enabled": true}}`), recording
  `knot_solved` and `global_solve` steps. Python and WASM expose
  `DiscountCurve.rate_calibration`, `ForwardCurve.rate_calibration`, and
  `HazardCurve.hazard_calibration`.
- **Rich intermediate result fields across domains**:
  - Margin: `ImResult.simm_detail`, `XvaResult.{cva_rows,dva_rows,fva_rows}`,
    `FrtbSbaResult.scenario_breakdown`.
  - Portfolio margin bindings: Python `portfolio.PortfolioMarginAggregator`,
    `PortfolioMarginResult`, `NettingSetMargin`; WASM
    `portfolio.PortfolioMarginAggregator`.
  - Attribution: `PnlAttribution.{pv_t0,pv_t1}`, `waterfall_steps`, and
    `sensitivity_steps`.
  - Statements & analytics: `StatementResult.node_sources` / `get_source`,
    `CheckFinding.comparison`, principal prepayment breakdown on
    `CashflowBreakdown`, `CorporateValuationResult` DCF bridge fields, and
    `GoalSeekResult` convergence diagnostics.
  - Scenarios & portfolio: `ApplicationReport.applied_shocks` and
    `PositionValue.{fx_rate,fx_triangulated}`.
- **Market conventions**: Added `AUD-BBSW-3M` and `AUD-BBSW-6M` rate indices;
  USD/PHP and USD/RUB T+1 spot pairs; Zurich (`chzh`), SIX (`six`), and `defr`
  weekend holiday rules updated.

## Bug Fixes

- `Explanation.breakdown` is now empty when a statement cell's value came from
  an explicit value, forecast, or `where` mask rather than evaluating its
  formula.
- Expression engine `rank` assigns minimum rank (`method="min"`) to tied
  values, matching its documentation and statement semantics.
- Curve-shift measurements in `market_data::diff` return an error on an empty
  or non-finite tenor grid or sample instead of averaging over fewer curves.
- Rolled basis-spread, inflation, and price curves preserve their source
  interpolation (`roll_transform`) when shocked after rolling.

## Deprecated

None. Removed names are deleted with no compatibility aliases.

Removed unused dependencies: `smallvec`, `thiserror`, and `tracing` from
`finstack-quant-valuations`; `rand` and `rand_pcg` from
`finstack-quant-models`; `finstack-quant-models` from
`finstack-quant-portfolio`; direct `rayon` from `finstack-quant-scenarios` and
`finstack-quant-attribution`. Upgraded `nalgebra` to `0.35` (`simba 0.10.2`) and
`rust_decimal` to `1.43.0`, eliminating `paste` (`RUSTSEC-2024-0436`) and
`rkyv` (`RUSTSEC-2026-0235`) from `Cargo.lock` and removing their advisory
ignores from `deny.toml` and `osv-scanner.toml`.

## Migration Checklist

1. If you are on `v0.7.0`, apply
   [`RELEASE_NOTES_0.8.0.md`](RELEASE_NOTES_0.8.0.md),
   [`RELEASE_NOTES_0.9.0.md`](RELEASE_NOTES_0.9.0.md), and
   [`RELEASE_NOTES_0.10.0.md`](RELEASE_NOTES_0.10.0.md) first.
2. Update Rust imports of `finstack_quant_models::factor::*` to
   `finstack_quant_factor::*` (or use `finstack_quant::models::factor::*`), and
   `finstack_quant_valuations::instruments::rates::hw1f::*` to
   `finstack_quant_models::rates::hw1f::*`.
3. Replace `Percentage` with `Rate::from_percent` / `Rate::as_percent`, and
   `CreditRating.from_name(x)` with `CreditRating(x)`.
4. Supply a `calendar_id` when constructing any `Bus/252` `DiscountCurve`.
5. Update calibration residual consumers from `target_value` / `fitted_value`
   (`target` / `fitted` DataFrame columns) to `quote_value`.
6. Regenerate persisted `VmResult` and `InstrumentCashflowEnvelope` payloads to
   include the new required fields.
