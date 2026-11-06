# CALIBRATION-TARGETS-002 — Rate quote preparation, knot grids and objective weighting

- **Decision status:** Draft — observed implementation; adoption pending.
- **Implementation status:** Observed for the stated source scope; behavior and scope limitations are described below.
- **Validation status:** Source and selected evidence inspected or located; not executed.
- **Audit date / source revision:** 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. Scope and evidence boundaries: [README](README.md).

## Choice and rationale

Targets prepare complete pricing instruments once and reuse a scratch market context while replacing the trial curve. Residuals are generally PV divided by calibration notional, separating numerical scale from trade size. The special forward-deposit residual and model surfaces are documented in their own records. The observed annuity weighting is a proxy, not a measured market PV01 or a bid/ask likelihood.

## Algorithm and controls

1. Extract rate quotes and fail if none. Use explicit curve day count when supplied; otherwise resolve each rate index (futures via contract index) and require identical index day counts. Discount/parametric/XCCY targets explicitly select ACT/365F by default; forward targets infer index day count unless overridden.
2. `BuildCtx(base_date,residual_notional,curve_roles)` receives an optional OIS compounding override. Every quote is prepared with swap payment-delay inclusion enabled. Prepared pillar times are computed from the resolved pillar date on the curve clock. Instrument construction and schedule ownership are in the [framework builders](../calibration-framework/04-deposit-fra-materialization.md), [futures](../calibration-framework/05-futures-materialization.md), and [swaps](../calibration-framework/06-swap-materialization.md).
3. Global scalar-knot targets sort `(time,seed,quote)` together; adjacent times within `1e-10` error. The shared target validator checks matching parameter/time lengths and strictly increasing times above zero; finiteness is additionally checked by the global framework/caller, rather than by this helper alone. Forward control times deliberately use a separate contract that permits origin parameters.
4. Scratch evaluation inserts the current curve by its actual ID, optionally rebinds a credit index, then executes the residual closure. It reuses the full market context; it does not rename a trial curve to every pricing role automatically.
5. For discount/forward weights, let `t+=max(t,1e-6)`, `r=abs(implied_rate)` (nonfinite falls back to zero). The annuity proxy is `A=t+` for `r<1e-8`, otherwise `(1-exp(-r*t+))/r`, floored at `1e-6`. Set `w=max(1e-3, s(t)/A²)`; nonfinite weights fall back to `1e-3`. Scheme factors are Equal `1`, LinearTime `t`, SqrtTime `sqrt(t)`, or InverseDuration `1/max(t,0.1)`. Default is SqrtTime. Hazard/inflation apply only `max(1e-3,s(t))`, without this annuity denominator.

The [global solver](../calibration-framework/12-weighted-global-solve.md) owns how weights enter least squares, convergence, multistart, smoothing and final acceptance. Native `quote_value` diagnostics retain their heterogeneous quote units; they are not the residual denominator. Discount and forward use `N=1,000,000` to limit monetary conversion noise; hazard/inflation take a validated caller notional. Deposit calibration delegates to raw deposit NPV before monetary rounding.

## Alternatives, limits and evidence

True instrument PV01/vega weighting, quote-error residuals, relative errors, and statistical observation weights give different fits. The absolute-rate annuity is not the true annuity in a negative-rate curve; it intentionally makes weighting positive and symmetric in rate sign. Generic scratch reuse makes role IDs and failed trial state material; see the discount alias discrepancy in [CALIBRATION-TARGETS-003](03-discount-calibration.md).

- Owning [util.rs](../../../../finstack-quant/calibration/src/targets/util.rs) is inspected; [config defaults](../../../../finstack-quant/calibration/src/config.rs), [constants](../../../../finstack-quant/calibration/src/constants.rs), [prepared quote builder](../../../../finstack-quant/calibration/src/build/prepared.rs) and [solver contracts](../../../../finstack-quant/calibration/src/solver/traits.rs) are inspected dependencies.
- `test_residual_normalization_invariance` in [discount tests](../../../../finstack-quant/calibration/src/targets/discount.rs), configured forward weight/bounds cases and index day-count deposit equations in [forward tests](../../../../finstack-quant/calibration/src/targets/forward.rs) supply located/selected inspected scaling evidence. They are not independent curve-quality qualification.

## Follow-up tracking

Adoption, reconciliation and qualification work is tracked in the
[calibration-target follow-ups](../FOLLOW_UPS.md#calibration-quotes-and-curvesmile-targets).
