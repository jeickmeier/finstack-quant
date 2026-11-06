# FX barrier monitoring, rebate state and analytic/Monte Carlo pricing

- Record: `VALUATIONS-FX-AND-EXOTICS-FX-BARRIERS`
- Decision status: **Draft — observed implementation, adoption pending**.
- Implementation status: **Observed**, limited to the source paths below and their named delegated kernels.
- Validation status: **Evidence inspected/located, not executed**. Code-level formulas and test assertions are evidence of implementation; no external numerical certification or current test pass is claimed.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice, rationale and scope

FX single barriers support analytical continuous-monitoring Reiner–Rubinstein pricing and GBM Monte Carlo with explicit monitoring grids. Observed breach state determines whether the remaining claim is a vanilla, a rebate or a live barrier.

The implementation establishes the choice; adoption, suitability and accuracy tolerances require agreement. Any tradeoff below is a proposed evaluation criterion, not evidence of prior approval.

## Algorithm and conventions

Resolve positive FX spot (explicit scalar ID or matrix), terminal-implied domestic/foreign rates, and strike volatility. Before simulation, use recorded breach: knocked-in becomes vanilla GK; knocked-out at-hit rebate is already paid and worth zero; at-expiry rebate remains `rebate×D_d`. At expiry explicit breach state controls intrinsic/rebate; after expiry zero is returned.

Analytical price is `barrier_price(BarrierParams(S,K,H,t,r_d,r_f,σ),type,callput)` plus the timing-specific `barrier_rebate`. Both kernels are audited with the models closed-form family. MC simulates GBM drift `r_d-r_f` and volatility `σ`; constructs continuous/date-restricted `BarrierOptionPayoff`, includes rebate per base unit and total notional, and discounts the payoff to expiry. At-hit rebate requests pass the domestic rate into the payoff.

MC uses merged path settings, instrument-derived seed, and the monitoring grid. Diagnostics expose standard error, estimator/simulated path counts, seed, actual time grid and antithetic/Sobol/bridge settings. Risk bumps the active spot source and volatility source, uses central delta/gamma and mixed vanna/volga repricing, and separates domestic/foreign curve rho from generic DV01.

## Selection and controls

Default selection follows the instrument monitoring/model contract; the family registers both `FxBarrierBSContinuous` and `MonteCarloGBM`. Direct `compute_pv` uses MC. Analytical pricing rejects discrete dates and future-start monitoring; MC builds the requested window/date grid.

Continuous monitoring requires a start date; seasoned monitoring requires `observed_barrier_breached`. Discrete dates are inspected for elapsed monitoring. The MC engine handles path randomness/continuous crossing treatment; local configuration is not evidence of a converged barrier probability. Rebate money is divided by notional for per-unit payoff construction.

## Limitations, failure and fallback

Flat terminal-implied domestic/foreign rates are used in FX barrier paths; a nonflat curve path is not preserved as a time-varying drift here. Monitoring inclusion, paid-at-hit state and rebate units follow the observed lifecycle above. Seeded reproducibility does not establish accuracy near barriers or convergence on sparse discrete schedules.

## Alternatives and tradeoffs

Continuous analytic pricing is fast within its domain; discrete MC is supported without pretending a continuity correction is exact. Time-dependent-rate PDE/local-volatility or stochastic-rate FX alternatives are not implemented by these local pricers.

## Evidence

- [finstack-quant/valuations/fx/src/fx_barrier_option/metrics.rs](../../../../finstack-quant/valuations/fx/src/fx_barrier_option/metrics.rs)
- [finstack-quant/valuations/fx/src/fx_barrier_option/pricer.rs](../../../../finstack-quant/valuations/fx/src/fx_barrier_option/pricer.rs)
- [finstack-quant/valuations/fx/src/fx_barrier_option/types.rs](../../../../finstack-quant/valuations/fx/src/fx_barrier_option/types.rs)

- [finstack-quant/valuations/fx/tests/fx_barrier_option/lifecycle.rs](../../../../finstack-quant/valuations/fx/tests/fx_barrier_option/lifecycle.rs): Observed-state, timing and expired-claim regressions.
- [finstack-quant/valuations/fx/tests/fx_barrier_option/greek_sources.rs](../../../../finstack-quant/valuations/fx/tests/fx_barrier_option/greek_sources.rs): Active spot/volatility source sensitivity assertions.
- [finstack-quant/valuations/fx/tests/fx_barrier_option/vanna_convention.rs](../../../../finstack-quant/valuations/fx/tests/fx_barrier_option/vanna_convention.rs): Mixed derivative normalization regression.

Canonical reference index: [Reiner Rubinstein](../../../REFERENCES.md#reiner-rubinstein-1991); [Glasserman](../../../REFERENCES.md#glasserman-2004-monte-carlo). The index supplies provenance pointers; agreement on the exact convention and validation domain remains open.

## Follow-up tracking

Adoption, implementation reconciliation and validation work is tracked in the
[FX, commodity and exotic follow-ups](../FOLLOW_UPS.md#fx-commodities-and-exotics).
