# Calibration Goldens

This directory is reserved for goldens whose **expected outputs are
calibration-specific** — residual diagnostics, calibrated knot values,
calibration reports — rather than pricer outputs. None exist yet; if and when
they do, they need a new runner registered in `tests/golden/runner.rs`.

## Where calibration round-trip and vendor-parity tests live today

Calibration math is exercised end-to-end through the **pricing-runner**
fixtures under `tests/golden/data/pricing/`. Each pricing fixture supplies a
`market` block of kind `envelope` that the runner feeds to
`engine::execute_with_diagnostics`. The calibrated `MarketContext` is then
used to price the fixture's `instrument`, and the resulting metrics are
compared against `expected`.

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

2. **Round-trip self-test** — the priced instrument *is* one of the
   calibration inputs. `expected.npv` ≈ 0 with a tight tolerance,
   demonstrating the bootstrap converged. These are formula-source and
   serve as bit-stable regression catches for any calibration code change.
  - Example: [`pricing/regression_goldens/cds/usd_5y_cds_self_test.json`](../pricing/regression_goldens/cds/usd_5y_cds_self_test.json)

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

When you add a fixture here, follow the same `finstack_quant.golden/2` schema
the pricing runner uses (`tests/golden/schema.rs`). The runner will need:

- Domain prefix `calibration.<asset_class>` (e.g. `calibration.discount`).
- `market` of kind `envelope` with the calibration plan + quotes.
- `expected` keyed on calibration-report metric names.
- Tolerances per metric, with a reason.
- `metadata` block per the standard template (vendor source, valuation
  date, regen command, screenshots if applicable).

Add your domain to a new `is_calibration_domain` matcher and route through
a `calibration_common::run_calibration_fixture` helper that returns
`BTreeMap<String, f64>` of report metrics.
