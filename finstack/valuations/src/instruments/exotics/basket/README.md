# Basket

## Features

- Generic basket instrument that mixes constituent references (embedded instruments or market-data prices) with weights/units.
- NAV calculation supports per-share or total modes, expense ratio drag, and FX conversion via `FxProvider`.
- Builder helpers (`BasketPricingConfig`, `ConstituentReference`) for controlling fees, currency, and validation.

## Methodology & References

- Deterministic aggregation of constituent PVs using the shared `BasketCalculator` with optional expense drag.
- Currency conversions performed through `MarketContext` FX queries; no stochastic correlation between names.
- Aligned with ETF/index basket conventions (per-share NAV, expense accrual).

## Usage Example

```rust
use finstack_valuations::instruments::exotics::basket::Basket;

let basket = Basket::example().unwrap();
let pv = basket.value(&market_context, as_of_date)?;
```

## Limitations / Known Issues

- No dynamic rebalancing or path-dependent constituent weights; holdings are static for valuation.
- Does not model constituent correlation or tracking error—relies on underlying instrument pricing.
- Expense treatment is deterministic; performance-fee or hurdle-style fees are out of scope.

## Metrics

- NAV and per-constituent contributions; expense drag impact.
- Optional DV01/FX exposure metrics via underlying instruments’ metrics when constituent instruments are provided.
- Aggregate currency exposure and AUM-style totals for reporting.
