# CALIBRATION-TARGETS-006 — Lagged CPI anchors and inflation-swap calibration

- **Decision status:** Accepted 2026-11-15 — complete CPI calibration package.
- **Implementation status:** Observed. Reconciliation to the accepted package is pending.
- **Validation status:** Independent anchor, identification, bound and native residual checks verified. An isolated Rust probe reproduced the inverted-bound clamp panic. Production calibration, market references, restoration and host qualification remain pending.
- **Audit date / source revision:** 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. Scope and evidence boundaries: [README](README.md).

## Accepted decision for explicit CPI anchors and delivered inflation calibration

1. **Give the CPI source and swap start separate anchors.** Bind one
   exact index series, currency, rebasing and seasonal basis, and an
   independent nominal discount source. Require an explicit unlagged
   curve origin `(B, I₀)`, source clock and required calendar context,
   interpolation and tails. Resolve the contractual start CPI under
   the contract's reference rules. An authorized fixed start CPI applies
   only to the zero-coupon start or first chronological periodic
   denominator.
   [Accepted CPI source](../core-market/inflation-projection.md#accepted-decision-explicit-cpi-levels-and-anchored-annual-parameter-shocks) ·
   [Accepted inflation swap cashflows](../valuations-rates/13-inflation-swaps.md#accepted-decision-for-checked-inflation-swap-cashflows-and-risk).

2. **Freeze the information cutoff and resolve each contributing
   month once.** Select no lag, day lag or month lag and the shared
   Step or Linear rule explicitly. Month lag moves the anchor month
   and preserves the original day weight. Day lag shifts the date
   before computing the weight. Equivalent zero lags give the same
   result. Use the exact eligible actual vintage first. Project a
   required month only when a matching release fact known at the
   cutoff places release strictly afterward. Missing or overdue
   fixings and unsupported pre-origin projections fail. Preserve
   observed prints on their published basis and keep forecast
   seasonality outside fixing lookup.
   [Accepted reference and information rules](../valuations-framework/inflation-references.md#accepted-decision-for-checked-inflation-references-and-information-dates) ·
   [Accepted fixing snapshots](../core-market/inflation-observations.md#accepted-decision-exact-monthly-fixing-snapshots-and-explicit-availability).

3. **Fit native pay-fixed swap residuals.** Use signed raw PV per
   exact positive notional and the accepted swap pricer. Zero-coupon
   quotes require `k > −1` and compare CPI ratio growth with
   `(1+k)^τ−1` on the actual payment and elected contractual
   exponent `τ`. Periodic YoY quotes compare each actual CPI ratio
   growth with signed linear `kαᵢ` using explicit frequency, accrual
   and payoff law. Apply dated discount factors, payment entitlement
   and any contractual cash rounding. Omit final PV rounding and
   generic convexity adjustments. Require genuine live CPI exposure
   for every consumed row, including at zero weight.
   [Accepted inflation swap cashflows](../valuations-rates/13-inflation-swaps.md#accepted-decision-for-checked-inflation-swap-cashflows-and-risk) ·
   [Bloomberg SEF zero-coupon convention](https://www.cftc.gov/filings/orgrules/rules05282522457.pdf).

4. **Derive pricing support from actual projected anchors.** Keep
   contractual maturity, actual payments, required reference months
   and fitted knots distinct. Include every contributing projected
   month from eligible payments, including a projected denominator
   and any contributing second month of a Linear reference. Require
   one identifying quote per new bootstrap knot at its latest
   required projected month, with proof that later knots cannot
   change its pricing inputs. Qualify Linear and LogLinear on
   supported prefixes. Elect simultaneous fitting for coupled cases
   or PCHIP on a separately declared grid. Multiple rows may share
   a projected month if the fit identifies every free level.
   [Accepted bootstrap](../calibration-framework/10-sequential-bootstrap.md#accepted-decision-for-sequential-bootstrap-and-final-repricing).

5. **Require explicit positive CPI bounds, seeds and solver controls.**
   Use finite positive native level bounds and aligned seeds. Use
   checked Brent and explicit scan and work controls for bootstrap.
   For simultaneous fitting use open logistic coordinates, checked
   central differences, augmented QR and frozen finite nonnegative
   weights per quote. Keep elected fixed levels outside the optimizer.
   Retire default hard levels, hidden seed-growth bands, quote-rate
   seeds and silent row or grid repair.
   [Accepted scalar search](../calibration-framework/11-scalar-root-search.md#accepted-decision-for-bounded-scalar-search-and-candidate-evidence) ·
   [Accepted weighted global solve](../calibration-framework/12-weighted-global-solve.md#accepted-decision-for-weighted-global-solve-and-final-candidate-gates).

6. **Reprice the exact delivered curve before publication.** Check
   positive levels, the exact origin anchor and actual pricing support
   under the elected literal Flat, Extend or Error tails. Apply any
   economic growth bands separately over explicit unlagged intervals
   on the source clock. Reprice every consumed quote in authored
   order against its native tolerance, including at zero weight and
   for all-fixed fits. Publish the evaluator, typed bindings, frozen
   fixing and release evidence, complete construction record and
   fit report together through the existing Rust-owned interfaces.
   Automatic CPI quote replay requires its own decision. Direct CPI
   parameter shocks retain their accepted contract.
   [Accepted fit reports](../calibration-framework/14-fit-reports.md#accepted-decision-for-fit-acceptance-and-report-semantics) ·
   [Accepted curve validation](../calibration-framework/16-curve-validation.md#accepted-decision-for-curve-validation-and-publication).

## Independent calculation and source checks

- April 15 with a three-month lag and Linear reference interpolation
  uses January CPI `300` and February CPI `306` at weight `14/30`
  on February. The contractual start is `302.8`, while the curve's
  January origin may be `300`.
- An equal-weight reference of `318` fits both monthly level pairs
  `(308, 328)` and `(312, 324)`. One such blended observation does
  not identify both levels.
- At origin CPI `20,000` and time one, the observed seed controls
  give the inverted interval `[18,000, 10,000]`. An isolated Rust
  probe reproduced the resulting `f64::clamp` panic.
- A two-year pay-fixed zero-coupon swap with CPI `200 → 208`, fixed
  rate 2% and factor `.96` gives `−.000384` raw PV per notional.
  Its native tolerance of `.0001` fails at any objective weight.

## Observed choice and domain at the audit revision

Fit CPI levels to quote-built zero-coupon and YoY inflation swap PV/N. The curve origin is the lagged **reference date**, not necessarily valuation date. Calendar-month observed anchors determine quote exposure; projected CPI interpolation remains a curve-level choice. The target accepts month lags or no lag and rejects day-based lags because the output curve cannot represent them.

## Observed algorithm

1. Require inflation quotes, discount curve and finite positive notional/base CPI through public preflight. Quote index must match parameter index or output curve ID. Resolve each inflation convention. When observed InflationIndex exists under curve ID, preflight requires currency, lag and base CPI consistency (`max(1e-8,1e-10*abs(expected))`); target additionally requires interpolation matching that observed index.
2. Effective base CPI is `index.value_on(base_date)` if available, otherwise explicit base CPI. This includes index lag/seasonality; preflight guards differing lag/base inputs. Set reference origin to `base_date.add_months(-lag)` preserving calendar date behavior. Curve stores lag months and selected interpolation (schema default Linear; the core's standalone default differs).
3. Build pay-fixed ZC/YoY contracts with start at valuation base date, explicit base CPI, curve/discount IDs, quote fixed rate, convention day count/interpolation/BDC/calendar and explicit lag. YoY also takes the quote frequency. Same production pricers provide residuals, so no additional convexity model is fitted here.
4. For a month lag, final required anchor is first day of maturity month minus lag; if convention interpolation is Linear and maturity day is after day one, advance one month. With no lag, use maturity directly. The parser distinguishes textual zero lags: `"0M"` normalizes to `None` and uses maturity, while `"00M"` remains `Months(0)` and uses the monthly first-day anchor (advanced one month for Linear interpolation when required). Pillar time is ACT/365F from the reference origin to that required fixing anchor; the stored PreparedQuote pillar date remains contractual maturity.
5. Bootstrap seeds `C0*(1+r_quote)^t`; trial construction removes knots within `abs(t)<=1e-8`, inserts `(0,C0)`, sorts and constructs the curve. Bootstrap scan is the framework default geometric scan; no inflation-specific scan is supplied. CPI knots must be finite and within configurable hard levels (defaults `[1,10000]`).
6. GlobalSolve skips nonpositive/nonfinite times. Seed bounds are `lo=max(C0*0.9^t,cpi_hard_min)` and `hi=min(C0*1.5^t,cpi_hard_max)`; clamp the quote-growth seed to those bounds. Subsequent trial parameters are direct CPI levels constrained only by hard level validation, not the -10%/+50% seed-growth assumptions. Weight is time scheme floored at `1e-3`; fit tolerance defaults `1e-8` PV/N.

## Observed limits and discrepancies

**Observed seed-domain gap:** positive base CPI is not required to lie within configured hard bounds. The global seed path calls `f64::clamp(lo,hi)` without checking `lo<=hi`. For example `C0=20000`, `t=1`, default bounds gives `lo=18000`, `hi=10000`; an inverted interval reaches a panic-capable clamp. The public calibration path was traced statically and the isolated clamp result is recorded above. A finite quote rate below -100% can also create an invalid fractional-power seed; public rate validation currently checks only finiteness.

Observed CPI versus projected level, month anchoring, seasonality and publication eligibility belong jointly to [core observations](../core-market/inflation-observations.md) and [inflation instrument pricing](../valuations-rates/13-inflation-swaps.md). Exact fit of the same pricing kernel does not independently qualify those conventions. Alternatives include fitting annual inflation rates/log CPI, anchor-month parameters, explicit seasonal term structures, and quote-rate residuals. Initial -10%/+50% growth limits are not final global bounds.

## Evidence

- Owning [inflation.rs](../../../../finstack-quant/calibration/src/targets/inflation.rs); inspected [preflight](../../../../finstack-quant/calibration/src/validation/preflight.rs) establishes that differing observed lag/base is rejected publicly.
- [Inflation integration tests](../../../../finstack-quant/calibration/tests/calibration/inflation.rs) locate reference-origin, lagged-fixing, observed-base consistency, no-override repricing, leap-day and day-lag rejection evidence. The selected ZC case in [tests/repricing.rs](../../../../finstack-quant/calibration/src/tests/repricing.rs) builds the same convention-based trade after calibration.

Production calibration tests and independent market comparisons remain pending.

## Follow-up tracking

Reconciliation and qualification work is tracked in the
[calibration-target follow-ups](../FOLLOW_UPS.md#calibration-quotes-and-curvesmile-targets).
