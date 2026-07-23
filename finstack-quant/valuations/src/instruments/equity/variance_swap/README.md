# Variance Swap

## Features

- Forward on realized variance with configurable strike variance, observation frequency, and realized-variance method (e.g., Parkinson).
- Pay/receive direction via `PayReceive`, variance notional in currency units, and explicit start/maturity dates.
- Uses discount curve for PV of payoff `(RealizedVar - StrikeVar) × Notional`.

## Methodology & References

- Realized variance computed from underlying returns per selected `RealizedVarMethod`; annualization follows chosen day-count/frequency.
- Deterministic discounting of the terminal payoff. Forward variance is sourced from a volatility surface via Carr-Madan replication (with ATM and scalar-vol fallbacks), so the instrument declares a volatility dependency.
- Aligns with standard equity variance swap payoff conventions.

## Usage Example

```rust
use finstack_quant_valuations::instruments::equity::variance_swap::VarianceSwap;

let swap = VarianceSwap::example().unwrap();
let pv = swap.value(&market_context, as_of_date)?;
```

## Limitations / Known Issues

- Requires underlying path/realized series from market context; no stochastic simulation in the pricer.
- Assumes continuous compounding approximation for variance; no corridor/conditional variance features.
- Single-currency settlement; quanto or dispersion structures are out of scope.

## Metrics

Registered metrics (see `metrics/mod.rs`): `Vega`, `VarianceVega`, `ExpectedVariance`,
`RealizedVariance`, `VarianceNotional`, `VarianceStrikeVol`, `VarianceTimeToMaturity`,
`Dv01`, `BucketedDv01`.

- `ExpectedVariance` shares `seasoned_expected_variance` with the PV path, so the
  reported expectation always equals the variance implied by the mark (W-32/W-33).
- `Vega` and `VarianceVega` are analytic sensitivities of the PV (chain-rule
  consistent: `vega = variance_vega · 2σ_fwd · 0.01`).
