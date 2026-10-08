# Finstack Quant 0.10.0

**Release Date**: 2026-10-06

**Bump type**: minor (pre-1.0; intentional breaking Rust, Python, WASM, and JSON changes)
**Status**: Prepared, not yet tagged (`v0.7.0` is the latest git tag; `v0.8.0` and `v0.9.0` were never created)

## Executive Summary

0.10.0 removes the parallel names left after the 0.9.0 engine move. Each
calculation keeps one entry point, host bindings stop inventing defaults Rust
does not have, and several results that used to be JSON text are now values.

Callers on `v0.7.0` must apply
[`RELEASE_NOTES_0.8.0.md`](RELEASE_NOTES_0.8.0.md) and
[`RELEASE_NOTES_0.9.0.md`](RELEASE_NOTES_0.9.0.md) before this one. There is
no `v0.8.0` or `v0.9.0` tag.

The itemized history is [`CHANGELOG.md`](CHANGELOG.md) under `[0.10.0]`.

## Who Should Upgrade

- Python and WASM callers of path simulation, finite-difference Greeks,
  portfolio construction, cashflow schedule helpers, or calibration plans.
- Anyone still passing invented binding defaults (recovery, settlement delay,
  overnight lookback, feature `params`, quote flags) that Rust required the
  caller to set.
- Rust callers of models, analytics, margin, portfolio, scenarios, and
  valuations whose short names replaced `try_*`, `_crn`, `_json`, and
  `_with_*` variants.

## Breaking Changes

### 1. One path-simulation entry point

`simulate_paths` / `simulatePaths` replaces `simulate_gbm_paths` /
`simulateGbmPaths`. The result is `PathSummary`, and
`PathSummary.to_dataframe()` is long: one row per `(path, time)` and one
column per factor. `ProcessSpec` selects the process, including `local_vol`
and the path-dependent `lmm`, `rough_bergomi`, `rough_heston`, and
`cheyette_rough`. An unsupported process/scheme pair is a validation error.

`RoughBergomiParams.xi` is required on input. It no longer defaults to a flat
4% forward-variance curve.

### 2. Finite-difference Greeks keep the paired estimator

`finite_diff_delta` / `finite_diff_gamma` (`finiteDiffDelta` /
`finiteDiffGamma`) return the old `_crn` paired standard error. They run
serially and reject adaptive stopping and path capture. The `_crn` names are
gone.

### 3. Host defaults that Rust did not have are gone

Representative cuts (the changelog lists every one):

- `CashFlow` requires `accrual_factor`. Builder Decimal text must be exactly
  representable.
- `ProtectionLegSpec` requires `settlement_delay`. `WeightingMethod.metric_weighted`
  requires `neutralize`. `ConversionSpec`, `PoolAsset`, and
  `SpecialServicingSpec` require the fields Rust already required.
- `RecoveryClaim` requires accrued interest, penalties, and collateral
  haircut. `compute_ecl` requires `stage`.
- Hull-White `SwaptionQuote` and `CapFloorQuote` require the normal-vol and
  cap flags.
- Calibration plans no longer carry attached quotes. Execution takes a
  `CalibrationEnvelope`.
- `neutralize_and_zscore` no longer takes `params`.

### 4. Portfolio, scenarios, and margin

- `Portfolio.from_spec` / `Portfolio.fromSpec` is the constructor.
  `build_portfolio_from_spec_json` and the WASM `Built` / `WithMarket`
  duplicates are gone. Python calculation `_json` helpers are gone; call
  `to_json()` on the typed result.
- Factor configuration no longer has `PricingMode`. `apply_and_revalue` and
  `scenario_pnl` return the view types directly.
- `instrument_envelope_from_spec` returns the envelope value (a dict in
  Python, an object in WASM), not JSON text.
- Python `finstack_quant.valuations.instrument_cashflows` is removed. Use
  `finstack_quant.valuations.instruments.instrument_cashflows`, which returns
  `InstrumentCashflowEnvelope`.
- Published margin inputs are `SimmSensitivitiesJson`. GIRR adders take the
  risk currency first. Regulatory inputs cannot manufacture fallback charges
  for unknown buckets or tenors.

### 5. Rust names are the short names

`try_*` accessors that returned `Result` are now the short names and still
return `Result`. Call/put pairs take `OptionType`. `ThetaStepper` is the one
PDE time stepper. COS pricing takes `CosMarketParams` and returns
`finstack_quant_core::Result`. Correlation repair is `nearest_correlation`
in both analytics and models.

`Performance.correlation_matrix` returns `(matrix, repaired)`.

## New Features and Improvements

- `LocalVolSurface` is complete: construct from nodes or from an implied
  surface (Dupire, total variance), with a smoothed builder, serde, and the
  `local_vol_surface` schema. `LocalVolProcess` and `LocalVolPde` price on it.
- Black-Karasinski is its own short-rate tree model (`black_karasinski`),
  separate from Black-Derman-Toy.
- Likelihood-ratio Greeks for a GBM arithmetic Asian:
  `PathDependentPricer.price_with_lrm_greeks` / `priceWithLrmGreeks`.
- Portfolio `FactorModel` is a reusable handle. `factor_stress_pnl` returns
  P&L only. `primitive_exposures` is bound on portfolio.
- `calculate_var_with_pricing` / `calculateVarWithPricing` returns a typed
  `VarResult`. Typed `CdsOption` is bound.
- Implied vol solvers `implied_vol_bachelier` and `implied_vol_black` are
  bound. SABR gains strike-from-delta, no-arbitrage check/repair, and the
  standard factories.
- Tear sheets read units, "% of total", tornado order, and VaR contributions
  from the Rust results.

## Bug Fixes

- `Percentage(1.1).as_bp_f64()` is `110.0`. The old binary conversion could
  surface `110.00000000000001`.
- Unitless `MarketContext.insert_price` rejects non-finite values, including
  from WASM.
- Carry, scenario, and calibration fixes already recorded under `[0.9.0]`
  remain in this tree. 0.9.0 was not tagged.

## Deprecated

None. Removed names are deleted. There is no compatibility alias.

`finstack-quant-models` no longer depends on `indexmap`.

## Migration Checklist

1. If you are on `v0.7.0`, apply
   [`RELEASE_NOTES_0.8.0.md`](RELEASE_NOTES_0.8.0.md) and
   [`RELEASE_NOTES_0.9.0.md`](RELEASE_NOTES_0.9.0.md) first.
2. Replace `simulate_gbm_paths` / `simulateGbmPaths` with `simulate_paths` /
   `simulatePaths`, and read `PathSummary` in long form.
3. Drop `_crn`, `_json`, `try_*`, and `Built` / `WithMarket` names. Use the
   short name in the `[0.10.0]` section of `CHANGELOG.md`.
4. Pass every argument Rust requires. Do not rely on a binding default for
   accrual factor, settlement delay, ECL stage, recovery claim components,
   or Hull-White quote flags.
5. Build calibration runs from a `CalibrationEnvelope`. Quote payloads live
   on the envelope, not on the plan.
6. Treat `instrument_envelope_from_spec` as a value. Serialize it only when
   you need wire text.
7. Set `RoughBergomiParams.xi` explicitly.

## Numerical Behavior

Local-vol construction now errors on butterfly or calendar arbitrage at a
node instead of falling back to implied volatility. `YieldPca.fit_yield_changes`
matches the previous level round trip to 1e-12 relative. Basis-point
conversion of `Rate` and `Percentage` no longer adds binary noise.
`RoughBergomiParams.xi` no longer silently becomes a flat 4% curve.

## Known Limitations

- `v0.8.0` and `v0.9.0` were never tagged. Publishing 0.10.0 from this tree
  is the first tag after `v0.7.0`.
- `cargo semver-checks` against unchanged 0.9.0 reports these breaks as
  requiring a new major version. `0.9.0 -> 0.10.0` is the bump that check
  accepts (`major change`, then `no semver update required`) for every
  publishable domain crate.
- The materialization Criterion baseline is still the 0.7.0 revision
  `b43fd9b0f` (2026-07-28). A fresh compare was not run for this cut.
- `pricing/bloomberg/fra/usd_fra_3x6.json` is allowlisted for a DV01 gap, but
  the Python golden runner now fails before that comparison:
  `ForwardCurve projection_grid` does not cover the last interpolation knot.
  The source DV01 target is unchanged.

See [`CHANGELOG.md`](CHANGELOG.md) for the complete itemized release history.
