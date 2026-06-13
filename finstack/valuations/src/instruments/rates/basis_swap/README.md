# Basis Swap

## Features

- Floating-versus-floating swap exchanging two indices with independent frequencies, day-counts, and spreads (`BasisSwapLeg`).
- Schedule generation with stub handling and optional calendar selection for both legs.
- Deterministic discounting via `GenericInstrumentPricer::discounting()` with forward curves per leg.

## Methodology & References

- Pricing follows standard multi-curve swap valuation: projected floating cashflows on each curve, discounted with the chosen discount curve.
- Built on shared cashflow builder utilities; aligns with ISDA day-count/BDC conventions for interbank basis swaps.
- No convexity or CSA-specific adjustments beyond deterministic forwards/discount factors.

## Usage Example

```rust
use finstack_core::{currency::Currency, dates::*, money::Money, types::CurveId};
use finstack_valuations::instruments::rates::basis_swap::{BasisSwap, BasisSwapLeg};
use time::Month;

let primary = BasisSwapLeg {
    forward_curve_id: CurveId::new("USD-SOFR-3M"),
    frequency: Tenor::quarterly(),
    day_count: DayCount::Act360,
    bdc: BusinessDayConvention::ModifiedFollowing,
    spread_bp: 5.0,
    payment_lag_days: 0,
    reset_lag_days: 0,
};
let reference = BasisSwapLeg { spread_bp: 0.0, forward_curve_id: CurveId::new("USD-SOFR-6M"), ..primary };

let swap = BasisSwap::new(
    "BASIS-001",
    Money::new(1_000_000.0, Currency::USD),
    Date::from_calendar_date(2024, Month::January, 3)?,
    Date::from_calendar_date(2025, Month::January, 3)?,
    primary,
    reference,
    CurveId::new("USD-OIS"),
)?;  // Returns Result<BasisSwap, Error>
let pv = swap.value(&market_context, Date::from_calendar_date(2024, Month::January, 3)?)?;
```

## Limitations / Known Issues

- **No notional exchange**: This instrument models the coupon exchange only; initial and final notional exchanges are not supported. For cross-currency basis swaps or structures requiring principal exchange, use a custom instrument or combine with notional cashflows.
- **Single-currency only**: Both legs use the same notional currency. For cross-currency basis swaps (XCCY), a dedicated cross-currency swap instrument is required.
- No convexity or funding-value adjustment; deterministic forwards/discount factors only.
- Assumes a single discount curve for both legs; CSA-specific multi-curve discounting must be modeled externally.
- Does not include optional early termination or compounding conventions beyond the provided leg specs.

## Metrics

- PV plus par basis spread, DV01 (parallel and key-rate) using generic DV01 calculators over the discount curve.
- Forward basis (spread to par) available by solving for zero-NPV spread.
- Cashflow PV breakdown by leg for attribution.
