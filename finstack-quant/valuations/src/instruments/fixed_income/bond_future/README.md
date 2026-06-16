# Bond future

Government bond futures (UST, Bund, Gilt): deliverable basket, CTD selection, conversion factor, and risk metrics.

## Contents

- [Overview](#overview)
- [Quick Start](#quick-start)
- [Core Concepts](#core-concepts)
- [Market Conventions](#market-conventions)
- [Pricing Formulas](#pricing-formulas)
- [API Documentation](#api-documentation)
- [Risk Metrics](#risk-metrics)
- [Error Handling](#error-handling)
- [Integration Examples](#integration-examples)
- [References](#references)

---

## Overview

Bond futures are standardized exchange-traded contracts to buy or sell government bonds at a specified price on a future date. The key features of bond futures include:

- **Deliverable Basket**: Multiple bonds eligible for delivery against the contract
- **Conversion Factors**: Exchange-published factors that normalize bonds with different coupons/maturities
- **Cheapest-to-Deliver (CTD)**: The short position holder selects which bond to deliver, typically the CTD
- **Invoice Price**: Settlement amount calculated from futures price, conversion factor, and accrued interest

### CTD Mechanics

The cheapest-to-deliver (CTD) bond is the bond in the deliverable basket that is most economical for the short position to deliver. In practice, the CTD is determined by comparing:

```
Basis = (Clean Price - Futures Price × Conversion Factor)
```

The bond with the **smallest basis** (most negative or least positive) is the CTD. This implementation requires you to specify the CTD bond explicitly.

### Supported Markets

- **U.S. Treasury (UST)**: 2Y, 5Y, 10Y futures on CBOT
- **German Bund**: 10Y futures on Eurex
- **UK Gilt**: Long Gilt futures on ICE Futures Europe

---

## Quick Start

### Creating a UST 10-Year Future

```rust
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{InstrumentId, CurveId};
use finstack_quant_valuations::instruments::fixed_income::bond_future::{
    BondFuture, DeliverableBond, Position,
};
use time::macros::date;

// Define the deliverable basket with conversion factors
let deliverable_basket = vec![
    DeliverableBond {
        bond_id: InstrumentId::new("US912828XG33"),
        conversion_factor: 0.8234,  // Published by CBOT
    },
    DeliverableBond {
        bond_id: InstrumentId::new("US912828XH15"),
        conversion_factor: 0.8456,
    },
    DeliverableBond {
        bond_id: InstrumentId::new("US912828XJ71"),
        conversion_factor: 0.8678,
    },
];

// Create UST 10Y future using convenience constructor
let future = BondFuture::ust_10y(
    "TYH5",                                      // Contract ID (March 2025)
    Money::from_code(1_000_000, "USD"),          // Notional (10 contracts × $100k)
    date!(2025-03-20),                           // Expiry date
    date!(2025-03-21),                           // Delivery start
    date!(2025-03-31),                           // Delivery end
    125.50,                                      // Quoted futures price
    Position::Long,                              // Position (Long or Short)
    deliverable_basket,                          // Deliverable basket
    "US912828XG33",                              // CTD bond ID
    "USD-TREASURY",                              // Discount curve ID
).expect("Failed to create bond future");

println!("Created UST 10Y future: {}", future.id());
```

### Pricing the Future

```rust
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_valuations::instruments::fixed_income::bond::Bond;
use finstack_quant_valuations::instruments::fixed_income::bond_future::pricer::BondFuturePricer;

// Create market context with discount curve (see examples below)
let market = create_market_context();

// Create the CTD bond
let ctd_bond = Bond::fixed_semiannual(
    "US912828XG33",
    Money::from_code(100_000, "USD"),
    0.0375,                                      // 3.75% coupon
    date!(2023-07-15),                           // Issue date
    date!(2034-07-15),                           // Maturity date
    "USD-TREASURY",                              // Discount curve ID
);

// Calculate NPV
let as_of = date!(2025-01-15);
let npv = BondFuturePricer::calculate_npv(
    &future,
    &ctd_bond,
    &market,
    as_of,
).expect("Failed to calculate NPV");

println!("NPV: {}", npv);

// Calculate invoice price (settlement amount)
let settlement_date = date!(2025-03-23);  // T+2 after expiry
let invoice_price = future.invoice_price(&ctd_bond, &market, settlement_date)
    .expect("Failed to calculate invoice price");

println!("Invoice price: {}", invoice_price);
```

---

## Core Concepts

### Deliverable Basket

The deliverable basket contains all bonds eligible for delivery against the futures contract. Each bond has a conversion factor that normalizes its value to a standard notional bond.

```rust
use finstack_quant_valuations::instruments::fixed_income::bond_future::DeliverableBond;

let deliverable = DeliverableBond {
    bond_id: InstrumentId::new("US912828XG33"),
    conversion_factor: 0.8234,
};
```

**Key Points**:

- Conversion factors are published by the exchange (CBOT, Eurex, ICE)
- Factors normalize bonds to a standard coupon and maturity
- Factors are typically 4 decimal places (e.g., 0.8234)

### Conversion Factor Calculation

Exchanges publish conversion factors; prefer the published value when one is
available (it is carried on `DeliverableBond.conversion_factor`). When no
exchange value is supplied, `calculate_conversion_factor` reproduces the CME
standard formula:

```rust
let bond = create_ust_bond();

let cf = BondFuturePricer::calculate_conversion_factor(
    &bond,
    0.06,   // notional coupon / yield (6% for all UST contracts)
    10.0,   // contract standard maturity (selects 10Y/30Y vs 2Y/3Y/5Y rules)
    date!(2025-03-01),  // any date within the delivery month
).expect("Failed to calculate conversion factor");

println!("Calculated conversion factor: {:.4}", cf);
```

The calculation follows CME Group, "Calculating U.S. Treasury Futures
Conversion Factors" (IR232):

- **Notional yield**: 6% for UST/Bund, 4% for Gilt — the conversion factor is
  the decimal price at which $1 par yields this rate.
- **Delivery-month anchor**: time to maturity (or first call) is measured from
  the first day of the delivery month, so the factor does **not** depend on
  the caller's valuation date.
- **Rounding**: the maturity is rounded down to the nearest three months for
  the 10Y/30Y contracts (nearest month for 2Y/3Y/5Y); the result is rounded
  to four decimal places.

### Position Types

```rust
use finstack_quant_valuations::instruments::fixed_income::bond_future::Position;

// Long position: benefits from price increase
let long_future = BondFuture::ust_10y(
    "TYH5",
    notional,
    expiry,
    delivery_start,
    delivery_end,
    quoted_price,
    Position::Long,  // Long position
    basket,
    ctd_id,
    curve_id,
)?;

// Short position: benefits from price decrease
let short_future = BondFuture::ust_10y(
    "TYH5",
    notional,
    expiry,
    delivery_start,
    delivery_end,
    quoted_price,
    Position::Short,  // Short position
    basket,
    ctd_id,
    curve_id,
)?;
```

**NPV Sign Convention**:

- **Long position**: Positive NPV when futures price > model price (profit)
- **Short position**: Negative NPV when futures price > model price (loss)

---

## Market Conventions

### Contract Specifications Table

| **Specification** | **UST 10Y** | **UST 5Y** | **UST 2Y** | **Bund** | **Gilt** |
|-------------------|-------------|------------|------------|----------|----------|
| **Exchange** | CBOT | CBOT | CBOT | Eurex | ICE Futures |
| **Currency** | USD | USD | USD | EUR | GBP |
| **Contract Size** | $100,000 | $100,000 | $200,000 | €100,000 | £100,000 |
| **Tick Size** | 1/64 (0.015625) | 1/128 (0.0078125) | 1/256 (0.00390625) | 0.01 | 0.01 |
| **Tick Value** | $15.625 | $7.8125 | $7.8125 | €10 | £10 |
| **Standard Coupon** | 6% | 6% | 6% | 6% | **4%** |
| **Standard Maturity** | 10 years | 5 years | 2 years | 10 years | 10 years |
| **Settlement Days** | T+2 | T+2 | T+2 | T+2 | T+2 |
| **Day Count** | ACT/ACT | ACT/ACT | ACT/ACT | ACT/ACT | ACT/ACT |
| **Deliverable Range** | ≥6.5 years | ≥4y 2m | ≥1y 9m | 8.5-10.5 years | 8.75-13 years |

### Creating Market-Specific Futures

```rust
use finstack_quant_valuations::instruments::fixed_income::bond_future::BondFutureSpecs;

// UST 10Y (default)
let ust_10y_specs = BondFutureSpecs::default();

// UST 5Y
let ust_5y_specs = BondFutureSpecs::ust_5y();

// UST 2Y
let ust_2y_specs = BondFutureSpecs::ust_2y();

// German Bund
let bund_specs = BondFutureSpecs::bund();

// UK Gilt
let gilt_specs = BondFutureSpecs::gilt();
```

**Example: Creating a German Bund Future**

```rust
use finstack_quant_valuations::instruments::fixed_income::bond_future::BondFutureBuilder;

let bund_future = BondFuture::builder()
    .id(InstrumentId::new("FGBLH5"))
    .notional(Money::from_code(1_000_000, "EUR"))
    .expiry_date(date!(2025-03-07))
    .delivery_start(date!(2025-03-10))
    .delivery_end(date!(2025-03-10))
    .quoted_price(132.45)
    .position(Position::Long)
    .contract_specs(BondFutureSpecs::bund())  // Bund specs
    .deliverable_basket(bund_basket)
    .ctd_bond_id("DE0001102424")
    .discount_curve_id("EUR-BUND")
    .build()?;
```

---

## Pricing Formulas

### 1. Conversion Factor

The conversion factor normalizes a bond's value to the standard notional bond defined by the futures contract.

**Formula**:

```
CF = PV(bond cashflows discounted at standard coupon) / Par Value
```

**Implementation**:

```
For each cashflow at time t (in years):
    DF(t) = 1 / (1 + r/2)^(2*t)  // Semi-annual compounding
    PV += cashflow × DF(t)

CF = PV / Notional
```

**Rounding**: 4 decimal places (exchange standard)

**Example Calculation**:

For a bond with 3.75% coupon, 9.5 years to maturity, discounted at 6% (standard coupon):

- PV ≈ $82,340 per $100,000 face value
- CF = 82,340 / 100,000 = **0.8234**

### 2. Model Futures Price

The theoretical fair value of the futures contract based on the CTD bond's market price.

**Formula**:

```
Model_Price = Clean_Price_Percent / Conversion_Factor
```

Where:

- `Clean_Price_Percent`: CTD bond's clean price as % of par (e.g., 98.5 for $98.50/$100)
- `Conversion_Factor`: The CTD bond's conversion factor

**Example**:

```
CTD clean price: 98.50%
Conversion factor: 0.8234
Model price = 98.50 / 0.8234 = 119.65
```

### 3. NPV (Net Present Value)

The mark-to-market value of the futures position.

**Formula**:

```
NPV = (Quoted_Price - Model_Price) × (Notional / 100) × Sign
```

Where:

- `Quoted_Price`: Market-quoted futures price
- `Model_Price`: Theoretical price from CTD bond
- `Notional`: Total notional amount (e.g., $1,000,000 for 10 × $100k contracts)
- `Sign`: +1 for Long, -1 for Short

**Example**:

```
Quoted price: 125.50
Model price: 124.75
Notional: $1,000,000
Position: Long

NPV = (125.50 - 124.75) × (1,000,000 / 100) × 1
    = 0.75 × 10,000
    = $7,500
```

### 4. Invoice Price

The settlement amount when the futures contract is delivered.

**Formula**:

```
Invoice_Price = (Futures_Price × Conversion_Factor) + Accrued_Interest
```

**Example**:

```
Futures price: 125.50
Conversion factor: 0.8234
Accrued interest: $2,500 (on $100,000 face)

Invoice = (125.50% × 0.8234) + 2.50%
        = 103.34% + 2.50%
        = 105.84%

For $100,000 contract: $105,840
```

**Implementation**:

```rust
let invoice = future.invoice_price(&ctd_bond, &market, settlement_date)?;
```

---

## API Documentation

### Types

#### `BondFuture`

The main bond future instrument type.

**Fields**:

- `id`: Unique instrument identifier (e.g., "TYH5" for March 2025)
- `notional`: Total notional amount (e.g., $1M for 10 contracts)
- `expiry_date`: Last trading day
- `delivery_start`: First delivery day
- `delivery_end`: Last delivery day
- `quoted_price`: Market-quoted futures price
- `position`: `Position::Long` or `Position::Short`
- `contract_specs`: Market-specific specifications
- `deliverable_basket`: Eligible bonds with conversion factors
- `ctd_bond_id`: User-specified CTD bond
- `discount_curve_id`: Curve for discounting
- `attributes`: Optional metadata

**Builder Pattern**:

```rust
use finstack_quant_valuations::instruments::fixed_income::bond_future::BondFutureBuilder;

let future = BondFuture::builder()
    .id(InstrumentId::new("TYH5"))
    .notional(Money::from_code(1_000_000, "USD"))
    .expiry_date(date!(2025-03-20))
    .delivery_start(date!(2025-03-21))
    .delivery_end(date!(2025-03-31))
    .quoted_price(125.50)
    .position(Position::Long)
    .contract_specs(BondFutureSpecs::default())
    .deliverable_basket(basket)
    .ctd_bond_id("US912828XG33")
    .discount_curve_id("USD-TREASURY")
    .build()?;
```

**Convenience Constructors**:

```rust
// UST 10Y
let future = BondFuture::ust_10y(id, notional, expiry, delivery_start, delivery_end,
                                  quoted_price, position, basket, ctd_id, curve_id)?;

// UST 5Y
let future = BondFuture::ust_5y(...)?;

// UST 2Y
let future = BondFuture::ust_2y(...)?;

// German Bund
let future = BondFuture::bund(...)?;

// UK Gilt
let future = BondFuture::gilt(...)?;
```

#### `DeliverableBond`

A bond in the deliverable basket.

```rust
pub struct DeliverableBond {
    pub bond_id: InstrumentId,
    pub conversion_factor: f64,
}
```

#### `BondFutureSpecs`

Contract specifications for a specific market.

```rust
pub struct BondFutureSpecs {
    pub contract_size: f64,
    pub tick_size: f64,
    pub tick_value: f64,
    pub standard_coupon: f64,
    pub standard_maturity_years: f64,
    pub settlement_days: u32,
}
```

#### `Position`

Position direction (re-exported from `ir_future` module).

```rust
pub enum Position {
    Long,   // +1 multiplier
    Short,  // -1 multiplier
}
```

### Methods

#### `BondFuturePricer::calculate_conversion_factor()`

Compute the CME standard conversion factor for a fixed-rate deliverable.

```rust
pub fn calculate_conversion_factor(
    bond: &Bond,
    standard_coupon: f64,
    standard_maturity_years: f64,
    delivery_month_first_day: Date,
) -> Result<f64>
```

**Returns**: Conversion factor rounded to 4 decimal places. The factor depends
only on the bond's coupon/maturity and the delivery month — not on any
valuation date.

#### `BondFuturePricer::calculate_model_price()`

Calculate theoretical futures price from CTD bond.

```rust
pub fn calculate_model_price(
    ctd_bond: &Bond,
    conversion_factor: f64,
    market: &MarketContext,
    as_of: Date,
) -> Result<f64>
```

**Returns**: Model futures price as a decimal (e.g., 125.50)

#### `BondFuturePricer::calculate_npv()`

Calculate present value of the futures position.

```rust
pub fn calculate_npv(
    future: &BondFuture,
    ctd_bond: &Bond,
    market: &MarketContext,
    as_of: Date,
) -> Result<Money>
```

**Returns**: NPV in the future's notional currency

#### `BondFuture::invoice_price()`

Calculate settlement invoice price.

```rust
pub fn invoice_price(
    &self,
    ctd_bond: &Bond,
    market: &MarketContext,
    settlement_date: Date,
) -> Result<Money>
```

**Returns**: Invoice amount for delivery

---

## Risk Metrics

### DV01 (Dollar Value of 1 Basis Point)

DV01 measures the change in the futures position's value for a 1 basis point parallel shift in the yield curve.

**Calculation Method**: Finite difference (bump and reprice)

```rust
use finstack_quant_valuations::metrics::{standard_registry, MetricId};

let registry = standard_registry();

// Price with metrics
let result = registry.price_bond_future_with_metrics(
    &future,
    "discounting",  // Model key
    &market,
    &[MetricId::Dv01],
)?;

// Extract DV01
let dv01 = result.metric("dv01").unwrap();
println!("Contract DV01: ${:.2}", dv01);
```

**Interpretation**:

- DV01 is the sensitivity to a 1bp shift in the **discount curve**
- Automatically accounts for conversion factor scaling:
  - CTD bond DV01 is scaled by `1 / Conversion_Factor`
  - Future DV01 = CTD bond DV01 / CF × Num_Contracts

**Example**:

```
CTD bond DV01: $850 per $100k face
Conversion factor: 0.8234
Num contracts: 10

Future DV01 = (850 / 0.8234) × 10 = $10,323
```

### Bucketed DV01

Key-rate DV01 shows risk distribution across the yield curve.

```rust
let result = registry.price_bond_future_with_metrics(
    &future,
    "discounting",
    &market,
    &[MetricId::BucketedDv01],
)?;

// Extract bucketed DV01
let bucketed = result.metric("bucketed_dv01").unwrap();
// Returns map of {tenor → DV01}
```

**Standard Buckets**:

- 3M, 6M, 1Y, 2Y, 3Y, 5Y, 7Y, 10Y, 15Y, 20Y, 30Y

**Use Cases**:

- Hedge ratio calculation for multi-tenor portfolios
- Identifying concentration risk
- Curve strategy analysis

### Theta

Time decay of the futures position (not typically large for bond futures).

```rust
let result = registry.price_bond_future_with_metrics(
    &future,
    "discounting",
    &market,
    &[MetricId::Theta],
)?;

let theta = result.metric("theta").unwrap();
```

**Notes**:

- Theta for bond futures is typically small (no option premium decay)
- Reflects carry/roll-down of the CTD bond

---

## Error Handling

### Common Errors

#### 1. Invalid Date Ordering

```rust
let result = BondFuture::builder()
    .expiry_date(date!(2025-03-31))      // After delivery!
    .delivery_start(date!(2025-03-21))
    .delivery_end(date!(2025-03-31))
    .build();

// Error: "Expiry date must be before delivery start"
assert!(result.is_err());
```

**Fix**: Ensure `expiry_date < delivery_start < delivery_end`

#### 2. Empty Deliverable Basket

```rust
let result = BondFuture::builder()
    .deliverable_basket(vec![])  // Empty!
    .build();

// Error: "Deliverable basket cannot be empty"
assert!(result.is_err());
```

**Fix**: Provide at least one deliverable bond

#### 3. CTD Not in Basket

```rust
let basket = vec![
    DeliverableBond {
        bond_id: InstrumentId::new("US912828XG33"),
        conversion_factor: 0.8234,
    },
];

let result = BondFuture::builder()
    .deliverable_basket(basket)
    .ctd_bond_id("UNKNOWN_ID")  // Not in basket!
    .build();

// Error: "CTD bond 'UNKNOWN_ID' not found in deliverable basket"
assert!(result.is_err());
```

**Fix**: Ensure CTD bond ID exists in `deliverable_basket`

#### 4. Invalid Conversion Factor

```rust
let basket = vec![
    DeliverableBond {
        bond_id: InstrumentId::new("US912828XG33"),
        conversion_factor: 0.0,  // Invalid!
    },
];

let result = BondFuture::builder()
    .deliverable_basket(basket)
    .build();

// Error: "Conversion factor must be positive"
assert!(result.is_err());
```

**Fix**: All conversion factors must be > 0

#### 5. Missing Market Data

```rust
let market = MarketContext::new();  // Empty market!

let npv_result = BondFuturePricer::calculate_npv(
    &future,
    &ctd_bond,
    &market,
    as_of,
);

// Error: "Discount curve 'USD-TREASURY' not found"
assert!(npv_result.is_err());
```

**Fix**: Ensure discount curve is present in `MarketContext`

### Best Practices

1. **Validate inputs early**: Use the builder pattern which validates during construction
2. **Check curve availability**: Verify discount curves exist before pricing
3. **Handle Result types**: Always check `Result` return values, don't unwrap
4. **Use meaningful IDs**: Bond and curve IDs should match your data sources

---

## Integration Examples

### Portfolio Integration

```rust
use finstack_quant_valuations::portfolio::{Portfolio, PortfolioBuilder};

// Create multiple bond futures
let ust_10y = BondFuture::ust_10y(...)?;
let ust_5y = BondFuture::ust_5y(...)?;
let bund = BondFuture::bund(...)?;

// Build portfolio
let portfolio = PortfolioBuilder::new()
    .base_ccy("USD")
    .as_of(date!(2025-01-15))
    .entity("Fund1", "Treasury Hedge Fund", btreemap! {
        "strategy" => "rates",
        "region" => "global",
    })
    .position("pos_1", &ust_10y, 10.0, btreemap! {
        "asset_class" => "bond_future",
        "market" => "UST",
    })
    .position("pos_2", &ust_5y, 20.0, btreemap! {
        "asset_class" => "bond_future",
        "market" => "UST",
    })
    .position("pos_3", &bund, 5.0, btreemap! {
        "asset_class" => "bond_future",
        "market" => "Bund",
    })
    .build()?;

// Value portfolio
let valuation = portfolio.value(&market)?;
println!("Total portfolio value: {}", valuation.total);

// Aggregate risk by market
let dv01_by_market = valuation.aggregate_metric_by_attribute(
    "dv01",
    "market",
)?;
for (market, dv01) in dv01_by_market {
    println!("{}: ${:.2}", market, dv01);
}
```

### Scenario Integration

```rust
use finstack_quant_scenarios::{ScenarioEngine, ScenarioSpec, OperationSpec};

// Define a parallel curve shift scenario
let scenario = ScenarioSpec {
    id: "rates_up_25bp".to_string(),
    description: "25bp parallel shift in USD rates".to_string(),
    priority: 0,
    operations: vec![
        OperationSpec::CurveParallelBp {
            curve_kind: CurveKind::Discount,
            curve_id: "USD-TREASURY",
            bp_shift: 25.0,  // +25bp
        },
    ],
};

// Apply scenario
let engine = ScenarioEngine::new();
let (shocked_market, report) = engine.apply(scenario, market)?;

// Revalue with shocked market
let shocked_npv = BondFuturePricer::calculate_npv(
    &future,
    &ctd_bond,
    &shocked_market,
    as_of,
)?;

println!("Base NPV: {}", base_npv);
println!("Shocked NPV: {}", shocked_npv);
println!("P&L: {}", shocked_npv - base_npv);
```

### Statement Integration

```rust
use finstack_quant_statements::builder::ModelBuilder;
use finstack_quant_statements::types::{NodeType, ForecastMethod};

// Define a financial model with bond future position
let model = ModelBuilder::new()
    .periods(monthly_periods)
    .node(
        "bond_future_pnl",
        NodeType::Formula("bond_future_mtm - lag(bond_future_mtm, 1)"),
        None,
    )
    .node(
        "bond_future_mtm",
        NodeType::Forecast(ForecastMethod::TimeSeries {
            source: "bond_future_valuations",
        }),
        None,
    )
    .build()?;

// Evaluate model
let results = model.evaluate(&market)?;
println!("Monthly P&L: {:?}", results.get_series("bond_future_pnl"));
```

### Creating Market Context

```rust
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::math::interp::InterpStyle;

fn create_market_context() -> MarketContext {
    let base_date = date!(2025-01-15);
    let rate = 0.04;  // 4% flat for testing

    // Build discount curve
    let maturities = vec![0.0, 0.25, 0.5, 1.0, 2.0, 3.0, 5.0, 7.0, 10.0, 20.0, 30.0];
    let mut knots = Vec::new();

    for t in maturities {
        let df = if t == 0.0 {
            1.0
        } else {
            1.0 / (1.0 + rate / 2.0).powf(2.0 * t)
        };
        knots.push((t, df));
    }

    let curve = DiscountCurve::builder(CurveId::new("USD-TREASURY"))
        .base_date(base_date)
        .knots(knots)
        .interp(InterpStyle::LogLinear)
        .build()
        .expect("Failed to build curve");

    MarketContext::new().insert(curve)
}
```

---

## References

### Exchange Documentation

#### U.S. Treasury Futures (CBOT/CME)

- **Contract Specifications**: [CME Group - U.S. Treasury Futures](https://www.cmegroup.com/markets/interest-rates/us-treasury.html)
- **Conversion Factors**: Published monthly at [CME Group - Conversion Factors](https://www.cmegroup.com/trading/interest-rates/us-treasury-conversion-factors.html)
- **Invoice Price Calculation**: [CME Group Invoice Price Guide](https://www.cmegroup.com/education/courses/introduction-to-treasuries/invoice-price-calculation.html)
- **CTD Analysis**: [CME Group - Understanding CTD](https://www.cmegroup.com/education/courses/introduction-to-treasuries/understanding-the-cheapest-to-deliver.html)

#### German Bund Futures (Eurex)

- **Contract Specifications**: [Eurex - Euro-Bund Future](https://www.eurex.com/ex-en/markets/int/fix/bund)
- **Conversion Factors**: [Eurex - Price Factors](https://www.eurex.com/ex-en/markets/int/fix/bund/price-factors)
- **Deliverable Basket**: Updated quarterly

#### UK Gilt Futures (ICE Futures Europe)

- **Contract Specifications**: [ICE - Long Gilt Futures](https://www.theice.com/products/4447212/Long-Gilt-Future)
- **Conversion Factors**: Published by ICE, updated monthly
- **Deliverable Range**: 8.75 to 13 years remaining maturity

### Academic References

1. **Hull, J.C.** (2018). *Options, Futures, and Other Derivatives* (10th ed.). Pearson.
   - Chapter 6: Interest Rate Futures

2. **Burghardt, G., Belton, T., Lane, M., & Papa, J.** (2005). *The Treasury Bond Basis: An In-Depth Analysis for Hedgers, Speculators, and Arbitrageurs* (3rd ed.). McGraw-Hill.
   - Comprehensive guide to bond futures basis trading and CTD analysis

3. **Fabozzi, F.J.** (2015). *Bond Markets, Analysis, and Strategies* (9th ed.). Pearson.
   - Chapter 27: Interest Rate Futures Contracts

### Related finstack Documentation

- **Bond Instrument**: `finstack-quant/valuations/src/instruments/bond/README.md`
- **Interest Rate Futures**: `finstack-quant/valuations/src/instruments/ir_future/README.md`
- **Discount Curves**: `finstack-quant/core/src/market_data/term_structures/discount_curve.rs`
- **Portfolio Framework**: `finstack-quant/portfolio/README.md`
- **Scenario Engine**: `finstack-quant/scenarios/README.md`

### Implementation Notes

- **Conversion Factor Algorithm**: See `pricer.rs::calculate_conversion_factor()` for detailed implementation
- **Invoice Price**: See `types.rs::BondFuture::invoice_price()` for delivery settlement calculation
- **Risk Metrics**: See `metrics/mod.rs` for DV01 and bucketed DV01 implementations
- **Validation Rules**: See `types.rs::BondFuture::validate()` for all construction-time checks

---

## Additional Examples

### Example 1: Full Workflow with Realistic Data

```rust
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CurveId, InstrumentId};
use finstack_quant_valuations::instruments::fixed_income::bond::Bond;
use finstack_quant_valuations::instruments::fixed_income::bond_future::{
    BondFuture, DeliverableBond, Position,
};
use finstack_quant_valuations::instruments::fixed_income::bond_future::pricer::BondFuturePricer;
use time::macros::date;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Create market context
    let market = create_market_context();
    let as_of = date!(2025-01-15);

    // 2. Create deliverable bonds
    let bond_1 = Bond::fixed_semiannual(
        "US912828XG33",
        Money::from_code(100_000, "USD"),
        0.0375,
        date!(2023-07-15),
        date!(2034-07-15),
        "USD-TREASURY",
    );

    let bond_2 = Bond::fixed_semiannual(
        "US912828XH15",
        Money::from_code(100_000, "USD"),
        0.04,
        date!(2023-01-15),
        date!(2035-01-15),
        "USD-TREASURY",
    );

    let bond_3 = Bond::fixed_semiannual(
        "US912828XJ71",
        Money::from_code(100_000, "USD"),
        0.0425,
        date!(2024-01-15),
        date!(2033-01-15),
        "USD-TREASURY",
    );

    // 3. Calculate conversion factors (delivery-month anchor, not a valuation date)
    let cf_1 = BondFuturePricer::calculate_conversion_factor(
        &bond_1, 0.06, 10.0, delivery_month_start,
    )?;
    let cf_2 = BondFuturePricer::calculate_conversion_factor(
        &bond_2, 0.06, 10.0, delivery_month_start,
    )?;
    let cf_3 = BondFuturePricer::calculate_conversion_factor(
        &bond_3, 0.06, 10.0, delivery_month_start,
    )?;

    println!("Conversion Factors:");
    println!("  Bond 1: {:.4}", cf_1);
    println!("  Bond 2: {:.4}", cf_2);
    println!("  Bond 3: {:.4}", cf_3);

    // 4. Create deliverable basket
    let basket = vec![
        DeliverableBond {
            bond_id: InstrumentId::new("US912828XG33"),
            conversion_factor: cf_1,
        },
        DeliverableBond {
            bond_id: InstrumentId::new("US912828XH15"),
            conversion_factor: cf_2,
        },
        DeliverableBond {
            bond_id: InstrumentId::new("US912828XJ71"),
            conversion_factor: cf_3,
        },
    ];

    // 5. Create UST 10Y future
    let future = BondFuture::ust_10y(
        "TYH5",
        Money::from_code(1_000_000, "USD"),  // 10 contracts
        date!(2025-03-20),
        date!(2025-03-21),
        date!(2025-03-31),
        125.50,                               // Quoted price
        Position::Long,
        basket,
        "US912828XG33",                       // CTD bond
        "USD-TREASURY",
    )?;

    println!("\nFuture Created: {}", future.id());

    // 6. Calculate NPV
    let npv = BondFuturePricer::calculate_npv(&future, &bond_1, &market, as_of)?;
    println!("NPV: {}", npv);

    // 7. Calculate invoice price
    let settlement = date!(2025-03-23);
    let invoice = future.invoice_price(&bond_1, &market, settlement)?;
    println!("Invoice Price: {}", invoice);

    // 8. Calculate DV01 (requires metrics registry - see Risk Metrics section)

    Ok(())
}
```

### Example 2: Multi-Contract Hedge

```rust
// Hedge a bond portfolio with UST 5Y and 10Y futures

fn calculate_hedge_ratios(
    portfolio_dv01: f64,
    ust_5y_dv01: f64,
    ust_10y_dv01: f64,
) -> (f64, f64) {
    // Simple duration-weighted hedge (in practice, use optimization)
    let total_dv01 = ust_5y_dv01 + ust_10y_dv01;
    let weight_5y = ust_5y_dv01 / total_dv01;
    let weight_10y = ust_10y_dv01 / total_dv01;

    let hedge_5y = portfolio_dv01 * weight_5y;
    let hedge_10y = portfolio_dv01 * weight_10y;

    (hedge_5y, hedge_10y)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let market = create_market_context();
    let as_of = date!(2025-01-15);

    // Portfolio has DV01 of $50,000
    let portfolio_dv01 = 50_000.0;

    // Create 5Y and 10Y futures
    let ust_5y = create_ust_5y_future()?;
    let ust_10y = create_ust_10y_future()?;

    // Calculate futures DV01s (using metrics registry)
    let ust_5y_dv01 = 4_500.0;   // Example value
    let ust_10y_dv01 = 8_500.0;  // Example value

    // Calculate hedge ratios
    let (num_5y, num_10y) = calculate_hedge_ratios(
        portfolio_dv01,
        ust_5y_dv01,
        ust_10y_dv01,
    );

    println!("Hedge Ratios:");
    println!("  5Y futures: {:.2} contracts", num_5y / 100_000.0);
    println!("  10Y futures: {:.2} contracts", num_10y / 100_000.0);

    Ok(())
}
```

---

## Support

For questions or issues, please refer to:

- **Finstack Quant Documentation**: Main project README
- **Issue Tracker**: GitHub issues
- **Code Examples**: `finstack-quant/valuations/tests/bond_future_integration.rs`

---

## License

This module is part of the finstack project and is subject to the project's license.
