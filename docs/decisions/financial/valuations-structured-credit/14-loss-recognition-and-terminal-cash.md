# VALUATIONS-STRUCTURED-CREDIT-14: Loss recognition, deal calls and terminal recovery treatment

- **Decision status:** Draft — observed implementation; adoption pending agreement.
- **Implementation status:** Observed in the owning paths below; scope: note balances, cleanup/forced calls and legal-final resolution.
- **Validation status:** Evidence inspected/located, not executed. Source and selected assertions establish implemented behavior, not general numerical accuracy or market approval.
- **Audit date:** 2026-11-05.
- **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; runtime source unchanged by this audit.

## Choice and rationale

Loss recognition and note-balance writedown are independent choices. RMBS/CMBS default to recognition at liquidation and note writedown; CLO/CBO/ABS/auto/card default to recognition at default with par-preserving notes. Par-preserving notes continue accruing contractual coupon until repaid or finally written down. This observed family dispatch needs deal-specific adoption.

## Algorithm and conventions

Incremental recognized net loss subtracts opening historical loss and already allocated writedowns, then is allocated bottom-up in note priority, capped at live outstanding principal. Principal repayments and previous writedowns therefore reduce remaining absorption capacity. Under par preservation, realized losses still affect tests but do not immediately reduce note coupon bases. Final unpaid face is written down regardless of interim policy.

A dated deal call or cleanup decision is made at a period boundary, then the period is simulated and remaining collateral liquidated. Cleanup requires proceeds sufficient for applicable claims; a forced contractual scenario call may create losses. Price and premium terms are percentages of par. Deal/cleanup calls include pending recovery amounts in immediate proceeds and drain those claims on redemption. Reserve, excess-spread and accumulated cash are released according to their terminal rules before unresolved balances are closed.

The loop can stop when the remaining performing pool is at or below 100 currency units. Ordinary terminal completion first releases current cash accounts, then preserves each pending recovery's full contractual lag: payment occurs at the calendar-adjusted later of default date plus lag and the last simulated payment date. Final unpaid-face writedown is dated at the last such recovery settlement, which may be after legal maturity. This differs from the immediate pending-claim treatment at calls. Residual flow recording identifies the maximum payment-priority class; this does not establish correct residual treatment for structures with no explicit equity or unusual residual placement.

## Selection, limitations and alternatives

Explicit `LossAllocationPolicy`, `LossRecognition`, call scope/date and liquidation price are supported. `CallScope` can limit redemption behavior; model inputs validate dates and debt IDs. Including pending recovery amounts immediately at a call is not the same as discounting or selling those claims. Ordinary completion permits recovery cash and final writedown after legal maturity; whether that matches each note's legal entitlement is a separate decision. Pool-exhaustion thresholds are absolute currency amounts and can materially affect very small deals. Tranche hypothetical call metrics are separate calculations and can treat premium as principal; they must not be confused with the deal liquidation path.

Recognize-at-default versus at-liquidation and writedown versus par preservation are implemented alternatives. Ordinary completion already realizes pending claims at lagged dates beyond legal final. Market-value disposal at final, or immediate discounted recovery proceeds at a call, would be different choices.

## Implementation and validation evidence

- [structured_credit/types/enums.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/types/enums.rs)
- [structured_credit/types/call.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/types/call.rs)
- [structured_credit/pricing/simulation_engine/orchestration.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/pricing/simulation_engine/orchestration.rs)
- [structured_credit/pricing/simulation_engine/simulate_period/losses.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/pricing/simulation_engine/simulate_period/losses.rs)
- [structured_credit/pricing/simulation_engine/simulate_period/record.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/pricing/simulation_engine/simulate_period/record.rs)
- [structured_credit/utils/simulation.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/utils/simulation.rs)

Existing evidence: [structured_credit/unit/loss_allocation_tests.rs](../../../../finstack-quant/valuations/structured-credit/tests/structured_credit/unit/loss_allocation_tests.rs); [structured_credit/unit/call_tests.rs](../../../../finstack-quant/valuations/structured-credit/tests/structured_credit/unit/call_tests.rs); [structured_credit/integration/simulation_edge_tests.rs](../../../../finstack-quant/valuations/structured-credit/tests/structured_credit/integration/simulation_edge_tests.rs). These are behavior/formula/invariant tests; no test was rerun for this audit. Where only a test location was inspected, no independent reference validation is claimed.

The delayed-loss recognition tests establish booking behavior, not the legal or economic appropriateness of either terminal policy.

## Follow-up tracking

Adoption questions, implementation reconciliation and validation work are tracked in
the [structured-credit follow-ups](../FOLLOW_UPS.md#structured-credit).
