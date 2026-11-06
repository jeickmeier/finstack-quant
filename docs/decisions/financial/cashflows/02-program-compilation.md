# CASHFLOWS-002 — Coupon and payment program compilation

Decision status: **Draft — observed implementation, adoption pending**. Implementation status: **Observed**, scoped below. Validation status: **Evidence inspected/located, not executed**. Audit date: 2026-11-05. Source revision: [slice audit](README.md).

## Choice, rationale and domain

The builder resolves coupon/payment programs into independent adjacent windows across the issue-to-maturity horizon. This supports fixed, floating, fixed-to-floating, margin-step, fixed step-up, cash, PIK and split coupons. Source comments explicitly warn that window boundaries change the generated coupon grid. This is a deliberate implementation rule, not evidence that an off-cycle contract should adopt it.

## Algorithm

1. Builder methods accumulate instructions; the first deferred error remains authoritative. `principal()` replaces principal/dates without clearing that error. Full-horizon bounds resolve only during build, allowing principal and coupon setup in either order.
2. Resolve every window and discard only the special empty issue/maturity edge pieces. Require all remaining windows to be positive and inside `[issue,maturity]`.
3. Sort and deduplicate issue, maturity, every coupon boundary and every payment boundary. For each adjacent interval, require exactly one coupon piece covering the whole interval. Missing coverage or overlapping coupon pieces fail.
4. Select the narrowest containing payment window, otherwise the coupon's own split. Partially overlapping payment windows fail. Equal payment windows follow the same containment branch: a later supplied matching window replaces the earlier one. This tie behavior is observed, with no distinct economic rationale located.
5. Generate a fresh schedule for each interval using [CASHFLOWS-001](01-schedule-periods.md). A short-front off-cycle cut changes earlier payment anchors in its window. Only the initial CDS window may extend front accrual before issue; subsequent windows clip the first accrual start at the switch to avoid double accrual.
6. Fixed step-up rates use the latest step date `<= unadjusted period start`. Step dates must strictly increase. An off-cycle rate step waits until the next period start and does not split that period. Adjacent equal-rate periods are grouped. Payment-program cuts still partition the horizon before this grouping.
7. Floating windows resolve the fixing calendar override or inherit their payment calendar; convert and validate rate controls. Check duplicate payment dates across the complete coupon program.

`float_margin_steps` uses the base spread until the first date, then each new spread from its date. `payment_split_program` is different: each supplied split runs **until** its date; cash covers the final remainder. Both reject repeated or decreasing dates. `fixed_to_float` explicitly splits at the switch date. An empty coupon and payment program produces principal-only cashflows; a payment program without coupon coverage errors.

## Selection, wire defaults and failures

`CashflowScheduleBuildSpec::build` dispatches every JSON leg/program variant to the same Rust builder, then calls `build(market)`. The JSON entry point has no explicit observation-date input; curve base dates supply fixing cutoffs. Principal exchange defaults to initial-and-final, lists to empty, and coupon split to cash. Unknown JSON fields are rejected, including flattened coupon input guards. Schema generation is a structural description and does not execute every economic validation.

Fee periods compile separately across the whole horizon; they do not cut coupon windows. Fixed fees after raw maturity are rejected; fees before issue are permitted. See [CASHFLOWS-009](09-fees.md).

## Alternatives

Supported alternatives are explicit coupon windows, fixed step-up on a retained grid, and floating-margin windows. A global anchored coupon grid with pro-rata intra-period rate changes is not the implemented window model.

## Evidence

- [compiler.rs](../../../../finstack-quant/cashflows/src/builder/compiler.rs), [coupon_api.rs](../../../../finstack-quant/cashflows/src/builder/coupon_api.rs), [json.rs](../../../../finstack-quant/cashflows/src/json.rs).
- Compiler unit tests locate default cash, nested override, partial-overlap rejection and out-of-horizon rejection. [Compiler regressions](../../../../finstack-quant/cashflows/tests/cashflows/builder/compiler_regressions.rs) locate `step_up_uses_contractual_start_under_both_adjustment_directions`; [principal program tests](../../../../finstack-quant/cashflows/tests/cashflows/builder/principal_events.rs) locate margin-step order/boundary cases. These are implementation regression evidence, not independently sourced contract examples.
- [ISDA 2006](../../../REFERENCES.md#isda-2006-definitions) supplies a general schedule-convention reference; no external standard establishing this program-partition policy was identified in the owned source.

## Follow-up tracking

Adoption questions, implementation reconciliation and missing validation are tracked in the
[cashflow follow-ups](../FOLLOW_UPS.md#cashflows).
