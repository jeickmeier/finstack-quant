# Finstack Quant 0.9.0

**Release Date**: 2026-10-02

**Bump type**: minor (pre-1.0; intentional breaking Rust, Python, WASM, and JSON changes, plus pricing changes)
**Status**: Not tagged. Superseded by 0.10.0 on this tree. `v0.7.0` is still the latest git tag; `v0.8.0` and `v0.9.0` were never created.

## Executive Summary

0.9.0 is the cut after the unpublished 0.8.0 engine move. It makes the WASM
facade match the Python and Rust contracts, renames persisted fields so units
are in the name, and corrects rates and credit pricing where the previous
model clock, curve fit, or risk metric did not match the selected model.

Callers on `v0.7.0` must apply the 0.8.0 migration in
[`RELEASE_NOTES_0.8.0.md`](RELEASE_NOTES_0.8.0.md) before this one. There is
no `v0.8.0` tag.

The itemized history is [`CHANGELOG.md`](CHANGELOG.md) under `[0.9.0]`.

## Who Should Upgrade

- WASM callers. Most namespaces were renamed, retyped, or given new
  constructors so they match Python. Zero-argument enum constructors were
  removed. Schema registries are now in the package, and the optimized build
  is larger.
- Python callers of instrument fields, metric overrides, and builders touched
  by the 2026-09-24 name-and-unit pass.
- Anyone pricing Hull-White path-dependent rates, IRS DV01, zero-coupon
  inflation swaps, cash-settled swaptions, callable range accruals, Bermudan
  swaptions, or CDS tranches. Several of those numbers move.
- Anyone requesting a metric the instrument type does not implement. That
  request now errors instead of disappearing from the result.

## Breaking Changes

### 1. WASM matches the Rust and Python contracts

Representative cuts (the changelog lists every rename):

- `StructuredCredit.price_stochastic(market, as_of, num_paths, antithetic)` is
  the only stochastic entry. `price_stochastic_monte_carlo` is gone. An
  omitted `antithetic` uses the deal's `mc_antithetic` setting. The bindings
  used to force `true`.
- Python `Portfolio.to_spec_json()` is `to_json()`. `to_spec()` returns a
  dict. WASM gains `Portfolio.toSpec()`.
- Hull-White curve-input calibrators are `calibrate_hull_white_to_swaptions`,
  `calibrate_hull_white_to_cap_floors`, and
  `bootstrap_hull_white_sigma_schedule_to_cap_floors`. Closure forms are
  `*_with_fn`.
- Forty zero-argument enum constructors in `scenarios` and `margin` are
  removed (`curveKindDiscount()`, `compoundingAnnual()`, and the rest). Pass
  the string or object literal of the generated type.
- `ScheduleBuilder` setters update the builder in place. `MarketContext`
  insert and collateral methods return the context.
- Cargo feature `console_panic_hook` is removed. The panic hook is always
  installed.
- Enum arguments accept the bare variant name (`'parallel'`, `'normal'`),
  not a quoted JSON string.
- `core.nextEquityOptionExpiry` is `core.nextThirdFriday`.
- `MarketContext.fromJson` / Python `from_json` is the strict persisted-state
  load and requires `schema_version: 1`.

Each published domain now has a `schema` namespace. Python returns JSON text;
WASM returns plain objects.

### 2. Field names carry their unit

`_pct` is percent points (`100.0` = 100%). Decimal ratios use `_decimal`,
`_rate`, or a ratio noun. Basis points use `_bp`. Where the scale changed,
the changelog gives the old and new numeric equivalent.

Examples:

- `BumpConfig.spot_bump_pct` / `vol_bump_pct` are `spot_bump_decimal` /
  `vol_bump_decimal`.
- Agency servicing and guarantee fees are annual basis points
  (`servicing_fee_bp`: `25.0` was `0.0025`).
- `BermudanCallProvision.call_price` is `price_pct_of_par` (`100.0` = par;
  the old value was the fraction `1.0`).
- `CashFlowSchedule.weighted_average_life` is `wal`.

`TermLoanSpec` is removed. `TermLoan` is the term sheet.

### 3. A requested metric is returned or it errors

`price_instrument` / `priceInstrument` / `Instrument::price_with_metrics`
raise `MetricNotApplicable` when the instrument type has no calculator for a
requested metric. 16,509 of 17,940 (instrument type, metric) pairs change
from a silent omission to an error. Portfolio valuation still narrows a
shared metric list to the calculators each position actually has, and records
the rest on `PositionValue.inapplicable_metrics`.

### 4. Rates and credit numbers move

- Hull-White 1F θ(t) follows the discount curve's interpolation. A 10MM 5Y
  Bermudan payer LSMC moves from 2.82MM to 0.25MM, in line with the tree.
  TARN, snowball, callable range accrual, MBS Monte Carlo OAS, and revolver
  rate paths use the same fit. Those exotics simulate on an ACT/365F model
  clock.
- A callable range accrual call pays the call price plus coupon accrued to
  the call date. Call dates after the final payment date are rejected.
- IRS `dv01` is a full revaluation: quote shock and re-bootstrap when the
  curve has calibration metadata, otherwise a parallel bump of the fitted
  curve.
- A zero-coupon inflation swap counts a short first fixed period as its share
  of a year.
- Par-yield cash-settled swaption annuity uses the fixed leg's own schedule.
- A Bermudan valued on an exercise date keeps that day's exercise right.
- Cap/floor `forward_pv01` and CMS option `delta`, `vega`, `rho`, and
  `volga` reprice with the selected model.
- CDS tranche pricing floors a negative pre-maturity tranchelet loss at zero
  so steep base-correlation skews still price. Arbitrage at the tranche
  maturity is still an error.

### 5. Removed unused agency-mortgage models

Rust-only modules that nothing called were deleted: the parallel agency
prepayment and servicing models, TBA settlement and generic-pool allocation,
and the unused CMO sequential / IO-PO helpers. Price TBA on
`AgencyTba::get_settlement_date` and the CMO waterfall. A TBA with an
explicit `assumed_pool` rejects a pool whose pass-through coupon or agency
program is not good delivery.

## New Features and Improvements

- WASM schema accessors for every published domain, generated TypeScript
  types, and executable `index.d.ts` examples checked by `mise run wasm-doc`
  and `mise run wasm-test`.
- Carry decomposition and iterative breakeven hold each crossed unpublished
  `FIXING:*` observation at its as-of projection, matching theta.
- `fx_delta`, `fx01`, and per-curve `pv01::{curve}` on cross-currency swaps.
- Every published schema artifact carries a validated example. The
  `common/1/day_count` schema is 5.1 KB (it had grown past the 16 KiB inline
  budget). Wire values are unchanged.
- UI registry WASM pins are derived from `finstack-quant-wasm/package.json`.

## Bug Fixes

- Variance-swap `vega` and `variance_vega` weight the forward leg by the
  unobserved share of contractual samples.
- Windowed statement formulas take their dimension from the series argument,
  so a debt corkscrew can add a lag, drawdowns, and repayments.
- The CDX IG 46 index-option test records the library NPV (112,047.41) and
  the open 6,734.35 difference to Bloomberg CDSO. No pricing change.

## Deprecated

None. Removed names are deleted. There is no compatibility alias.

## Migration Checklist

1. If you are on `v0.7.0`, apply
   [`RELEASE_NOTES_0.8.0.md`](RELEASE_NOTES_0.8.0.md) first.
2. Rebuild WASM callers against `index.d.ts`. Replace removed enum
   constructors with string or object literals. Pass bare variant names, not
   quoted JSON strings.
3. Rename persisted fields using the `[0.9.0]` sections of `CHANGELOG.md`,
   and rescale any field whose unit changed (`_pct` percent points, `_bp`
   basis points, `_decimal` ratios).
4. Stop requesting metrics an instrument type does not implement, or catch
   `MetricNotApplicable`. Mixed portfolios should read
   `inapplicable_metrics`.
5. Re-baseline Hull-White exotics, IRS DV01, inflation swaps, cash-settled
   swaptions, callable range accruals, and CDS tranches.
6. Expect the optimized WASM package to grow by about 24% (29.7 MB to
   36.8 MB; brotli 5.15 MB to 6.07 MB) because schema registries and the
   JSON Schema validator are compiled in.

## Numerical Behavior

The rates and credit section above is the deliberate pricing delta. Normal
SABR beta = 0 parity pins Hagan's exact form; QuantLib has no beta = 0
special case and differs by about 0.13% in vol off the money. That is a test
pin, not a library pricing change.

## Known Limitations

- Theta at 1M and 6M still fails for caps/floors, structured credit, and
  equity/FX variance swaps when the roll crosses an unpublished fixing or
  price observation. `valuations/tests/golden/theta_horizons.rs` lists each
  remaining failure. Carry and iterative breakeven are fixed for coupon
  fixings; those theta cases are not.
- The CDX IG 46 index-option NPV is not reconciled to Bloomberg CDSO
  (forward-spread convention and variance clock). The difference is recorded,
  not closed.
- `v0.8.0` and `v0.9.0` were never tagged. The next tag from this tree is
  `v0.10.0`.
- `cargo semver-checks check-release` against `v0.7.0` skips 0.x API-diff
  lints once the minor bump is already applied (`no semver update required`,
  253 checks skipped on core). A forced patch check reports removed features
  such as `ts_export`. This file and `CHANGELOG.md` are the break list.

See [`CHANGELOG.md`](CHANGELOG.md) for the complete itemized release history.
