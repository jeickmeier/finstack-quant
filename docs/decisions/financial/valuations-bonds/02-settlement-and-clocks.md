# VALUATIONS-BONDS-02: Settlement carry, ex-coupon entitlement and model clocks

**Decision status:** Draft — observed implementation; adoption pending.
**Implementation status:** Observed in the linked bond production scope; discrepancies are identified explicitly.
**Validation status:** Evidence inspected or located, not executed; no accuracy certification.
**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice and scope

PV is anchored at valuation date; market quote metrics are anchored at settlement. The distinction is explicit in source and affects coupons paid or detached during the settlement interval. Contract accrual, quoted-yield time and stochastic model time need not be the same clock.

## Algorithm and controls

`settlement_date` adds configured business days using the bond's strict payment-calendar lookup, applies its business-day convention, then clamps to issue. With no configured lag it returns `max(as_of, issue)` without business-day adjustment. Unknown calendars and integer overflow error. Quote accrued is computed from the complete schedule at settlement, including negative accrued inside ex-coupon windows.

Let `D` be the bond discount-curve factor from valuation to settlement, and `C_detached` the discounted difference between valuation-holder cashflows and settlement-buyer cashflows. Carry conversions are `P_settle=(PV_asof−C_detached)/D` and `PV_asof=P_settle D+C_detached`. The difference includes intervening payments and coupons lost by settlement ex-coupon entitlement. It uses the bond discount curve, even if a selected tree uses another curve, and does not apply survival/default recovery to detached cash.

Quoted Street/Moosmüller cashflow times normally use the coupon-spec day count, frequency and ICMA quasi-period context. ACT/365L instead sums actual contractual coupon slices. An adjusted coupon payment is timed at its contractual accrual end for quoted yield; settlement after that end but before payment is rejected. Different contractual ends sharing one payment date are rejected. Custom ACT/365L schedules require explicit full, nonoverlapping coupon metadata. Carry ACT/365L similarly slices actual coupon periods and rejects uncovered intervals.

Rates-only tree time follows the selected discount curve; joint rates-credit uses a daily ACT/365F grid. Merton uses the bond coupon day count/ICMA reference context, except ACT/365L uses ACT/365F model time at actual payment dates. Its remaining coupon grid retains full cash/stub/PIK coupon accrual factors, deduplicates split cash/PIK entries by date and omits ex-coupon dates. Horizon is the later of maturity and final coupon date. Merton discount-factor knots combine rounded calendar simulation-grid dates, exact coupon dates and horizon; equal times prefer exact cashflow knots. The drift/discount distinction is covered in record 08.

## Limitations, alternatives and evidence

Cash-date recovery balance replay clears principal-date overrides on amortization, prepayment, notional and revolving rows, while retaining economic PIK/default dates; see [hazard pricing](03-pv-models.md). This is separate from coupon principal replay.

Owning code: [settlement/carry](../../../../finstack-quant/valuations/fixed-income/src/bond/pricing/settlement.rs), [clock helpers](../../../../finstack-quant/valuations/fixed-income/src/bond/pricing/time_basis.rs), [ACT/365L quote timing](../../../../finstack-quant/valuations/fixed-income/src/bond/pricing/quote_conversions/yield_price.rs), [quote accrued metric](../../../../finstack-quant/valuations/fixed-income/src/bond/metrics/accrued.rs). Inline settlement/time-basis tests and [settlement convention tests](../../../../finstack-quant/valuations/fixed-income/tests/bond/validation/settlement_conventions.rs) locate carry/clock invariants; not rerun. [ISDA](../../../REFERENCES.md#isda-2006-definitions) and [ICMA](../../../REFERENCES.md#icma-rule-book) provide general convention provenance, not security-specific agreement. Using one clock everywhere is simpler but would change implemented economics.

## Follow-up tracking

Adoption, reconciliation and validation work is tracked in the
[scoped follow-up register](../FOLLOW_UPS.md#bond-valuations).
