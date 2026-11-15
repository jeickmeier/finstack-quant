# CALIBRATION-FRAMEWORK-006: Term and overnight swap construction

- Decision status: **Accepted 2026-11-15 — complete term and overnight swap construction, quote pillar and replay package**.
- Implementation status: **Observed for the stated source scope.** Reconciliation to the accepted contract is pending.
- Validation status: **Independent date, coupon, raw PV and quote-bump checks completed.** Production pricing, calibration, replay and host qualification remain pending.
- Audit date: **2026-11-05**; source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` (see [scope and coverage](README.md)).

## Accepted decision for term and overnight swap calibration

1. **Materialize one qualified pay-fixed swap.** Require positive exact
   notional in the contract currency, no principal exchange, the original
   annual Decimal fixed quote and an explicit signed annual Decimal
   floating spread. Persist literal zero for a no-spread contract,
   including after expansion of an elected preset. Convert the spread once
   to fractional Decimal bp by multiplying by 10,000 in checked Decimal
   arithmetic. Bind the exact index, qualified dated convention, and
   typed discount and projection roles. A shared evaluator requires
   explicit qualification for both roles.
   [Accepted IRS contract](../valuations-rates/03-interest-rate-swaps.md#accepted-decision-for-checked-interest-rate-swap-cashflows-and-risk).

2. **Resolve each leg once.** Derive spot from trade date and the qualified
   settlement rule. Distinguish a spot-relative tenor, nominal date subject
   to the elected roll, and exact effective date. Resolve each leg's
   frequency, day-count references, stub, roll/EOM, accrual, payment and
   fixing calendars, and applicable lags. Remove EOM inference from the
   adjusted spot date and the hard-coded short-front stub. Preserve
   nominal maturity, adjusted accrual, fixing and observation coordinates,
   and actual payments as distinct dates. Use the accepted IRS adjustment
   and lag order and fail on invalid or conflicting terms.
   [Contractual schedule grids](../core-conventions/008-schedule-generation.md) ·
   [Strata payment schedules](https://strata.opengamma.io/apidocs/com/opengamma/strata/product/swap/PaymentSchedule.html).

3. **Use one exact term observation per floating coupon.** Term cash is
   `N × A × (F + s)`, where `N` is notional, `A` is the coupon fraction,
   `F` is the annual benchmark rate and `s` is the annual spread in decimal
   units. Use unit gearing and no cap or floor. Project from the
   benchmark's own full value interval and accrual. For term and overnight
   use eligible published rates at the qualified information cut and project
   only expressly unpublished observations through the exact active source.
   Fail on missing required fixing or publication data, or failed projection.
   [Accepted term observations](../cashflows/06-floating-rate-projection.md#accepted-decision-resolved-term-observations-exact-rate-controls-and-strict-failures).

4. **Use the accepted overnight compounding elections.** Require
   compounding in arrears with explicit lookback, observation shift, or
   rate cutoff. Scope an explicit step override to eligible overnight
   rows, validate it in preflight and freeze the effective election.
   Reject simple compounding, arithmetic averaging and combined observation
   offsets. Retain full benchmark quote intervals, actual partial-block
   weights and the accepted shift/cutoff geometry. With weights `w_i`
   and rates `r_i`, use `N × (∏(1 + r_i w_i) − 1 + sA)`. Apply spread
   outside the compound product. Payment delay changes the cash date
   independently of observation geometry.
   [Accepted overnight projection](../valuations-framework/overnight-projection.md#accepted-decision-for-checked-overnight-projection-and-quote-risk) ·
   [ARRC SOFR guide](https://www.newyorkfed.org/medialibrary/Microsites/arrc/files/2021/users-guide-to-sofr2021-update.pdf).

5. **Use the latest actual payment as the prepared quote pillar.** For
   discount and projection calibration, take the maximum payment date
   across both resolved leg schedules. Cache this date and its positive
   finite year fraction on the target's explicit base date, clock and
   context. Remove the payment-delay boolean. Both leg lags apply in all
   valuations. Feed the forward target the same prepared value intervals
   for observations requiring projection. Check source coverage at all
   required dates. The targets own solver knot placement, residual
   normalization, weighting and fit acceptance.
   [Rate preparation and residuals](../calibration-targets/02-rate-preparation-and-residuals.md).

6. **Reuse the prepared swap through trial, final repricing and replay.**
   Use the accepted IRS cash ledger and signed native raw PV, floating
   minus fixed, at the admitted cut. Preserve contractual cash rounding
   and evaluate dated discount factors at actual eligible payments.
   Require genuine dependence on the candidate curve. A fully fixed
   swap can still carry discount exposure through its eligible cash.
   Replay matches exact quote identity and preserves authored terms,
   resolved schedules, the publication cut, known fixings,
   effective compounding and typed role bindings. A swap-rate bump
   changes only the fixed quote. Fail on invalid or incomplete replay.
   Use the same Rust admission in Python, WASM and schemas.
   [Accepted replay provenance](../core-market/calibration-provenance.md#accepted-decision-lossless-construction-records-and-request-time-replay-qualification).

## Independent calculation checks

- With weekdays-only spot settlement, February 26, 2025 plus two
  business days gives February 28. Explicit 3M EOM gives May 31 nominal
  maturity and May 30 modified-following accrual end. A one-business-day
  float lag gives June 2. A two-business-day fixed lag on a calendar
  with a June 2 holiday gives June 4. The prepared pillar is June 4,
  with ACT/365F time `98/365` from February 26.
- For 1 million notional and a `90/360` coupon fraction, fixed 4% and
  term 3.9% plus 10 bp give two 10,000 coupons. Dated float and fixed
  factors 0.97 and 0.96 give raw pay-fixed PV +100.
  A 1 bp increase in the fixed quote at the same trial curves gives +76.
- A Saturday-to-Tuesday overnight coupon with 1 million notional uses
  5% at `2/360` from the clipped Friday-to-Monday block and 4% at
  `1/360` from Monday to Tuesday. With 10 bp outside spread and actual
  coupon fraction `3/365`, cash before contractual rounding is 397.138931.

## Observed choice and rationale

Swap calibration uses explicit fixed and floating legs driven by one rate-index convention. It retains payment/reset lags and compounding rather than treating every swap as a simple terminal exchange. The shared instrument path is a consistency choice, with independent evidence still required.

## Input and algorithm

Resolve spot from as-of, settlement lag and calendar. Resolve maturity from spot-relative tenor or adjusted explicit date. The optional `swap_use_payment_delay` flag moves the **calibration pillar** to the final payment date; the actual leg payment lags are populated in either case. Targets choose the flag, so observation maturity and calibration support need not coincide.

Fixed frequency/day count and float payment frequency/day count are read from the registry. Both legs use the market calendar/BDC, ShortFront stub and payment lag; EOM is true exactly when adjusted start equals its calendar month end. The float leg also receives reset lag and fixing calendar. Quote rate converts to Decimal fixed coupon. A Pay-side swap with positive notional pays fixed and receives floating under the instrument convention.

Term indices always select Simple compounding. Overnight indices use an explicit step override if present, otherwise require registry `ois_compounding`. Simple is rejected for overnight indices; other supplied compounding forms are passed through. The optional quote spread is decimal and becomes float-leg bp by multiplication by 10000. Missing spread leaves zero.

Prepared quotes cache the instrument and target-clock pillar so solver iterations do not rebuild schedules. The actual target's curve roles, solver residual normalization and payment-lag pillar choice are in [calibration targets](../calibration-targets/README.md).

## Failures and alternatives

Missing registry records, bad calendars, invalid monetary/decimal inputs, absent curve roles or invalid dates fail construction. No business-day or compounding fallback is invented. Target-specific replay must preserve the OIS override and all observed fixings.

An analytic par-swap sum would be simpler to differentiate, while shared valuation instruments preserve richer schedule features.

## Source and validation evidence

- Implementation: [build/rates.rs](../../../../finstack-quant/calibration/src/build/rates.rs) — swap schedules, compounding selection, spread conversion and lagged pillar.
- Implementation: [build/context.rs](../../../../finstack-quant/calibration/src/build/context.rs) — OIS override carriage.
- Implementation: [build/prepared.rs](../../../../finstack-quant/calibration/src/build/prepared.rs) — preparation flag and curve-clock caching.
- Test evidence: [src/build/rates.rs](../../../../finstack-quant/calibration/src/build/rates.rs) — swap conventions/override and pillar tests located; inspected or located, not executed.
- Test evidence: [src/build/tests/rates.rs](../../../../finstack-quant/calibration/src/build/tests/rates.rs) — IRS field assertions; inspected or located, not executed.
- Test evidence: [src/recalibration/rates.rs](../../../../finstack-quant/calibration/src/recalibration/rates.rs) — SOFR cutoff replay and observed-fixing preservation regressions; inspected or located, not executed.
- Reference context: [ISDA 2021 definitions](../../../REFERENCES.md#isda-2021-definitions) — external convention context only; a catalogue entry is not proof of implementation equivalence.

## Follow-up tracking

Implementation reconciliation and missing evidence are tracked in the
[calibration-framework follow-ups](../FOLLOW_UPS.md#calibration-orchestration-solvers-and-replay).
