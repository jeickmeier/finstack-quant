# Finstack Quant 0.7.0

**Release Date**: 2026-08-17
**Bump type**: minor (pre-1.0; includes intentional breaking API, JSON, Python, and WASM changes)
**Status**: Cut from `CHANGELOG.md` after release-prep audit

## Executive Summary

0.7.0 removes silent legacy pathways and standardizes how results cross the
Rust / Python / WASM boundary. Opt-in flags, `None` arms, and `serde(default)`
fallbacks that restored older semantics are gone. Computation entry points
return typed results; `*_json` / `*Json` surfaces are the wire twins.

## Who Should Upgrade

- Anyone who persisted **instrument, margin, attribution, scenario, or
  waterfall** JSON — many previously optional fields are now required, and
  several field names or types changed with no alias.
- Python callers of **pricers**, **performance metrics**, **scenarios**, or
  **statement results** (typed objects replace JSON strings and bare lists).
- WASM callers of **portfolio**, **valuations**, or **statements** exports
  (structured objects replace JSON strings; maps are plain objects).
- Anyone pricing **callables, CMS, Bermudans, or SIMM** — numeric defaults and
  silent clamps that previously invented a price now error or change value.

## Breaking Changes

The full list is in [`CHANGELOG.md`](CHANGELOG.md). The migrations that hit
the most callers:

### 1. NPV no longer has an include-on-valuation-date option

`NpvOptions` and `npv_with_options` are removed. `npv` / `npv_with_ctx` always
exclude flows dated on or before the valuation date. For a project NPV that
must contain the time-0 outlay, use `npv_amounts` or value one day before the
earliest flow.

### 2. Waterfall cash cap is mandatory

`WaterfallSpec.available_cash_node` is required. The engine no longer reports
every fee, coupon, and amortization as paid in full when the model did not
generate the cash. Specs must list `Fees`, `Interest`, and `Amortization` in
`priority_of_payments`. Python `WaterfallSpec(...)` takes the same required
node.

### 3. Typed results instead of JSON strings

Rust, Python, and WASM now return the same shaped objects. Call `.to_json()`
when you still need the wire payload.

```python
# before
result = ValuationResult.from_json(price_instrument(spec, market, as_of))

# after
result = price_instrument(spec, market, as_of)
wire = result.to_json()
```

WASM `priceInstrument*` and most portfolio exports return objects. Maps that
used to arrive as ES `Map`s (dropped by `JSON.stringify`) are now plain
objects.

### 4. CDS option strike is a typed enum

The old scalar strike is rejected:

```json
{ "strike": "0.0325" }                       // rejected
{ "strike": { "spread": "0.0325" } }         // forward-spread
{ "strike": { "clean_price_pct": "107.0" } } // CDX HY clean price
```

### 5. SIMM v2.5 and fabricated credit-qualifying tables are gone

`SimmVersion::V2_5` / `"v2_5"` no longer parse. ISDA credit-qualifying bucket
tables are required. A netting set with no `margin_spec` records an `MO-16`
degradation instead of reporting gross MTM as variation margin.

### 6. Silent pricing defaults became errors or explicit zeros

- Bermudan / Cheyette / LMM Bermudan `enforce_calibration` defaults to `true`.
- Callable Hull-White no longer invents 100 bp short-rate vol.
- `Instrument::market_dependencies` is required (empty means no market data).
- Two-factor `correlation: 0.0` no longer collapses into single-factor
  (implied +1). `factor_correlation` returns `Some(rho)` for any two-factor
  spec.

### 7. Python performance metrics return Series

`perf.sharpe()` is a `pandas.Series` indexed by ticker, not `list[float]`.
Use `perf.sharpe()["FUND"]` or `.iloc[i]`.

### 8. Feature-operation rename

`clip_by_quantile` / `ClipByQuantile` → `winsorize` / `Winsorize`.
`dollar_neutral_weights` / `DollarNeutralWeights` → `long_short_weights` /
`LongShortWeights`.

## Highlights

- Clean-price CDS-option strikes and the CDX HY market convention, with
  strike-kind-specific delta and gamma.
- Two-factor rates-credit lattice for callable credit-risky bonds and term
  loans, with Fréchet-bound correlation checks at calibration.
- Node-valued future floating coupons on the lattice; option-free floater PV
  is invariant as `rate_vol → 0`.
- Shared market-anchored credit-volatility mapping used by the callable
  lattice and revolving-credit CIR path.
- Python: ~90 `to_dataframe` exports, pickle on JSON-round-trippable wrappers,
  `finstack_quant.__version__`, and `FinstackError` as the common exception
  base.
- WASM numeric vectors cross as `Float64Array`.

## Deprecated

None. This release does not keep deprecated aliases.

## Migration Checklist

1. Drop `NpvOptions` / `npv_with_options`; use `npv` or `npv_amounts`.
2. Set `WaterfallSpec.available_cash_node` and list cash-consuming categories
   in `priority_of_payments`.
3. Stop wrapping pricer / scenario / statement results in `from_json(...)`;
   call `.to_json()` only when you need the wire string.
4. Migrate CDS-option JSON to `{ "strike": { "spread": ... } }` or
   `{ "strike": { "clean_price_pct": ... } }`.
5. Remove `"v2_5"` SIMM selections; supply ISDA CQ tables for remaining
   versions.
6. Set `hw1f_sigma` (or accept deterministic rates) on callables; set
   `hazard_volatility` explicitly if you want a stochastic credit factor.
7. Replace `perf.sharpe()[0]` with ticker or `.iloc` access.
8. Rename `clip_by_quantile` → `winsorize` and `dollar_neutral_weights` →
   `long_short_weights`.
9. Import `BarrierType` from `finstack_quant_core::types` and `Position` from
   `finstack_quant_valuations::instruments` (re-export chains removed).
10. WASM: `validateMaterializationJson` → `validateMaterialization`.

## Fixes

- Deposit total-return carry no longer books the opening notional draw on
  the start date as period income (bonds already skipped the issue-date
  draw).
- Amortizing-bond MOIC/XIRR-to-worst redemption uses outstanding principal
  after scheduled principal paid through the exercise date, with independent
  cashflow regressions covering both metrics.
- SIMM credit-qualifying delta now always uses explicit ISDA sector buckets.
  CDS and CDS-index products using SIMM require `simm_credit_classification`;
  the scalar CQ approximation and boolean-classification APIs are removed.

## Known Limitations

- Umbrella-crate `cargo semver-checks` skips 0.x API-diff checks once the
  minor bump is already applied; treat this file and `CHANGELOG.md` as
  authoritative for breaks.

See [`CHANGELOG.md`](CHANGELOG.md) for the full itemized list.
