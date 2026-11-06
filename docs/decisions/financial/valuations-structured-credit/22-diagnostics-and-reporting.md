# VALUATIONS-STRUCTURED-CREDIT-22: Cashflow classification, summary metrics and diagnostics

- **Decision status:** Draft — observed implementation; adoption pending agreement.
- **Implementation status:** Observed in the owning paths below; scope: tranche/deal reporting, equity returns, WAL and pool/deal ratios.
- **Validation status:** Evidence inspected/located, not executed. Source and selected assertions establish implemented behavior, not general numerical accuracy or market approval.
- **Audit date:** 2026-11-05.
- **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; runtime source unchanged by this audit.

## Choice and rationale

The reporting layer distinguishes paid cash, principal, PIK, deferred interest and writedown. That classification drives valuation and risk metrics; totals must not conflate noncash changes with receipts. Diagnostics expose how the waterfall conserved cash and changed credit support.

## Algorithm and conventions

Simulation records payment-date cashflows and separately tagged principal/interest/notional funding. PIK/deferred/writedown events are noncash classifications. `TrancheCashflows::to_table` validates currency, groups each series by date and sums six output columns. Metric seeding reuses the deterministic run's tagged flows and accrual periods to avoid a second simulation; seed failures can leave optional context absent, so downstream calculation must still validate.

WAL uses principal repayment weights and Act/365F time from the measurement date; cash interest does not count. Tagged-flow fallback recognizes principal kinds and can use absolute negative funding when principal labels are missing, making draw treatment material. Stochastic WAL additionally excludes no-principal paths from its average (see stochastic record). Equity summary uses current face × clean-price factor as invested capital, measures cash multiple/payback and Act/365F XIRR of future equity cash; unsuccessful XIRR becomes `None`, not a zero return.

Pool/deal ratios differ by denominator. ABS credit enhancement adds performing collateral, collection/reserve/excess-spread accounts and initial default recovery claims, then compares the most senior class with that collateral. ABS cumulative charge-off divides cumulative defaults by current pool balance. CPR/CDR metrics annualize the model's monthly rate at pool seasoning, not realized trailing collections. WARF/WAS and WAM use their pool-statistic definitions. CMBS DSCR uses only performing assets with supplied NOI.

The main Monte Carlo summary reports expected loss, population unexpected loss, 95% tail loss, mean PV and sampling error; its clean/dirty price identity and PV-weighted “credit duration” are specific conventions. Finite-value checks validate summaries, OAS and scenario cells; they do not certify statistical estimates or accounting completeness.

## Limitations and alternatives

Opening/static ratios are not projected covenant outcomes. Missing NOI, missing ratings, empty statistics and failed IRR have different handling, requiring meaningful null/zero presentation. Detail/report models do not add new empirical credit information. Alternative original-face denominators, total-return exposure weighting or realized trailing rates must be separately named and implemented rather than inferred from these outputs.

## Implementation and validation evidence

- [structured_credit/pricing/simulation_engine/simulate_period/record.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/pricing/simulation_engine/simulate_period/record.rs)
- [structured_credit/pricing/simulation_engine/state.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/pricing/simulation_engine/state.rs)
- [structured_credit/types/results.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/types/results.rs)
- [structured_credit/types/instrument.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/types/instrument.rs)
- [structured_credit/types/tranche_view.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/types/tranche_view.rs)
- [structured_credit/metrics/pricing/wal.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/metrics/pricing/wal.rs)
- [structured_credit/metrics/summary.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/metrics/summary.rs)
- [structured_credit/metrics/finite.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/metrics/finite.rs)
- [structured_credit/metrics/pool.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/metrics/pool.rs)
- [structured_credit/metrics/deal_specific/abs.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/metrics/deal_specific/abs.rs)
- [structured_credit/metrics/deal_specific/cmbs.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/metrics/deal_specific/cmbs.rs)

Existing evidence: [structured_credit/unit/diagnostics_tests.rs](../../../../finstack-quant/valuations/structured-credit/tests/structured_credit/unit/diagnostics_tests.rs); [structured_credit/unit/metrics/pool_tests.rs](../../../../finstack-quant/valuations/structured-credit/tests/structured_credit/unit/metrics/pool_tests.rs); [structured_credit/unit/metrics/deal_specific_tests.rs](../../../../finstack-quant/valuations/structured-credit/tests/structured_credit/unit/metrics/deal_specific_tests.rs); [structured_credit/academic_golden.rs](../../../../finstack-quant/valuations/structured-credit/tests/structured_credit/academic_golden.rs). These are behavior/formula/invariant tests; no test was rerun for this audit. Where only a test location was inspected, no independent reference validation is claimed.

The diagnostic and hand-formula tests support accounting identities; they are not an independent validation of every economic metric interpretation.

## Follow-up tracking

Adoption questions, implementation reconciliation and validation work are tracked in
the [structured-credit follow-ups](../FOLLOW_UPS.md#structured-credit).
