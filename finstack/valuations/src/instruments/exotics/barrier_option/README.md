# Barrier Option

## Features

- Supports up/down and in/out structures with optional cash rebate, governed by `BarrierType`.
- Call/put payoffs with explicit barrier level, Gobet–Miri adjustment toggle, and optional dividend yield.
- Explicit dispatch by monitoring mode:
  - `use_gobet_miri = false`: analytical Reiner–Rubinstein (continuous monitoring)
  - `use_gobet_miri = true`: Monte Carlo discrete-monitoring-corrected pricing (`mc` feature required)

## Methodology & References

- Closed-form valuation based on Reiner & Rubinstein (1991) for continuously monitored barriers.
- Optional Gobet–Miri (2001) correction for discrete monitoring via the `use_gobet_miri` flag.
- Monte Carlo path-dependent pricer from the shared GBM engine for cases where analytics are insufficient.

## Usage Example

```rust
use finstack_valuations::instruments::exotics::barrier_option::BarrierOption;
use finstack_core::dates::Date;
use time::Month;

let as_of = Date::from_calendar_date(2024, Month::January, 2)?;
let option = BarrierOption::example().unwrap();
let pv = option.value(&market_context, as_of)?;
```

## Limitations / Known Issues

- Continuous analytics assume log-normal GBM dynamics.
- Discrete-monitoring mode requires `mc`; without it, pricing returns an explicit validation error (no silent fallback).
- Monte Carlo pricing requires the `mc` feature and does not model stochastic volatility or jumps.
- No American-style early exercise; rebates are paid at expiry only.

## Pricing Methodology

- Reiner–Rubinstein continuous-monitoring closed forms under GBM with dividend yield for up/down, in/out structures.
- Optional Gobet–Miri shift to approximate discrete monitoring; Monte Carlo GBM fallback for complex setups.
- Discounting via domestic curve; vol/dividend from market surfaces with clamping at extreme strikes/tenors.
- Expired contracts require `observed_barrier_breached`; terminal spot alone is not enough to reconstruct realized barrier state.
- Zero-vol and very-low-vol analytical paths are resolved deterministically to avoid undefined touch-probability/rebate behavior.

## Metrics

- PV plus Greeks (delta/gamma/vega/theta/rho) from analytical formulas; MC Greeks via bump-and-revalue when enabled.
- Barrier sensitivities (vanna/volga-style) accessible through surface bumps; digital probability of knock-in/out observable from MC paths.
- Scenario PVs for barrier shifts and vol skews supported through registry bump hooks.
