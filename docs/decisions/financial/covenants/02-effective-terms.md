# COVENANTS-02: Threshold schedules, windows and waivers

**Decision status:** Accepted 2026-11-17 for replacement windows, effective thresholds, waiver admission and effective-term reporting.

**Implementation status:** Replacement windows, inclusive dates and numeric threshold precedence are implemented. Waiver applicability checks, explicit waived status and effective-term reporting remain pending.

**Validation status:** The 115-test Covenants Rust baseline passed on 2026-11-17, including existing window, waiver, forecast and schedule-restoration tests. The accepted admission and reporting changes still need qualification.

**Audit date / revision:** 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. See the [scope inventory and coverage ledger](README.md).

## Accepted decision

1. Treat each inclusive window as a complete replacement of the base set. An empty window selects no tests. Resume the base set outside windows. Reject reversed or overlapping windows, including shared endpoints. Window input order has no effect on selection. Reuse an exact label across sets only for the same continuing obligation.
2. Use the same resolution rules for assessment and forecast. For an applicable test, a full waiver suppresses evaluation and yields a waived outcome. Otherwise use the active amendment, the latest scheduled threshold effective on or before the test date, or the static threshold, in that priority order. Schedule dates are unique. An amendment applies through its inclusive expiry date, then ordinary threshold selection resumes.
3. Require each waiver to name an exact label that appears in an effective set during at least part of the waiver interval. Reject overlapping waivers for that label, reversed date ranges and threshold amendments for nonnumeric tests. Treat an omitted expiry as open-ended. Select a waiver on dates when both its interval and the named obligation apply.
4. Report the effective threshold and its source in assessments and forecasts. Descriptions must agree with the effective numeric terms shown. Preserve captured episode terms when resolving later test dates. [COVENANTS-05](05-breach-and-cure.md) owns the effect of later events on cure and breach history.

## Observed selection

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

These tests passed in the Rust baseline. They cover current behavior. The accepted waiver-applicability and effective-reporting cases remain to be tested.

## Follow-up tracking

Implementation reconciliation and qualification are tracked
in the [covenant follow-ups](../FOLLOW_UPS.md#covenants).
