# FX Barrier Option

## Features

- Up/down, in/out FX barrier options with optional rebate and Gobet–Miri adjustment toggle.
- Separate domestic/foreign discount curves, FX spot ID, and FX vol surface; supports call/put on the FX rate.
- Explicit dispatch by monitoring mode:
  - `use_gobet_miri = false`: analytical Reiner–Rubinstein (continuous monitoring)
  - `use_gobet_miri = true`: Monte Carlo discrete-monitoring-corrected pricing

## Methodology & References

- Reiner & Rubinstein (1991) closed-form formulas adapted to FX (Garman–Kohlhagen carry).
- Optional Gobet–Miri (2001) barrier shift for discrete monitoring.
- Monte Carlo fallback uses the shared path-dependent engine under GBM assumptions.

## Usage Example

```rust
use finstack_valuations::instruments::fx::fx_barrier_option::FxBarrierOption;

let option = FxBarrierOption::example();
let pv = option.value(&market_context, as_of_date)?;
```

## Limitations / Known Issues

- Assumes log-normal FX dynamics with deterministic domestic/foreign rates.
- Discrete-monitoring mode requires `mc`; otherwise pricing returns a validation error instead of silently switching models.
- Monte Carlo path does not model stochastic volatility or jumps.
- European-style payoff only; no early exercise or windowed monitoring beyond the supplied schedule.

## Metrics

- PV plus Greeks (delta/gamma/vega/theta/rho) analytically; MC bump-and-revalue when enabled.
- Barrier hit probability proxies via MC path statistics; carry sensitivity via domestic/foreign curve bumps.
- Scenario PVs for barrier shifts and vol surface tweaks.
