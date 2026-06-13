# Quanto Option

## Features

- Equity option whose payoff is settled in a different currency with explicit equity/FX correlation input.
- Separate domestic and foreign discount curves, equity vol surface, optional FX vol and FX rate IDs, and dividend yield support.
- Analytical quanto-adjusted Black-Scholes pricing with optional Monte Carlo.

## Methodology & References

- Quanto adjustment applies correlation between equity and FX plus FX volatility to modify drift; priced with Black–Scholes in domestic currency.
- Deterministic discounting on domestic/foreign curves; optional FX vol provides volatility-of-vol adjustment.
- Aligns with standard quanto equity option practice (Garman–Kohlhagen style with correlation shift).

## Usage Example

```rust
use finstack_valuations::instruments::fx::quanto_option::QuantoOption;

let option = QuantoOption::example();
let pv = option.value(&market_context, as_of_date)?;
```

## Limitations / Known Issues

- Correlation assumed constant; no stochastic correlation or local-vol effects.
- Monte Carlo path requires a compatible Monte Carlo pricer; otherwise use the analytic path.
- No early exercise support; payoff is European.

## Metrics

- PV plus Greeks (delta/gamma/vega/theta/rho) to equity and FX via analytic or MC bump-and-revalue.
- Correlation and FX vol sensitivity through scenario bumps; implied vol solver in domestic currency.
- DV01 on domestic curve for discounting exposure.
