# CALIBRATION-TARGETS-006 — Lagged CPI anchors and inflation-swap calibration

- **Decision status:** Draft — observed implementation; adoption pending.
- **Implementation status:** Observed for the stated source scope; behavior and scope limitations are described below.
- **Validation status:** Source and selected evidence inspected or located; not executed.
- **Audit date / source revision:** 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. Scope and evidence boundaries: [README](README.md).

## Choice and domain

Fit CPI levels to quote-built zero-coupon and YoY inflation swap PV/N. The curve origin is the lagged **reference date**, not necessarily valuation date. Calendar-month observed anchors determine quote exposure; projected CPI interpolation remains a curve-level choice. The target accepts month lags or no lag and rejects day-based lags because the output curve cannot represent them.

## Algorithm

1. Require inflation quotes, discount curve and finite positive notional/base CPI through public preflight. Quote index must match parameter index or output curve ID. Resolve each inflation convention. When observed InflationIndex exists under curve ID, preflight requires currency, lag and base CPI consistency (`max(1e-8,1e-10*abs(expected))`); target additionally requires interpolation matching that observed index.
2. Effective base CPI is `index.value_on(base_date)` if available, otherwise explicit base CPI. This includes index lag/seasonality; preflight guards differing lag/base inputs. Set reference origin to `base_date.add_months(-lag)` preserving calendar date behavior. Curve stores lag months and selected interpolation (schema default Linear; the core's standalone default differs).
3. Build pay-fixed ZC/YoY contracts with start at valuation base date, explicit base CPI, curve/discount IDs, quote fixed rate, convention day count/interpolation/BDC/calendar and explicit lag. YoY also takes the quote frequency. Same production pricers provide residuals, so no additional convexity model is fitted here.
4. For a month lag, final required anchor is first day of maturity month minus lag; if convention interpolation is Linear and maturity day is after day one, advance one month. With no lag, use maturity directly. The parser distinguishes textual zero lags: `"0M"` normalizes to `None` and uses maturity, while `"00M"` remains `Months(0)` and uses the monthly first-day anchor (advanced one month for Linear interpolation when required). Pillar time is ACT/365F from the reference origin to that required fixing anchor; the stored PreparedQuote pillar date remains contractual maturity.
5. Bootstrap seeds `C0*(1+r_quote)^t`; trial construction removes knots within `abs(t)<=1e-8`, inserts `(0,C0)`, sorts and constructs the curve. Bootstrap scan is the framework default geometric scan; no inflation-specific scan is supplied. CPI knots must be finite and within configurable hard levels (defaults `[1,10000]`).
6. GlobalSolve skips nonpositive/nonfinite times. Seed bounds are `lo=max(C0*0.9^t,cpi_hard_min)` and `hi=min(C0*1.5^t,cpi_hard_max)`; clamp the quote-growth seed to those bounds. Subsequent trial parameters are direct CPI levels constrained only by hard level validation, not the -10%/+50% seed-growth assumptions. Weight is time scheme floored at `1e-3`; fit tolerance defaults `1e-8` PV/N.

## Limits and discrepancies

**Observed seed-domain gap:** positive base CPI is not required to lie within configured hard bounds. The global seed path calls `f64::clamp(lo,hi)` without checking `lo<=hi`. For example `C0=20000`, `t=1`, default bounds gives `lo=18000`, `hi=10000`; an inverted interval reaches a panic-capable clamp. This is a static arithmetic/control-flow discrepancy, not a runtime result. A finite quote rate below -100% can also create an invalid fractional-power seed; public rate validation currently checks only finiteness.

Observed CPI versus projected level, month anchoring, seasonality and publication eligibility belong jointly to [core observations](../core-market/inflation-observations.md) and [inflation instrument pricing](../valuations-rates/13-inflation-swaps.md). Exact fit of the same pricing kernel does not independently qualify those conventions. Alternatives include fitting annual inflation rates/log CPI, anchor-month parameters, explicit seasonal term structures, and quote-rate residuals. Initial -10%/+50% growth limits are not final global bounds.

## Evidence

- Owning [inflation.rs](../../../../finstack-quant/calibration/src/targets/inflation.rs); inspected [preflight](../../../../finstack-quant/calibration/src/validation/preflight.rs) establishes that differing observed lag/base is rejected publicly.
- [Inflation integration tests](../../../../finstack-quant/calibration/tests/calibration/inflation.rs) locate reference-origin, lagged-fixing, observed-base consistency, no-override repricing, leap-day and day-lag rejection evidence. The selected ZC case in [tests/repricing.rs](../../../../finstack-quant/calibration/src/tests/repricing.rs) builds the same convention-based trade after calibration.

No numerical tests were executed.

## Relationship to the accepted CPI source boundary

[CORE-MARKET-10's complete accepted package](../core-market/inflation-projection.md#accepted-decision-explicit-cpi-levels-and-anchored-annual-parameter-shocks) requires an explicit source
origin (0,I0), clock/style/tails and exact positive source grid, while removing
core lag metadata and duplicated CPI0 state. Reconcile this target's source
anchor separately from its contractual start reference CPI: shifting a date by
a lag does not identify a blended daily reference CPI as a raw source observation.
This needs financial input/factory reconciliation, not a mechanical builder
rename. Source-parameter shocks remain distinct from quote recalibration. The
algorithm above records current runtime. Record10 was accepted on **2026-11-08**
with reconciliation/qualification pending; this target stays Draft, including
its solver and bounds.

## Relationship to the accepted monthly fixing boundary

[CORE-MARKET-11's complete accepted package](../core-market/inflation-observations.md#accepted-decision-exact-monthly-fixing-snapshots-and-explicit-availability) removes observed-source
lag/interpolation/seasonal multiplication and requires exact selected monthly
versions with explicit availability/cutoff. Reconcile preflight and target
comparisons against the actual index identity and contract convention, rather
than inheriting source controls or using a seasonally changed value_on. Keep
accepted10's source origin separate from contractual reference CPI and preserve
the complete selected fixing snapshot/dependencies in construction provenance.
Record11 was accepted on **2026-11-08**, with reconciliation/qualification
pending; this target's remaining calibration methodology stays Draft.

## Follow-up tracking

Adoption, reconciliation and qualification work is tracked in the
[calibration-target follow-ups](../FOLLOW_UPS.md#calibration-quotes-and-curvesmile-targets).
