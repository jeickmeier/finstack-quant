# CASHFLOWS-003 — Principal events and date-by-date emission order

Decision status: **Draft — observed implementation, adoption pending**. Implementation status: **Observed**, scoped below. Validation status: **Evidence inspected/located, not executed**. Audit date: 2026-11-05. Source revision: [slice audit](README.md).

## Choice and domain

The builder tracks principal in `Decimal` by economic effective date and emits independently dated cash settlements. Nonnegative opening notional, deterministic draws/repayments, cash amounts different from par changes, pre-issue movements and PIK capitalization are supported. Decimal balance arithmetic limits accumulation drift; no benchmark for the source comment's quantitative drift claim was inspected.

## Ordered algorithm

1. Require principal, issue and maturity; require issue before maturity; validate notional/amortization. Sort principal events stably by economic date. Require event delta currency to match opening notional and cash currency to match delta. `Amortization` delta must be nonpositive and `Notional` delta nonnegative. Other flow kinds have no equivalent sign restriction in the fluent method.
2. Determine terminal economic and payment dates from the coupon ending at raw maturity; use their maximum dates if necessary. Without coupons both are raw maturity. Reject principal events or dated amortization beyond the effective terminal boundary, even if before raw maturity. No lower issue bound is imposed on principal events.
3. With default `InitialAndFinal`, emit `-initial` as issue-date Notional funding. Start balance at `initial`; apply all events dated on/before issue once and record their actual historical economic dates, including CDS pre-issue accrual. Then apply issue-date scheduled amortization.
4. Visit the sorted union of issue, maturity, redemption cash date, coupon/fee accrual and payment dates, dated amortization, principal events and LinearBetween start. For dates after issue, execute exactly: **fixed coupons + floating coupons → add positive net PIK → explicit principal events → scheduled amortization → periodic fees → initialize/update LinearBetween installments → terminal redemption → record closing economic balance**.
5. Coupons read the balance history over `[accrual_start,accrual_end)`; an event at an endpoint affects the next interval. PIK at the final accrual boundary is included before amortization/redemption even when BDC pays cash earlier or later. Periodic fees use historical balances; point-in-time fees use the opening balance of their own period.
6. Emit final positive balance as Notional at redemption payment date with `principal_date=terminal economic date`; zero the economic balance. `PrincipalExchange::None` suppresses both automatic issue funding and terminal redemption, while explicit principal events and scheduled amortization still operate.

## Signs, controls and precision

An explicit event adds `delta` to economic outstanding. If `cash` is absent, amortization cash is `-delta`, other cash is `delta`. Emission uses positive supplied cash for `Amortization` and negative supplied cash for other kinds. Thus discount/premium cash need not equal par change. Each event carries the explicit delta and economic date. Checked Decimal additions reject overflow and any negative balance immediately; same-day events are applied in insertion order, so a repayment before a funding event may fail even when the day's net delta is nonnegative.

Fixed fees are emitted once before the loop. Representative schedule day count is the first fixed leg's, otherwise first floating leg's, otherwise ACT/365F; flow-specific accrual metadata remains authoritative. Calendar IDs and projected fixing observations are normalized after emission. The completed schedule receives full validation.

## Alternatives and limitations

Cash-date balance recognition, netting all same-day events, and applying amortization before PIK are possible unimplemented alternatives. They change economic exposure and interest. This builder does not run prepayment/default models; those are standalone specifications consumed by other engines. Canonical output row sorting is distinct from this emission order; [CASHFLOWS-012](12-schedule-lifecycle.md) documents balance replay.

## Evidence

- [orchestrator.rs](../../../../finstack-quant/cashflows/src/builder/orchestrator.rs), [pipeline.rs](../../../../finstack-quant/cashflows/src/builder/pipeline.rs), [principal event definitions](../../../../finstack-quant/cashflows/src/builder/specs/principal.rs), [fluent validation](../../../../finstack-quant/cashflows/src/builder/coupon_api.rs).
- Located formula/regression cases in [audit_regressions.rs](../../../../finstack-quant/cashflows/tests/audit_regressions.rs): `b14_midperiod_draw_accrues_from_its_event_date`, `b13_preceding_final_payment_includes_contractual_pik`, `discounted_draw_roundtrips_principal_independently_of_cash`. [principal_credit_regressions.rs](../../../../finstack-quant/cashflows/tests/principal_credit_regressions.rs) checks pre-issue CDS exposure and amortization funded by PIK/draws.
- No external normative source for general event ordering was located. Tests were not run.

## Follow-up tracking

Adoption questions, implementation reconciliation and missing validation are tracked in the
[cashflow follow-ups](../FOLLOW_UPS.md#cashflows).
