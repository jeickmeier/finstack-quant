# Asian Option

## Features

- Supports arithmetic or geometric averaging via `AveragingMethod`, with optional past fixings for seasoned trades.
- Call and put payoffs on an explicit fixing calendar, using discount and dividend/volatility surfaces from `MarketContext`.
- Analytical pricing (Kemna–Vorst for geometric, Turnbull–Wakeman semi-analytical for arithmetic) with optional GBM Monte Carlo and control variates.

## Methodology & References

- Geometric closed form based on Kemna & Vorst (1990); arithmetic handled with Turnbull & Wakeman (1991) approximation.
- Monte Carlo pricer reuses the shared path-dependent GBM engine with configurable steps and variance reduction.
- Relies on deterministic discount/volatility curves; no stochastic rates or local volatility adjustments.

## Usage Example

```rust
use finstack_quant_valuations::instruments::exotics::asian_option::{AsianOption, AveragingMethod};
use finstack_quant_core::dates::Date;
use time::Month;

let as_of = Date::from_calendar_date(2024, Month::January, 2)?;
let option = AsianOption::example().unwrap();
let pv = option.value(&market_context, as_of)?;
```

## Limitations / Known Issues

- Monte Carlo path requires a compatible Monte Carlo pricer; analytic pricing remains available for supported averaging methods.
- Assumes GBM dynamics with flat volatility; does not model stochastic rates or local volatility.
- European-style payoff only; no early exercise or American-style averaging features.

## Metrics

- PV (base) plus option Greeks (delta, gamma, vega, theta, rho) from analytic or MC paths.
- Scenario PV / bump-and-revalue hooks through the shared metrics registry; supports stressed vol/spot/rate bumps.
- Path diagnostics (payoff distributions, standard error) available when MC is enabled and path capture is on.
