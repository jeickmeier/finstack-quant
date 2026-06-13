# Equity Option

## Features

- European, American, or Bermudan (fallback to American) call/put with configurable strike, expiry, contract size, and dividend yield source.
- Supports continuous-dividend Black–Scholes pricing with Leisen–Reimer tree fallback for early exercise.
- Vol surface lookup with implied-vol override, plus pricing overrides and standard discount curve inputs.

## Methodology & References

- Black–Scholes analytic pricing for European style; American priced with Leisen–Reimer binomial tree (201 steps).
- Dividend yield pulled from market scalar; deterministic rates/vols from discount and vol surfaces.
- Bermudan currently priced conservatively using the American tree due to lack of exercise schedule input.

## Usage Example

```rust
use finstack_valuations::instruments::{
    Attributes, EquityOption, ExerciseStyle, Instrument, OptionType, PricingOverrides,
    SettlementType,
};
use finstack_core::currency::Currency;
use finstack_core::dates::{Date, DayCount};
use finstack_core::market_data::context::MarketContext;
use finstack_core::market_data::scalars::MarketScalar;
use finstack_core::market_data::surfaces::VolSurface;
use finstack_core::market_data::term_structures::DiscountCurve;
use finstack_core::money::Money;
use finstack_core::types::{CurveId, InstrumentId};
use time::Month;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let as_of = Date::from_calendar_date(2025, Month::January, 2)?;
    let expiry = Date::from_calendar_date(2025, Month::July, 2)?;

    let discount_curve = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .day_count(DayCount::Act365F)
        .knots([(0.0, 1.0), (10.0, (-0.04_f64 * 10.0).exp())])
        .build()?;

    let vol_surface = VolSurface::builder("SPX-VOL")
        .expiries(&[0.25, 0.5, 1.0])
        .strikes(&[90.0, 100.0, 110.0])
        .row(&[0.22, 0.20, 0.21])
        .row(&[0.23, 0.21, 0.22])
        .row(&[0.24, 0.22, 0.23])
        .build()?;

    let market = MarketContext::new()
        .insert(discount_curve)
        .insert_surface(vol_surface)
        .insert_price("SPX-SPOT", MarketScalar::Unitless(100.0))
        .insert_price("SPX-DIV", MarketScalar::Unitless(0.015));

    let option = EquityOption::builder()
        .id(InstrumentId::new("SPX-CALL-20250702"))
        .underlying_ticker("SPX".to_string())
        .strike(100.0)
        .option_type(OptionType::Call)
        .exercise_style(ExerciseStyle::European)
        .expiry(expiry)
        .notional(Money::new(100.0, Currency::USD))
        .day_count(DayCount::Act365F)
        .settlement(SettlementType::Cash)
        .discount_curve_id(CurveId::new("USD-OIS"))
        .spot_id("SPX-SPOT".into())
        .vol_surface_id(CurveId::new("SPX-VOL"))
        .div_yield_id_opt(Some(CurveId::new("SPX-DIV")))
        .pricing_overrides(PricingOverrides::default())
        .attributes(Attributes::new())
        .build()?;

    let pv = option.value(&market, as_of)?;
    assert_eq!(pv.currency(), Currency::USD);

    Ok(())
}
```

## Limitations / Known Issues

- Bermudan exercise schedule not modeled; American tree is used as an upper bound.
- No stochastic volatility or jumps; relies on supplied vol surface or override.
- Cash settlement vs. physical is parameterized via `settlement`; exotic payoffs (barrier/Asian) use dedicated modules.

## Metrics

- PV plus Greeks (delta/gamma/vega/theta/rho) from analytic (Euro) or tree (Amer/Bermudan) methods.
- Implied volatility solver; scenario PVs via bump-and-revalue on spot/vol/rates.
- Contract-size scaling and cash/physical settlement support in reporting.
