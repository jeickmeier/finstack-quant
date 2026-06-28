# Monte Carlo module layout

Crate-level usage, examples, feature flags, and runtime constraints are in
[`../README.md`](../README.md). This file maps source directories only.

## Entry points

- `prelude` — common engine, RNG, process, discretization, payoff, and pricer imports
- `engine::McEngine` — generic simulation loop
- `pricer::european::EuropeanPricer` — GBM European shortcut
- `traits` — contracts for new processes, schemes, and payoffs

## Cargo features

- Default build: control variates, antithetic pairing (via `McEngineConfig`), full
  process/payoff/pricer surface, Rayon parallel paths.

## Directory map

| Path | Role |
|------|------|
| `barriers/` | Brownian-bridge hits and continuity corrections |
| `discretization/` | Time-stepping and exact transitions |
| `engine/` | `McEngine`, config, path capture |
| `greeks/` | Pathwise, LRM, finite-difference (including CRN paired FD) |
| `payoff/` | Vanilla, Asian, barrier, basket, lookback |
| `pricer/` | European, path-dependent, LSMC, basis functions |
| `process/` | SDE definitions and correlation helpers |
| `rng/` | Philox, Sobol, Poisson, fractional noise helpers |
| `variance_reduction/` | Control variate (always available) |

## Conventions

- Rates and volatilities are decimals; times are year fractions.
- `McEngine::price` takes an undiscounted payoff and a caller-supplied discount
  factor (typically `exp(-rT)` under flat continuous compounding).
- Captured-path statistics describe the retained subset, not necessarily all paths.
