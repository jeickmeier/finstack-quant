# Cliquet Option

## Features

- Periodic-reset option with local caps/floors and global caps/floors on cumulative return, configurable payoff aggregation (additive or multiplicative).
- Uses explicit reset schedule, discount/volatility/dividend inputs, and notional currency alignment.
- Monte Carlo GBM pricer with piecewise parameters to handle term-structure of rates/dividends/vols.

## Methodology & References

- Path-dependent pricing through the shared Monte Carlo engine (exact GBM discretization with Philox RNG and configurable time grid).
- Supports additive and multiplicative payoff accumulation consistent with market cliquet structures.
- Deterministic market data; no stochastic volatility or jump processes.

## Usage Example

```rust
use finstack_valuations::instruments::equity::cliquet_option::CliquetOption;
use finstack_core::dates::Date;
use time::Month;

let as_of = Date::from_calendar_date(2024, Month::January, 2)?;
let option = CliquetOption::example().unwrap();
let pv = option.value(&market_context, as_of)?;
```

## Limitations / Known Issues

- Pricing requires the `mc` feature; no closed-form approximation is provided.
- Assumes GBM dynamics with deterministic dividend yield; no stochastic vol or jumps.
- No early exercise; payoff is strictly European at final reset/payment.

## Pricing Methodology

- Monte Carlo GBM with piecewise parameters for term-structured rates/dividends/vols; exact discretization with Philox RNG.
- Payoff aggregates local period returns (capped/floored) and global cap/floor based on additive or multiplicative rule.
- Discounting via instrument curve; optional dividend yield and vol surface drive drift/vol inputs.

## Metrics

- PV plus MC bump-and-revalue Greeks (delta/vega/theta) and path statistics (expected local/global payoffs).
- Probability of hitting local/global caps/floors derivable from captured paths when enabled.
- Scenario sensitivities to vol and spot via registry bump hooks.
