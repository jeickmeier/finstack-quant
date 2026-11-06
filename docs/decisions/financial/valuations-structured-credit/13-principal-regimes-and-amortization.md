# VALUATIONS-STRUCTURED-CREDIT-13: Step-down, shifting-interest and early-amortization regimes

- **Decision status:** Draft — observed implementation; adoption pending agreement.
- **Implementation status:** Observed in the owning paths below; scope: rule-driven changes to principal allocation.
- **Validation status:** Evidence inspected/located, not executed. Source and selected assertions establish implemented behavior, not general numerical accuracy or market approval.
- **Audit date:** 2026-11-05.
- **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; runtime source unchanged by this audit.

## Choice and rationale

The engine resolves a fresh waterfall for the current period using the live collateral/note state. Step-down and shifting-interest are conditional allocation formulas, while early amortization is a persistent event. Those state semantics differ and require explicit agreement.

## Algorithm and controls

Step-down becomes active on/after the specified date only if every supplied loss, OC, credit-enhancement and delinquency predicate passes. It switches principal to pro-rata current-balance weights; a later failed predicate can reverse it. Shifting-interest chooses the most recent dated schedule step measured in months from closing. In the default `ShiftOfSubordinate` mode, scheduled principal stays at the senior's pro-rata share, while the schedule shifts a fraction of subordinate unscheduled principal to senior. `SeniorShare` directly specifies the senior unscheduled share. The blend uses the period's unscheduled-principal fraction, clamped to [0,1]; remaining principal weights go to surviving subordinate balances. Trigger failure restores protective allocation.

Target OC computes `target = max(fraction_current × current_pool, floor_fraction_original × original_pool)`. Required note paydown is `max(0, notes - max(pool - target, 0))`, allocated in note priority and implemented as principal target balances. Controlled accumulation caps principal claims during the accumulation window and releases cash at the bullet.

Early amortization fires when cumulative realized loss/original pool reaches the loss threshold or the trailing realized annualized excess spread falls below its floor. It permanently ends revolving/reinvestment, even if later metrics improve. Despite the field name `min_excess_spread_3m`, the code averages the last three **payment periods**; quarterly deals therefore use a different calendar horizon from monthly deals.

## Conventions, limitations and alternatives

Loss/CE/delinquency limits and shifting shares are decimal fractions; OC is a ratio. Date boundaries and inclusive comparisons matter. Combining rules is order-sensitive: weights, target balances and lockouts modify the same waterfall. No rule interprets a legal covenant automatically. Pure sequential, unconditionally pro-rata and omitted optional regimes are supported alternatives; an event-driven daily covenant monitor is not implemented here.

## Implementation and validation evidence

- [structured_credit/pricing/resolve.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/pricing/resolve.rs)
- [structured_credit/pricing/simulation_engine/simulate_period/losses.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/pricing/simulation_engine/simulate_period/losses.rs)
- [structured_credit/pricing/simulation_engine/state.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/pricing/simulation_engine/state.rs)
- [structured_credit/types/waterfall/rules.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/types/waterfall/rules.rs)

Existing evidence: [structured_credit/unit/shifting_interest_tests.rs](../../../../finstack-quant/valuations/structured-credit/tests/structured_credit/unit/shifting_interest_tests.rs); [structured_credit/unit/target_oc_tests.rs](../../../../finstack-quant/valuations/structured-credit/tests/structured_credit/unit/target_oc_tests.rs); [structured_credit/unit/reserve_rules_tests.rs](../../../../finstack-quant/valuations/structured-credit/tests/structured_credit/unit/reserve_rules_tests.rs); [structured_credit/unit/card_master_trust_tests.rs](../../../../finstack-quant/valuations/structured-credit/tests/structured_credit/unit/card_master_trust_tests.rs). These are behavior/formula/invariant tests; no test was rerun for this audit. Where only a test location was inspected, no independent reference validation is claimed.

## Follow-up tracking

Adoption questions, implementation reconciliation and validation work are tracked in
the [structured-credit follow-ups](../FOLLOW_UPS.md#structured-credit).
