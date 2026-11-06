# Commodity spot-option Black pricing, exercise trees and Schwartz–Smith simulation

- Record: `VALUATIONS-FX-AND-EXOTICS-COMMODITY-SPOT-OPTION`
- Decision status: **Draft — observed implementation, adoption pending**.
- Implementation status: **Observed**, limited to the source paths below and their named delegated kernels.
- Validation status: **Evidence inspected/located, not executed**. Code-level formulas and test assertions are evidence of implementation; no external numerical certification or current test pass is claimed.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice, rationale and scope

European commodity spot options use Black-76 on the delivery forward. American/Bermudan spot exercise uses an observed spot and a carry inferred from that forward. An explicit Schwartz–Smith MC alternative supplies two-factor commodity dynamics.

The implementation establishes the choice; adoption, suitability and accuracy tolerances require agreement. Any tradeoff below is a proposed evaluation criterion, not evidence of prior approval.

## Algorithm and conventions

Resolve a positive quoted forward or curve price at expiry. European price is `Q M DF [FΦ(d1)-KΦ(d2)]` (signed form for put), with expiry/strike volatility. For early exercise resolve spot, recover `r=-ln(DF)/t`, and infer `q=r-ln(F/S)/t`; a near-zero/invalid carry calculation returns `q=r`. Use a Leisen–Reimer binomial tree, default 201 steps, comparing continuation with intrinsic on all American dates or supplied future Bermudan dates.

Recorded exercise resolves the claim before model pricing. Cash exercise locks recorded intrinsic until settlement; physical exercise retains signed `(delivery_forward-K)` exposure until settlement. Post-expiry without a recorded expiry observation errors; on expiry a current spot/forward observation supplies intrinsic. After recorded settlement the claim is zero.

Schwartz–Smith MC requires European exercise and spot. Build the two-factor process from spot, then shift long drift by `ln(F_market/F_model(t))/t` to pin its expected terminal price to the market forward. Use exact Schwartz–Smith steps, uniform configured grid, Philox seed (explicit or 42), parallel MC and European payoff, scaled by quantity×multiplier. Family defaults are 100,000 paths/252 steps; instrument positive path-count override is honored.

Default European delta/vega are analytic forward sensitivities; gamma, vanna and volga reprice quoted forward or the whole price curve and active volatility. Vega is per 0.01 absolute vol, vanna per forward-unit/vol-point, volga per vol-point squared. The selected Schwartz–Smith route only supports forward delta/gamma; Black-surface vega/vanna/volga are rejected because its volatilities are model parameters.

## Selection and controls

The family registers the default `Black76` route, whose instrument method branches by exercise style, plus `MonteCarloSchwartzSmith`. The latter is registered with kappa 1, short-factor vol 0.30, long-factor vol 0.15, correlation 0.30, long drift 0 and short risk premium 0; these are defaults, not a calibration result.

Bermudan dates must be increasing and include a usable future exercise date; non-European exercise requires spot ID. MC is European-only. Diagnostics report scaled standard error, paths, seed and grid; antithetic/Sobol/bridge flags are false for this local engine.

## Limitations, failure and fallback

Registered Schwartz–Smith coefficients are uncalibrated constants. Forward pinning changes drift at each expiry but does not fit the entire forward curve jointly. Early-exercise analytic Greeks are rejected. Near-expiry carry fallback and physical-settlement exposure are separate from the European forward-sensitivity contract.

## Alternatives and tradeoffs

A calibrated term-structure model and dynamic convenience yield can improve cross-maturity consistency at extra parameter/market-data cost. Supported Black and two-factor routes are materially different dynamics; path count alone does not make them equivalent.

## Evidence

- [finstack-quant/valuations/exotics/src/commodity/commodity_spot_option/metrics/mod.rs](../../../../finstack-quant/valuations/exotics/src/commodity/commodity_spot_option/metrics/mod.rs)
- [finstack-quant/valuations/exotics/src/commodity/commodity_spot_option/metrics/selected_model.rs](../../../../finstack-quant/valuations/exotics/src/commodity/commodity_spot_option/metrics/selected_model.rs)
- [finstack-quant/valuations/exotics/src/commodity/commodity_spot_option/pricer.rs](../../../../finstack-quant/valuations/exotics/src/commodity/commodity_spot_option/pricer.rs)
- [finstack-quant/valuations/exotics/src/commodity/commodity_spot_option/types.rs](../../../../finstack-quant/valuations/exotics/src/commodity/commodity_spot_option/types.rs)

- [finstack-quant/valuations/exotics/tests/commodity_spot_option/pricing.rs](../../../../finstack-quant/valuations/exotics/tests/commodity_spot_option/pricing.rs): Black benchmark formula, put-call parity, American/Bermudan ordering and expiry-state assertions.
- [finstack-quant/valuations/exotics/tests/commodity_spot_option/mc_pricing.rs](../../../../finstack-quant/valuations/exotics/tests/commodity_spot_option/mc_pricing.rs): GBM-limit comparison, path-count consistency, seeded replay and MC diagnostic scaling; internal cross-model evidence.

Canonical reference index: [Black 1976](../../../REFERENCES.md#black-1976); [Schwartz Smith](../../../REFERENCES.md#schwartz-smith-2000). The index supplies provenance pointers; agreement on the exact convention and validation domain remains open.

## Follow-up tracking

Adoption, implementation reconciliation and validation work is tracked in the
[FX, commodity and exotic follow-ups](../FOLLOW_UPS.md#fx-commodities-and-exotics).
