# VALUATIONS-STRUCTURED-CREDIT-20: Settlement quotes, yields, spreads and discount margin

- **Decision status:** Draft — observed implementation; adoption pending agreement.
- **Implementation status:** Observed in the owning paths below; scope: deal/tranche price metrics and inverse fixed-cashflow solvers.
- **Validation status:** Evidence inspected/located, not executed. Source and selected assertions establish implemented behavior, not general numerical accuracy or market approval.
- **Audit date:** 2026-11-05.
- **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; runtime source unchanged by this audit.

## Choice and rationale

Structured-credit quotes use current note face and payments strictly after settlement. Cashflow generation is reused, while inverse price calculations hold the generated cashflows fixed. This is a quote convention rather than a promise that collateral behavior responds to every trial spread.

## Algorithm and conventions

Settlement defaults to valuation date and cannot precede valuation or closing. Accrued equals opening note balance × coupon rate × note-day-count fraction from accrual start to min(settlement, accrual end); zero applies outside the open accrual/payment interval. Delayed payment periods can retain accrued after accrual end until payment. Clean target cash is clean-price percent × current face/100 plus accrued. Dirty/clean outputs reverse that transformation.

Z-spread computes `sum(CF × DF(settlement,date) × exp(-s × Act365F_time))` and solves the relative-price residual `PV/target - 1`. `calculate_tranche_z_spread` returns basis points; the registry converts to decimal and rejects results outside [-0.05,0.50] after the solve. The shared kernel uses tolerance 1e-12 and initial bracket size 0.05, without those explicit post-solve plausibility bounds. Neumaier summation reduces cancellation. Discount margin is restricted to floating notes but uses the same fixed-cashflow exponential-spread objective; it does not rebuild coupon forwards at every trial margin. The standalone discount-margin function returns decimal, while summary `dm_bp` returns bp; both reject magnitudes above 5,000 bp after solving.

YTM solves `sum(CF × (1 + y/m)^(-m × t)) = target` with Neumaier summation and Brent, initial guess 5% and tolerance 1e-6. In tranche scope, `m` is note coupon frequency and `t` uses note day count; deal scope uses annual compounding and Act/365F. The `valuations.structured_credit.ytm.v1` extension can override compounding to annual, semiannual, quarterly or monthly; it does not replace the selected time basis. Nonpositive target/no positive future flow errors. Invalid compounding base returns infinity; failed date conversion signals NaN to the solver. Quote validation enforces target inputs; the tranche-summary zero-face/nonpositive-model-value branch reports zero prices/spreads/risks and no yield rather than substituting original principal. Tranche summary deliberately removes the deal call for maturity metrics and reports hypothetical call values separately.

## Limits, failures and alternatives

Failed bracketing/convergence errors; finite-result guards reject nonfinite summaries. Multiple-sign-change flows may lack a unique yield; the inspected evidence does not establish consistent future-draw sign/accounting treatment in each helper. Spread is continuous discount spread, not a hazard intensity or additive floating reset margin. Supported clean/dirty inputs and settlement dates are alternatives; forward-reset discount margin or endogenous prepayment response would require another objective.

## Implementation and validation evidence

- [structured_credit/metrics/quote.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/metrics/quote.rs)
- [structured_credit/metrics/pricing/prices.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/metrics/pricing/prices.rs)
- [structured_credit/metrics/pricing/mod.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/metrics/pricing/mod.rs)
- [structured_credit/types/results.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/types/results.rs)
- [structured_credit/metrics/risk/spreads.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/metrics/risk/spreads.rs)
- [structured_credit/metrics/risk/ytm.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/metrics/risk/ytm.rs)
- [structured_credit/types/constants.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/types/constants.rs)
- [structured_credit/metrics/summary.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/metrics/summary.rs)

Existing evidence: [structured_credit/unit/price_basis_tests.rs](../../../../finstack-quant/valuations/structured-credit/tests/structured_credit/unit/price_basis_tests.rs); [structured_credit/unit/metrics/pricing_tests.rs](../../../../finstack-quant/valuations/structured-credit/tests/structured_credit/unit/metrics/pricing_tests.rs); [structured_credit/unit/metrics/risk_tests.rs](../../../../finstack-quant/valuations/structured-credit/tests/structured_credit/unit/metrics/risk_tests.rs). These are behavior/formula/invariant tests; no test was rerun for this audit. Where only a test location was inspected, no independent reference validation is claimed.

Reference context: [Brent](../../../REFERENCES.md#brent-1973), [fixed-income context](../../../REFERENCES.md#tuckman-serrat-fixed-income). These references identify the method family; they do not certify the preset parameters or every implementation branch.

Roundtrip price/spread tests validate self-consistency; independent quote and yield references were not established.

## Follow-up tracking

Adoption questions, implementation reconciliation and validation work are tracked in
the [structured-credit follow-ups](../FOLLOW_UPS.md#structured-credit).
