# CALIBRATION-FRAMEWORK-018: Dated Hull–White quote preparation and persisted output

- Decision status: **Accepted 2026-11-15 — complete dated Hull–White quote preparation, fit and publication package**.
- Implementation status: **Observed.** Reconciliation to the accepted package is pending.
- Validation status: **Independent date, rate-basis and sigma support checks verified.** Production pricing, fit, publication, restoration and host qualification remain pending.
- Audit date: **2026-11-05**; source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` (see [scope and coverage](README.md)).

## Accepted decision for dated Hull-White quote preparation and publication

1. **Bind exact quotes, sources and clocks.** Resolve the authored
   quote IDs in order, one currency, the dated conventions and exact
   typed curve roles before solving. A shared projection and discount
   evaluator requires explicit qualification. Use ACT/365F from the
   declared fit origin for model time. Query each curve on its native
   clock, rebase discounts to the fit origin and propagate dated-factor
   errors. Qualify the market-volatility clock independently for
   premium conversion and implied-volatility inversion.
   [Accepted plan inputs](03-dependencies-and-execution.md#accepted-decision-for-calibration-dependencies-preflight-and-ordered-execution) ·
   [Accepted Hull–White clocks and projection](../models-analytic/rate-clock-and-projection.md#accepted-decision-for-checked-model-clocks-curve-fit-and-term-projection).

2. **Use contractual Gaussian integration for dated ATM swaptions.**
   Select the European physical payer quote profile. Prepare actual
   exercise, swap start, index fixing and value dates, coupon accruals
   and payments once on the expressly shared single-curve evaluator.
   Admit qualified term coupons, exact eligible overnight telescoping
   or an explicitly elected continuous overnight approximation.
   Compute ATM from the actual floating PV and positive fixed annuity.
   Treat a supplied ATM strike as an assertion with an explicit
   decimal-rate tolerance, and use the computed ATM basis for pricing
   and inversion. Admit Normal at signed rates and unshifted Black
   at positive rates. Label this method contractual Gaussian integration
   in reports and schema. Remove the synthetic frequency argument and
   reset-date fallback from this adapter.
   [Accepted swaption payoff rules](../valuations-rates/08-hull-white-swaptions.md#accepted-decision-for-checked-hull-white-swaption-exercise-and-risk) ·
   [Accepted ATM quote preparation and inversion](../calibration-rate-models/03-atm-swaption-quotes.md#accepted-decision-for-exact-atm-swaption-targets-and-native-quote-reconstruction) ·
   [Accepted contractual Gaussian integration](../calibration-rate-models/05-contractual-gaussian-integration.md#accepted-decision-for-checked-contractual-gaussian-swaption-integration) ·
   [Henrard’s Hull–White implementation](https://quant.opengamma.io/Hull-White-One-Factor-Model-OpenGamma.pdf).

3. **Select the cap or floor strip explicitly.** Admit flat Normal
   volatility quotes on dated term-index strips with literal cap or
   floor side, signed strike and contractual final maturity. Resolve
   the schedule and first-coupon convention. Include the first coupon
   when the quoted convention includes it and it has a qualified
   positive live horizon. Reject unsupported included observations,
   expired selected quotes and empty strips. This dated fit uses live
   term optionlets.
   [Accepted cap and floor terms](../valuations-rates/05-cap-floor-pricing.md#accepted-decision-for-checked-cap-and-floor-optionlet-pricing) ·
   [Strata's one-tenor-forward quote convention](https://github.com/OpenGamma/Strata/blob/main/modules/pricer/src/main/java/com/opengamma/strata/pricer/capfloor/DirectIborCapletFloorletVolatilityCalibrator.java#L125-L148).

4. **Preserve benchmark and payment roles on both sides of the fit.**
   Keep coupon accrual `A` separate from actual index accrual `αx`.
   Compute the simple forward from the projection-factor ratio at the
   actual benchmark value dates and `αx`. Use the same strip, strike
   and side in market and model pricing. Apply the accepted Hull–White
   payment-measure law and discount once at the actual payment date.
   [Accepted term-optionlet kernel](../models-analytic/hull-white-analytic-kernels.md#accepted-decision-for-checked-hullwhite-analytic-kernels-and-payment-measures) ·
   [Accepted cap and floor calibration strips](../calibration-rate-models/02-cap-schedules-and-quote-inversion.md#accepted-decision-for-checked-cap-and-floor-strips-and-implied-normal-inversion).

5. **Use effective controls for the elected mode.** Require applicable
   settings or a versioned preset. Free two-parameter fits take a
   complete seed or an elected preset. Fixed-κ fits reject unused seed
   pairs. Piecewise fitting requires a fixed κ and explicit
   per-segment branch brackets. Report the actual method, objective and
   work. Reprice every selected quote on the final artifact and retain the signed
   implied-volatility error in its original convention. Gate its
   magnitude on the applicable native Normal or Black tolerance.
   The rate-model records own
   numerical solve and inversion choices.
   [Accepted calibration controls](02-controls-and-market-context.md#accepted-decision-for-calibration-controls-freshness-and-initial-market) ·
   [Accepted fit reports](14-fit-reports.md#accepted-decision-for-fit-acceptance-and-report-semantics) ·
   [Accepted rate-model entry and quote domains](../calibration-rate-models/01-curve-entry-and-quote-domains.md#accepted-decision-for-explicit-hull-white-calibration-entry-and-quote-domains) ·
   [Accepted free two-parameter fit](../calibration-rate-models/06-two-parameter-fit.md#accepted-decision-for-bounded-identified-two-parameter-hull-white-fitting) ·
   [Accepted fixed-κ cap and floor fit](../calibration-rate-models/07-fixed-kappa-cap-fit.md#accepted-decision-for-bounded-fixed-kappa-cap-and-floor-premium-fitting) ·
   [Accepted piecewise sigma bootstrap](../calibration-rate-models/08-piecewise-sigma-bootstrap.md#accepted-decision-for-live-fixing-hullwhite-sigma-bootstrapping) ·
   [Rate-model calibration](../calibration-rate-models/README.md).

6. **Publish a complete fitted family block.** Keep canonical family
   keys, fit origin, curve and index roles, provenance, and exactly
   one scalar sigma or schedule. Store κ per model year and absolute
   decimal short-rate sigma per square root model year. A dated
   piecewise schedule starts at the fit origin and retains the prior
   strips' original last eligible fixing dates as subsequent segment
   starts and the final strip's last eligible fixing as the end of
   calibrated support. It is right-continuous with a flat final segment.
   Numeric schedules retain their model times. Retire numeric-to-date rounding.
   Replacing one form with the other retires the old form atomically.
   Apply the same admission rules on restoration and through valuation,
   Python and WASM consumers.
   [Accepted dated fitted-state rules](../models-analytic/rate-clock-and-projection.md#accepted-decision-for-checked-model-clocks-curve-fit-and-term-projection).

## Independent calculation checks

- For one same-curve term coupon with `D(S)=.99,D(E)=.98,D(P)=.97`
  and fixed accrual `.5` paid at `E`, the dated swap par rate is
  `97/4802`. Telescoping to the index end gives `1/49`, about
  2.08247 bp higher.
- Projection gross `401/400` gives a 1% forward on index accrual
  `90/360`. Coupon accrual `60/360` would give 1.5%. With strike
  1.25% and payment discount .99, the deterministic-limit caplet
  unit PV changes from zero to `33/80000` under that substitution.
- Rounding a synthetic `3/4`-year sigma breakpoint to `274/365`
  moves it by one quarter of a day. For sigma .01 before and .02
  after the breakpoint, the analytic zero-κ limit gives one-year
  state variance `7/40000` on the original grid. Rounding changes
  the variance by `−3/14600000`.

## Observed choice and rationale at the audit revision

The envelope adapter translates dated market quotes into the rate-model calibrator's time/schedule contract. It separates discount-curve clock from model Act365F time and always supplies contractual schedules for this path. The underlying analytic/optimization choices are detailed in [rate-model calibration](../calibration-rate-models/README.md).

## Observed swaption preparation

Filter swaption-vol quotes, validate them, resolve quote leg conventions and require convention currency equal step currency. Accept Normal or unshifted Black; shifted Black fails because no displacement is represented by this HW quote contract. Resolve underlying swap start/end, fixed payment dates/accruals and floating periods. Support Simple or overnight compounding with zero lookback/shift/cutoff only. For compounded coupons the model fixing time is accrual start; for simple coupons it is reset date (fallback accrual start).

Convert expiry, swap start/end, fixed payments and float fixing/accrual/payment dates to model time from step base date. `ModelDiscountCurve` rebases/converts actual discount dates; failed DF lookup becomes NaN for the numeric callback. Require positive expiry/tenor and strike within 1e-8 decimal rate of contractual ATM forward. Initial kappa/sigma must be both present or both absent. Call HW calibration with schedules and annual frequency placeholder (schedules govern actual dates), then persist canonical curve-qualified unitless kappa/sigma scalars.

## Observed cap/floor preparation

Use discount curve and optional separate forward curve; convert model times to the forward curve's date/day-count clock and divide by its base-date DF. Same-ID forward uses discount DF. Filter cap/floor quotes and skip expired/nonpositive Act365F maturities. Resolve the explicit index, require matching currency and **term** kind; compounded overnight cap calibration is rejected. Build from spot to quote maturity with index payment frequency, ShortBack stub, registry BDC/calendar/day count/payment/reset lags, no accrual adjustment/EOM, then unconditionally omit the first period. Remaining caplets retain fixing/accrual/payment times and accrual fractions.

Scalar mode passes fit tolerance, optional fixed kappa and paired initial guess to the rate-model calibrator. Piecewise mode requires fixed kappa and hardcodes sigma bounds [1e-5,2]; it persists kappa plus a sigma series whose dates are base+round(365×time) days. Initial guess fields are parsed but are not passed to piecewise bootstrap. Both modes have dedicated canonical output keys. Global plan solver/diagnostics settings are not forwarded to these standalone calibration calls; their internal controls apply.

## Observed discrepancies and alternatives

`HullWhiteStepParams`/model-version prose names Jamshidian, whereas passing schedules selects contractual Gaussian integration. Piecewise schema prose describes expiry knots although the rate-model bootstrap uses live fixing support. These are confirmed documentation discrepancies, not newly selected financial methods. First-period omission assumes market cap quoting excludes that coupon.

A uniform direct API with schedule and clock modes and additional cap/floor quote conventions were alternatives considered at the audit revision.

## Source and validation evidence

- Implementation: [step_runtime.rs](../../../../finstack-quant/calibration/src/step_runtime.rs) — dated preparation, method dispatch, clocks, outputs and limitations.
- Implementation: [api/schema.rs](../../../../finstack-quant/calibration/src/api/schema.rs) — HW parameter/volatility modes and descriptions.
- Implementation: [versions.rs](../../../../finstack-quant/calibration/src/versions.rs) — human-readable HW method label.
- Test evidence: [src/step_runtime.rs](../../../../finstack-quant/calibration/src/step_runtime.rs) — ATM threshold, Act360/Act365F equivalence, timing roles, shifted rejection, term-only caps and piecewise persistence; inspected or located, not executed.
- Reference context: [Hull–White 1990](../../../REFERENCES.md#hull-white-1990-pricing-ird) — model-family context; numerical methods covered by separate rate-model records; a catalogue entry is not proof of implementation equivalence.

## Follow-up tracking

Implementation reconciliation and missing evidence are tracked in the
[calibration-framework follow-ups](../FOLLOW_UPS.md#calibration-orchestration-solvers-and-replay).
