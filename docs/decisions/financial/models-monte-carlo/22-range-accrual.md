# MODELS-MONTE-CARLO-22: Observation-count range accrual with history and carry multipliers

- Decision status: Draft — observed implementation; adoption pending.
- Implementation status: Observed in payoff/range_accrual.rs.
- Validation status: Runtime inspected and named test evidence located; selected test bodies inspected; tests not executed.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope and coverage: [slice inventory](README.md), [file ledger](coverage.json).

## Choice and domain

Range accrual pays notional*coupon_rate*(observations in range/observations seen), including historical counts. It counts observations equally, not day-weighted accrual; coupon_rate is the total coupon fraction for this payoff period, not automatically annualized by elapsed time. Bounds are inclusive. Observation times must be finite/nonnegative/nondecreasing, and duplicates represent distinct observations. Past in-range count cannot exceed total history.

## Algorithm and controls

Process every date reached by current event time within 1e-6 years, multiply event spot by its positive finite per-observation multiplier, and count it in range when lower≤scaled spot≤upper. Multiple observations at one step retain separate multipliers. Reset clears simulated counts/cursor while preserving history. No observations returns zero. Constructor requires lower<upper and a nonnegative, non-NaN coupon but does not reject positive-infinite coupon or all infinite bounds. Runtime checks date/multiplier lengths after public date mutation.

## Failure behavior and alternatives

If SPOT is absent, processing silently stops until a later event; nonfinite SPOT is counted as an observation but normally fails range comparisons. The denominator includes only observations actually consumed, so missing/unreached observations can materially change the payout fraction. This differs from other payoff families' finite-state errors. Coarse grids use the later endpoint for skipped dates. Day-weighted accrual, missing-fixing policies and exact observation grids are alternative contracts, not automatic fallbacks.

## Evidence

[Implementation/tests](../../../../finstack-quant/models/src/monte_carlo/payoff/range_accrual.rs) cover boundary inclusivity, history/reset, duplicate-date multipliers, coarse-step counts and malformed inputs. No independent financial standard for this counting/missing-observation policy is established. [Overhaus](../../../REFERENCES.md#overhaus-2007-equity-derivatives) is general context, not contract approval.

## Follow-up tracking

Adoption questions, implementation reconciliation and validation work are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#monte-carlo-models).
