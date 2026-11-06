# VALUATIONS-STRUCTURED-CREDIT-09: Collateral call and put exercise assumptions

- **Decision status:** Draft — observed implementation; adoption pending agreement.
- **Implementation status:** Observed in the owning paths below; scope: callable/putable bond and loan collateral.
- **Validation status:** Evidence inspected/located, not executed. Source and selected assertions establish implemented behavior, not general numerical accuracy or market approval.
- **Audit date:** 2026-11-05.
- **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; runtime source unchanged by this audit.

## Choice and rationale

Early exercise of collateral is selected by a stated policy, not optimized from the deal's value function. Calls are processed before puts. The rule set makes scenario intent expressible but should not be represented as rational option valuation.

## Algorithm and conventions

Contractual-only preserves the no-early-exercise schedule. First-opportunity uses the earliest eligible exercise candidate. Yield-to-worst builds candidate redemption schedules, computes their XIRR and chooses the minimum successfully solved yield. Failed candidate solves are skipped. Refinancing compares contractual coupon with a simple forward refinancing rate over Act/365F horizon plus configured threshold; step-up rules compare the new coupon with the initial coupon. Selected exercise changes projected collateral termination cash and the period to which it is assigned.

Prices expressed as percent of par are converted to cash on outstanding balance. Candidate dates, call/put schedules and override identifiers come from collateral terms. The exercise pass runs before cashflow bucketing and subsequent credit/prepayment overlays; it does not revise a policy in response to every downstream coverage test.

## Selection, failure and limitations

No-exercise and policy overrides are supported controls. Missing/unresolvable required market inputs error where fetched; yield candidate failure is a distinct silent exclusion. The minimum-yield procedure need not identify the lowest investor value under arbitrary nonconventional cashflows, and call-before-put ordering can resolve competing opportunities asymmetrically. Threshold refinancing is not a calibrated borrower option model.

## Alternatives and tradeoffs

The explicit deterministic policies are supported alternatives. Lattice/PDE or least-squares continuation-value optimization would be a separate option model requiring rate/spread dynamics, exercise costs and calibration; it is not used by this policy module. Proposed rationale for rule-based scenarios is transparency, awaiting agreement for the intended analytical use.

## Implementation and validation evidence

- [structured_credit/types/collateral.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/types/collateral.rs)
- [structured_credit/pricing/simulation_engine/exercise.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/pricing/simulation_engine/exercise.rs)
- [structured_credit/pricing/simulation_engine/instrument_flows.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/pricing/simulation_engine/instrument_flows.rs)

Existing evidence: [structured_credit/unit/instrument_collateral_tests.rs](../../../../finstack-quant/valuations/structured-credit/tests/structured_credit/unit/instrument_collateral_tests.rs); [structured_credit/unit/instrument_pool_tests.rs](../../../../finstack-quant/valuations/structured-credit/tests/structured_credit/unit/instrument_pool_tests.rs). These are behavior/formula/invariant tests; no test was rerun for this audit. Where only a test location was inspected, no independent reference validation is claimed.

Reference context: [fixed-income option context](../../../REFERENCES.md#fabozzi-fixed-income-handbook). These references identify the method family; they do not certify the preset parameters or every implementation branch.

## Follow-up tracking

Adoption questions, implementation reconciliation and validation work are tracked in
the [structured-credit follow-ups](../FOLLOW_UPS.md#structured-credit).
