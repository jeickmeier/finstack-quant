# FX Spot

## Features

- Base/quote currency spot trade with optional notional, settlement date/lag, business-day adjustment, and embedded spot rate override.
- Pulls FX rates from `MarketContext` FX matrix or uses an explicit `spot_rate` when provided.
- Supports currency-safe PV via `FxProvider` integration and optional calendar adjustments.

## Methodology & References

- Valuation is deterministic conversion of base notional into quote currency at spot; settlement lag and BDC apply to cashflow date.
- Uses `FxConversionPolicy::CashflowDate` against the FX matrix; no forward points or carry modeled internally.
- Aligns with standard T+2 spot convention when settlement lag is set accordingly.

## Usage Example

```rust
use finstack_valuations::instruments::fx::fx_spot::FxSpot;
use finstack_core::{currency::Currency, dates::Date, money::Money, types::InstrumentId};
use time::Month;

let trade = FxSpot::new(InstrumentId::new("EURUSD-SPOT"), Currency::EUR, Currency::USD)
    .with_notional(Money::new(1_000_000.0, Currency::EUR))?
    .with_settlement_lag_days(2);

let as_of = Date::from_calendar_date(2024, Month::January, 3)?;
let pv = trade.value(&market_context, as_of)?;
```

## Limitations / Known Issues

- No forward points, carry, or discounting; use FX swaps for forwards.
- Requires FX matrix in `MarketContext` when `spot_rate` is not set.
- Does not model settlement risk, bid/ask spreads, or optionality.

## Pricing Methodology

- Converts base notional to quote currency using explicit `spot_rate` or FX matrix rate on cashflow date policy.
- Optional settlement lag/BDC adjusts settlement date; no discounting or forward points applied.
- Deterministic single-cashflow valuation.

## Metrics

- PV in quote currency; FX delta equal to base notional in base currency and −PV in quote currency.
- Exposure reporting by currency; simple scenario shocks via FX rate bumps.
- No DV01/CS01 since no rate discounting unless user-layer applies it.
