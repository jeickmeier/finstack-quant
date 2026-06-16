# Lookback Option

## Features

- Fixed- and floating-strike lookback options with call/put payoffs, optional observed min/max for seasoned positions.
- Uses discount, vol, and dividend inputs plus notional scaling; supports continuous monitoring closed forms.
- Optional Monte Carlo GBM pricer for path-dependent verification.

## Methodology & References

- Continuous-monitoring analytic formulas for fixed/floating lookbacks under GBM assumptions.
- Monte Carlo path-dependent pricer with exact GBM discretization as a fallback.
- Deterministic market data; no stochastic volatility or jumps.

## Usage Example

```rust
use finstack_quant_valuations::instruments::exotics::lookback_option::LookbackOption;

let option = LookbackOption::example().unwrap();
let pv = option.value(&market_context, as_of_date)?;
```

## Limitations / Known Issues

- Analytical formulas assume continuous monitoring; discrete monitoring bias not explicitly adjusted.
- Monte Carlo path may need sufficient steps for barrier-like sensitivity.
- European payoff only; no early exercise.

## Metrics

- PV plus Greeks (delta/gamma/vega/theta/rho) from analytic formulas; MC bump-and-revalue available when enabled.
- Path stats (expected min/max, payoff distribution) via MC path capture.
- Scenario shocks on spot/vol/rates through registry bump hooks.
