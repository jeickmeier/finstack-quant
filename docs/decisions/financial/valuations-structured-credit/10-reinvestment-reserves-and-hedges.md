# VALUATIONS-STRUCTURED-CREDIT-10: Reinvestment, reserves and hedge cash accounts

- **Decision status:** Draft — observed implementation; adoption pending agreement.
- **Implementation status:** Observed in the owning paths below; scope: period cash accounts, collateral purchases and contractual hedge overlays.
- **Validation status:** Evidence inspected/located, not executed. Source and selected assertions establish implemented behavior, not general numerical accuracy or market approval.
- **Audit date:** 2026-11-05.
- **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; runtime source unchanged by this audit.

## Choice and rationale

The engine treats account movements as cash uses/sources around each period's waterfall. Reinvestment changes future collateral; reserve balances provide liquidity; hedge cash is projected from actual swap contracts. Their shared concern is conservation of cash, not a common return model.

## Algorithm and controls

Period order is external note draws, collateral flows, realized loss/tests, reinvestment/account movements, and waterfall. Ordinary collateral principal can be reinvested during the active period; recoveries are excluded. Amounts needed for named amortizing tranches are reserved first. With no new-collateral assumptions, eligible surviving rows are scaled proportionally subject to price and coupon/price current-yield criteria. With assumptions, cash/price buys par of a new first-lien cohort; maturity is capped at legal final and spread/floor assumptions determine floating terms. The new cohort is unrated unless additional metadata is provided. Instrument collateral does not reinvest.

Reserve targets can be fixed money, fractions of current/original pool, or the maximum of two targets. Opening reserve accrues simple Act/360 interest when an eligible rate is configured, routed to reserve or collections. Reserve and excess-spread accounts can support leading fees and debt interest; replenishment sits in its configured waterfall tier. Excess reserve may release, and final principal support is controlled by the reserve rule. Controlled accumulation traps ordinary principal before its bullet date and releases at the bullet, while recoveries follow their separate path.

Hedge swaps project raw non-notional IRS flows over the deal interval. The hedge notional is fixed or scaled to opening collateral/note balance. Positive hedge cash augments collections; a payable becomes a senior or junior fee recipient. Thus hedge priority affects credit support; it is not merely added to final PV.

## Conventions, limits and alternatives

Cash and par are distinct; purchase prices are percentages, spreads/floors bp, rates decimals. Conservation checks permit residual max(1 currency unit, 1e-9 relative scale). That tolerance is a diagnostic threshold, not an authorized loss budget. Hedging is contractual; no automatic hedge optimization or counterparty-default model is fitted. Proportional survivor scaling differs from purchasing a new underwriting population, and current yield differs from yield-to-maturity.

Supported alternatives are no reinvestment, explicit new-cohort assumptions, and different reserve/hedge priorities. A loan-level replenishment portfolio or forward-market purchase model would require different inputs and validation.

## Implementation and validation evidence

- [structured_credit/pricing/simulation_engine/simulate_period/cash.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/pricing/simulation_engine/simulate_period/cash.rs)
- [structured_credit/pricing/simulation_engine/reserve.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/pricing/simulation_engine/reserve.rs)
- [structured_credit/pricing/simulation_engine/hedges.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/pricing/simulation_engine/hedges.rs)
- [structured_credit/pricing/simulation_engine/conservation.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/pricing/simulation_engine/conservation.rs)
- [structured_credit/types/hedges.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/types/hedges.rs)
- [structured_credit/types/pool.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/types/pool.rs)
- [structured_credit/types/waterfall/rules.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/types/waterfall/rules.rs)

Existing evidence: [structured_credit/unit/reinvestment_tests.rs](../../../../finstack-quant/valuations/structured-credit/tests/structured_credit/unit/reinvestment_tests.rs); [structured_credit/unit/reserve_rules_tests.rs](../../../../finstack-quant/valuations/structured-credit/tests/structured_credit/unit/reserve_rules_tests.rs); [structured_credit/unit/hedge_swap_tests.rs](../../../../finstack-quant/valuations/structured-credit/tests/structured_credit/unit/hedge_swap_tests.rs); [structured_credit/unit/funding_tests.rs](../../../../finstack-quant/valuations/structured-credit/tests/structured_credit/unit/funding_tests.rs). These are behavior/formula/invariant tests; no test was rerun for this audit. Where only a test location was inspected, no independent reference validation is claimed.

Cash conservation does not prove the assumed reinvestment prices or hedge economics.

## Follow-up tracking

Adoption questions, implementation reconciliation and validation work are tracked in
the [structured-credit follow-ups](../FOLLOW_UPS.md#structured-credit).
