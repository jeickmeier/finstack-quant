# VALUATIONS-STRUCTURED-CREDIT-04: Delinquency migration, modification and servicer advancing

- **Decision status:** Draft — observed implementation; adoption pending agreement.
- **Implementation status:** Observed in the owning paths below; scope: statistical pools with delinquency buckets.
- **Validation status:** Evidence inspected/located, not executed. Source and selected assertions establish implemented behavior, not general numerical accuracy or market approval.
- **Audit date:** 2026-11-05.
- **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; runtime source unchanged by this audit.

## Choice and rationale

With delinquency configured, the default model primarily supplies entry into delinquency, and terminal roll-through creates charge-offs. This is a different interpretation from applying the same MDR immediately as default. The implementation models expected bucket balances, not simulated discrete loan states.

## Algorithm and input domain

Each monthly step sizes new delinquency entries, processes previously existing buckets through modification/cure/roll branches, and carries the remainder. Cure returns balance to current performance in that period. Arrears on curing balances repay servicer advances first; unrecovered arrears reach collections according to the state recursion. Modification can forgive arrears, lower coupon and extend term through weighted asset-level terms; it does not create independently tracked borrower vintages.

For timing/cumulative-loss default inputs, the engine looks ahead to expected future charge-offs and grosses the required delinquency entry up by the product of roll rates. Every roll rate must therefore be positive for that inversion. This is a heuristic conversion, not a calibrated competing-risk estimation. Buckets and cure/roll/modification arrays must have compatible shape and probabilities; opening bucket currency must match the asset and bucket totals cannot exceed balance.

Advances replace missing scheduled interest, capped as a fraction of delinquent balance. Recoveries/cures repay those advances according to recoverability and policy. Nonrecoverable advances may be reimbursed from general collections when configured, reducing cash available to the waterfall. Advancing does not create new collateral principal.

## Controls, conventions and failure behavior

Monthly probabilities are fractions; bucket amounts and arrears are pool currency. The deal's month frequency is decomposed into monthly transitions. Missing deal delinquency configuration with seeded buckets errors, as does use with instrument-backed collateral. Modification rate/term aggregation, no-forward-roll conditions and initial-bucket behavior are material state choices.

## Alternatives and tradeoffs

Immediate hazard defaults are supported by omitting delinquency. Explicit per-loan Markov states and separate modification cohorts would preserve more path dependence but require different state and calibration data; they are not performed by the weighted recursion. The current expected-balance method is inexpensive and reproducible but combines vintages with potentially different cure/arrears behavior.

## Implementation and validation evidence

- [structured_credit/types/delinquency.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/types/delinquency.rs)
- [structured_credit/pricing/simulation_engine/delinquency.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/pricing/simulation_engine/delinquency.rs)
- [structured_credit/pricing/simulation_engine/pool_flows.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/pricing/simulation_engine/pool_flows.rs)
- [structured_credit/pricing/simulation_engine/state.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/pricing/simulation_engine/state.rs)

Existing evidence: [structured_credit/unit/delinquency_tests.rs](../../../../finstack-quant/valuations/structured-credit/tests/structured_credit/unit/delinquency_tests.rs). These are behavior/formula/invariant tests; no test was rerun for this audit. Where only a test location was inspected, no independent reference validation is claimed.

The inspected hand schedule uses 100% roll/no cure and checks interest to 0.02 currency units and terminal balances to 1 unit; more general transition calibration and independent mixed cure/modification references remain unestablished.

## Follow-up tracking

Adoption questions, implementation reconciliation and validation work are tracked in
the [structured-credit follow-ups](../FOLLOW_UPS.md#structured-credit).
