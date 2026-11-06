# VALUATIONS-STRUCTURED-CREDIT-05: NPL resolution and reperformance

- **Decision status:** Draft — observed implementation; adoption pending agreement.
- **Implementation status:** Observed in the owning paths below; scope: statistical assets carrying explicit liquidation terms.
- **Validation status:** Evidence inspected/located, not executed. Source and selected assertions establish implemented behavior, not general numerical accuracy or market approval.
- **Audit date:** 2026-11-05.
- **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; runtime source unchanged by this audit.

## Choice and rationale

An NPL row remains dormant until resolution, then splits its balance in expectation between a reperforming slice and a liquidated slice. This is an explicit scenario convention rather than an inferred recovery model.

## Algorithm and domain

The asset produces no contractual interest or scheduled principal before its resolution date, anchored to acquisition/origination or deal closing plus `months_to_resolution`. At resolution, the reperforming probability determines a surviving slice; the remainder liquidates using specified net proceeds after carrying costs. Reperforming collateral begins its resumed schedule in the next period. The state books defaults, recoveries and losses on that timeline rather than through an already-defaulted opening claim.

Validation rejects simultaneous `defaulted`, `default_date` or `recovery_amount`, incompatible currencies/probabilities, instrument-backed collateral, and resolution beyond legal maturity. Acquisition-date anchoring matters for seasoned pools; the same months-to-resolution does not mean the same valuation-date horizon for every row.

## Selection, limitations and conventions

All amounts are in pool currency, probabilities are fractions, time is whole months. Expected reperformance is fractional collateral and can change later amortization/coupon state. The general recovery-lag/default model is a separate path; it must not also account for the same NPL loss. Costs and proceeds are scenario inputs; there is no endogenous collateral-price, foreclosure-cost or borrower-cure model.

## Alternatives and tradeoffs

Initial default claims with known pending recovery are supported separately. A discrete resolution simulation, stochastic recovery distribution or multiple workout stages would represent uncertainty differently; those are not implied by the expected split. Adopting the present model requires choosing whether its deterministic expected balances preserve the relevant nonlinear waterfall effects.

## Implementation and validation evidence

- [structured_credit/types/npl.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/types/npl.rs)
- [structured_credit/types/instrument.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/types/instrument.rs)
- [structured_credit/pricing/simulation_engine/pool_flows.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/pricing/simulation_engine/pool_flows.rs)

Existing evidence: [structured_credit/unit/npl_tests.rs](../../../../finstack-quant/valuations/structured-credit/tests/structured_credit/unit/npl_tests.rs). These are behavior/formula/invariant tests; no test was rerun for this audit. Where only a test location was inspected, no independent reference validation is claimed.

Reference context: [LGD context](../../../REFERENCES.md#schuermann-2004-lgd). These references identify the method family; they do not certify the preset parameters or every implementation branch.

Independent workout datasets and parameter provenance were not established.

## Follow-up tracking

Adoption questions, implementation reconciliation and validation work are tracked in
the [structured-credit follow-ups](../FOLLOW_UPS.md#structured-credit).
