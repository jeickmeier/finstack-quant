# VALUATIONS-OTHER-FIXED-INCOME-07: Term-loan funding, amortization, covenants and fee compilation

- **Decision status:** Draft — observed implementation; adoption and rationale require agreement.
- **Audit date/source:** 2026-11-05; `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- **Implementation status:** Observed in the entry points and kernels linked below.
- **Validation status:** Source and named evidence inspected or located; tests and numerical experiments were not executed. Evidence limits are stated below.
- **Scope inventory:** [README](README.md); [complete source mapping](coverage.json).

## Choice and rationale

The loan compiles typed events into the shared cashflow builder rather than implementing a second coupon engine. [cashflows.rs](../../../../finstack-quant/valuations/fixed-income/src/term_loan/cashflows.rs), [spec.rs](../../../../finstack-quant/valuations/fixed-income/src/term_loan/spec.rs) and [types.rs](../../../../finstack-quant/valuations/fixed-income/src/term_loan/types.rs) own the product-specific event ordering and domain restrictions. Economic treatment of these events awaits adoption.

## Scope and algorithm

1. Non-DDTL loans fund the limit at issue. A delayed-draw loan emits face increases only for draws within availability and strictly before the earliest draw-stop date. Funding cash can differ from principal: basis-point OID is applied per draw; absolute OID is allocated by each valid draw's share of total valid draws. Withheld OID reduces lender funding; separate OID produces positive fee receipts.
2. Upfront fees are based on DDTL commitment when present, otherwise the limit. Cash sweeps and custom principal create amortization events. Percent-of-original amortization uses cumulative draws strictly before each coupon accrual end; percent-of-remaining and linear-between pass to the shared builder. `LinearTo` and `StepRemaining` are rejected.
3. Sort principal events by date and cap each repayment to the running nonnegative funded balance. Excess requested amortization is reduced, not rejected at emission. The schedule starts from zero and replays these funding/principal events; maturity redemption comes from the builder.
4. Fixed-rate covenant margin changes accumulate in Decimal and use the step-up coupon path. Floating spread changes are snapped forward to the first accrual start on/after the event, combined on that boundary, and applied through the floating-margin program. The floating index constraints, gearing, reset lag/tenor and compounding remain contractual inputs.
5. PIK toggles produce a payment-split program. Cash and PIK composition affects outstanding principal through the common builder; PIK is not a cash coupon paid twice.
6. Usage fees use time-weighted drawn balance. Commitment fees stop at availability end or the earlier draw-stop. The integration grid includes coupon accrual boundaries, commitment changes and draw dates. Each interval charges `fee_rate × base × accrual_fraction`; the base is either commitment minus cumulative funding, or commitment minus outstanding. Amounts aggregate to the enclosing adjusted coupon payment date.

## Selection and conventions

The direct loan cashflow/export path calls this compiler; pricing adds the fixing restatement in [record 08](08-term-loan-pricing.md). Dates use unadjusted accruals, contractual frequency/day count/stub, adjusted payment dates, no payment lag and no end-of-month roll. Calendar defaults to weekends-only. All funding and repayments use loan currency; rates are decimals, margin/fee steps basis points. DDTL commitment reductions cannot increase the limit and cannot carry reduction fees in this product.

## Limitations and failure behavior

Validation requires positive finite limit, ordered dates, in-life events, matching currencies and valid splits. Cumulative draws are checked against commitment in force. Cash sweep and amortization amounts can be capped by available funded balance. Fixed margin events can affect a coupon differently from floating events because the latter snap to reset/accrual starts. `set_default_status(true)` rejects without explicit recovery economics; percentage cash sweeps and collateral requirements reject without typed economics. Covenant `increase_rate` rounds the decimal increment to whole basis points before changing rates. Those are concrete semantics, not blanket support for arbitrary covenant consequences.

## Alternatives and tradeoffs

Typed dated events make changes auditable and composable. Modeling draw stops as probabilities, optimizing borrower funding, or inferring cash sweeps from financial statements would introduce different economic models and are not performed here. The two commitment-fee bases intentionally distinguish nonrevolving availability from reusable paid-down principal.

## Evidence

[Cashflow tests](../../../../finstack-quant/valuations/fixed-income/tests/term_loan/cashflows.rs), [finance regressions](../../../../finstack-quant/valuations/fixed-income/tests/term_loan/finance_regressions.rs), [overnight-floor tests](../../../../finstack-quant/valuations/fixed-income/tests/term_loan/overnight_floor.rs) and colocated validation tests cover event and convention examples. No external loan-servicing tape was reconciled. [ISDA day-count reference](../../../REFERENCES.md#isda-2006-definitions) supplies context, not adoption of these covenant semantics.

## Follow-up tracking

Adoption, implementation and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#other-fixed-income).
