# Equity

## Features

- Spot equity position with optional explicit share count, price quote, dividend yield ID, and discount curve.
- Uses market data IDs for spot/dividend lookup or accepts embedded price quotes for offline valuation.
- Integrates with generic DV01 metrics via `CurveDependencies` and supports JSON serialization for pipeline use.

## Methodology & References

- Valuation is direct spot × shares discounted for dividend yield where provided; no option-style optionality.
- Pulls quotes from `MarketContext` scalars (`Price` or `Unitless`) with deterministic discounting.
- Aligns with standard equity carry conventions; no borrowing/lending spread modeled internally.

## Usage Example

```rust
use finstack_quant_valuations::instruments::equity::Equity;

let equity = Equity::example();
let pv = equity.value(&market_context, as_of_date)?;
```

## Limitations / Known Issues

- No modeling of borrow costs, financing spreads, or corporate actions beyond dividend yield input.
- Assumes long-only positions; shorting effects (rehypothecation, margin) are not modeled.
- Price resolution depends on provided market data IDs; missing data results in validation errors.

## Metrics

- PV, share count, and currency exposure; simple delta = shares with respect to spot.
- DV01 (if discounting applied) via generic calculators.
- Dividend yield sensitivity via bump-and-revalue when yield ID provided.
