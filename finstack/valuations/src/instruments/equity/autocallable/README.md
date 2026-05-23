# Autocallable

## Features

- Path-dependent structured note with observation dates, autocall barriers, and coupon schedule, configurable via `Autocallable` fields.
- Multiple final payoff types (`CapitalProtection`, `Participation`, `KnockInPut`) plus configurable cap/participation rates.
- Monte Carlo GBM pricer with configurable steps and seed; integrates dividend yields, vol surfaces, and discount curves from `MarketContext`.

## Methodology & References

- Valued with the shared path-dependent Monte Carlo engine (GBM process, optional path capture) in `autocallable::pricer`.
- Payoff logic mirrors market-standard autocall step-up notes with discrete monitoring.
- Deterministic discounting; no jump or stochastic volatility modeling.

## Usage Example

```rust
use finstack_valuations::instruments::equity::autocallable::Autocallable;
use finstack_core::dates::Date;
use time::Month;

let as_of = Date::from_calendar_date(2024, Month::January, 2)?;
let note = Autocallable::example().unwrap();
let pv = note.value(&market_context, as_of)?;
```

## Limitations / Known Issues

- Pricing uses Monte Carlo; no closed-form approximation is provided.
- Assumes GBM dynamics with discrete observation; no quanto/correlation or stochastic vol support.
- Does not model secondary-market features like issuer call delays, partial redemption, or path-dependent fees beyond the configured schedule.

## Pricing Methodology

- Path-dependent GBM Monte Carlo with configurable steps/seed; uses per-observation barrier checks for autocall triggers and coupon accrual.
- Final payoff handled via mapped payoff type (capital protection, participation, knock-in put) with discounting on the domestic curve.
- Spot/dividend/vol from market surfaces; deterministic discounting with no correlation or jump modeling.

## Metrics

- PV plus path-based Greeks (delta/vega/theta) via bump-and-revalue in the metrics registry when MC is enabled.
- Scenario metrics for probability of call, expected coupons, and final payoff distribution (via captured paths).
- Sensitivity to barriers/vol levels can be computed through ad-hoc bump scripts using the pricer hooks.
