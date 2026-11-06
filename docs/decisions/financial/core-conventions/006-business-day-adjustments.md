# CORE-CONVENTIONS-006: Business-day rolling, joint calendars and bounded stepping

- Decision status: **Draft — observed implementation, adoption pending**.
- Implementation status: **Observed**, scoped to `dates/calendar/{business_days,composite}.rs` and business-day methods in `date_extensions.rs`.
- Validation status: **Evidence inspected/located, not executed**. The linked tests establish available evidence, not a passing result or methodology approval.
- Audit date: **2026-11-05**. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; see [scope and inventory](README.md).

## Choice, scope and rationale

Settlement and payment adjustment uses a named convention over one calendar or the intersection of member business days. `BusinessDayConvention::default()` is ModifiedFollowing; schedules do not automatically invoke it because their convention field initially is `None`. The source's rationale is to preserve monthly payment association for modified conventions; contract adoption remains pending.

## Algorithm and conventions

If date is already business, return unchanged. Unadjusted returns it even when closed. Following/Preceding seek forward/backward one calendar day at a time. ModifiedFollowing reverses to Preceding if the forward result changes month; ModifiedPreceding is symmetric. Nearest runs both searches, picks the smallest calendar-day distance and chooses following on ties; if only one direction is representable it uses that one.

The search starts on the supplied date and fails when more than 100 steps have been attempted; it never accepts a business day reached after that limit. Checked date addition prevents date-range overflow. `add_business_days(n)` excludes the starting day, repeats directional seeking for each requested day, and returns the input unchanged for zero; it prechecks that at least `n` calendar days fit. `add_weekdays` handles an initial weekend directionally, then uses whole five-day/seven-day week blocks plus remainder; it excludes holidays by design.

`count_business_days` uses `[start,end)` and returns zero for reversed/equal dates rather than a signed count. Built-ins use cached prefix sums; custom/composite calendars scan.

A joint calendar marks a holiday if any member does, and a business day only if every member does. An empty internal composite behaves weekends-only. Registry expressions `a+b` lowercase/trim, drop empty members, sort and deduplicate, then intern the union of closures; unknown members error. A single deduplicated member resolves directly. `resolve_calendar(None)` and `resolve_joint_calendar(None,None)` explicitly select weekends-only; one supplied joint leg uses that leg alone.

## Selection and controls

The public functions accept an explicit calendar/convention; IDs may be registered composites. Composite IDs round-trip in day-count contexts if every member is restorable. The 100-day search bound is internal and not caller-configurable. This is bounded per consecutive search, not a total 100-day tenor cap.

## Limitations, failure and fallback

Unknown explicit IDs never silently become weekends-only in these helpers. Schedule warning policies can skip adjustment separately (008). Modified rolls compare month, and their fallback is contractual behavior, not a missing-data repair. Generic calendars have no non-Saturday/Sunday built-in weekend configuration. An `is_holiday` result alone is insufficient to determine openness where weekends are not labeled holidays.

## Alternatives and tradeoffs

All six conventions and explicit weekends-only/joint IDs are supported. Alternatives such as union of business days, settlement lags counting the trade day, or unlimited closed-day search are not implemented. Daily scanning is slower for custom calendars but uses their exact supplied predicate.

## Evidence

[Adjustment and registry](../../../../finstack-quant/core/src/dates/calendar/business_days.rs), [joint behavior](../../../../finstack-quant/core/src/dates/calendar/composite.rs), [day stepping](../../../../finstack-quant/core/src/dates/date_extensions.rs). Inline tests `modified_following_is_evaluated_on_the_union_calendar`, `canonical_id_normalizes_union_members`, and `test_add_business_days_error_on_all_holidays` exercise dispatch/failure. [Schedule test](../../../../finstack-quant/core/tests/dates/schedule.rs) `nearest_payment_tie_rolls_following` verifies a concrete tie. These are deterministic boundary examples/invariants, not proof that every caller selected the correct legal calendars. [ISDA 2006](../../../REFERENCES.md#isda-2006-definitions) is the conventions reference.

Calendar completeness limits whether a rolled date can be treated as a certified settlement date.

## Follow-up tracking

Adoption questions, implementation reconciliation and missing evidence are tracked in the
[core-conventions follow-ups](../FOLLOW_UPS.md#core-conventions).
