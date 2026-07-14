//! Market scenario tests for interest rate options.
//!
//! Tests with realistic market conditions.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{BusinessDayConvention, Date, DayCount, StubKind, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::surfaces::VolSurface;
use finstack_quant_core::market_data::term_structures::{DiscountCurve, ForwardCurve};
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::rates::cap_floor::{CapFloor, RateOptionType};
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::instruments::{ExerciseStyle, SettlementType};
use finstack_quant_valuations::metrics::MetricId;
use rust_decimal::Decimal;
use time::macros::date;
use time::Duration;

fn build_realistic_forward_curve(base_date: Date) -> ForwardCurve {
    // Realistic upward-sloping forward curve
    ForwardCurve::builder("USD_SOFR_3M", 0.25)
        .base_date(base_date)
        .day_count(DayCount::Act360)
        .knots([
            (0.0, 0.0525),  // 5.25% spot
            (0.25, 0.0530), // Slight increase
            (1.0, 0.0540),  // 1Y forward
            (2.0, 0.0520),  // Peak then decline
            (5.0, 0.0480),  // Mean reversion
            (10.0, 0.0450), // Long-term equilibrium
        ])
        .build()
        .unwrap()
}

fn build_realistic_discount_curve(base_date: Date) -> DiscountCurve {
    DiscountCurve::builder("USD_OIS")
        .base_date(base_date)
        .day_count(DayCount::Act360)
        .knots([
            (0.0, 1.0),
            (0.25, (-0.0525_f64 * 0.25).exp()),
            (1.0, (-0.0540_f64 * 1.0).exp()),
            (2.0, (-0.0520_f64 * 2.0).exp()),
            (5.0, (-0.0480_f64 * 5.0).exp()),
            (10.0, (-0.0450_f64 * 10.0).exp()),
        ])
        .build()
        .unwrap()
}

fn build_realistic_vol_surface(_base_date: Date) -> VolSurface {
    // Realistic vol smile and term structure
    VolSurface::builder("USD_CAP_VOL")
        .expiries(&[0.25, 1.0, 2.0, 5.0, 10.0])
        .strikes(&[0.02, 0.04, 0.05, 0.06, 0.08]) // 2%, 4%, 5%, 6%, 8%
        .row(&[0.40, 0.35, 0.30, 0.32, 0.38]) // 3M: vol smile
        .row(&[0.35, 0.30, 0.25, 0.28, 0.33]) // 1Y
        .row(&[0.32, 0.27, 0.22, 0.25, 0.30]) // 2Y
        .row(&[0.28, 0.23, 0.18, 0.21, 0.26]) // 5Y: lower vol
        .row(&[0.25, 0.20, 0.15, 0.18, 0.23]) // 10Y: lowest vol
        .build()
        .unwrap()
}

#[test]
fn test_realistic_usd_cap_pricing() {
    let as_of = date!(2024 - 01 - 01);
    let start = as_of + Duration::days(2);
    let end = date!(2029 - 01 - 01);

    let cap = CapFloor {
        id: "USD_CAP_5Y_5%".into(),
        rate_option_type: RateOptionType::Cap,
        notional: Money::new(10_000_000.0, Currency::USD),
        strike: Decimal::try_from(0.05).expect("valid decimal"), // 5% ATM
        start_date: start,
        maturity: end,
        frequency: Tenor::quarterly(),
        day_count: DayCount::Act360,
        stub: StubKind::ShortFront,
        bdc: BusinessDayConvention::ModifiedFollowing,
        calendar_id: None,
        exercise_style: ExerciseStyle::European,
        settlement: SettlementType::Cash,
        discount_curve_id: "USD_OIS".into(),
        forward_curve_id: "USD_SOFR_3M".into(),
        vol_surface_id: "USD_CAP_VOL".into(),
        vol_type: Default::default(),
        vol_shift: 0.0,
        overnight_coupon: None,
        spread: Decimal::ZERO,
        pricing_overrides: finstack_quant_valuations::instruments::PricingOverrides::default(),
        attributes: Default::default(),
    };

    let market = MarketContext::new()
        .insert(build_realistic_discount_curve(as_of))
        .insert(build_realistic_forward_curve(as_of))
        .insert_surface(build_realistic_vol_surface(as_of));

    let pv = cap.value(&market, as_of).unwrap();

    // 5Y ATM cap on $10MM should have meaningful value
    assert!(
        pv.amount() > 10_000.0,
        "5Y cap should have substantial value"
    );
    assert!(
        pv.amount() < 1_000_000.0,
        "5Y cap value should be reasonable: {}",
        pv.amount()
    );
}

#[test]
fn test_realistic_otm_floor_pricing() {
    let as_of = date!(2024 - 01 - 01);
    let start = as_of + Duration::days(2);
    let end = date!(2027 - 01 - 01);

    let floor = CapFloor {
        id: "USD_FLOOR_3Y_3%".into(),
        rate_option_type: RateOptionType::Floor,
        notional: Money::new(5_000_000.0, Currency::USD),
        strike: Decimal::try_from(0.03).expect("valid decimal"), // 3% OTM floor (forwards ~5%)
        start_date: start,
        maturity: end,
        frequency: Tenor::quarterly(),
        day_count: DayCount::Act360,
        stub: StubKind::ShortFront,
        bdc: BusinessDayConvention::ModifiedFollowing,
        calendar_id: None,
        exercise_style: ExerciseStyle::European,
        settlement: SettlementType::Cash,
        discount_curve_id: "USD_OIS".into(),
        forward_curve_id: "USD_SOFR_3M".into(),
        vol_surface_id: "USD_CAP_VOL".into(),
        vol_type: Default::default(),
        vol_shift: 0.0,
        overnight_coupon: None,
        spread: Decimal::ZERO,
        pricing_overrides: finstack_quant_valuations::instruments::PricingOverrides::default(),
        attributes: Default::default(),
    };

    let market = MarketContext::new()
        .insert(build_realistic_discount_curve(as_of))
        .insert(build_realistic_forward_curve(as_of))
        .insert_surface(build_realistic_vol_surface(as_of));

    let pv = floor.value(&market, as_of).unwrap();

    // OTM floor should have small positive value
    assert!(pv.amount() > 0.0, "OTM floor should have some value");
    assert!(
        pv.amount() < 100_000.0,
        "OTM floor value should be modest: {}",
        pv.amount()
    );
}

#[test]
fn test_all_greeks_with_realistic_market() {
    let as_of = date!(2024 - 01 - 01);
    let start = as_of + Duration::days(2);
    let end = date!(2029 - 01 - 01);

    let cap = CapFloor {
        id: "USD_CAP_GREEKS".into(),
        rate_option_type: RateOptionType::Cap,
        notional: Money::new(10_000_000.0, Currency::USD),
        strike: Decimal::try_from(0.05).expect("valid decimal"),
        start_date: start,
        maturity: end,
        frequency: Tenor::quarterly(),
        day_count: DayCount::Act360,
        stub: StubKind::ShortFront,
        bdc: BusinessDayConvention::ModifiedFollowing,
        calendar_id: None,
        exercise_style: ExerciseStyle::European,
        settlement: SettlementType::Cash,
        discount_curve_id: "USD_OIS".into(),
        forward_curve_id: "USD_SOFR_3M".into(),
        vol_surface_id: "USD_CAP_VOL".into(),
        vol_type: Default::default(),
        vol_shift: 0.0,
        overnight_coupon: None,
        spread: Decimal::ZERO,
        pricing_overrides: finstack_quant_valuations::instruments::PricingOverrides::default(),
        attributes: Default::default(),
    };

    let market = MarketContext::new()
        .insert(build_realistic_discount_curve(as_of))
        .insert(build_realistic_forward_curve(as_of))
        .insert_surface(build_realistic_vol_surface(as_of));

    let metrics = vec![
        MetricId::Delta,
        MetricId::Gamma,
        MetricId::Vega,
        MetricId::Theta,
        MetricId::Rho,
        MetricId::Dv01,
        MetricId::ForwardPv01,
    ];

    let result = cap
        .price_with_metrics(
            &market,
            as_of,
            &metrics,
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    // Verify all metrics computed successfully
    assert!(result.measures.contains_key("delta"));
    assert!(result.measures.contains_key("gamma"));
    assert!(result.measures.contains_key("vega"));
    assert!(result.measures.contains_key("theta"));
    assert!(result.measures.contains_key("rho"));
    assert!(result.measures.contains_key("dv01"));
    assert!(result.measures.contains_key("forward_pv01"));

    // All should be finite
    for (name, value) in &result.measures {
        assert!(value.is_finite(), "{} should be finite", name);
    }
}

#[test]
fn test_semi_annual_vs_quarterly_frequency() {
    let as_of = date!(2024 - 01 - 01);
    let start = as_of + Duration::days(2);
    let end = date!(2029 - 01 - 01);

    let quarterly_cap = CapFloor {
        id: "CAP_QUARTERLY".into(),
        rate_option_type: RateOptionType::Cap,
        notional: Money::new(10_000_000.0, Currency::USD),
        strike: Decimal::try_from(0.05).expect("valid decimal"),
        start_date: start,
        maturity: end,
        frequency: Tenor::quarterly(),
        day_count: DayCount::Act360,
        stub: StubKind::ShortFront,
        bdc: BusinessDayConvention::ModifiedFollowing,
        calendar_id: None,
        exercise_style: ExerciseStyle::European,
        settlement: SettlementType::Cash,
        discount_curve_id: "USD_OIS".into(),
        forward_curve_id: "USD_SOFR_3M".into(),
        vol_surface_id: "USD_CAP_VOL".into(),
        vol_type: Default::default(),
        vol_shift: 0.0,
        overnight_coupon: None,
        spread: Decimal::ZERO,
        pricing_overrides: finstack_quant_valuations::instruments::PricingOverrides::default(),
        attributes: Default::default(),
    };

    let semi_annual_cap = CapFloor {
        id: "CAP_SEMIANNUAL".into(),
        rate_option_type: RateOptionType::Cap,
        notional: Money::new(10_000_000.0, Currency::USD),
        strike: Decimal::try_from(0.05).expect("valid decimal"),
        start_date: start,
        maturity: end,
        frequency: Tenor::semi_annual(),
        day_count: DayCount::Act360,
        stub: StubKind::ShortFront,
        bdc: BusinessDayConvention::ModifiedFollowing,
        calendar_id: None,
        exercise_style: ExerciseStyle::European,
        settlement: SettlementType::Cash,
        discount_curve_id: "USD_OIS".into(),
        forward_curve_id: "USD_SOFR_3M".into(),
        vol_surface_id: "USD_CAP_VOL".into(),
        vol_type: Default::default(),
        vol_shift: 0.0,
        overnight_coupon: None,
        spread: Decimal::ZERO,
        pricing_overrides: finstack_quant_valuations::instruments::PricingOverrides::default(),
        attributes: Default::default(),
    };

    let market = MarketContext::new()
        .insert(build_realistic_discount_curve(as_of))
        .insert(build_realistic_forward_curve(as_of))
        .insert_surface(build_realistic_vol_surface(as_of));

    let quarterly_pv = quarterly_cap.value(&market, as_of).unwrap().amount();
    let semi_annual_pv = semi_annual_cap.value(&market, as_of).unwrap().amount();

    // Quarterly cap has more fixing dates, typically slightly more valuable
    // (though depends on forward curve shape)
    assert!(quarterly_pv > 0.0 && semi_annual_pv > 0.0);
    assert!(
        (quarterly_pv / semi_annual_pv - 1.0).abs() < 0.25,
        "Quarterly and semi-annual caps on same risk should have similar values (within 25%)"
    );
}
