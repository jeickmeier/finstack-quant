# VALUATIONS-STRUCTURED-CREDIT-07: Card master-trust allocation

- **Decision status:** Draft — observed implementation; adoption pending agreement.
- **Implementation status:** Observed in the owning paths below; scope: statistical credit-card pools with CardPortfolioSpec.
- **Validation status:** Evidence inspected/located, not executed. Source and selected assertions establish implemented behavior, not general numerical accuracy or market approval.
- **Audit date:** 2026-11-05.
- **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; runtime source unchanged by this audit.

## Choice and rationale

Card portfolios use portfolio-level yield, charge-off and payment rates rather than interpreting the stored asset coupon and amortization schedule as ordinary installment-loan terms. The investor/seller allocation changes at the end of revolving. This is an expected-cash master-trust model, not account-level card behavior.

## Algorithm, domain and conventions

Finance charges use annual decimal portfolio yield. Annual charge-off rate is converted to the applicable monthly/period default fraction, while the monthly payment rate is a separate fraction of eligible receivables. During revolving, investor share follows current investor balance relative to the receivables base. When amortization begins, the principal-allocation share is frozen so principal collection allocation does not shrink automatically with investor paydown. Seller cash and investor available collections remain separately accounted for in the pool-flow calculation.

Card-specific inputs validate finite ranges and pool eligibility; instrument-backed collateral is rejected. Revolving end, loss/excess-spread events and controlled accumulation alter downstream reinvestment and principal allocation. They do not change annual charge-off into a monthly payment input.

The registered excess-spread metric is portfolio yield minus weighted note coupon divided by pool balance, minus servicing rate, minus annual charge-off rate, expressed as percent. Floating note coupons use current market projection. The payment-rate metric reports monthly payment fraction ×100. The charge-off metric instead reports cumulative defaults/current pool balance ×100; it is not the same annual input rate. Delinquency is opening delinquent bucket balance/current balance ×100.

## Limitations and alternatives

There is no account-level limit migration, purchase generation or stochastic payment-rate calibration in this branch. Annualized excess-spread metrics and the realized trailing-period trigger are distinct quantities. Ordinary statistical installment-loan terms are a supported separate mode, not an equivalent alternative for card economics. A richer account-cohort master trust would require calibrated purchase/payment/default dependencies.

## Implementation and validation evidence

- [structured_credit/types/card.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/types/card.rs)
- [structured_credit/pricing/simulation_engine/pool_flows.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/pricing/simulation_engine/pool_flows.rs)
- [structured_credit/pricing/simulation_engine/simulate_period/cash.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/pricing/simulation_engine/simulate_period/cash.rs)
- [structured_credit/metrics/deal_specific/abs.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/metrics/deal_specific/abs.rs)

Existing evidence: [structured_credit/unit/card_master_trust_tests.rs](../../../../finstack-quant/valuations/structured-credit/tests/structured_credit/unit/card_master_trust_tests.rs); [structured_credit/unit/metrics/deal_specific_tests.rs](../../../../finstack-quant/valuations/structured-credit/tests/structured_credit/unit/metrics/deal_specific_tests.rs). These are behavior/formula/invariant tests; no test was rerun for this audit. Where only a test location was inspected, no independent reference validation is claimed.

Independent master-trust cashflow references and correlated payment/default behavior remain unvalidated.

## Follow-up tracking

Adoption questions, implementation reconciliation and validation work are tracked in
the [structured-credit follow-ups](../FOLLOW_UPS.md#structured-credit).
