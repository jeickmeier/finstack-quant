# CASHFLOWS-005 — Balance-segment coupon amounts and cash/PIK allocation

Decision status: **Draft — observed implementation, adoption pending**. Implementation status: **Observed**, scoped below. Validation status: **Evidence inspected/located, not executed**. Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` ([slice audit](README.md)).

## Choice and scope

Coupon amounts accrue on each constant-economic-balance interval, with cash and PIK components emitted separately. Fixed coupons use checked Decimal multiplication; floating settlement uses finite-checked f64. Fixed-rate step selection belongs to [CASHFLOWS-002](02-program-compilation.md), floating rates to [CASHFLOWS-006](06-floating-rate-projection.md), and overnight increments to [CASHFLOWS-007](07-overnight-compounding.md).

## Algorithm and conventions

1. Find balance at accrual start from the last history entry on/before start, otherwise use the supplied fallback. Split only at actual balance changes strictly before accrual end; same-balance date entries do not split coupons.
2. For each `[s,e)` segment compute contractual day-count fraction `α(s,e)` with the parent period's calendar, frequency, reference coupon and terminal context. This is not a generic actual-day weighting approximation. ACT/365L preserves the full-period 365/366 denominator.
3. Fixed total is `C = B × r × α`, with checked Decimal products before f64 output. `r` is an annual decimal rate. Classify fixed cash as Stub only when the original full period is irregular; balance subdivisions do not independently create stubs.
4. Resolve Cash to `(cash,pik)=(1,0)`, Pik to `(0,1)`, or explicit Split. Each Decimal fraction must lie in `[0,1]`, and their sum must differ from one by no more than `1e-9`; fractions are not renormalized.
5. Emit cash whenever cash fraction is positive, including zero and negative cash amounts. Fixed cash uses Fixed/Stub; floating cash uses FloatReset and carries reset date. Amount is `C × cash_fraction`.
6. PIK is excluded from settlement cash but increases principal at the parent period's economic end, independently of payment date. Fixed negative PIK segment amounts are rejected. Floating segments may be signed when allocating bounds; the **net PIK for the whole period** must be finite and nonnegative. Nonzero signed PIK rows all carry the same final economic capitalization boundary.
7. Store per-flow accrual start/end, day count, calendar, nominal reference period, terminal flag, rate, accrual factor and, for floating rows, raw index rate. These fields drive later accrued interest and balance replay; they are not reconstructed from payment dates when present.

## Selection, failures and rationale

Coupon split is explicit or cash by default; a payment program can override it. The builder adds aggregate positive PIK before explicit principal movements and amortization. Negative cash coupon rates are supported; de-capitalizing a net PIK coupon is not. Overflow or nonfinite settlement results fail. Source comments justify segmenting by economic principal changes rather than adjusted payment dates.

`CouponType::split_parts` accepts sum error up to `1e-9`, while `CompiledFloatingCoupon::compile` requires `1e-12`. A split accepted for a fixed coupon can fail for a floating coupon without economic changes.

## Alternatives and evidence

Period-opening notional for the entire coupon and cash-date balance changes are unimplemented alternatives with different economics. Negative net PIK and contractual fractional normalization would require a separate adoption decision.

- [balance_segments](../../../../finstack-quant/cashflows/src/builder/emission/balances.rs), [coupon emission](../../../../finstack-quant/cashflows/src/builder/emission/coupons.rs), [CouponType](../../../../finstack-quant/cashflows/src/builder/specs/coupon.rs), [compiled settlement](../../../../finstack-quant/cashflows/src/builder/floating_replay.rs).
- Inspected formula/regression evidence: `bounded_split_pik_capitalizes_net_coupon_on_one_contractual_boundary` in [compiler_regressions.rs](../../../../finstack-quant/cashflows/tests/cashflows/builder/compiler_regressions.rs), which explicitly expects a negative segment and nonnegative net PIK. Located `b14_midperiod_draw_accrues_from_its_event_date` and ACT/365L balance-segment cases in [audit_regressions.rs](../../../../finstack-quant/cashflows/tests/audit_regressions.rs).
- General convention pointers: [ISDA 2006](../../../REFERENCES.md#isda-2006-definitions), [ICMA](../../../REFERENCES.md#icma-rule-book). Independent accuracy/provenance evidence for the complete split-PIK lifecycle was not located.

## Follow-up tracking

Adoption questions, implementation reconciliation and missing validation are tracked in the
[cashflow follow-ups](../FOLLOW_UPS.md#cashflows).
