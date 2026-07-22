# Inflation-Linked Bond

## Features

- Supports multiple indexation methods (Canadian, TIPS, UK, French, Japanese) with standard lags and interpolation rules.
- Deflation protection configurable (none, maturity-only, all payments) plus deflation floors on principal/coupons.
- Uses inflation curves (`InflationCurve`/`InflationIndex`) to project nominal cashflows (real amount × index ratio).

## Methodology & References

- Cashflows generated with index ratios using lag/interpolation conventions per `IndexationMethod`; discounting on the **nominal** discount curve (`discount_curve_id`). The schedule contains inflation-projected nominal amounts, so a real curve here would double-count inflation; the real curve is never used for PV.
- Aligns with market conventions for linkers (e.g., 3m/8m lag; daily-interpolated reference CPI for TIPS/Canadian/French/JGBi and modern UK gilts; step for legacy UK gilts).
- Deterministic inflation; no seasonality or stochastic CPI modeled beyond supplied curve/index.

## Usage Example

```rust
use finstack_quant_valuations::instruments::fixed_income::inflation_linked_bond::InflationLinkedBond;

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
