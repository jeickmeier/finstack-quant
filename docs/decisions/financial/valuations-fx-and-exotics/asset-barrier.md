# Asset barrier analytic, Monte Carlo and log-space PDE algorithms

- Record: `VALUATIONS-FX-AND-EXOTICS-ASSET-BARRIER`
- Decision status: **Draft — observed implementation, adoption pending**.
- Implementation status: **Observed**, limited to the source paths below and their named delegated kernels.
- Validation status: **Evidence inspected/located, not executed**. Code-level formulas and test assertions are evidence of implementation; no external numerical certification or current test pass is claimed.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice, rationale and scope

Asset barriers support continuous analytic pricing, GBM/Heston Monte Carlo, and a one-dimensional Black–Scholes PDE with explicit discrete monitoring projections. All share observed breach and realized expiry state but retain model-specific dynamics.

The implementation establishes the choice; adoption, suitability and accuracy tolerances require agreement. Any tradeoff below is a proposed evaluation criterion, not evidence of prior approval.

## Algorithm and conventions

Resolve expiry DF, instrument-time effective rate, spot, optional dividend yield and active expiry/strike volatility. Recorded knock-out pays only an unpaid at-expiry rebate; at-hit rebate is already settled. Recorded knock-in becomes vanilla in analytic/GBM/PDE paths. Expiry requires breach state and, after expiry, an explicit expiry fixing; the realized claim is retained. Total cash rebate is scaled separately from quantity; MC converts it to per-unit rebate.

Analytical value calls the shared barrier and rebate formulas. GBM uses a daily date-derived cumulative drift `-ln DF(date)-q t`; the monitoring helper builds continuous/discrete payoff grids. Heston uses strict parameters, QE discretization, the same drift schedule, initial variance and observed breach state; 100,000 default paths, 252 steps/year, minimum ten steps and instrument-derived seed. Continuous crossing treatment belongs to the shared payoff, with volatility/state inputs provided here.

PDE evolves log spot with diffusion `σ²/2`, convection `r-q-σ²/2`, reaction `-r`. Continuous knock-out domain ends exactly at the barrier, with Dirichlet boundary equal to rebate (at-hit) or rebate discounted from expiry. Other boundaries use zero/linear asymptotes. The grid is sinh-concentrated near strike, width `max(5σ√t,0.5)`; discrete monitoring instead uses a wider domain around barrier/spot/strike with width `max(7σ√t,1)` and concentration 0.1. Two Rannacher implicit steps precede Crank–Nicolson.

Discrete PDE merges uniform times and observation times, runs backward, resets hit nodes to rebate only at observation events and restarts Rannacher smoothing after each projection. Knock-in uses same-grid vanilla minus zero-rebate knock-out plus survival-rebate value; parity below `-1e-6×max(abs(vanilla),1)` errors, tiny negatives clamp to zero. Continuous parity extends the grid while preserving knock-out nodes. Interpolate solved value at log spot.

Generic finite-difference Greeks and curve DV01/key rates reprice the selected model. Theta carries observed state using held spot at crossed observations; it does not infer an actual historical market path.

## Selection and controls

Continuous monitoring defaults to `BarrierBSContinuous`; discrete to GBM MC. Explicit alternatives are `PdeCrankNicolson1D` and Heston MC. Analytical pricing rejects discrete monitoring. The PDE uses internal default 200 space points and 100 time steps, rather than instrument tree settings.

Strike, barrier, quantity and fixing are positive; rebate finite/nonnegative and currency matched; discrete dates sorted/nonempty/no later than expiry. Shared GBM defaults are listed in the Asian record. Heston path count must be valid. PDE numerical parameters are fixed private fields in the registered default.

## Limitations, failure and fallback

Analytic/PDE constant effective rates differ from datewise-drift MC on nonflat curves. Continuous equity barriers lack an explicit monitoring-start field/history requirement; missing breach state can therefore mean unseasoned even if the economic trade is seasoned. The retained post-expiry claim, paid-rebate treatment and fixed PDE domain/grid are observed implementation choices, not independently qualified lifecycle or convergence guarantees.

## Alternatives and tradeoffs

Analytic formulas are efficient for flat continuous coefficients; PDE and MC support explicit discrete schedules. Local-volatility/time-dependent PDE and stochastic-rate barrier models are absent. Heston volatility does not make its continuous-crossing approximation exact.

## Evidence

- [finstack-quant/valuations/exotics/src/exotics/barrier_option/heston_mc_pricer.rs](../../../../finstack-quant/valuations/exotics/src/exotics/barrier_option/heston_mc_pricer.rs)
- [finstack-quant/valuations/exotics/src/exotics/barrier_option/metrics.rs](../../../../finstack-quant/valuations/exotics/src/exotics/barrier_option/metrics.rs)
- [finstack-quant/valuations/exotics/src/exotics/barrier_option/pde_pricer.rs](../../../../finstack-quant/valuations/exotics/src/exotics/barrier_option/pde_pricer.rs)
- [finstack-quant/valuations/exotics/src/exotics/barrier_option/pricer.rs](../../../../finstack-quant/valuations/exotics/src/exotics/barrier_option/pricer.rs)
- [finstack-quant/valuations/exotics/src/exotics/barrier_option/types.rs](../../../../finstack-quant/valuations/exotics/src/exotics/barrier_option/types.rs)

- [finstack-quant/valuations/exotics/tests/barrier_option/test_day_count_basis.rs](../../../../finstack-quant/valuations/exotics/tests/barrier_option/test_day_count_basis.rs): Instrument/curve day-count regression comparisons.
- [finstack-quant/valuations/exotics/tests/barrier_option/theta_observed_state.rs](../../../../finstack-quant/valuations/exotics/tests/barrier_option/theta_observed_state.rs): Theta equals held-spot explicit observed-state repricing; internal consistency, not external market-path evidence.

Canonical reference index: [Reiner Rubinstein](../../../REFERENCES.md#reiner-rubinstein-1991); [Andersen QE](../../../REFERENCES.md#andersen-2008-heston-qe); [Press](../../../REFERENCES.md#press-numerical-recipes). The index supplies provenance pointers; agreement on the exact convention and validation domain remains open.

## Follow-up tracking

Adoption, implementation reconciliation and validation work is tracked in the
[FX, commodity and exotic follow-ups](../FOLLOW_UPS.md#fx-commodities-and-exotics).
