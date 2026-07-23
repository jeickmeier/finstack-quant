# Equity Index Future

This module provides the `EquityIndexFuture` instrument for pricing and risk analysis of equity index futures.

## Overview

Equity index futures are exchange-traded derivatives that allow market participants to gain exposure to equity indices without owning the underlying stocks. They are cash-settled contracts based on the value of the underlying index at expiration.

## Supported Contracts

| Contract | Exchange | Index | Multiplier | Tick Size | Tick Value | Currency |
|----------|----------|-------|------------|-----------|------------|----------|
| ES (E-mini S&P 500) | CME | SPX | $50 | 0.25 | $12.50 | USD |
| MES (Micro E-mini S&P) | CME | SPX | $5 | 0.25 | $1.25 | USD |
| NQ (E-mini Nasdaq-100) | CME | NDX | $20 | 0.25 | $5.00 | USD |
| FESX (Euro Stoxx 50) | Eurex | SX5E | €10 | 1.0 | €10.00 | EUR |
| FDAX (DAX) | Eurex | DAX | €25 | 0.5 | €12.50 | EUR |
| Z (FTSE 100) | ICE | UKX | £10 | 0.5 | £5.00 | GBP |
| NK (Nikkei 225) | CME/OSE | NKY | ¥500 | 5.0 | ¥2,500 | JPY |

## Pricing Modes

### 1. Mark-to-Market (Quoted Price)

When a `quoted_price` is provided, the present value is calculated as:

$$
\text{NPV} = (\text{quoted\_price} - \text{entry\_price}) \times \text{multiplier} \times \text{quantity} \times \text{position\_sign}
$$

where `position_sign` is +1 for Long and -1 for Short.

### 2. Fair Value (Cost-of-Carry Model)

When no quoted price is available, the fair forward price is calculated using the cost-of-carry model:

$$
F = S_0 \times e^{(r - q) \times T}
$$

where:

- $S_0$ = Current spot index level
- $r$ = Risk-free rate (from discount curve)
- $q$ = Continuous dividend yield
- $T$ = Time to expiry in years

The present value is then:

$$
\text{NPV} = (F - \text{entry\_price}) \times \text{multiplier} \times \text{quantity} \times \text{position\_sign}
$$

## Usage Examples

### Creating an E-mini S&P 500 Future

```rust
use finstack_quant_valuations::instruments::equity::equity_index_future::{
    EquityIndexFuture, EquityFutureSpecs,
};
use finstack_quant_valuations::instruments::Position;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::types::{CurveId, InstrumentId};
use time::Month;

// Using the builder
let es_future = EquityIndexFuture::builder()
    .id(InstrumentId::new("ESH5"))
    .index_ticker("SPX".to_string())
    .currency(Currency::USD)
    .quantity(10.0)
    .expiry_date(Date::from_calendar_date(2025, Month::March, 21).unwrap())
    .last_trading_date(Date::from_calendar_date(2025, Month::March, 20).unwrap())
    .entry_price_opt(Some(4500.0))
    .quoted_price_opt(Some(4550.0))
    .position(Position::Long)
    .contract_specs(EquityFutureSpecs::sp500_emini())
    .discount_curve_id(CurveId::new("USD-OIS"))
    .spot_id("SPX-SPOT".to_string())
    .build()
    .expect("Valid future");

// Using convenience constructor
let es_future2 = EquityIndexFuture::sp500_emini(
    "ESH5",
    10.0,
    Date::from_calendar_date(2025, Month::March, 21).unwrap(),
    Date::from_calendar_date(2025, Month::March, 20).unwrap(),
    Some(4500.0),
    Position::Long,
    "USD-OIS",
).expect("Valid future");
```

### Calculating Delta

```rust
use finstack_quant_valuations::instruments::equity::equity_index_future::EquityIndexFuture;

let future = EquityIndexFuture::example().unwrap();
let delta = future.delta()?; // errors if entry_price is unset
// For 10 long ES contracts: delta = 50 × 10 × 1 = 500
// This means $500 P&L per 1-point index move
```

### Pricing with Market Data

```rust
use finstack_quant_valuations::instruments::equity::equity_index_future::EquityIndexFuture;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::MarketScalar;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::dates::Date;
use time::Month;

// Setup market context
let base_date = Date::from_calendar_date(2025, Month::January, 1).unwrap();
let discount_curve = DiscountCurve::builder("USD-OIS")
    .base_date(base_date)
    .knots(vec![(0.0, 1.0), (1.0, 0.95)])
    .build()
    .unwrap();

let market = MarketContext::new()
    .insert(discount_curve)
    .insert_price("SPX-SPOT", MarketScalar::Unitless(4500.0));

// Price the future
let future = EquityIndexFuture::example().unwrap();
let npv = future.value(&market, base_date).expect("should price");
println!("NPV: {}", npv);
```

## Market Data Requirements

### For Mark-to-Market Pricing

- Discount curve (for DV01 calculations)

### For Fair Value Pricing

- Discount curve (for risk-free rate and DV01)
- Spot index level (via `spot_id`)
- Optional: Dividend yield (via `div_yield_id`)

## Risk Metrics

The following metrics are available:

| Metric | Description |
|--------|-------------|
| `present_value` | Net present value of the position |
| `delta` | Futures point sensitivity: `multiplier × contracts × position_sign`; requires `entry_price` |
| `dv01` | Interest rate sensitivity (via discount curve) |
| `bucketed_dv01` | Key-rate DV01 by tenor bucket |
| `theta` | Time decay (1-day change in PV) |

## References

- Hull, J. C. (2018). "Options, Futures, and Other Derivatives." Chapter 5: Determination of Forward and Futures Prices.
- CME Group. "E-mini S&P 500 Futures Contract Specifications."
- Eurex. "EURO STOXX 50 Index Futures."
- ICE. "FTSE 100 Index Future."
