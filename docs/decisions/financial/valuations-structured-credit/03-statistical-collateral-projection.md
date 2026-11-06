# VALUATIONS-STRUCTURED-CREDIT-03: Statistical collateral cashflow projection

- **Decision status:** Draft — observed implementation; adoption pending agreement.
- **Implementation status:** Observed in the owning paths below; scope: performing asset and representative-line pools, amortizing and bullet collateral.
- **Validation status:** Evidence inspected/located, not executed. Source and selected assertions establish implemented behavior, not general numerical accuracy or market approval.
- **Audit date:** 2026-11-05.
- **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; runtime source unchanged by this audit.

## Choice and rationale

Statistical collateral is an expected-balance recursion. Default, scheduled principal and prepayment compete for the same balance in an ordered period calculation. The implementation avoids charging each rate independently to opening principal; economic approval of its within-period timing remains separate.

## Algorithm and conventions

1. Compute seasoning from origination/acquisition (or closing). For integer multi-month periods, aggregate each underlying month's SMM/MDR by multiplying survival; the period receives `1 - product(1 - monthly_rate)`. Contractual exposure counts whole calendar months plus the actual fraction of the next calendar month. For monthly and noninteger stub periods, the monthly-rate helper samples the end-of-period seasoning rate; downstream survival raises it to that fractional month count.
2. Apply default to opening performing balance first. Interest is approximately opening balance × annual coupon × day-count accrual × `(1 - 0.5 × period default probability)`, with special-servicing/appraisal adjustments when configured. Thus defaults occur halfway through the interest period in expectation; prepaid principal otherwise earns the period's interest.
3. For amortizing asset types, compute scheduled payment from an explicit contractual payment or stable level-payment annuity. The helper uses `ln_1p`/`exp_m1` forms, a zero-rate branch and at least one remaining period. Interest-only terms suppress scheduled amortization; remaining term may exceed contractual maturity, creating a balloon.
4. Subtract scheduled principal from default survivors, then apply prepayment to the surviving balance. At contractual maturity remaining balance pays as a balloon unless CMBS or NPL terms alter it.
5. Enqueue recovery proceeds and losses with the configured recovery lag. Cumulative loss/timing specifications and per-asset overrides pass through their own probability transformations; existing default claims initialize a separate recovery queue.

Fixed rates are annual decimals. Floating row coupons use a forward-curve identifier, its tenor/reset lag and a weekends-only reset calendar. The index floor is in bp and spread is added after it. Future reset coupons project from the curve and floor the resulting all-in rate at zero. Historical observed fixing plus spread is not given that final zero floor; if no fixing series exists at all, the stored all-in asset rate is returned directly. An existing series missing the required observation errors. Thus negative-rate treatment differs by historical/projected branch.

## Selection, limitations and alternatives

`DeterministicPoolFlowSource` uses model expectations. Stochastic and OAS sources reuse the same cashflow/waterfall engine with path-specific hazards, prepayment and recovery. Explicit contractual-payment and nonamortizing branches are supported. Daily event-time default/prepayment, stochastic borrower-level payment behavior, and full cohort recalibration are not implicit alternatives.

The default/prepay ordering and half-period interest adjustment are approximations. The inspected evidence does not establish cached level-payment and term-reset accuracy for modifications and extended slices. Noninteger stubs apply the final month's model rate over the fractional exposure rather than integrating a varying seasoning curve. Recovery scheduling and the different ordinary-terminal versus call treatment are governed by the terminal-policy record.

## Implementation and validation evidence

- [structured_credit/pricing/simulation_engine/pool_flows.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/pricing/simulation_engine/pool_flows.rs)
- [structured_credit/pricing/simulation_engine/pool_flow_source.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/pricing/simulation_engine/pool_flow_source.rs)
- [structured_credit/pricing/simulation_engine/period_helpers.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/pricing/simulation_engine/period_helpers.rs)
- [structured_credit/utils/amortization.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/utils/amortization.rs)
- [structured_credit/utils/simulation.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/utils/simulation.rs)

Existing evidence: [structured_credit/unit/amortization_term_tests.rs](../../../../finstack-quant/valuations/structured-credit/tests/structured_credit/unit/amortization_term_tests.rs); [structured_credit/unit/seasoning_tests.rs](../../../../finstack-quant/valuations/structured-credit/tests/structured_credit/unit/seasoning_tests.rs); [structured_credit/unit/index_floor_tests.rs](../../../../finstack-quant/valuations/structured-credit/tests/structured_credit/unit/index_floor_tests.rs); [structured_credit/unit/stochastic_pricing_tests.rs](../../../../finstack-quant/valuations/structured-credit/tests/structured_credit/unit/stochastic_pricing_tests.rs). These are behavior/formula/invariant tests; no test was rerun for this audit. Where only a test location was inspected, no independent reference validation is claimed.

Reference context: [MBS formulas](../../../REFERENCES.md#sifma-mbs-standard-formulas). These references identify the method family; they do not certify the preset parameters or every implementation branch.

The inspected stochastic_waterfall_matches_independent_cashflow_vectors assertion checks explicit one/four-quarter expected vectors to 1e-6; it does not validate heterogeneous borrower timing or lagged-recovery market accuracy.

## Follow-up tracking

Adoption questions, implementation reconciliation and validation work are tracked in
the [structured-credit follow-ups](../FOLLOW_UPS.md#structured-credit).
