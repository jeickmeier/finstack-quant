# VALUATIONS-STRUCTURED-CREDIT-16: Stochastic revolver utilization and draw-option attribution

- **Decision status:** Accepted 2026-11-15 — complete revolver utilization, funding and matched draw-spread attribution package.
- **Implementation status:** Observed in the owning paths below. Reconciliation to the accepted contract is pending.
- **Validation status:** Independent utilization, funding, tagged balance and tranche PV calculations checked. Production credit, nonlinear replay, empirical calibration and host qualification remain pending.
- **Audit date:** 2026-11-05.
- **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; runtime source unchanged by this audit.

## Accepted decision for revolver utilization and draw-spread attribution

1. **Elect the funding and spread laws explicitly.** Select contractual
   funding or qualified stochastic utilization for each eligible revolver.
   In the stochastic route select a literal constant spread, including
   zero, or checked CIR with the shared quadratic-exponential step.
   Market anchoring requires explicit election, the qualified hazard
   source, dated fit horizon and residual tolerance of the one-horizon
   mean fit. Remove inferred parameters, spread floors and extraction
   of correlations from standalone model inputs. Modeled spread,
   contractual margin and authoritative default source are separate.
   Use deterministic dated relative discounting under record 15.
   [Accepted revolver credit model](../valuations-other-fixed-income/11-revolver-monte-carlo-credit.md#accepted-decision-for-checked-revolver-simulation-and-default-valuation) ·
   [Accepted stochastic waterfall](15-stochastic-waterfall.md#accepted-decision-for-stochastic-waterfall-pricing-and-estimation).

2. **Use a frozen opening target and retain mean reversion at zero volatility.**
   Use actual ACT/365F model steps and the facility's own cash accrual clock.
   For opening spread `s_a`, set the interval mean target to
   `clip(θ+β(s_a−s_ref)/s_ref,0,1)` with a strictly positive frozen
   initial reference for nonzero `β`. With zero sensitivity use `θ`
   directly. Reuse the checked exact frozen-target OU proposal and clip the
   receiving value to `[0,1]`. The receiving spread first affects the next
   interval. Remove the `1e−8` utilization freeze. Exact zero volatility
   retains deterministic mean reversion in utilization and CIR spread.
   Clipping produces boundary mass and grid-dependent behavior.
   [Accepted checked credit-line transitions](../models-monte-carlo/11-credit-line.md).

3. **Use fresh innovations on qualified matching intervals.** For elected
   active innovations use `z_s=−(sqrt(ρ_s)G+sqrt(1−ρ_s)η_s)` and
   `z_u=c z_s+sqrt((1−c)(1+c))η_u`, where the drivers are fresh standard
   normals with independent common and idiosyncratic roles. Require
   `ρ_s ∈ [0,1]`, `c ∈ [−1,1]` and exact endpoint handling.
   Share a fresh credit innovation only on matching declared intervals
   and draw assignments. Persistent factor levels change the conditional
   innovation law. Separate these loadings from copula correlation and
   reject links to inactive innovations. Use stable draw addresses and
   antithetic reflection. The modeled stress link requires qualified
   inputs for its sign, size, regime and pricing measure.
   [Empirical corporate credit lines](https://www.bde.es/informes/be/docs/dt0821e.pdf).

4. **Fund only at authorized dates and phases.** Keep latent utilization
   separate from funded par. Cap a qualified funding candidate by live
   commitment less outstanding letters of credit and apply authorized
   funding or repayment. Use eligible reserve cash first, then eligible
   principal already available. Give mandatory claims priority and fail
   valuation for unfundable mandatory claims. Cap discretionary requests
   only when the policy permits shortfall. Keep default conversions on
   disjoint exposures and enter funded conversion into default and
   recovery once. Reset utilization from actual funded par over live
   commitment after funding. Validate zero commitment and any authorized
   restart without division. Suppress discretionary funding at integration-only
   or terminal nodes. Report request, funding and shortfall separately
   from unused capacity and the physical-path shortfall fraction.
   [Accepted collateral funding](08-instrument-collateral.md#accepted-decision-for-instrument-collateral-and-liquidity) ·
   [Filed funding-account terms, §2.17](https://www.sec.gov/Archives/edgar/data/1974793/000119312525172599/d18620dex101.htm).

5. **Track spread attribution on outstanding draw par.** Replace
   `draw_option_cost` with explicitly requested `draw_spread_attribution`.
   On the reference path, each qualifying modeled discretionary draw
   that funds adds `draw × (spread at funding − frozen anchor spread)`
   to signed tagged spread exposure. Apply the held tag to the original
   contractual accrual slices and actual payable dates under a qualified
   additive linear margin. Opening funded par, mandatory draws, default
   conversions and unfunded requests add no tag. On fungible principal,
   authorized retirement of `r` from prior balance `D` scales tags by
   `(D−r)/D`. Full retirement clears tags exactly. Require qualified
   event order and balance allocation. Interacting all-in caps and floors
   or unresolved allocation make the requested attribution unqualified.
   [Accepted standalone spread attribution](../valuations-other-fixed-income/12-revolver-quotes-risk.md#accepted-decision-for-checked-revolver-quotes-exposure-and-risk).

6. **Replay the full waterfall under the reference margin.** Match opening
   state, contracts, market inputs and logical shocks. Change the draw-margin
   rule and recompute funding, coverage, reserves and calls. Tag each
   reference path's own funded draws so its post-action balances determine
   the margin adjustment. Report actual PV minus reference PV in signed
   deal currency by note and reconcile the sum within each trajectory.
   The measure includes the spread intervention's waterfall effects.
   Estimate uncertainty from matched path differences using record 15's
   independent units. A qualified complete zero is available. An
   unrequested result is absent. A failed requested replay or missing
   note fails the requested calculation. Reconcile the renamed result,
   optional path capture and funding diagnostics across Rust, Python,
   WASM and stored results.

## Independent calculation checks

- Opening utilization 20%, target 70% and annual mean reversion `ln 2`
  give 45% utilization after a one-year zero-volatility interval.
  With commitment 100, LC use 20 and opening funded par 20, the request
  is 25. Funding only 9 leaves par 29 and resets utilization to 29%.
- A discretionary draw of 20 at a 3 percentage-point widening gives
  a spread tag of 0.60. Repaying 60 of the total 120 funded par
  reduces the tag to 0.30. A subsequent mandatory draw adds no tag.
  A quarter-year accrues 0.075 of reference interest. With actual
  interest 0.95, a senior cap of 1.00 and discount factor 0.98,
  actual-minus-reference PV is −0.049 senior, −0.0245 residual,
  and −0.0735 total.

## Observed implementation and scope

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

Implementation reconciliation and validation work are tracked in
the [structured-credit follow-ups](../FOLLOW_UPS.md#structured-credit).
