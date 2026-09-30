# Calibration Goldens

This directory is reserved for goldens whose **expected outputs are
calibration-specific** — residual diagnostics, calibrated knot values,
calibration reports — rather than pricer outputs. None exist yet; the directory
holds only this README.

A fixture dropped here today would be half-covered: `all_fixtures_well_formed`
in [`../../walk.rs`](../../walk.rs) validates every JSON file under `data/`, so
the schema would be checked, but the executing walk only collects paths under
`data/pricing/`, and `run_fixture` in [`../../runner.rs`](../../runner.rs) errors
on any `metadata.domain` it has no runner for. Adding fixtures here therefore
means adding a runner, not just data.

## Where calibration round-trip and vendor-parity tests live today

Calibration math is exercised end-to-end through the **pricing-runner**
fixtures under `tests/golden/data/pricing/`. A pricing fixture's `market` block
is one of two kinds. `envelope` fixtures (49 of the 72 committed today) carry a
`CalibrationEnvelope` that `resolve_market` in
[`../../pricing_common.rs`](../../pricing_common.rs) feeds to
`engine::calibrate`; `snapshot` fixtures (the other 23) carry a
materialized `MarketContext` and skip calibration entirely. Only the `envelope`
half exercises calibration. In both cases the resulting `MarketContext` prices
the fixture's `instrument`, and the metrics are compared against `expected`.

This pattern covers two distinct test shapes with one runner:

1. **Vendor parity** — `expected` values are sourced from a vendor
   (Bloomberg SWPM, QuantLib `IsdaCdsEngine`, etc.). Any drift in
   calibration math surfaces as a parity failure.
   - Example: [`pricing/bloomberg/irs/usd_sofr_5y_receive_fixed_swpm.json`](../pricing/bloomberg/irs/usd_sofr_5y_receive_fixed_swpm.json)
     — bootstraps USD-SOFR from 26 SWPM Curve 490 quotes, prices a 5Y
     receive-fixed swap, expected NPV from the SWPM screen.
   - Example: [`pricing/quantlib/cds/cds_quantlib_flat_hazard_decomposition.json`](../pricing/quantlib/cds/cds_quantlib_flat_hazard_decomposition.json)
     — flat 1% hazard / flat 2% discount, prices each CDS leg, expected
     values from QuantLib `IsdaCdsEngine`.

2. **Round-trip regression** — `expected` is captured from Finstack's own
   calibrate-then-price pipeline rather than from a vendor, and pinned at a
   tolerance tight enough (typically `abs: 1e-6`) that any change in the
   bootstrap moves it. These are `formula`-source and live under
   `regression_goldens/`; they assert stability, not a par value, so
   `expected.npv` is generally not zero.
   - Example: [`pricing/regression_goldens/cds/usd_5y_cds_self_test.json`](../pricing/regression_goldens/cds/usd_5y_cds_self_test.json)
     — five CDS par-spread quotes (1Y–10Y) bootstrap the `ACME-HZD` hazard
     curve against a SOFR-bootstrapped discount curve; the priced 100 bp
     contract's NPV, DV01, and CS01 are pinned to 1e-6.

## When would a calibration-specific runner make sense?

If we ever need to assert on:
- **Calibration report metadata** — solver iterations, multi-start
  restart count, RMSE residuals, worst-quote IDs.
- **Knot-level diagnostics** — calibrated DF / hazard λ / Hull-White
  (κ, σ) values directly.
- **Per-quote residuals** as a vector (not aggregated max/RMSE).

… then a calibration runner could read fixtures from this directory and
compare against a `CalibrationReport`-shaped `expected` block. Until
that need is concrete, the pricing runner's "calibrate then reprice"
contract delivers stronger invariants (fits *and* prices) than a
calibration-only runner would.

## Conventions for any future calibration goldens

When you add a fixture here, follow the same `finstack_quant.golden/1` schema
the pricing runner uses ([`../../schema.rs`](../../schema.rs)). The runner will
need:

- Domain prefix `calibration.<asset_class>` (e.g. `calibration.discount`).
- `market` of kind `envelope` with the calibration plan + quotes.
- `expected` keyed on calibration-report metric names.
- Tolerances per metric, with a reason.
- `metadata` block per the standard template (vendor source, valuation
  date, regen command, screenshots if applicable).

On the Rust side, add the domain to a matcher alongside `is_pricing_domain` in
[`../../runner.rs`](../../runner.rs), route it through a helper shaped like
`pricing_common::run_pricing_fixture` that returns `BTreeMap<String, f64>` of
report metrics, and add a discovery entry point mirroring
`collect_fixture_paths_under("pricing")`. The Python layer under
`finstack-quant-py/tests/golden/` mirrors the same runner registry and would
need the equivalent addition.
