# CALIBRATION-FRAMEWORK-006: Term and overnight swap construction

- Decision status: **Draft (observed implementation; adoption pending)**.
- Implementation status: **Observed for the stated source scope**; discrepancies and limits are stated below.
- Validation status: **Evidence inspected/located, not executed**. No numerical qualification or external compliance certification is implied.
- Audit date: **2026-11-05**; source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` (see [scope and coverage](README.md)).

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
- Test evidence: [src/build/tests_rates.rs](../../../../finstack-quant/calibration/src/build/tests_rates.rs) — IRS field assertions; inspected or located, not executed.
- Test evidence: [src/recalibration/rates.rs](../../../../finstack-quant/calibration/src/recalibration/rates.rs) — SOFR cutoff replay and observed-fixing preservation regressions; inspected or located, not executed.
- Reference context: [ISDA 2021 definitions](../../../REFERENCES.md#isda-2021-definitions) — external convention context only; a catalogue entry is not proof of implementation equivalence.

## Follow-up tracking

Implementation reconciliation, adoption questions and missing evidence are tracked in the
[calibration-framework follow-ups](../FOLLOW_UPS.md#calibration-orchestration-solvers-and-replay).
