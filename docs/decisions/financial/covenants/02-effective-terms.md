# COVENANTS-02: Threshold schedules, windows and waivers

**Decision status:** Draft — observed implementation; adoption pending for choices not already settled. Existing accepted decisions retain their scope.

**Implementation status:** Observed in the exact source paths identified below, including stated discrepancies and limitations.

**Validation status:** Source and identified test assertions inspected; tests, builds, numerical experiments and external benchmarks not executed. Test existence is not adoption or independent qualification.

**Audit date / revision:** 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. See the [scope inventory and coverage ledger](README.md).

## Ordered selection

1. Validate all top-level and window specifications, even inactive or currently unselected ones. A window requires `start <= end`; any inclusive intersection between two windows is rejected. Windows need not be stored chronologically despite `validate` rustdoc saying they are checked as ordered.
2. On the test date, a matching inclusive `[start,end]` window replaces the complete top-level specification set. It does not append to it. An empty matching window means no tests; outside every window, top-level tests resume. Pairwise nonoverlap makes stored window order immaterial to selection.
3. For an instance label, an active waiver has `effective_date <= date <= expiry_date`, or no expiry. Date-reversed waivers and overlapping inclusive intervals for the same label are rejected; touching endpoints therefore overlap. Unknown/empty waiver IDs are not rejected or resolved to a specification by validation.
4. A full waiver (`amended_threshold=None`) suppresses evaluation and returns a passing report without numeric fields. An amendment selects its finite threshold first; otherwise use the last schedule entry effective on or before the date, then the static covenant threshold.

`ThresholdSchedule::new` sorts effective dates, rejects duplicate dates and nonfinite thresholds, and permits an empty schedule. Its private vector and custom deserializer preserve the same invariant through construction and serde. Lookup is a linear scan retaining the latest effective value; before the first entry it returns `None`, causing static fallback. No interpolation, business-day rolling, calendar lookup, threshold monotonicity, or mandatory step-down direction is applied. Negative schedule/amendment values are accepted when finite. A schedule on a nonnumeric covenant has no numerical effect.

## State, rationale and alternatives

Calendar dates are exact and inclusive, with no time-of-day or contract-notice cut-off. Source comments explicitly assign full-set replacement and amendment precedence. They do not prove that a particular lending agreement uses those choices. Sparse patch windows, half-open intervals, notice-delayed waivers and business-day effectiveness are alternative contracts requiring explicit adoption. Changing a current threshold does not rewrite captured historical consequences; full waivers also do not cure history, as detailed in [COVENANTS-05](05-breach-and-cure.md).

Descriptions are produced from the static `CovenantType` before threshold resolution. Reports/forecasts can consequently display a description containing the original threshold alongside a different effective numeric threshold. This is an observed representational mismatch, not evidence that the numeric selection uses the wrong threshold.

## Evidence

- Owning [schedule constructor/deserializer/lookup](../../../../finstack-quant/covenants/src/schedule.rs), [window and waiver terms](../../../../finstack-quant/covenants/src/engine/types.rs), [validation and resolution](../../../../finstack-quant/covenants/src/engine/covenant_engine.rs), and [forecast effective-term use](../../../../finstack-quant/covenants/src/forward.rs).
- Inspected `threshold_schedule_deserialization_uses_constructor_validation` in [serialization tests](../../../../finstack-quant/covenants/tests/serialization.rs), `window_fallback_to_base_specs_is_explicit` in [engine conventions](../../../../finstack-quant/covenants/tests/engine_conventions.rs), and `forecasts_respect_windows_waivers_and_inactivity` / `window_breach_retains_cure_and_consequences_after_window_expires` in [audit regressions](../../../../finstack-quant/covenants/tests/audit_regressions.rs).

Inspected tests establish intended examples only and were not run.

## Follow-up tracking

Pending choices, implementation reconciliation, and qualification work are tracked
in the [covenant follow-ups](../FOLLOW_UPS.md#covenants).
