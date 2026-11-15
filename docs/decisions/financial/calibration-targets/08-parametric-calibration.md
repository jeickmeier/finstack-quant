# CALIBRATION-TARGETS-008 — Calibration on the analytic NS/NSS discount source

- **Decision status:** Accepted 2026-11-15 — complete analytic NS/NSS calibration package.
- **Implementation status:** Observed. Reconciliation to the accepted package is pending.
- **Validation status:** Independent analytic factor, decay repair, identification and native fit checks verified. Production calibration, market references, restoration and host qualification remain pending.
- **Audit date / source revision:** 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. Scope and evidence boundaries: [README](README.md).

## Accepted decision for calibration on the analytic NS/NSS discount source

1. **Use the analytic evaluator throughout the fit.** Elect
   Nelson–Siegel (NS) or Nelson–Siegel–Svensson (NSS), the source
   base date and complete source clock explicitly. Route the actual
   instrument discount role through the checked analytic source
   for every trial, derivative probe and final reprice. Retire the
   daily LogLinear surrogate and dummy parameter-time grid. Keep
   valuation and fixing cutoffs separate from the source origin.
   Apply the analytic domain to every required query, including
   rejection of pre-origin dates.
   [Accepted analytic discount source](../core-market/parametric-discount.md#accepted-decision-validated-nsnss-sources-and-analytic-factor-queries).

2. **Fit live instruments in native residual units.** Freeze exact
   quote IDs, prepared dates and contractual terms. Admit eligible
   deposits and discount-sensitive swaps. Hold independent projections
   fixed. A futures row requires an explicitly qualified overnight link
   under which the candidate supplies actual live projection. Route
   term FRAs to the projection target. Use signed raw PV per exact
   positive notional or the elected unrounded futures points or
   cash scale. Require explicit per-quote weights, mixed-unit scales
   and native fit tolerances.
   [Accepted native rate residuals](02-rate-preparation-and-residuals.md#accepted-decision-for-explicit-rate-preparation-and-native-fit-residuals) ·
   [Accepted projection fit](04-forward-calibration.md#accepted-decision-for-simultaneous-calibration-of-index-projection-factors) ·
   [Accepted linked overnight roles](../calibration-framework/20-linked-ois-replay.md#accepted-decision-for-linked-single-curve-ois-replay-and-publication).

3. **Require explicit bounds and seeds for named parameters.** Use
   signed annual decimal `β` loadings and positive `τ` in source-clock
   years. Elect finite bounds, aligned native seeds and fixed values
   directly or through a named versioned preset. Use the checked open
   logistic map for free bounded parameters. Keep fixed values and
   elected endpoints outside the optimizer. Retire automatic seeds,
   hard-coded bounds and decay repairs.
   [Accepted bounded global solve](../calibration-framework/12-weighted-global-solve.md#accepted-decision-for-weighted-global-solve-and-final-candidate-gates).

4. **Require identifiable NSS decay and curvature parameters.** When a
   decay is free, elect nonoverlapping `τ₁` and `τ₂` ranges in either
   orientation and retain the parameter labels. A fixed decay is a
   single value for this check. If both decays are fixed equal, allow
   at most one free curvature loading. Equal decays remain valid for
   analytic evaluation. Require independent exposure to every free
   parameter in the positive-weight quotes. At equal decays the two
   curvature loadings enter only through their sum. At zero second
   curvature, the second decay has no effect on the curve. Keep
   optional conditioning diagnostics separate from fit acceptance.
   [NSS conditioning study](https://enricoschumann.net/COMISEF/wps031.pdf) ·
   [Accepted sensitivity diagnostics](../calibration-framework/15-sensitivity-diagnostics.md#accepted-decision-for-calibration-sensitivities-and-conditioning).

5. **Elect simultaneous fitting and gate the actual result.** Use
   augmented QR and checked central differences in free solver
   coordinates with explicit tolerances and work limits. Rank
   deterministic restart candidates only after their final quote
   and curve gates pass. Preserve the actual solver stop reason.
   Reprice every consumed quote in authored order against its
   native tolerance, including zero-weight rows. Apply full gates
   to all-fixed fits with zero optimizer work.
   [Accepted deterministic multi-start](../calibration-framework/13-deterministic-multi-start.md#accepted-decision-for-deterministic-multi-start-and-final-selection) ·
   [Accepted fit reporting](../calibration-framework/14-fit-reports.md#accepted-decision-for-fit-acceptance-and-report-semantics).

6. **Publish and restore the fitted analytic model.** Require actual
   pricing support and apply elected economic bands over their declared
   dates and intervals. Publish the exact model variant, parameters,
   source clock, typed bindings, construction record and fit report
   together. Restore the declared discount role and any qualified
   overnight link. Automatic NS/NSS quote replay requires its own
   qualified contract. Direct analytic source shocks retain their
   existing operation.
   [Accepted curve validation](../calibration-framework/16-curve-validation.md#accepted-decision-for-curve-validation-and-publication) ·
   [Accepted construction provenance](../core-market/calibration-provenance.md#accepted-decision-lossless-construction-records-and-request-time-replay-qualification).

## Independent calculation checks

- For NS `(.03, −.02, .01, 1.5)`, the analytic half-day
  factor is about `.99998628270631355`. A LogLinear surrogate
  built from exact day-zero and day-one factors gives about
  `.99998626396415846` on the ACT/365F clock. The difference
  illustrates off-grid behavior.
- With NSS loadings `(.03, −.02, .01, .02)` and `τ₁ = 1`,
  changing `τ₂` from `1.00999` to `1.01001` changes the
  five-year zero by about `.00062749` bp. Under the observed
  repair, the first `τ₂` becomes `1.5` and the change is
  about `12.02646` bp.
- At equal decay horizons, curvature pairs `(.01, .02)` and
  `(.015, .015)` produce identical yields. At unit accrual,
  otherwise identical deposits at 2% and 3% have minimum worst
  absolute residual `1/205` per unit notional across all shared
  discount factors.

## Observed choice and model at the audit revision

Fit Nelson–Siegel (four parameters) or Nelson–Siegel–Svensson (six) directly to rate-instrument PV/N through a simultaneous solve. The core analytic zero/DF formulas are owned by [CORE-MARKET-07](../core-market/parametric-discount.md); this record owns parameter controls, date sampling, residual construction and numerical selection.

## Observed algorithm

1. Prepare rate quotes at N=1,000,000, ACT/365F, one candidate curve ID for discount and projection, and no OIS override. Default NS seed is `(beta0,beta1,beta2,tau)=(.03,-.02,.01,1.5)`; NSS adds beta3=.01 and uses tau1=1.5, tau2=5. A supplied model seed replaces these values.
2. Construct a finite sample-date set containing base date, each deposit's start/maturity, and **every calendar day** through the largest nondeposit prepared pillar. Exclude pre-base dates and convert to ACT/365F. This covers contractual date queries, including payment delays, without imposing a monthly approximation.
3. The framework's dummy parameter-time grid is `1..number_of_parameters`; it is not a maturity grid. Parameter bounds are beta coefficients `[-2,2]` and each tau `[.01,30]` years. Trial building also clamps taus. If NSS taus are within .01, move tau2 up .5 (capped 30), or tau1 down .5 if still too close. This changes the proposed parameter vector discontinuously near coincident taus.
4. Build the analytic ParametricCurve from repaired parameters. For residuals only, materialize analytic `DF(t)` at the date samples as a LogLinear DiscountCurve with Raw nonmonotone/no-floor validation. Insert that curve and reprice every prepared quote; residual is raw PV/N. Under the inspected date-based instrument paths, queried dates are exact samples. The delivered artifact remains the analytic parametric curve.
5. Always call global optimization with custom-Jacobian use disabled, so shared numerical differentiation applies. Default/effective weighting is the global target default (equal, no rate-annuity override here). Acceptance uses discount-curve fit tolerance, default `1e-8` PV/N. No generic rate recipe is attached by this target.

## Observed alternatives and limits

Fitting par yields, zero yields or log DFs instead of PV produces different estimates. Variable projection, transformed positive decay constants, multistart controls and an explicit ordered-tau parameterization are alternatives to clipping/separation repair. Long-horizon daily sampling costs memory and construction work per residual evaluation; interpolation-exactness must be requalified if future pricers query non-date times or beyond prepared pillars. NS/NSS parsimonious models need not fit arbitrary benchmark sets at machine-scale PV tolerance. The core tiny-positive-time cancellation concern remains separate from this fitting adapter.

## Evidence

- Owning [parametric.rs](../../../../finstack-quant/calibration/src/targets/parametric.rs) fully inspected.
- [Parametric integration tests](../../../../finstack-quant/calibration/tests/calibration/parametric.rs) locate inconsistent-quote failure, explicit tolerance, per-notional residual, analytic delivered-model short-deposit and delayed-OIS repricing cases. These compare the delivered model/formulas with generated quotes or same-library cashflows; they are not independent market-fit evidence.
- [Nelson–Siegel](../../../REFERENCES.md#nelson-siegel-1987) and [Svensson](../../../REFERENCES.md#svensson-1994) identify the model family; their presence does not justify the specific seed, bounds, repair or objective.

## Follow-up tracking

Reconciliation and qualification work is tracked in the
[calibration-target follow-ups](../FOLLOW_UPS.md#calibration-quotes-and-curvesmile-targets).
