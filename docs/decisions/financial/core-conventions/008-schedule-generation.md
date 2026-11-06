# CORE-CONVENTIONS-008: Anchored coupon grids, stubs, payment/fixing lags and failure policy

- Decision status: **Draft — observed implementation, adoption pending**.
- Implementation status: **Observed**, scoped to all four files under `dates/schedule/`.
- Validation status: **Evidence inspected/located, not executed**. The linked tests establish available evidence, not a passing result or methodology approval.
- Audit date: **2026-11-05**. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; see [scope and inventory](README.md).

## Choice, scope and rationale

Schedules keep contractual accrual boundaries unadjusted and compute payment/fixing dates separately. The default spec is monthly, no stubs, no adjustment/calendar, no EOM, no IMM, zero payment lag, no fixing series and Strict error policy. This preserves day-count inputs while allowing settlement adjustments; per-product adoption remains pending.

## Algorithm

Generate each roll as original anchor plus `n * tenor`, then apply Natural, EndOfMonth or ThirdWednesday roll. Do not iterate from a previously clamped date. ShortBack generates forward from start and appends end. No-stub uses that path but requires an exact grid hit at end. ShortFront generates backwards from end and prepends start. LongFront removes the first interior anchor only if the start is off-grid; LongBack merges the last irregular period by inspecting the next-after-next roll. Preserve contractual start/end even when off EOM. Enforce at most 100,000 anchors with checked roll-index/date arithmetic. Afterwards keep only strictly increasing dates, dropping duplicates/backwards values and logging the drop.

Standard IMM mode defaults quarterly/ShortBack; requires whole-quarter frequency, and no-stub endpoints on quarterly third Wednesdays. It selects the first roll on/after start for forward generation, or last roll on/before end for front stubs, preserving external endpoints. If there is no internal roll, return `[start,end]`.

CDS IMM mode also defaults quarterly/ShortBack, but snaps a non-roll start to the strictly previous quarterly 20th, retaining front accrual. It then invokes ordinary Natural generation with supplied end/frequency/stub. Unlike standard IMM, it does not separately reject non-quarter frequencies or off-roll maturities after caller overrides. EOM is rejected for day/week tenors and either IMM mode; both modes cannot coexist. Setter calls switch off the other mode.

For each accrual period end, apply business-day convention, **then** add payment lag in business days. For each period start, fixing lag produces `start - lag business days`; zero lag retains start without adjustment, and absent lag omits the fixing series. Negative lags error. Positive lags require a resolved calendar. Calendar resolution is attempted only when both convention and calendar ID are present; an ID alone does not enable lag calculation.

## Selection, conventions and failure

`ScheduleSpec::new` permits equal endpoints, but generation rejects `start >= end`; Strict therefore errors and GracefulEmpty can return an empty schedule with warning. Public-spec deserialization validates order and mutually exclusive modes; more checks occur on build.

Strict propagates errors. MissingCalendarWarning skips a missing adjustment calendar and records a warning; a positive lag still errors without a calendar. GracefulEmpty converts errors inside `build_impl` to empty date series plus error warning, but outer checks for incompatible EOM/modes and reversed endpoints still error. Monotonic deduplication logs rather than adding a `ScheduleWarning`; payment-date collisions are retained because accrual periods remain distinct.

## Alternatives and tradeoffs

All five stub choices and three error policies are explicit alternatives. Contract-preserving accrual dates differ from adjusting the accrual grid itself. Returning empty schedules is an implemented opt-in behavior, not blanket permission to suppress missing contractual payments. CDS strict quarterly validation and stronger typed stage separation are unimplemented alternatives.

## Evidence

[Builder/lag/fallback ordering](../../../../finstack-quant/core/src/dates/schedule/builder.rs), [generation](../../../../finstack-quant/core/src/dates/schedule/generation.rs), [spec defaults](../../../../finstack-quant/core/src/dates/schedule/spec.rs), [public schedule/warnings](../../../../finstack-quant/core/src/dates/schedule/mod.rs). [Integration tests](../../../../finstack-quant/core/tests/dates/schedule.rs) include anchored month-end drift, all stub variants, finite date boundaries, quarterly IMM validation, payment-lag-after-adjustment, fixing T-minus, and unadjusted CDS 20ths. These primarily assert constructed expected grids and invariants. [ISDA](../../../REFERENCES.md#isda-2006-definitions) and [ICMA](../../../REFERENCES.md#icma-rule-book) are relevant convention references, not confirmation of every override combination.

## Follow-up tracking

Adoption questions, implementation reconciliation and missing evidence are tracked in the
[core-conventions follow-ups](../FOLLOW_UPS.md#core-conventions).
