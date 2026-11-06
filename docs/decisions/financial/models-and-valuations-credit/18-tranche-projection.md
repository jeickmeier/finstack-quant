# MODELS-AND-VALUATIONS-CREDIT-18: Tranche premium, protection, recovery writedown and settlement

- Decision status: **Draft — observed implementation, adoption pending**.
- Implementation status: Observed in the paths listed below; discrepancies are stated explicitly.
- Validation status: Evidence inspected or located, **not executed**; no external accuracy certification.
- Audit date: 2026-11-05.
- Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice and rationale

Tranche PV discounts a common internal projected-row representation, using loss/recovery erosion for premium notionals and an estimated within-period default time for protection. Public cashflow rows omit that internal discount timing.

## Scope and input domain

Running-coupon tranches with optional signed upfront, configurable coupon schedule, realized pool losses and index data. An explicit `start_date` makes settlement `as_of`, bypassing synthetic settlement lag. Standard indices are recognized by case-sensitive CDX/iTraxx/ITRAXX name prefixes.

## Algorithm

1. Standard construction uses quarterly ACT/360, CDS-IMM roll, short-front stub, Following payment adjustment and unadjusted accrual boundaries. Missing explicit start uses the preceding CDS date for CDS-IMM, otherwise settlement; default settlement is one business day for named standard indices and three for bespoke.
2. Generate coupon periods and retain payment dates on/after as-of. Already accrued periods use historical loss/writedown; future accrual ends use the loss curve.
3. Premium is coupon times `[full_accrual × prior_outstanding − (1−f) × remaining_accrual × incremental_erosion]`, floored at zero. Erosion includes both loss and recovery writedown.
4. Protection is incremental expected tranche loss, with opposite premium sign. From interval index-survival endpoints infer constant hazard λ; default-time fraction is `[1/λ − Δ exp(−λΔ)/(1−exp(−λΔ))]/Δ`, clipped to [0,.5]. Invalid, flat or very small hazard intervals use .5.
5. Premium/upfront discount at payment date. Protection discounts at the rounded-day interval fraction, clamped to as-of/end, although its public row date is coupon payment date. A fully wiped tranche retains only an unpaid upfront.

## Selection and controls

Index-name prefixes choose synthetic lag; a supplied calendar must resolve and selects business days, while an omitted calendar uses weekdays only. These name-based rules and explicit-start overrides require adoption alongside the loss model. Coupon sign is PayReceive.sign and protection opposite. Tiny premiums are omitted except during unit-coupon par/spread calculations.

## Conventions

Coupon is basis points divided by 10,000; discount factors are ratios from as-of; upfront must share notional currency. Protection date is an expected timing approximation, not an explicit default-event simulation.

## Limitations, failure and fallback

Public projected schedules discard `DiscountAt::WithinPeriod`, so discounting returned cashflows by their dates does not reproduce PV. Public schedule returns empty when as-of >= maturity, while the pricing path may retain rolled unpaid coupons/upfront. Index-level hazard sets within-period timing even in heterogeneous pricing.

## Alternatives and tradeoffs

Exact survival-weighted protection integration would remove the timing approximation; preserving an explicit expected default date in public rows would make schedule discounting consistent. Neither change is selected by this draft.

## Evidence

Owners: [engine.rs](../../../../finstack-quant/valuations/credit/src/cds_tranche/pricing/engine.rs), [types.rs](../../../../finstack-quant/valuations/credit/src/cds_tranche/types.rs), [parameters.rs](../../../../finstack-quant/valuations/credit/src/cds_tranche/parameters.rs). [Internal tests](../../../../finstack-quant/valuations/credit/src/cds_tranche/pricing/tests.rs) and [integration tests](../../../../finstack-quant/valuations/credit/tests/cds_tranche) locate schedule/seasoning evidence; no tests were executed.

## Follow-up tracking

Adoption questions and implementation or validation work are tracked in the
[credit-model and valuation follow-ups](../FOLLOW_UPS.md#credit-models-and-valuations).
