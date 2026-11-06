# CORE-CONVENTIONS-002: Coupon reference periods for ACT/ACT ICMA and ACT/365L

- Decision status: **Draft — observed implementation, adoption pending**.
- Implementation status: **Observed**, scoped to `dates/daycount/{act_act,other,context}.rs` and `Tenor::to_years_with_context`.
- Validation status: **Evidence inspected/located, not executed**. The linked tests establish available evidence, not a passing result or methodology approval.
- Audit date: **2026-11-05**. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; see [scope and inventory](README.md).

## Choice, scope and rationale

Coupon-sensitive accrual uses explicit contractual reference boundaries. The source distinguishes full regular periods from stubs and partial accrued interest; replacing absent coupon information with an approximate year basis is not implemented. The source cites [ICMA Rule Book](../../../REFERENCES.md#icma-rule-book); adoption and the exact contractual applicability remain pending.

## Algorithm

**ACT/ACT ICMA/ISMA:** frequency is mandatory even when an explicit reference period is supplied. Without reference boundaries, only month/year tenors are supported. Generate `start + k * tenor_months` from the original start, for at most 512 steps; accept only if a boundary equals end, returning the sum of `tenor_months/12`. An end between grid points requires an explicit coupon period.

With reference boundaries `(a,b)`, require `a < b`; infer nominal months `m = round(12*(b-a).days/365)` and reject zero. Use the endpoint with the larger day-of-month as the unclamped anchor (start wins ties). If both endpoints are month-end, snap generated boundaries to month-end. Regenerate boundary indices 0 and 1 and reject if they do not reproduce `(a,b)`. Walk backwards/forwards to the period containing start, then sum `overlap_actual_days / reference_segment_actual_days * m/12` over quasi-coupons. Absolute boundary indices must remain below 512; date and offset overflow error. In this branch the supplied frequency is required but does not determine `m` or get cross-checked against it.

**ACT/365L:** require frequency and an enclosing strictly increasing coupon period with the requested accrual inside it. Annual means exactly `1Y` or `12M`: use denominator 366 if February 29 is in `(coupon_start,coupon_end]`, else 365. Every other frequency uses 366 if coupon end is in a leap year, otherwise 365. Return actual accrual days divided by that coupon denominator. A partial accrual keeps the enclosing coupon's denominator; multi-coupon accrual requires external slicing.

## Selection, conventions and controls

`DayCountContext` carries optional frequency, coupon period, calendar, basis and termination flag. Boundaries for ICMA are unadjusted nominal coupon dates, not adjusted payment dates. For `Tenor::to_years_with_context`, ICMA reference end is the unadjusted tenor end, while the measured accrual end is optionally business-day adjusted; other conventions receive the adjusted end as coupon end. The helper returns zero if adjustment moves end to/before start.

## Limitations and failure

ICMA reference periods inferred by rounded actual days have no independent frequency-consistency validation. The explicit-reference branch permits a supplied day/week frequency because that frequency is not used after presence checking. ACT/365L similarly treats any nonannual tenor as the nonannual rule. Neither convention silently switches to ACT/365F. Both become unusable for nonempty XIRR date intervals through the context-free XIRR signature; see [017](017-irr.md).

## Alternatives and tradeoffs

Frequency-only regular ICMA and explicit-reference quasi-coupon ICMA are both supported. Exact schedule metadata avoids ambiguity but creates a caller obligation. A general schedule-driven accrual engine or a frequency-consistency rejection would change the current contract and has not been adopted.

## Evidence

[ICMA implementation](../../../../finstack-quant/core/src/dates/daycount/act_act.rs), [365L implementation](../../../../finstack-quant/core/src/dates/daycount/other.rs), [context](../../../../finstack-quant/core/src/dates/daycount/context.rs), [tenor bridge](../../../../finstack-quant/core/src/dates/tenor.rs). Inspected inline tests include `act_act_isma_reference_period_preserves_eom_anchor_across_stubs`, `act_act_isma_coupon_period_mid_coupon_accrual`, and the three February-29 ACT/365L boundary cases; [day-count integration tests](../../../../finstack-quant/core/tests/dates/daycount.rs) provide additional expected fractions. [Stored reference cases](../../../../finstack-quant/core/tests/golden/data/daycount_quantlib.json) are finite fixtures, not independent verification of every accepted context.

The input behaviors above are observed implementation behavior, not methodology
approval.

## Follow-up tracking

Adoption questions, implementation reconciliation and missing evidence are tracked in the
[core-conventions follow-ups](../FOLLOW_UPS.md#core-conventions).
