//! Tests for lookback option seasoning (historical extrema).

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CurveId, InstrumentId};
use finstack_quant_valuations::instruments::exotics::lookback_option::{
    LookbackOption, LookbackOptionBuilder, LookbackType,
};
use finstack_quant_valuations::instruments::Instrument;
use time::Month;

// Helper to avoid clone issues with builder
fn get_base_builder(as_of: Date) -> LookbackOptionBuilder {
    let expiry = Date::from_calendar_date(as_of.year() + 1, as_of.month(), as_of.day()).unwrap();

    LookbackOption::builder()
        .id(InstrumentId::new("TEST-LOOKBACK"))
        .underlying_ticker("SPX".to_string())
        .strike_opt(Some(100.0))
        .option_type(finstack_quant_valuations::instruments::OptionType::Call)
        .lookback_type(LookbackType::FixedStrike)
        .expiry(expiry)
        .notional(Money::new(1.0, Currency::USD))
        .day_count(DayCount::Act365F)
        .discount_curve_id(CurveId::new("USD-OIS"))
        .spot_id("SPX-SPOT".into())
        .vol_surface_id(CurveId::new("SPX-VOL"))
        .div_yield_id_opt(Some(CurveId::new("SPX-DIV")))
        .pricing_overrides(finstack_quant_valuations::instruments::PricingOverrides::default())
        .attributes(finstack_quant_valuations::instruments::Attributes::new())
}

fn create_test_market(as_of: Date) -> MarketContext {
    let mut market = MarketContext::new();

    // Discount curve: flat 5%
    let curve =
        finstack_quant_core::market_data::term_structures::DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .day_count(DayCount::Act365F)
            .knots([(0.0, 1.0), (10.0, (-0.05 * 10.0_f64).exp())])
            .build()
            .unwrap();

    market = market.insert(curve);

    // Spot price: 100.0
    market = market.insert_price(
        "SPX-SPOT",
        finstack_quant_core::market_data::scalars::MarketScalar::Price(Money::new(
            100.0,
            Currency::USD,
        )),
    );

    // Vol surface: flat 20%
    use finstack_quant_core::market_data::surfaces::VolSurface;
    let surface = VolSurface::from_grid(
        "SPX-VOL",
        &[0.0, 10.0],
        &[0.0, 10000.0],
        &[0.20, 0.20, 0.20, 0.20],
    )
    .unwrap();
    market = market.insert_surface(surface);

    // Div yield: flat 0%
    market = market.insert_price(
        "SPX-DIV",
        finstack_quant_core::market_data::scalars::MarketScalar::Unitless(0.0),
    );

    market
}

#[test]
fn test_fixed_strike_call_seasoning() {
    let as_of = Date::from_calendar_date(2023, Month::January, 1).unwrap();
    let market = create_test_market(as_of);

    // Base Option: Fixed Strike Call
    // Strike = 100, Spot = 100
    // Payoff = max(S_max - K, 0)
    // Unseasoned: S_max = Spot = 100 -> Payoff at t=0 is 0.

    // 1. Unseasoned (default)
    let unseasoned = get_base_builder(as_of)
        .observed_max_opt(None)
        .build()
        .unwrap();

    let pv_unseasoned = unseasoned.value(&market, as_of).unwrap().amount();
    println!("Unseasoned PV: {}", pv_unseasoned);

    // 2. Seasoned with High Max (e.g., 120)
    // Current Spot = 100. Max so far = 120.
    // Payoff is max(120, S_max_future) - 100.
    // Current intrinsic = 120 - 100 = 20.
    // Should be much more valuable than unseasoned.
    let seasoned_high = get_base_builder(as_of)
        .observed_max_opt(Some(Money::new(120.0, Currency::USD)))
        .build()
        .unwrap();

    let pv_seasoned_high = seasoned_high.value(&market, as_of).unwrap().amount();
    println!("Seasoned High PV: {}", pv_seasoned_high);

    assert!(
        pv_seasoned_high > pv_unseasoned,
        "Seasoned high max should have higher value"
    );

    // 3. Seasoned with Low Max (e.g., 80)
    // Current Spot = 100. Max so far = 80.
    // Effective Max = max(80, 100) = 100.
    // Should be equal to unseasoned.
    let seasoned_low = get_base_builder(as_of)
        .observed_max_opt(Some(Money::new(80.0, Currency::USD)))
        .build()
        .unwrap();

    let pv_seasoned_low = seasoned_low.value(&market, as_of).unwrap().amount();
    println!("Seasoned Low PV: {}", pv_seasoned_low);

    // Use small epsilon for float comparison
    assert!(
        (pv_seasoned_low - pv_unseasoned).abs() < 1e-10,
        "Seasoned low max should equal unseasoned (effective max is spot)"
    );
}

#[test]
fn test_floating_strike_put_seasoning() {
    let as_of = Date::from_calendar_date(2023, Month::January, 1).unwrap();
    let market = create_test_market(as_of);

    // 1. Unseasoned
    let unseasoned = get_base_builder(as_of)
        .id(InstrumentId::new("TEST-LOOKBACK-FLOAT-PUT"))
        .strike_opt(None) // Floating strike
        .option_type(finstack_quant_valuations::instruments::OptionType::Put)
        .lookback_type(LookbackType::FloatingStrike)
        .observed_max_opt(None)
        .build()
        .unwrap();
    let pv_unseasoned = unseasoned.value(&market, as_of).unwrap().amount();

    // 2. Seasoned High (Max = 120)
    // Payoff = max(120, S_max_future) - S_T
    // Should be higher value.
    let seasoned_high = get_base_builder(as_of)
        .id(InstrumentId::new("TEST-LOOKBACK-FLOAT-PUT"))
        .strike_opt(None) // Floating strike
        .option_type(finstack_quant_valuations::instruments::OptionType::Put)
        .lookback_type(LookbackType::FloatingStrike)
        .observed_max_opt(Some(Money::new(120.0, Currency::USD)))
        .build()
        .unwrap();
    let pv_seasoned_high = seasoned_high.value(&market, as_of).unwrap().amount();

    assert!(
        pv_seasoned_high > pv_unseasoned,
        "Seasoned high max should increase Floating Put value"
    );
}

#[test]
fn test_fixed_strike_put_seasoning() {
    let as_of = Date::from_calendar_date(2023, Month::January, 1).unwrap();
    let market = create_test_market(as_of);

    // 1. Unseasoned (Min = Spot = 100)
    let unseasoned = get_base_builder(as_of)
        .id(InstrumentId::new("TEST-LOOKBACK-FIXED-PUT"))
        .option_type(finstack_quant_valuations::instruments::OptionType::Put)
        .observed_min_opt(None)
        .build()
        .unwrap();
    let pv_unseasoned = unseasoned.value(&market, as_of).unwrap().amount();

    // 2. Seasoned Low (Min = 80)
    // Payoff = max(100 - min(80, S_min_future), 0)
    // Current intrinsic = 100 - 80 = 20.
    // Should be higher value.
    let seasoned_low = get_base_builder(as_of)
        .id(InstrumentId::new("TEST-LOOKBACK-FIXED-PUT"))
        .option_type(finstack_quant_valuations::instruments::OptionType::Put)
        .observed_min_opt(Some(Money::new(80.0, Currency::USD)))
        .build()
        .unwrap();
    let pv_seasoned_low = seasoned_low.value(&market, as_of).unwrap().amount();

    assert!(
        pv_seasoned_low > pv_unseasoned,
        "Seasoned low min should increase Fixed Put value"
    );

    // 3. Seasoned High (Min = 120)
    // Effective Min = min(120, 100) = 100.
    // Should be equal to unseasoned.
    let seasoned_high = get_base_builder(as_of)
        .id(InstrumentId::new("TEST-LOOKBACK-FIXED-PUT"))
        .option_type(finstack_quant_valuations::instruments::OptionType::Put)
        .observed_min_opt(Some(Money::new(120.0, Currency::USD)))
        .build()
        .unwrap();
    let pv_seasoned_high = seasoned_high.value(&market, as_of).unwrap().amount();

    assert!(
        (pv_seasoned_high - pv_unseasoned).abs() < 1e-10,
        "Seasoned high min should equal unseasoned (effective min is spot)"
    );
}

#[test]
fn test_expired_fixed_strike_call_returns_realized_payoff() {
    let as_of = Date::from_calendar_date(2024, Month::January, 1).unwrap();
    let market = create_test_market(as_of).insert_price(
        "SPX-SPOT",
        finstack_quant_core::market_data::scalars::MarketScalar::Price(Money::new(
            120.0,
            Currency::USD,
        )),
    );

    let expired = LookbackOption::builder()
        .id(InstrumentId::new("TEST-LOOKBACK-EXPIRED"))
        .underlying_ticker("SPX".to_string())
        .strike_opt(Some(100.0))
        .option_type(finstack_quant_valuations::instruments::OptionType::Call)
        .lookback_type(LookbackType::FixedStrike)
        .expiry(as_of)
        .notional(Money::new(1.0, Currency::USD))
        .day_count(DayCount::Act365F)
        .discount_curve_id(CurveId::new("USD-OIS"))
        .spot_id("SPX-SPOT".into())
        .vol_surface_id(CurveId::new("SPX-VOL"))
        .div_yield_id_opt(Some(CurveId::new("SPX-DIV")))
        .pricing_overrides(finstack_quant_valuations::instruments::PricingOverrides::default())
        .observed_min_opt(None)
        .observed_max_opt(Some(Money::new(130.0, Currency::USD)))
        .attributes(finstack_quant_valuations::instruments::Attributes::new())
        .build()
        .unwrap();

    let pv = expired.value(&market, as_of).unwrap().amount();
    assert!(
        (pv - 30.0).abs() < 1e-10,
        "Expired lookback should settle to realized intrinsic value, got {pv}"
    );
}
