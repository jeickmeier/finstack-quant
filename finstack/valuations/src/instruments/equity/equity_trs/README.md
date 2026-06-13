# Equity Total Return Swap (TRS)

## Features

- Synthetic equity index or single-stock exposure via total return swap
- Supports receive/pay total return via `TrsSide`
- Dividend yield forward model for accurate pricing
- ETF replication convenience constructor

## Methodology & References

- PV = PV(total-return leg) − PV(financing leg)
- Forward price model: F_t = S_0 × e^{(r-q)t}
- Total return = Price return + Dividend return
- Seasoned trades: the in-progress period anchors to the level observed at
  the period start (`past_fixings`, or `initial_level` for the first period)
  against the forward of the live spot, so the realized move enters the PV
  (equity delta). Fully-future periods are pure forward-ratio carry. Pricing
  errors when the current period's start level is unavailable.
- Deterministic curves and spot prices; no stochastic equity modeling

## Usage Example

```rust
use finstack_valuations::instruments::equity::equity_trs::EquityTotalReturnSwap;

let trs = EquityTotalReturnSwap::example().unwrap();
let pv = trs.value(&market_context, as_of_date)?;
```

## Complete Construction Example

```rust
use finstack_core::currency::Currency;
use finstack_core::dates::{Date, DayCount, BusinessDayConvention, StubKind, Tenor};
use finstack_core::money::Money;
use finstack_core::types::CurveId;
use finstack_valuations::cashflow::builder::ScheduleParams;
use finstack_valuations::instruments::FinancingLegSpec;
use finstack_valuations::instruments::EquityUnderlyingParams;
use finstack_valuations::instruments::equity::equity_trs::{
    EquityTotalReturnSwap, TrsScheduleSpec, TrsSide,
};

// 1. Define the financing leg specification
let financing_spec = FinancingLegSpec {
    discount_curve_id: CurveId::new("USD-OIS"),      // OIS curve for discounting
    forward_curve_id: CurveId::new("USD-SOFR-3M"),  // SOFR 3M for floating rate
    spread_bp: 50.0,                                 // 50bp spread over SOFR
    day_count: DayCount::Act360,
};

// 2. Define the schedule parameters
let schedule_params = ScheduleParams {
    freq: Tenor::quarterly(),                        // Quarterly resets
    dc: DayCount::Act360,                           // Day count for accrual
    bdc: BusinessDayConvention::ModifiedFollowing,  // Business day adjustment
    calendar_id: "NYC".to_string(),           // New York calendar
    stub: StubKind::ShortFront,                     // Short stub at front
    end_of_month: false,
    payment_lag_days: 0,
};

// 3. Create the TRS schedule specification
let start_date = Date::from_calendar_date(2024, time::Month::January, 15).unwrap();
let end_date = Date::from_calendar_date(2025, time::Month::January, 15).unwrap();
let schedule_spec = TrsScheduleSpec::from_params(start_date, end_date, schedule_params);

// 4. Define the underlying equity parameters
let underlying = EquityUnderlyingParams::new("SPX", "SPX-SPOT", Currency::USD)
    .with_dividend_yield("SPX-DIV-YIELD")
    .with_contract_size(1.0);

// 5. Build the equity TRS
let trs = EquityTotalReturnSwap::builder()
    .id("TRS-SPX-1Y".into())
    .notional(Money::new(10_000_000.0, Currency::USD))
    .underlying(underlying)
    .financing(financing_spec)
    .schedule(schedule_spec)
    .side(TrsSide::ReceiveTotalReturn)  // Long equity exposure
    .build()
    .unwrap();

// 6. Price the instrument
let npv = trs.value(&market_context, as_of_date)?;
let financing_pv = trs.pv_financing_leg(&market_context, as_of_date)?;
let total_return_pv = trs.pv_total_return_leg(&market_context, as_of_date)?;
```

## ETF Replication (Shorthand)

```rust
use finstack_valuations::instruments::equity::equity_trs::EquityTotalReturnSwap;

// Using the same financing_spec and schedule_spec from above
let spy_trs = EquityTotalReturnSwap::replicate_etf(
    "SPY",                                          // ETF ticker
    "SPY-SPOT",                                     // Market data ID for spot price
    Money::new(10_000_000.0, Currency::USD),        // Notional
    financing_spec,                                 // Financing leg
    schedule_spec,                                  // Payment schedule
    Some("SPY-DIV"),                                // Optional dividend yield ID
);
```

## Margining

Equity TRS implement full margin support following **ISDA CSA** standards with SIMM equity bucket classification.

| SIMM Risk Class | Sensitivity Type |
|-----------------|------------------|
| Equity | Equity delta (100% of notional) |

## Metrics

- **Delta**: Sensitivity to underlying equity level (notional / spot)
- **Dividend01**: Sensitivity to dividend yield (1bp bump)
- **DV01**: Sensitivity to financing rate
- **BucketedDV01**: Key-rate DV01 on financing leg
- **ParSpread**: Spread that makes NPV = 0
- **FinancingAnnuity**: PV01 of financing leg

## Limitations / Known Issues

- Total-return path is deterministic from supplied prices/yields
- Underlying is a single index identifier; constituent-level basket decomposition is not modeled
- No simulation of underlying equity volatility
- Does not model early termination or bespoke fee structures
