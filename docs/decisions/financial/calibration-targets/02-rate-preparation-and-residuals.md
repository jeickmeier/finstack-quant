# CALIBRATION-TARGETS-002 — Rate quote preparation, knot grids and objective weighting

- **Decision status:** Accepted 2026-11-15 — complete rate preparation, knot, residual, weighting and repricing package.
- **Implementation status:** Observed. Reconciliation to the accepted package is pending.
- **Validation status:** Independent knot, role-binding, rounding and mixed-unit calculations verified. Production calibration, replay and host qualification remain pending.
- **Audit date / source revision:** 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. Scope and evidence boundaries: [README](README.md).

## Accepted decision for explicit rate preparation and native fit residuals

1. **Freeze the prepared quotes and require an explicit curve clock.**
   Prepare the exact selected quote rows once. Freeze their resolved
   contracts, calendars, fixing cutoffs, payment rules, elected
   overnight compounding and typed curve roles. Require the fitted
   curve's base date, day count and any required calendar context.
   Use the instrument's contractual accrual independently of that
   clock. Deposit and FRA pillars use resolved accrual end, listed
   futures use actual reference-period end, and swaps use the latest
   actual payment across both legs. Validate support for all dates
   pricing queries, including dates beyond the pillar. Retire
   inferred curve day counts, automatic class filtering and generic
   payment-delay selection.
   [Accepted quote selection](01-quote-contracts.md#accepted-decision-for-exact-typed-calibration-quotes-and-native-units) ·
   [Deposit and FRA preparation](../calibration-framework/04-deposit-fra-materialization.md#accepted-decision-for-deposit-and-fra-materialization-and-prepared-clocks) ·
   [Futures preparation](../calibration-framework/05-futures-materialization.md#accepted-decision-for-listed-rate-futures-calibration-and-reference-periods) ·
   [Swap preparation](../calibration-framework/06-swap-materialization.md#accepted-decision-for-term-and-overnight-swap-calibration).

2. **Keep quote rows distinct from fitted knots.** Use a fixed exact
   `(0, 1)` factor anchor and finite, strictly positive, distinct
   free knot times. Where the solver orders the grid, move dates,
   seeds and associated identities together. Preserve authored quote
   order in reports. Remove the generic `1e−10` spacing cutoff and
   reject unresolvable geometry. Sequential fits require one eligible
   quote per new knot. A simultaneous fit may retain multiple rows
   at one represented time against a separately declared grid when
   the target meets its identification requirements. Reject requests
   for two free knots at the same represented time.
   [Accepted rate-curve construction](0032-curve-interpolation-and-calibration.md#accepted-decision-for-explicit-rate-curve-interpolation-and-calibration) ·
   [Accepted bootstrap](../calibration-framework/10-sequential-bootstrap.md#accepted-decision-for-sequential-bootstrap-and-final-repricing).

3. **Bind the exact candidate in every fitted pricing role.** Use an
   isolated market view for each trial, derivative probe and final
   reprice. Preserve independent sources, require explicit authorization
   for shared roles and refresh affected composites before pricing.
   Discard the staged state on an invalid replacement or failed trial.
   Apply the final gates to the exact evaluator and role bindings
   delivered to callers.
   [Accepted preflight and execution](../calibration-framework/03-dependencies-and-execution.md#accepted-decision-for-calibration-dependencies-preflight-and-ordered-execution) ·
   [Linked OIS roles](../calibration-framework/20-linked-ois-replay.md#accepted-decision-for-linked-single-curve-ois-replay-and-publication).

4. **Specify signed residuals and an explicit cash scale.** Where
   preparation or normalization uses notional, require an exact positive
   native-currency amount supplied directly or resolved from an elected
   named versioned preset. Freeze this amount and remove the implicit
   one-million notional. Discount deposits use raw PV of admitted
   funding and repayment cash divided by notional. Projection deposits
   use benchmark growth minus quoted growth, `P(S)/P(E) − 1 − rα`, on
   actual benchmark value dates and contractual accrual. Live FRAs use
   receive-fixed upfront raw PV divided by notional. Pay-fixed swaps
   use floating PV minus fixed PV divided by notional. Futures use the
   elected unrounded model-minus-observed IMM points or their
   cash-per-notional equivalent using the contract multiplier and
   fractional synthetic quantity. Apply contractual cash rounding
   where required before discounting. Omit final PV rounding from
   residuals. Contractual cash rounding can make a normalized
   residual depend on notional.

5. **Require explicit objective weights and scales.** Freeze one
   finite nonnegative effective weight per exact quote ID, supplied
   directly or expanded from an elected named versioned policy.
   For mixed residual units, declare the scales built into those
   weights. A simultaneous fit uses `√w × residual`. Remove the
   generic annuity proxy, default square-root-of-time weighting,
   floors and invalid-weight fallbacks. Objective weights express
   the selected scaling and priority. Financial fit tolerances
   are specified separately in native units.
   [Accepted global solve](../calibration-framework/12-weighted-global-solve.md#accepted-decision-for-weighted-global-solve-and-final-candidate-gates) ·
   [NIST weighted least squares](https://itl.nist.gov/div898/handbook/pmd/section4/pmd432.htm).

6. **Reprice every quote before publication.** Report one finite
   signed native residual per consumed quote in authored order,
   with its exact ID, basis, units and tolerance. Include zero-weight
   rows in unweighted financial acceptance. Keep solver termination,
   quote fit and curve validation distinct. Preserve the prepared
   inputs and effective controls through restoration and replay.
   Target records own their grid and bound choices and hazard and
   inflation weighting.
   [Accepted fit reports](../calibration-framework/14-fit-reports.md#accepted-decision-for-fit-acceptance-and-report-semantics) ·
   [Accepted curve checks](../calibration-framework/16-curve-validation.md#accepted-decision-for-curve-validation-and-publication) ·
   [Accepted rate replay](../calibration-framework/19-rate-quote-replay.md#accepted-decision-for-exact-rate-quote-replay-and-publication).

## Independent calculation checks

- On 30U/360, January 30 to March 30 and January 30 to March 31
  both give `60/360`. The two dated observations may remain distinct
  rows while a fit uses one declared coordinate at that time.
- A unit deposit with 4% interest and accrual of one has zero
  residual with trial factor `25/26`. The delivered factor `.96`
  instead gives `−.0016`. Final repricing through the trial binding
  could therefore admit the wrong curve.
- At a quarter-year accrual, fixed rate `.04`, projected rate
  `.040016`, factor `.96` and elected nearest-cent coupon
  rounding, the swap residual is zero at notional 1,000 and
  `.0000048` at notional 2,000.
- A one-basis-point move can produce `.000025` in quarterly deposit
  growth, `.000097` in swap PV per unit notional with an annuity
  of `.97`, and `−.01` futures points. Equal numerical weights give
  the futures move 160,000 times the squared contribution of the
  deposit move.

## Observed choice and rationale at the audit revision

Targets prepare complete pricing instruments once and reuse a scratch market context while replacing the trial curve. Residuals are generally PV divided by calibration notional, separating numerical scale from trade size. The special forward-deposit residual and model surfaces are documented in their own records. The observed annuity weighting is a proxy, not a measured market PV01 or a bid/ask likelihood.

## Observed algorithm and controls

1. Extract rate quotes and fail if none. Use explicit curve day count when supplied; otherwise resolve each rate index (futures via contract index) and require identical index day counts. Discount/parametric/XCCY targets explicitly select ACT/365F by default; forward targets infer index day count unless overridden.
2. `BuildCtx(base_date,residual_notional,curve_roles)` receives an optional OIS compounding override. Every quote is prepared with swap payment-delay inclusion enabled. Prepared pillar times are computed from the resolved pillar date on the curve clock. Instrument construction and schedule ownership are in the [framework builders](../calibration-framework/04-deposit-fra-materialization.md#accepted-decision-for-deposit-and-fra-materialization-and-prepared-clocks), [futures](../calibration-framework/05-futures-materialization.md#accepted-decision-for-listed-rate-futures-calibration-and-reference-periods), and [swaps](../calibration-framework/06-swap-materialization.md#accepted-decision-for-term-and-overnight-swap-calibration).
3. Global scalar-knot targets sort `(time,seed,quote)` together; adjacent times within `1e-10` error. The shared target validator checks matching parameter/time lengths and strictly increasing times above zero; finiteness is additionally checked by the global framework/caller, rather than by this helper alone. Forward control times deliberately use a separate contract that permits origin parameters.
4. Scratch evaluation inserts the current curve by its actual ID, optionally rebinds a credit index, then executes the residual closure. It reuses the full market context; it does not rename a trial curve to every pricing role automatically.
5. For discount/forward weights, let `t+=max(t,1e-6)`, `r=abs(implied_rate)` (nonfinite falls back to zero). The annuity proxy is `A=t+` for `r<1e-8`, otherwise `(1-exp(-r*t+))/r`, floored at `1e-6`. Set `w=max(1e-3, s(t)/A²)`; nonfinite weights fall back to `1e-3`. Scheme factors are Equal `1`, LinearTime `t`, SqrtTime `sqrt(t)`, or InverseDuration `1/max(t,0.1)`. Default is SqrtTime. Hazard/inflation apply only `max(1e-3,s(t))`, without this annuity denominator.

The [accepted global solver](../calibration-framework/12-weighted-global-solve.md#accepted-decision-for-weighted-global-solve-and-final-candidate-gates) owns weighting and final candidate gates. Native `quote_value` diagnostics retain their heterogeneous quote units; they are not the residual denominator. Discount and forward use `N=1,000,000` to limit monetary conversion noise; hazard/inflation take a validated caller notional. Deposit calibration delegates to raw deposit NPV before monetary rounding.

## Observed alternatives, limits and evidence

True instrument PV01/vega weighting, quote-error residuals, relative errors, and statistical observation weights give different fits. The absolute-rate annuity is not the true annuity in a negative-rate curve; it intentionally makes weighting positive and symmetric in rate sign. Generic scratch reuse makes role IDs and failed trial state material; see the discount alias discrepancy in [CALIBRATION-TARGETS-003](03-discount-calibration.md).

- Owning [util.rs](../../../../finstack-quant/calibration/src/targets/util.rs) is inspected; [config defaults](../../../../finstack-quant/calibration/src/config.rs), [constants](../../../../finstack-quant/calibration/src/constants.rs), [prepared quote builder](../../../../finstack-quant/calibration/src/build/prepared.rs) and [solver contracts](../../../../finstack-quant/calibration/src/solver/traits.rs) are inspected dependencies.
- `test_residual_normalization_invariance` in [discount tests](../../../../finstack-quant/calibration/src/targets/discount.rs), configured forward weight/bounds cases and index day-count deposit equations in [forward tests](../../../../finstack-quant/calibration/src/targets/forward.rs) supply located/selected inspected scaling evidence. They are not independent curve-quality qualification.

## Follow-up tracking

Reconciliation and qualification work is tracked in the
[calibration-target follow-ups](../FOLLOW_UPS.md#calibration-quotes-and-curvesmile-targets).
