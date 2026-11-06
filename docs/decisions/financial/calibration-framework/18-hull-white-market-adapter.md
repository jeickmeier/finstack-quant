# CALIBRATION-FRAMEWORK-018: Dated Hull–White quote preparation and persisted output

- Decision status: **Draft (observed implementation; adoption pending)**.
- Implementation status: **Observed for the stated source scope**; discrepancies and limits are stated below.
- Validation status: **Evidence inspected/located, not executed**. No numerical qualification or external compliance certification is implied.
- Audit date: **2026-11-05**; source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` (see [scope and coverage](README.md)).

## Observed choice and rationale

The envelope adapter translates dated market quotes into the rate-model calibrator's time/schedule contract. It separates discount-curve clock from model Act365F time and always supplies contractual schedules for this path. The underlying analytic/optimization choices are detailed in [rate-model calibration](../calibration-rate-models/README.md).

## Swaption preparation

Filter swaption-vol quotes, validate them, resolve quote leg conventions and require convention currency equal step currency. Accept Normal or unshifted Black; shifted Black fails because no displacement is represented by this HW quote contract. Resolve underlying swap start/end, fixed payment dates/accruals and floating periods. Support Simple or overnight compounding with zero lookback/shift/cutoff only. For compounded coupons the model fixing time is accrual start; for simple coupons it is reset date (fallback accrual start).

Convert expiry, swap start/end, fixed payments and float fixing/accrual/payment dates to model time from step base date. `ModelDiscountCurve` rebases/converts actual discount dates; failed DF lookup becomes NaN for the numeric callback. Require positive expiry/tenor and strike within 1e-8 decimal rate of contractual ATM forward. Initial kappa/sigma must be both present or both absent. Call HW calibration with schedules and annual frequency placeholder (schedules govern actual dates), then persist canonical curve-qualified unitless kappa/sigma scalars.

## Cap/floor preparation

Use discount curve and optional separate forward curve; convert model times to the forward curve's date/day-count clock and divide by its base-date DF. Same-ID forward uses discount DF. Filter cap/floor quotes and skip expired/nonpositive Act365F maturities. Resolve the explicit index, require matching currency and **term** kind; compounded overnight cap calibration is rejected. Build from spot to quote maturity with index payment frequency, ShortBack stub, registry BDC/calendar/day count/payment/reset lags, no accrual adjustment/EOM, then unconditionally omit the first period. Remaining caplets retain fixing/accrual/payment times and accrual fractions.

Scalar mode passes fit tolerance, optional fixed kappa and paired initial guess to the rate-model calibrator. Piecewise mode requires fixed kappa and hardcodes sigma bounds [1e-5,2]; it persists kappa plus a sigma series whose dates are base+round(365×time) days. Initial guess fields are parsed but are not passed to piecewise bootstrap. Both modes have dedicated canonical output keys. Global plan solver/diagnostics settings are not forwarded to these standalone calibration calls; their internal controls apply.

## Discrepancies and alternatives

`HullWhiteStepParams`/model-version prose names Jamshidian, whereas passing schedules selects contractual Gaussian integration. Piecewise schema prose describes expiry knots although the rate-model bootstrap uses live fixing support. These are confirmed documentation discrepancies, not newly selected financial methods. First-period omission assumes market cap quoting excludes that coupon.

Alternatives include a uniform direct API with explicit schedule/clock modes, or additional normal/shifted/overnight contracts; these expand supported financial behavior and remain unadopted.

## Source and validation evidence

- Implementation: [step_runtime.rs](../../../../finstack-quant/calibration/src/step_runtime.rs) — dated preparation, method dispatch, clocks, outputs and limitations.
- Implementation: [api/schema.rs](../../../../finstack-quant/calibration/src/api/schema.rs) — HW parameter/volatility modes and descriptions.
- Implementation: [versions.rs](../../../../finstack-quant/calibration/src/versions.rs) — human-readable HW method label.
- Test evidence: [src/step_runtime.rs](../../../../finstack-quant/calibration/src/step_runtime.rs) — ATM threshold, Act360/Act365F equivalence, timing roles, shifted rejection, term-only caps and piecewise persistence; inspected or located, not executed.
- Reference context: [Hull–White 1990](../../../REFERENCES.md#hull-white-1990-pricing-ird) — model-family context; numerical methods covered by separate rate-model records; a catalogue entry is not proof of implementation equivalence.

## Follow-up tracking

Implementation reconciliation, adoption questions and missing evidence are tracked in the
[calibration-framework follow-ups](../FOLLOW_UPS.md#calibration-orchestration-solvers-and-replay).
