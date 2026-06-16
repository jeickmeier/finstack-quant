# FX Option

## Features

- Garman–Kohlhagen FX options with base/quote currencies, strike, settlement type, and exercise style fields.
- Uses domestic and foreign discount curves, FX vol surface, and optional implied-vol override for pricing/greeks.
- Helpers for canonical construction (`example`, `european`) plus implied-vol solver and greeks calculator.

## Methodology & References

- Garman–Kohlhagen (1983) / Black–76 style analytics with continuous foreign/domestic carry.
- Deterministic inputs from `MarketContext` (discount curves, vol surface, FX spot); no quanto or stochastic volatility.
- **European exercise only**: American and Bermudan styles are explicitly rejected and will return a validation error.

## Usage Example

```rust
use finstack_quant_valuations::instruments::fx::fx_option::FxOption;

let option = FxOption::example().unwrap();
let pv = option.value(&market_context, as_of_date)?;
```

## Limitations / Known Issues

- **European exercise only**: American and Bermudan exercise styles will return a validation error.
- Assumes log-normal FX dynamics; no support for local-vol or stochastic-vol pricing.
- Quanto adjustments are not included; cross-currency risks handled via chosen curves only.

## Metrics

- PV plus Greeks (delta/gamma/vega/theta/rho) from analytic formulas.
- Implied volatility solver and bump-and-revalue scenario metrics on spot, carry, and vol.
- DV01 on domestic curve for discounting exposure; FX delta in both base/quote terms.
