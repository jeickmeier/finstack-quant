# CASHFLOWS-008 — Compiled floating coupon replay and settlement

Decision status: **Draft — observed implementation, adoption pending**. Implementation status: **Observed**, scoped to `floating_replay.rs` and its shared overnight/rate kernels. Validation status: **Evidence inspected/located, not executed**. Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` ([slice audit](README.md)).

## Choice and supported use

Reusable compiled coupons separate term-rate observation from accrual-start notional capture. This supports simulation/replay consumers whose principal can change between reset and accrual start. A replay state locks one nonnegative notional for one contractual coupon; the builder separately settles balance segments using `settle_index_rate`.

## Compile and state transitions

1. Require accrual end after start, finite positive accrual factor, finite split fractions in `[0,1]`, sum within `1e-12`, and valid rate parameters.
2. Verify accrual factor against the supplied day-count context to relative tolerance `1e-12 × max(|computed accrual|,1)`. For ACT/365L, instead require the factor's implied denominator to be 365 or 366 within `1e-9`; later partial accrual prorates the stored fraction.
3. For overnight observation, require exactly matching accrual endpoints and a constructible accumulator. Daily index bounds in coupon economics must match any explicit economic bound to the observation constraint; then clear economic index bounds to avoid applying them twice. Period constraints are preserved.
4. `observe_term` accepts one finite index fixing only for Term observation. It does **not** capture notional. Repeated fixing errors without replacing the locked value. `capture_notional` accepts one finite nonnegative notional, initializing overnight accumulation if needed; duplicate capture errors.
5. `advance_overnight` requires an overnight coupon and prior notional capture, delegates to the atomic monotone accumulator and preserves the full contractual grid. Term observation cannot be applied to overnight coupons or vice versa.
6. `settle` requires captured notional and a term fixing, or a complete overnight replay. Compute all-in rate with the shared bounds/spread/gearing kernel, then `total=N×accrual_factor×rate`, `cash=total×cash_fraction`, `PIK=total×pik_fraction`. Reject nonfinite results. `settle_index_rate` accepts a supplied constrained/raw index pair and a finite nonnegative segment accrual fraction; it does not itself require that fraction to be at most the full coupon accrual.

## Accrued amount and conventions

`accrued_amount` returns zero on/before accrual start or on/after `max(payment_date,accrual_end)`. Otherwise it accrues through `min(requested_date,accrual_end)`, with contractual calendar/frequency/reference context and termination flag only at the end. ACT/365L preserves its denominator by actual-day proration of the stored full factor. A term coupon requires the locked fixing; an overnight coupon requires accumulator progress exactly through the requested elapsed endpoint, rejecting stale or future replay state. Amount is `N×elapsed_yf×all_in_rate`; this method returns the total coupon amount, without an ex-coupon adjustment or cash/PIK exclusion.

The recorded reset date is metadata: the replay methods do not themselves receive a clock that enforces invocation on that date. The caller orchestrates observation/capture timing. `settle` is observational and can be called repeatedly; it does not consume/reset state. Negative settlement PIK can be returned here; builder emission imposes the separate nonnegative net-capitalization restriction.

## Alternatives and limitations

A changing-notional replay state would be a different API; callers must currently use builder-style segment settlement or compile suitable segments. Schedule-based accrued interest uses different payment/ex-coupon selection rules in [CASHFLOWS-013](13-accrued-interest.md); do not assume it is identical to this path's accrued amount.

## Evidence

- [floating_replay.rs](../../../../finstack-quant/cashflows/src/builder/floating_replay.rs) contains compile, transition, amount and accrued-interest algorithms.
- Located tests there: `term_fixing_precedes_and_does_not_capture_accrual_notional`, `overnight_incremental_state_matches_one_shot_coupon_settlement`, `unfinished_overnight_coupons_cannot_settle`, `overnight_accrued_interest_rejects_stale_and_future_replay_state`, `duplicate_fixing_and_notional_errors_preserve_locked_state`, and ACT/365L denominator examples. These are state/invariant and production-kernel comparisons, not independent pricing-engine validation.
- [ISDA 2006](../../../REFERENCES.md#isda-2006-definitions) and [ARRC](../../../REFERENCES.md#arrc-sofr-users-guide) are relevant convention pointers. No independently adopted replay state machine or error-tolerance standard was located. No tests executed.

## Follow-up tracking

Adoption questions, implementation reconciliation and missing validation are tracked in the
[cashflow follow-ups](../FOLLOW_UPS.md#cashflows).
