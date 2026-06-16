# Interest Rate Future

## Features

- Exchange-style interest rate future with explicit contract specs (face, tick size/value, delivery months, convexity adjustment).
- Uses quoted price (e.g., 99.25) to derive implied rate, with forward/discount curves for model rate.
- Supports position side, explicit fixing/period dates, and optional convexity adjustment override.

## Methodology & References

- PV formula: `(ImpliedRate - ModelRateAdjusted) × FaceValue × τ × contracts × position_sign`, with τ from instrument day-count.
- Model rate uses forward curve projection; convexity adjustment derived from vol surface when provided or taken from contract specs.
- Deterministic curves; aligns with Eurodollar/IBOR-style futures pricing conventions.

## Usage Example

```rust
use finstack_quant_valuations::instruments::rates::ir_future::InterestRateFuture;

let fut = InterestRateFuture::example().unwrap();
let pv = fut.value(&market_context, as_of_date)?;
```

## Limitations / Known Issues

- Convexity adjustment requires either explicit override or volatility surface; otherwise relies on simplified calculation.
- Assumes parallel shift sensitivity only; margining and daily settlement effects are not modeled.
- No support for exchange-specific delivery options or cheapest-to-deliver mechanics.

## Metrics

- PV, implied rate, convexity adjustment amount, and tick PV based on contract specs.
- DV01 to discount/forward curves via bumping; sensitivity to convexity vol when supplied.
- Carry/roll to next IMM via forward curve interpolation.
