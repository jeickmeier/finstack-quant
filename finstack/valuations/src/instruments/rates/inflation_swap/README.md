# Inflation Swap (Zero-Coupon)

## Features

- Zero-coupon inflation swap exchanging fixed real rate versus cumulative inflation over life, with configurable lag override and base CPI.
- Supports standard lags (3m/8m) via `InflationLag`, business-day adjustments, and day-count selection for fixed leg compounding.
- Pay/receive direction controlled via `PayReceive`; integrates inflation index and discount curves from `MarketContext`.

## Methodology & References

- PV = discounted difference between inflation leg `CPI(T)/CPI(0)` and compounded fixed leg `(1+fixed)^τ`, consistent with market zero-coupon structures.
- Inflation projections taken from supplied inflation curve/index with lag/interpolation applied; discounting via chosen discount curve.
- Deterministic framework; no seasonality or stochastic CPI beyond the provided curve.

## Usage Example

```rust
use finstack_valuations::instruments::rates::inflation_swap::InflationSwap;

let swap = InflationSwap::example();
let pv = swap.value(&market_context, as_of_date)?;
```

## Limitations / Known Issues

- Assumes deterministic CPI path from the inflation curve; no model for seasonality or stochastic inflation.
- Only zero-coupon structure supported; couponized inflation swaps would need schedule-level extensions.
- No convexity adjustments between real/nominal discounting beyond provided curves.

## Metrics

- PV, break-even inflation (solve fixed rate to zero PV), and DV01 on discount curve.
- Inflation sensitivity via CPI/index curve bumps; lag sensitivity through schedule recomputation.
- Contribution split between fixed and inflation legs for attribution.
