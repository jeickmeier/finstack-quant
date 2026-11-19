//! Market scenario tests for interest rate options.
//!
//! Tests with realistic market conditions.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{BusinessDayConvention, Date, DayCount, StubKind, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::surfaces::{
    VolInterpolationMode, VolQuoteType, VolSurface, VolSurfaceAxis,
};
use finstack_quant_core::market_data::term_structures::{DiscountCurve, ForwardCurve};
use finstack_quant_core::money::Money;
use finstack_quant_models::types::ExerciseStyle;
use finstack_quant_valuations_core::instruments::Instrument;
use finstack_quant_valuations_core::instruments::SettlementType;
use finstack_quant_valuations_rates::cap_floor::{CapFloor, CapFloorVolType, RateOptionType};
use rust_decimal::Decimal;
use time::Duration;
use time::macros::date;

fn build_realistic_forward_curve(base_date: Date) -> ForwardCurve {
    // Realistic upward-sloping forward curve
    ForwardCurve::builder("USD_SOFR_3M")
        .base_date(base_date)
        .day_count(DayCount::Act360)
        .knots([
            (0.0, 1.0),
            (0.125, 0.993443463501034),
            (0.25, 0.986899074094975),
            (0.375, 0.980372265524844),
            (0.5, 0.973868332547788),
            (0.625, 0.967387393499255),
            (0.75, 0.960929564496091),
            (0.875, 0.954494959441667),
            (1.0, 0.948083690031192),
            (1.125, 0.941720389406374),
            (1.25, 0.935429029364396),
            (1.375, 0.929208737388482),
            (1.5, 0.923058653417911),
            (1.625, 0.91697792965804),
            (1.75, 0.910965730393491),
            (1.875, 0.905021231804452),
            (2.0, 0.899143621786045),
            (2.1875, 0.890440439788432),
            (2.375, 0.881862835830899),
            (2.5625, 0.873408799925484),
            (2.75, 0.865076358693593),
            (2.9375, 0.856863574646785),
            (3.125, 0.84876854548282),
            (3.3125, 0.840789403396603),
            (3.5, 0.832924314405709),
            (3.6875, 0.825171477690151),
            (3.875, 0.817529124946073),
            (4.0625, 0.809995519753069),
            (4.25, 0.802568956954803),
            (4.4375, 0.795247762052655),
            (4.625, 0.788030290612084),
            (4.8125, 0.780914927681445),
            (5.0, 0.773900087222964),
            (5.3125, 0.762400551609671),
            (5.625, 0.751115899295473),
            (5.9375, 0.740041637018937),
            (6.25, 0.729173374606828),
            (6.5625, 0.718506822432785),
            (6.875, 0.708037788942641),
            (7.1875, 0.697762178244528),
            (7.5, 0.687675987761994),
            (7.8125, 0.677775305948387),
            (8.125, 0.668056310060825),
            (8.4375, 0.658515263992105),
            (8.75, 0.649148516158965),
            (9.0625, 0.639952497445161),
            (9.375, 0.63092371919784),
            (9.6875, 0.622058771275773),
            (10.0, 0.61335432014801),
        ])
        .interp(finstack_quant_core::math::interp::InterpStyle::LogLinear)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
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
        .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .unwrap()
}

fn build_realistic_vol_surface(_base_date: Date) -> VolSurface {
    // Realistic vol smile and term structure
    VolSurface::builder("USD_CAP_VOL")
        .secondary_axis(VolSurfaceAxis::Strike)
        .quote_type(VolQuoteType::BlackLognormal)
        .interpolation_mode(VolInterpolationMode::Vol)
        .expiries(&[0.25, 1.0, 2.0, 5.0, 10.0])
        .secondary_values(&[0.02, 0.04, 0.05, 0.06, 0.08]) // 2%, 4%, 5%, 6%, 8%
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
        notional: Money::new(10_000_000.0, Currency::USD).expect("valid money fixture"),
        strike: Decimal::try_from(0.05).expect("valid decimal"), // 5% ATM
        start_date: start,
        maturity: end,
        frequency: Tenor::quarterly(),
        day_count: DayCount::Act360,
        stub: StubKind::ShortFront,
        business_day_convention: BusinessDayConvention::ModifiedFollowing,
        calendar_id: None,
        exercise_style: ExerciseStyle::European,
        settlement: SettlementType::Cash,
        discount_curve_id: "USD_OIS".into(),
        forward_curve_id: "USD_SOFR_3M".into(),
        vol_surface_id: "USD_CAP_VOL".into(),
        vol_type: CapFloorVolType::Lognormal,
        vol_shift: 0.0,
        overnight_coupon: None,
        premium: None,
        spread_bp: Decimal::ZERO,
        instrument_pricing_overrides: Default::default(),
        metric_pricing_overrides: Default::default(),
        scenario_pricing_overrides: Default::default(),
        attributes: Default::default(),
    };

    let market = MarketContext::new()
        .try_insert(build_realistic_discount_curve(as_of))
        .expect("insert market curve")
        .try_insert(build_realistic_forward_curve(as_of))
        .expect("insert market curve")
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
        notional: Money::new(5_000_000.0, Currency::USD).expect("valid money fixture"),
        strike: Decimal::try_from(0.03).expect("valid decimal"), // 3% OTM floor (forwards ~5%)
        start_date: start,
        maturity: end,
        frequency: Tenor::quarterly(),
        day_count: DayCount::Act360,
        stub: StubKind::ShortFront,
        business_day_convention: BusinessDayConvention::ModifiedFollowing,
        calendar_id: None,
        exercise_style: ExerciseStyle::European,
        settlement: SettlementType::Cash,
        discount_curve_id: "USD_OIS".into(),
        forward_curve_id: "USD_SOFR_3M".into(),
        vol_surface_id: "USD_CAP_VOL".into(),
        vol_type: CapFloorVolType::Lognormal,
        vol_shift: 0.0,
        overnight_coupon: None,
        premium: None,
        spread_bp: Decimal::ZERO,
        instrument_pricing_overrides: Default::default(),
        metric_pricing_overrides: Default::default(),
        scenario_pricing_overrides: Default::default(),
        attributes: Default::default(),
    };

    let market = MarketContext::new()
        .try_insert(build_realistic_discount_curve(as_of))
        .expect("insert market curve")
        .try_insert(build_realistic_forward_curve(as_of))
        .expect("insert market curve")
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
fn test_semi_annual_vs_quarterly_frequency() {
    let as_of = date!(2024 - 01 - 01);
    let start = as_of + Duration::days(2);
    let end = date!(2029 - 01 - 01);

    let quarterly_cap = CapFloor {
        id: "CAP_QUARTERLY".into(),
        rate_option_type: RateOptionType::Cap,
        notional: Money::new(10_000_000.0, Currency::USD).expect("valid money fixture"),
        strike: Decimal::try_from(0.05).expect("valid decimal"),
        start_date: start,
        maturity: end,
        frequency: Tenor::quarterly(),
        day_count: DayCount::Act360,
        stub: StubKind::ShortFront,
        business_day_convention: BusinessDayConvention::ModifiedFollowing,
        calendar_id: None,
        exercise_style: ExerciseStyle::European,
        settlement: SettlementType::Cash,
        discount_curve_id: "USD_OIS".into(),
        forward_curve_id: "USD_SOFR_3M".into(),
        vol_surface_id: "USD_CAP_VOL".into(),
        vol_type: CapFloorVolType::Lognormal,
        vol_shift: 0.0,
        overnight_coupon: None,
        premium: None,
        spread_bp: Decimal::ZERO,
        instrument_pricing_overrides: Default::default(),
        metric_pricing_overrides: Default::default(),
        scenario_pricing_overrides: Default::default(),
        attributes: Default::default(),
    };

    let semi_annual_cap = CapFloor {
        id: "CAP_SEMIANNUAL".into(),
        rate_option_type: RateOptionType::Cap,
        notional: Money::new(10_000_000.0, Currency::USD).expect("valid money fixture"),
        strike: Decimal::try_from(0.05).expect("valid decimal"),
        start_date: start,
        maturity: end,
        frequency: Tenor::semi_annual(),
        day_count: DayCount::Act360,
        stub: StubKind::ShortFront,
        business_day_convention: BusinessDayConvention::ModifiedFollowing,
        calendar_id: None,
        exercise_style: ExerciseStyle::European,
        settlement: SettlementType::Cash,
        discount_curve_id: "USD_OIS".into(),
        forward_curve_id: "USD_SOFR_3M".into(),
        vol_surface_id: "USD_CAP_VOL".into(),
        vol_type: CapFloorVolType::Lognormal,
        vol_shift: 0.0,
        overnight_coupon: None,
        premium: None,
        spread_bp: Decimal::ZERO,
        instrument_pricing_overrides: Default::default(),
        metric_pricing_overrides: Default::default(),
        scenario_pricing_overrides: Default::default(),
        attributes: Default::default(),
    };

    let market = MarketContext::new()
        .try_insert(build_realistic_discount_curve(as_of))
        .expect("insert market curve")
        .try_insert(build_realistic_forward_curve(as_of))
        .expect("insert market curve")
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
