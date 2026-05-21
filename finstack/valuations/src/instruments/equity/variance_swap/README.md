# Variance Swap

## Features

- Forward on realized variance with configurable strike variance, observation frequency, and realized-variance method (e.g., Parkinson).
- Pay/receive direction via `PayReceive`, variance notional in currency units, and explicit start/maturity dates.
- Uses discount curve for PV of payoff `(RealizedVar - StrikeVar) × Notional`.

## Methodology & References

- Realized variance computed from underlying returns per selected `RealizedVarMethod`; annualization follows chosen day-count/frequency.
- Deterministic discounting of terminal payoff; no volatility surface dependency inside the instrument.
- Aligns with standard equity variance swap payoff conventions.

## Usage Example

```rust
use finstack_valuations::instruments::equity::variance_swap::VarianceSwap;

let swap = VarianceSwap::example().unwrap();
let pv = swap.value(&market_context, as_of_date)?;
```

## Limitations / Known Issues

- Requires underlying path/realized series from market context; no stochastic simulation in the pricer.
- Assumes continuous compounding approximation for variance; no corridor/conditional variance features.
- Single-currency settlement; quanto or dispersion structures are out of scope.

## Pricing Methodology

- Payoff at maturity: `(RealizedVar - StrikeVar) × Notional`; realized variance computed from observed returns per selected method (e.g., Parkinson) and annualized.
- Discount terminal payoff on chosen curve; deterministic path unless underlying return series provided from market context.
- Side (pay/receive) sets sign; no continuous mark-to-market modeled inside instrument.

## Metrics

- PV, implied variance/vol (solve strike for zero PV), and realized variance diagnostics from input series.
- Delta/vega proxies via bumping underlying price path or strike; DV01 on discount curve via generic calculator.
- Exposure reports in variance and volatility terms for hedging alignment.
