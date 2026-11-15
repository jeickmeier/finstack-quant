# CALIBRATION-TARGETS-004 — Simultaneous calibration of index projection factors

- **Decision status:** Accepted 2026-11-15 — complete index projection calibration package.
- **Implementation status:** Observed. Reconciliation to the accepted package is pending.
- **Validation status:** Independent accrual, identification and dependency-refit calculations verified. Production calibration, restoration, market references and host qualification remain pending.
- **Audit date / source revision:** 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. Scope and evidence boundaries: [README](README.md).

## Accepted decision for simultaneous calibration of index projection factors

1. **Fit one explicitly identified index projection curve.** Bind
   the output to the exact index, currency, convention and fixing
   series. Freeze the trade date, valuation date and information
   cutoff separately from the projection curve's base date and clock.
   Hold the independent discount source fixed for rows that consume
   it. Deposit growth and futures points require no discount read.
   Shared single-curve OIS stays under its qualified linked contract.
   [Accepted projection source](../core-market/forward-projection.md#accepted-decision-explicit-index-projection-factors-and-contractual-rates) ·
   [Accepted linked OIS roles](../calibration-framework/20-linked-ois-replay.md#accepted-decision-for-linked-single-curve-ois-replay-and-publication) ·
   [Ametrano and Bianchetti on multi-curve construction](https://www.bianchetti.org/finance/bootstrappingtheilliquidity-v1.0.pdf).

2. **Admit quotes with live candidate projection and preserve native
   residuals.** Deposits use `P(S)/P(E) − 1 − rα` on the actual
   benchmark dates and contractual index accrual through checked
   factor arithmetic. Live receive-fixed FRAs use upfront raw PV per
   notional. Still-live futures preserve published observations and
   signed convexity and use unrounded model-minus-observed IMM points
   or the elected cash-per-notional scale. Pay-fixed swaps use floating
   minus fixed raw PV per notional on actual payment dates. Honor
   frozen fixing and observation rules, including term and overnight
   conventions. Fully fixed or settled instruments are ineligible
   as projection observations, including at zero weight.
   [Accepted rate residuals](02-rate-preparation-and-residuals.md#accepted-decision-for-explicit-rate-preparation-and-native-fit-residuals) ·
   [Accepted deposit and FRA preparation](../calibration-framework/04-deposit-fra-materialization.md#accepted-decision-for-deposit-and-fra-materialization-and-prepared-clocks) ·
   [Accepted futures preparation](../calibration-framework/05-futures-materialization.md#accepted-decision-for-listed-rate-futures-calibration-and-reference-periods) ·
   [Accepted swap preparation](../calibration-framework/06-swap-materialization.md#accepted-decision-for-term-and-overnight-swap-calibration).

3. **Declare the fitted knot grid independently of quote dates.**
   Freeze the dated grid directly or through an elected versioned
   policy. Keep the fixed exact `(0, 1)` anchor and require strictly
   increasing positive represented times for the remaining knots.
   Reject distinct knot dates that map to the same time. Preserve
   separate quote rows at a common represented pillar when the fit
   remains identified. Deposit and FRA pillars use resolved accrual
   end, futures use reference end, and swaps use latest actual payment.
   Evaluate actual benchmark endpoints separately. Retire the synthetic
   reset grid, collision-driven knot moves and terminal duplication.

4. **Fit positive factors with explicit bounds, interpolation and
   tails.** Use signed annual continuously compounded projection zeros
   on the source clock and checked `Pᵢ = exp(−zᵢtᵢ)`. Require
   finite explicit bounds and aligned native seeds. Use the accepted
   open logistic map for free rates and keep fixed values and elected
   endpoints outside the solver vector. Reject collapsed free ranges
   and unusable represented factors or probes. Admit signed rates
   and factors above one. Allow Linear, LogLinear, CubicHermite/PCHIP,
   MonotoneConvex and PiecewiseQuadraticForward where they support
   the grid and required queries. Elect literal Flat, Extend or Error
   tails, including for pre-origin queries. Retire currency bounds,
   rate clamps and implicit CubicHermite or terminal-forward behavior.
   [Accepted rate-curve construction](0032-curve-interpolation-and-calibration.md#accepted-decision-for-explicit-rate-curve-interpolation-and-calibration).

5. **Elect a simultaneous solve explicitly and require actual
   identification.** Use the accepted augmented-QR solver with checked
   central differences in free coordinates, explicit tolerances and
   work limits, and frozen `√w × residual` objective rows. Any
   restart follows the elected deterministic policy. Require enough
   positive-weight rows and that they constrain all free directions.
   Keep optional conditioning diagnostics separate from this requirement.
   [Accepted weighted solve](../calibration-framework/12-weighted-global-solve.md#accepted-decision-for-weighted-global-solve-and-final-candidate-gates) ·
   [Accepted deterministic multi-start](../calibration-framework/13-deterministic-multi-start.md#accepted-decision-for-deterministic-multi-start-and-final-selection).

6. **Reprice and publish the exact delivered projection curve.** Bind
   the candidate to the actual output role in every trial, probe
   and final reprice. Gate every consumed row in authored order,
   including zero-weight rows, against its native tolerance. Require
   actual pricing support and apply elected economic bands to
   contractual simple forwards over their declared scope. Apply
   the same gates with zero optimizer work when all parameters
   are fixed. Keep solver status separate from financial acceptance.
   Publish the evaluator, index binding, construction record and report
   atomically. Replay uses the complete recorded fit and refits when
   a used discount dependency changes, even with a zero quote shock.
   [Accepted curve validation](../calibration-framework/16-curve-validation.md#accepted-decision-for-curve-validation-and-publication) ·
   [Accepted rate replay](../calibration-framework/19-rate-quote-replay.md#accepted-decision-for-exact-rate-quote-replay-and-publication).

## Independent calculation checks

- A 1% factor-implied growth over 90 days gives 4% on contractual
  ACT/360. Annualizing on ACT/365F curve time would give about 4.05556%.
- Ratios on `[1,2]`, `[2,3]` and `[1,3]` give growths
  `1/48`, `1/31` and `5/93` for both factor vectors
  `(.98,.96,.93)` and `(.882,.864,.837)`. With the origin
  unqueried by those rows, three residuals leave a free common scale.
- In a two-payment par swap with equal quarterly accruals, fixed
  rate 4%, first floating rate fixed at 3% and first payment factor
  `.99`, changing the second payment factor from `.96` to `.93`
  moves the fitted second forward from 5.03125% to about 5.06452%.
  Reusing the original projection would give `−.00007734375` PV
  per notional despite a zero quote shock.

## Observed choice and inputs at the audit revision

Projection curves fit all quotes simultaneously on actual contractual reset intervals; sequential Bootstrap is explicitly rejected. The retained forward curve uses a dense projection grid for DF chaining and a separate, sparse control-knot grid with approximately one parameter per quote. The source rationale is that off-grid/stub reset periods couple adjacent forwards. Discounting is supplied by an existing discount curve.

`ForwardCurveParams` deserialization defaults to GlobalSolve with numerical derivatives and CubicHermite. The [schema rationale](../../../../finstack-quant/calibration/src/api/schema.rs) identifies smooth shape preservation with zero/negative rates. `InterpStyle::default()` is Linear, distinct from omission of the serialized field. MonotoneConvex is rejected by the forward builder because it operates on discount factors; direct-builder defaults and inferred index conventions belong to [forward projection](../core-market/forward-projection.md).

## Observed algorithm

1. Prepare rates at `N=1,000,000`, with output projection ID, discount dependency and optional OIS override. Clock is explicit or inferred from matching index day counts. Default method is GlobalSolve; preserve the original quotes/clock/role in the replay recipe.
2. Recover deposit start/end, FRA start/maturity, futures resolved reference dates, or swap floating accrual periods. Swap schedules use their frequency/stub/BDC/calendar/EOM/payment lag, no reset-lag adjustment, raw accrual endpoints and no roll rule. Include zero and every interval start/end on the curve clock, require finite `0<=start<end`, sort and deduplicate within `1e-12`.
3. Control times: FRA/futures use interval start; swaps use the last floating accrual start with maturity as collision fallback. Deposits sorted by start/end use their own start for the first common-start deposit and the preceding deposit's end for subsequent deposits. Resolve collisions by moving the new swap, or an existing swap, to its maturity; unresolvable duplicates error. Sort controls and aligned quotes again. Seeds are quote implied rates, including the futures convexity subtraction.
4. Construct a ForwardCurve with selected interpolation (default CubicHermite), numeric tenor, dense projection grid and **FlatZero** extrapolation (constant final forward level). If first control is after `1e-12`, prepend origin with its rate; a single control is duplicated at projection-grid terminal time. Rate bounds are explicit lower/upper vectors from effective currency policy.
5. A projection deposit does not use its discount-only instrument PV. Its residual is `growth - r_quote*alpha_index`, where `growth=rate_between(t_start,t_end)*(t_end-t_start)=Pproj(start)/Pproj(end)-1`. This matches index accrual even when curve time uses a different day count. Other quotes use the instrument raw PV/N. Weights are the [annuity proxy](02-rate-preparation-and-residuals.md), based on prepared pillar maturity rather than control time. The target supplies no custom efficient Jacobian; framework numerical differentiation applies.
6. Solve with the forward-specific acceptance tolerance (default `1e-8` on native unweighted residuals, labeled PV/N in the report despite the deposit growth exception), attach the recipe and mark `forward_parameterization=contractual_reset_grid`.

## Observed limits and alternatives

The deposit residual is undiscounted accrual growth, while other PV/N residuals include discounting; both are dimensionless but not identical quote-error scalings. A dense reset grid does not mean every reset is a free fit parameter. The control-time collision policy is part of the model: unsupported overlapping instruments fail rather than automatically regularizing an overdetermined curve. Controls and weights can produce different interpolated forwards even when all benchmarks fit.

Alternative models include log projection DFs, piecewise forwards per reset interval with regularization, or fitting independent overlapping quote sets by least squares. Their identifiability and smoothness tradeoffs need explicit agreement. The default frozen terminal forward and the difference between point/average/DF-implied forwards follow [core projection behavior](../core-market/forward-projection.md).

## Evidence

- Owning [forward.rs](../../../../finstack-quant/calibration/src/targets/forward.rs) fully inspected, including grid/collision dispatch and residual construction.
- The inspected `deposit_residual_uses_index_day_count_for_accrual` establishes the explicit `365/360` rate conversion. Multiple-deposit control, configured bounds/weights, futures curve-clock, explicit day-count and quote-replay tests are located in the same file. FRA/futures/swaps are repriced through the public flow in [tests/repricing.rs](../../../../finstack-quant/calibration/src/tests/repricing.rs); these share production pricing kernels.

No test execution or independent vendor reconciliation was performed.

## Follow-up tracking

Reconciliation and qualification work is tracked in the
[calibration-target follow-ups](../FOLLOW_UPS.md#calibration-quotes-and-curvesmile-targets).
