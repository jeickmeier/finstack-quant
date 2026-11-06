# VALUATIONS-BONDS-01: Schedule construction, balance events and holder cashflows

**Decision status:** Draft — observed implementation; adoption pending.
**Implementation status:** Observed in the linked bond production scope; discrepancies are identified explicitly.
**Validation status:** Evidence inspected or located, not executed; no accuracy certification.
**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice, scope and rationale

The bond adapter uses the canonical cashflow builder for fixed, floating, step-up and singly wrapped amortizing specifications, or takes a supplied custom schedule verbatim. Keeping the schedule as the cash/balance source is the source-stated design; accepting its product-specific defaults remains an adoption decision. Projection, fixed/step-up accrual and PIK accrual kernels belong to the cashflows scope, while this record owns their bond selection and filtering.

## Algorithm and controls

1. `full_cashflow_schedule` returns a clone of `custom_cashflows` when supplied. Otherwise it creates issue/maturity principal and applies `fixed_cf`, `floating_cf`, `step_up_cf`, or amortization followed by the base coupon specification, then builds against the market.
2. Holder valuation rows have dates strictly after `as_of`, exclude PIK rows and negative `Notional` exchanges, and drop interest-like rows detached by the ex-coupon rule at the separately supplied entitlement date. Other signed rows, including fees, survive; this is not simply a positive-receipts filter. Same-date rows are included only by the explicit inclusive exercise helper.
3. Accrual configuration defaults to linear, includes PIK, supplies coupon frequency, and optionally supplies an ex-coupon window/calendar. Cashflow export preserves the signed schedule; static export of exercise-contingent bonds is rejected. `metrics_equivalent` makes the coupon cash-pay and removes Merton configuration.
4. The bare `CashflowSpec` constructors use Following, ShortFront, weekends-only, no end-of-month roll, no payment lag and unadjusted accrual dates. Default spec is zero-rate semiannual 30/360. Floating defaults use index-registry calendar/reset lag when found, otherwise weekends-only/two days; gearing is one, spread included in gearing, no caps/floors, no compounding override. Registry lookup failure is silently converted into those constructor defaults.
5. `Bond::fixed` selects US corporate conventions; `with_convention` applies convention frequency/day count/stub/calendar/settlement/ex-coupon and turns on end-of-month only when both endpoints are month ends. Floating currency inference is USD corporate, EUR corporate, GBP gilt or JPY JGB; other currencies require an explicit convention. Zero coupon uses annual ACT/365F without inferred settlement.
6. `from_cashflows` validates the schedule, takes metadata issue/maturity or first/last dates, and infers quote frequency from the modal interval of Fixed/Stub/FloatReset payment dates. Recognized day buckets are annual 355–375, semiannual 175–192, quarterly 85–98, bimonthly 55–68, monthly 25–37 and weekly 5–9; ties favor shorter intervals, fewer than two coupons imply annual. It creates a zero fixed-rate placeholder with the inferred frequency and schedule day count.

## Domain, failures and alternatives

Public pricing validates positive notional, maturity after issue, nonnegative fixed/step-up rates, no nested amortization, option windows/prices, floors and overrides. Deserialization can infer issue from the first custom-schedule date; it does not itself run all economic validation. `with_custom_cashflows` also just installs the schedule. A custom schedule's amounts override coupon-spec amounts, but the coupon spec still supplies frequency, day-count and model capability tests. Explicit conventions/frequency metadata are the available alternative to heuristic construction; a fully schema-owned custom coupon convention would require a new decision.

## Evidence

Owning code: [cashflow specification](../../../../finstack-quant/valuations/fixed-income/src/bond/cashflow_spec.rs), [construction and custom-frequency inference](../../../../finstack-quant/valuations/fixed-income/src/bond/types/construction.rs), [definitions and option validation](../../../../finstack-quant/valuations/fixed-income/src/bond/types/definitions.rs), [holder filter](../../../../finstack-quant/valuations/fixed-income/src/bond/types/pricing.rs), [trait/export behavior](../../../../finstack-quant/valuations/fixed-income/src/bond/types/traits.rs). [Construction tests](../../../../finstack-quant/valuations/fixed-income/src/bond/types/tests.rs) inspect custom schedule preservation; [step-up/serde tests](../../../../finstack-quant/valuations/fixed-income/src/bond/types/tests_stepup.rs) inspect missing-issue handling and floating row signs. These are behavior/invariant tests, not external valuation benchmarks. [ICMA](../../../REFERENCES.md#icma-rule-book) is a convention reference, not proof these defaults fit every security.

## Follow-up tracking

Adoption, reconciliation and validation work is tracked in the
[scoped follow-up register](../FOLLOW_UPS.md#bond-valuations).
