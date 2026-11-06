# CALIBRATION-TARGETS-008 — NS/NSS calibration through exact-date discount samples

- **Decision status:** Draft — observed implementation; adoption pending.
- **Implementation status:** Observed for the stated source scope; behavior and scope limitations are described below.
- **Validation status:** Source and selected evidence inspected or located; not executed.
- **Audit date / source revision:** 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. Scope and evidence boundaries: [README](README.md).

## Choice and model

Fit Nelson–Siegel (four parameters) or Nelson–Siegel–Svensson (six) directly to rate-instrument PV/N through a simultaneous solve. The core analytic zero/DF formulas are owned by [CORE-MARKET-07](../core-market/parametric-discount.md); this record owns parameter controls, date sampling, residual construction and numerical selection.

## Algorithm

1. Prepare rate quotes at N=1,000,000, ACT/365F, one candidate curve ID for discount and projection, and no OIS override. Default NS seed is `(beta0,beta1,beta2,tau)=(.03,-.02,.01,1.5)`; NSS adds beta3=.01 and uses tau1=1.5, tau2=5. A supplied model seed replaces these values.
2. Construct a finite sample-date set containing base date, each deposit's start/maturity, and **every calendar day** through the largest nondeposit prepared pillar. Exclude pre-base dates and convert to ACT/365F. This covers contractual date queries, including payment delays, without imposing a monthly approximation.
3. The framework's dummy parameter-time grid is `1..number_of_parameters`; it is not a maturity grid. Parameter bounds are beta coefficients `[-2,2]` and each tau `[.01,30]` years. Trial building also clamps taus. If NSS taus are within .01, move tau2 up .5 (capped 30), or tau1 down .5 if still too close. This changes the proposed parameter vector discontinuously near coincident taus.
4. Build the analytic ParametricCurve from repaired parameters. For residuals only, materialize analytic `DF(t)` at the date samples as a LogLinear DiscountCurve with Raw nonmonotone/no-floor validation. Insert that curve and reprice every prepared quote; residual is raw PV/N. Under the inspected date-based instrument paths, queried dates are exact samples. The delivered artifact remains the analytic parametric curve.
5. Always call global optimization with custom-Jacobian use disabled, so shared numerical differentiation applies. Default/effective weighting is the global target default (equal, no rate-annuity override here). Acceptance uses discount-curve fit tolerance, default `1e-8` PV/N. No generic rate recipe is attached by this target.

## Alternatives and limits

Fitting par yields, zero yields or log DFs instead of PV produces different estimates. Variable projection, transformed positive decay constants, multistart controls and an explicit ordered-tau parameterization are alternatives to clipping/separation repair. Long-horizon daily sampling costs memory and construction work per residual evaluation; interpolation-exactness must be requalified if future pricers query non-date times or beyond prepared pillars. NS/NSS parsimonious models need not fit arbitrary benchmark sets at machine-scale PV tolerance. The core tiny-positive-time cancellation concern remains separate from this fitting adapter.

## Evidence

- Owning [parametric.rs](../../../../finstack-quant/calibration/src/targets/parametric.rs) fully inspected.
- [Parametric integration tests](../../../../finstack-quant/calibration/tests/calibration/parametric.rs) locate inconsistent-quote failure, explicit tolerance, per-notional residual, analytic delivered-model short-deposit and delayed-OIS repricing cases. These compare the delivered model/formulas with generated quotes or same-library cashflows; they are not independent market-fit evidence.
- [Nelson–Siegel](../../../REFERENCES.md#nelson-siegel-1987) and [Svensson](../../../REFERENCES.md#svensson-1994) identify the model family; their presence does not justify the specific seed, bounds, repair or objective.

## Follow-up tracking

Adoption, reconciliation and qualification work is tracked in the
[calibration-target follow-ups](../FOLLOW_UPS.md#calibration-quotes-and-curvesmile-targets).
