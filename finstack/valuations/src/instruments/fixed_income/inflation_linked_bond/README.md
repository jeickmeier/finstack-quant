# Inflation-Linked Bond

## Features

- Supports multiple indexation methods (Canadian, TIPS, UK, French, Japanese) with standard lags and interpolation rules.
- Deflation protection configurable (none, maturity-only, all payments) plus deflation floors on principal/coupons.
- Uses inflation curves (`InflationCurve`/`InflationIndex`) alongside discount curves for real/nominal cashflow projection.

## Methodology & References

- Cashflows generated with index ratios using lag/interpolation conventions per `IndexationMethod`; discounting via chosen discount curve.
- Aligns with market conventions for linkers (e.g., 3m/8m lag, daily interpolation for TIPS/Canadian).
- Deterministic inflation; no seasonality or stochastic CPI modeled beyond supplied curve/index.

## Usage Example

```rust
use finstack_valuations::instruments::fixed_income::inflation_linked_bond::InflationLinkedBond;

let linker = InflationLinkedBond::example();
let pv = linker.value(&market_context, as_of_date)?;
```

## Limitations / Known Issues

- Assumes provided inflation index/curve already embeds seasonality; no seasonality adjustment inside the module.
- No convexity adjustment for real/nominal conversion; relies on deterministic curves.
- Callable/putable structures are not modeled; use bond module for optionality.

## Metrics

- PV, real yield/par real rate solving, break-even inflation (difference vs nominal curve), and DV01 on discount curve.
- Inflation sensitivity via index/curve bumps; deflation floor value attribution where applicable.
- Accrued indexation and coupon accrual reporting.
