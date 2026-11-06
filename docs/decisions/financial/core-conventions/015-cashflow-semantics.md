# CORE-CONVENTIONS-015: Cash settlement, principal balance events and accrual metadata

- Decision status: **Draft — observed implementation, adoption pending**.
- Implementation status: **Observed**, scoped to `cashflow/primitives.rs`.
- Validation status: **Evidence inspected/located, not executed**. The linked tests establish available evidence, not a passing result or methodology approval.
- Audit date: **2026-11-05**. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; see [scope and inventory](README.md).

## Choice, scope and rationale

A cashflow may represent either cash settlement or a principal balance event. The distinction prevents PIK/default reductions from necessarily being interpreted as paid money. `CashFlow` carries a payment date, optional reset date, monetary amount, kind, nonnegative accrual factor, optional effective rate, optional accrual metadata and optional principal delta/date. This layer classifies and validates metadata; it does not project coupons or infer signs from kind.

## Algorithm and actual selection

Interest-like means Fixed, FloatReset, InflationCoupon or Stub. Principal-like means Notional, Pik, Amortization, PrePayment, RevolvingDraw, RevolvingRepayment or DefaultedNotional. Principal-repayment means Amortization, Notional, PrePayment or RevolvingRepayment. Cash settlement is every kind except Pik and DefaultedNotional. Recovery and AccruedOnDefault are cash but not principal-like according to these predicates; fee and margin/collateral kinds are also cash.

`get_balance_date` returns explicit principal_date, otherwise payment date. The constructor populates neither accrual metadata nor principal delta/date. Fluent setters merely attach values. `validate()` rejects nonfinite/negative accrual factor, nonfinite optional rate, reset after payment, principal-delta currency mismatch, nonincreasing accrual/reference period and nonfinite projected index rate. Negative finite rates and positive or negative amounts/deltas are allowed. Optional rates also have finite-value checks on serialization.

## Conventions and controls

Accrual metadata records unadjusted start/end, day count, projected index rate, optional calendar, optional regular coupon reference and termination flag. Rates are decimal representations; the cashflow amount remains authoritative. `principal_delta` is explicit balance impact, while `principal_date` can differ from delayed settlement. The predicate `is_principal_repayment` is a kind classification, not a sign check; Notional may still have either signed amount.

## Limitations and alternatives

Construction/deserialization do not automatically call `validate`. Public fields can express an inconsistent amount/rate/accrual-factor relationship, unsupported calendar ID or principal event inconsistent with its kind. This validator does not calculate accrual, require a calendar for Bus252, verify frequency metadata or ensure the supplied reference period matches the actual schedule. It also does not filter events from NPV itself: [016](016-discounting.md) receives `(Date,Money)` pairs without kind.

The current explicit principal-event representation is supported. Inferring balance changes solely from signed settlement amounts, eagerly validating every construction, or deriving amount from rate/accrual would be different contracts and are not implemented here.

## Evidence

[Owning implementation and inline tests](../../../../finstack-quant/core/src/cashflow/primitives.rs): `cfkind_is_interest_like_classifies_coupon_flows_only`, `cfkind_is_principal_like_classifies_balance_changing_flows`, `cashflow_validation_works`, and optional-rate serialization tests. These assert semantic categories and metadata checks; they do not validate a projected financial schedule. [Core cashflow exports](../../../../finstack-quant/core/src/cashflow/mod.rs) add no selection logic. No external standard defines this exact internal classification table.

Settlement date and balance date remain distinct in downstream models.

## Follow-up tracking

Adoption questions, implementation reconciliation and missing evidence are tracked in the
[core-conventions follow-ups](../FOLLOW_UPS.md#core-conventions).
