# VALUATIONS-STRUCTURED-CREDIT-16: Stochastic revolver utilization and draw-option attribution

- **Decision status:** Draft — observed implementation; adoption pending agreement.
- **Implementation status:** Observed in the owning paths below; scope: revolving instruments inside a structured-credit pool.
- **Validation status:** Evidence inspected/located, not executed. Source and selected assertions establish implemented behavior, not general numerical accuracy or market approval.
- **Audit date:** 2026-11-05.
- **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; runtime source unchanged by this audit.

## Choice and rationale

The instrument-path engine links adverse spread changes to future utilization and attributes draw economics by rerunning the waterfall with a counterfactual interest adjustment. This captures where incremental draw-spread value goes across notes; it is not a separately traded option price with an independently calibrated payoff model.

## Algorithm and conventions

For each live revolver and period, two idiosyncratic normals combine with the systematic factor: `eps_spread = -(sqrt(rho) Z + sqrt(1-rho) eta_s)` and `eps_util = c eps_spread + sqrt(1-c²) eta_u`. Spread evolves with the shared CIR quadratic-exponential step. Utilization uses an exact OU step toward `clamp(theta + beta (spread/initial_spread - 1), 0, 1)`, then is clipped to [0,1]. The target uses beginning-period spread. Utilization volatility below 1e-8 freezes utilization entirely, including mean reversion. Maturity forces utilization to zero. Actual funding still obeys liquidity caps.

The actual run records funded draws, period, survival scale and `current spread - initial spread`. A second run reuses the same systematic shocks, per-name engine and instrument RNG. For each prior draw, later interest is augmented by draw amount × spread difference × contract accrual × survival since draw. The funded draw's fair spread is thus anchored to the contractual margin at valuation; it is not the absolute market spread minus the nominal margin input. Each tranche's actual-minus-counterfactual PV is recorded as draw-option cost; deal cost aggregates those path differences.

Spread/rates are decimal annual values, correlations fractions, utilization [0,1]. Clock day count comes from the revolving-credit engine; contractual interest uses the facility day count. Antithetic partners negate process normals and mirror default/prepayment streams.

## Limitations and alternatives

The comparison preserves shocks but changes waterfall income, so coverage, reserves or calls can make attribution nonlinear. Clipping is not reflection or a bounded diffusion; zero-volatility freeze differs from deterministic OU mean reversion. The fair-spread convention is a relative-spread counterfactual. Deterministic contractual utilization is supported; bounded utilization diffusions and a full borrower exercise optimization would require new model assumptions and calibration.

## Implementation and validation evidence

- [structured_credit/pricing/simulation_engine/instrument_paths.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/pricing/simulation_engine/instrument_paths.rs)
- [structured_credit/pricing/simulation_engine/instrument_flows.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/pricing/simulation_engine/instrument_flows.rs)
- [structured_credit/pricing/stochastic/pricer/engine.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/pricing/stochastic/pricer/engine.rs)

Existing evidence: [structured_credit/unit/instrument_pool_option_cost_tests.rs](../../../../finstack-quant/valuations/structured-credit/tests/structured_credit/unit/instrument_pool_option_cost_tests.rs); [structured_credit/unit/instrument_pool_stochastic_tests.rs](../../../../finstack-quant/valuations/structured-credit/tests/structured_credit/unit/instrument_pool_stochastic_tests.rs). These are behavior/formula/invariant tests; no test was rerun for this audit. Where only a test location was inspected, no independent reference validation is claimed.

Reference context: [CIR/QE context](../../../REFERENCES.md#andersen-2008-heston-qe). These references identify the method family; they do not certify the preset parameters or every implementation branch.

Existing zero-option-cost and allocation tests do not establish a unique economic option-value decomposition under nonlinear trigger/call changes.

## Follow-up tracking

Adoption questions, implementation reconciliation and validation work are tracked in
the [structured-credit follow-ups](../FOLLOW_UPS.md#structured-credit).
