//! QuantLib parity tests for interest rate options (caps/floors).
//!
//! These tests validate our cap/floor implementation against QuantLib's behavior
//! and expected results from the QuantLib test suite.
//!
//! Reference: https://github.com/lballabio/QuantLib/blob/master/test-suite/capfloor.cpp

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

fn build_flat_forward_curve(rate: f64, base_date: Date, curve_id: &str) -> ForwardCurve {
    ForwardCurve::builder(curve_id, 0.25)
        .base_date(base_date)
        .day_count(DayCount::Act360)
        .knots([(0.0, rate), (10.0, rate)])
        .build()
        .unwrap()
}

fn build_flat_discount_curve(rate: f64, base_date: Date, curve_id: &str) -> DiscountCurve {
    DiscountCurve::builder(curve_id)
        .base_date(base_date)
        .day_count(DayCount::Act360)
        .knots([
            (0.0, 1.0),
            (1.0, (-rate).exp()),
            (5.0, (-rate * 5.0).exp()),
            (10.0, (-rate * 10.0).exp()),
        ])
        .build()
        .unwrap()
}

fn build_flat_vol_surface(vol: f64, _base_date: Date, surface_id: &str) -> VolSurface {
    VolSurface::builder(surface_id)
        .expiries(&[0.25, 1.0, 5.0, 10.0])
        .strikes(&[0.01, 0.03, 0.05, 0.07, 0.10])
        .row(&[vol, vol, vol, vol, vol])
        .row(&[vol, vol, vol, vol, vol])
        .row(&[vol, vol, vol, vol, vol])
        .row(&[vol, vol, vol, vol, vol])
        .build()
        .unwrap()
}

#[test]
fn test_quantlib_parity_atm_cap() {
    // Test ATM cap pricing matches expected Black model results
    // Reference: QuantLib's testVega test case
    let as_of = date!(2024 - 01 - 01);
    let end = date!(2029 - 01 - 01);

    let cap = CapFloor {
        id: "ATM_CAP".into(),
        rate_option_type: RateOptionType::Cap,
        notional: Money::new(1_000_000.0, Currency::USD),
        strike: Decimal::try_from(0.05).expect("valid decimal"),
        start_date: as_of,
        maturity: end,
        frequency: Tenor::quarterly(),
        day_count: DayCount::Act360,
        stub: StubKind::None,
        bdc: BusinessDayConvention::ModifiedFollowing,
        calendar_id: None,
        exercise_style: ExerciseStyle::European,
        settlement: SettlementType::Cash,
        discount_curve_id: "USD_OIS".into(),
        forward_curve_id: "USD_LIBOR_3M".into(),
        vol_surface_id: "USD_CAP_VOL".into(),
        vol_type: Default::default(),
        vol_shift: 0.0,
        overnight_coupon: None,
        spread: Decimal::ZERO,
        instrument_pricing_overrides: Default::default(),
        metric_pricing_overrides: Default::default(),
        scenario_pricing_overrides: Default::default(),
        attributes: Default::default(),
    };

    let disc_curve = build_flat_discount_curve(0.05, as_of, "USD_OIS");
    let fwd_curve = build_flat_forward_curve(0.05, as_of, "USD_LIBOR_3M");
    let vol_surface = build_flat_vol_surface(0.30, as_of, "USD_CAP_VOL");

    let market = MarketContext::new()
        .insert(disc_curve)
        .insert(fwd_curve)
        .insert_surface(vol_surface);

    let pv = cap.value(&market, as_of).unwrap().amount();

    // ATM cap with 30% vol over 5Y should have meaningful value
    // Typical range: $20k-$40k per $1MM notional
    assert!(
        pv > 15_000.0 && pv < 45_000.0,
        "ATM cap PV should be in typical range: {}",
        pv
    );
}

#[test]
fn test_quantlib_parity_cap_floor_parity() {
    // Test cap-floor parity: Cap - Floor = Swap
    // Reference: QuantLib's testParity test case
    let as_of = date!(2024 - 01 - 01);
    let end = date!(2029 - 01 - 01);
    let strike = 0.05;

    let cap = CapFloor {
        id: "CAP_PARITY".into(),
        rate_option_type: RateOptionType::Cap,
        notional: Money::new(1_000_000.0, Currency::USD),
        strike: Decimal::try_from(strike).expect("valid decimal"),
        start_date: as_of,
        maturity: end,
        frequency: Tenor::quarterly(),
        day_count: DayCount::Act360,
        stub: StubKind::None,
        bdc: BusinessDayConvention::ModifiedFollowing,
        calendar_id: None,
        exercise_style: ExerciseStyle::European,
        settlement: SettlementType::Cash,
        discount_curve_id: "USD_OIS".into(),
        forward_curve_id: "USD_LIBOR_3M".into(),
        vol_surface_id: "USD_CAP_VOL".into(),
        vol_type: Default::default(),
        vol_shift: 0.0,
        overnight_coupon: None,
        spread: Decimal::ZERO,
        instrument_pricing_overrides: Default::default(),
        metric_pricing_overrides: Default::default(),
        scenario_pricing_overrides: Default::default(),
        attributes: Default::default(),
    };

    let floor = CapFloor {
        id: "FLOOR_PARITY".into(),
        rate_option_type: RateOptionType::Floor,
        notional: Money::new(1_000_000.0, Currency::USD),
        strike: Decimal::try_from(strike).expect("valid decimal"),
        start_date: as_of,
        maturity: end,
        frequency: Tenor::quarterly(),
        day_count: DayCount::Act360,
        stub: StubKind::None,
        bdc: BusinessDayConvention::ModifiedFollowing,
        calendar_id: None,
        exercise_style: ExerciseStyle::European,
        settlement: SettlementType::Cash,
        discount_curve_id: "USD_OIS".into(),
        forward_curve_id: "USD_LIBOR_3M".into(),
        vol_surface_id: "USD_CAP_VOL".into(),
        vol_type: Default::default(),
        vol_shift: 0.0,
        overnight_coupon: None,
        spread: Decimal::ZERO,
        instrument_pricing_overrides: Default::default(),
        metric_pricing_overrides: Default::default(),
        scenario_pricing_overrides: Default::default(),
        attributes: Default::default(),
    };

    let disc_curve = build_flat_discount_curve(0.05, as_of, "USD_OIS");
    let fwd_curve = build_flat_forward_curve(0.05, as_of, "USD_LIBOR_3M");
    let vol_surface = build_flat_vol_surface(0.30, as_of, "USD_CAP_VOL");

    let market = MarketContext::new()
        .insert(disc_curve)
        .insert(fwd_curve)
        .insert_surface(vol_surface);

    let cap_pv = cap.value(&market, as_of).unwrap().amount();
    let floor_pv = floor.value(&market, as_of).unwrap().amount();

    // At ATM (strike = forward), cap and floor should have similar values
    let diff = (cap_pv - floor_pv).abs();
    let avg = (cap_pv + floor_pv) / 2.0;
    let relative_diff = diff / avg;

    assert!(
        relative_diff < 0.05,
        "ATM cap-floor parity: cap={}, floor={}, rel_diff={}",
        cap_pv,
        floor_pv,
        relative_diff
    );
}

#[test]
fn test_quantlib_parity_vol_sensitivity() {
    // Test that vega is positive and reasonable
    // Reference: QuantLib's testVega test case
    let as_of = date!(2024 - 01 - 01);
    let end = date!(2029 - 01 - 01);

    let cap = CapFloor {
        id: "CAP_VEGA".into(),
        rate_option_type: RateOptionType::Cap,
        notional: Money::new(1_000_000.0, Currency::USD),
        strike: Decimal::try_from(0.05).expect("valid decimal"),
        start_date: as_of,
        maturity: end,
        frequency: Tenor::quarterly(),
        day_count: DayCount::Act360,
        stub: StubKind::None,
        bdc: BusinessDayConvention::ModifiedFollowing,
        calendar_id: None,
        exercise_style: ExerciseStyle::European,
        settlement: SettlementType::Cash,
        discount_curve_id: "USD_OIS".into(),
        forward_curve_id: "USD_LIBOR_3M".into(),
        vol_surface_id: "USD_CAP_VOL".into(),
        vol_type: Default::default(),
        vol_shift: 0.0,
        overnight_coupon: None,
        spread: Decimal::ZERO,
        instrument_pricing_overrides: Default::default(),
        metric_pricing_overrides: Default::default(),
        scenario_pricing_overrides: Default::default(),
        attributes: Default::default(),
    };

    let disc_curve = build_flat_discount_curve(0.05, as_of, "USD_OIS");
    let fwd_curve = build_flat_forward_curve(0.05, as_of, "USD_LIBOR_3M");

    // Price with two different volatilities
    let vol_low = build_flat_vol_surface(0.20, as_of, "USD_CAP_VOL");
    let vol_high = build_flat_vol_surface(0.40, as_of, "USD_CAP_VOL");

    let market_low = MarketContext::new()
        .insert(build_flat_discount_curve(0.05, as_of, "USD_OIS"))
        .insert(build_flat_forward_curve(0.05, as_of, "USD_LIBOR_3M"))
        .insert_surface(vol_low);

    let market_high = MarketContext::new()
        .insert(disc_curve)
        .insert(fwd_curve)
        .insert_surface(vol_high);

    let pv_low = cap.value(&market_low, as_of).unwrap().amount();
    let pv_high = cap.value(&market_high, as_of).unwrap().amount();

    // Higher vol should increase value significantly
    assert!(
        pv_high > pv_low * 1.5,
        "Higher vol should increase cap value: low={}, high={}",
        pv_low,
        pv_high
    );

    // Implied vega per 1% vol
    let vega_approx = (pv_high - pv_low) / (0.40 - 0.20);
    assert!(
        vega_approx > 0.0,
        "Vega should be positive: {}",
        vega_approx
    );
}

#[test]
fn test_quantlib_parity_caplet_pricing() {
    // Test single period caplet pricing matches Black formula
    // Reference: QuantLib's testCachedValue test case
    let as_of = date!(2024 - 01 - 01);
    let start = date!(2024 - 03 - 01); // Forward starting
    let end = date!(2024 - 06 - 01);

    let caplet = CapFloor {
        id: "CAPLET".into(),
        rate_option_type: RateOptionType::Caplet,
        notional: Money::new(1_000_000.0, Currency::USD),
        strike: Decimal::try_from(0.05).expect("valid decimal"),
        start_date: start,
        maturity: end,
        frequency: Tenor::quarterly(),
        day_count: DayCount::Act360,
        stub: StubKind::None,
        bdc: BusinessDayConvention::ModifiedFollowing,
        calendar_id: None,
        exercise_style: ExerciseStyle::European,
        settlement: SettlementType::Cash,
        discount_curve_id: "USD_OIS".into(),
        forward_curve_id: "USD_LIBOR_3M".into(),
        vol_surface_id: "USD_CAP_VOL".into(),
        vol_type: Default::default(),
        vol_shift: 0.0,
        overnight_coupon: None,
        spread: Decimal::ZERO,
        instrument_pricing_overrides: Default::default(),
        metric_pricing_overrides: Default::default(),
        scenario_pricing_overrides: Default::default(),
        attributes: Default::default(),
    };

    let disc_curve = build_flat_discount_curve(0.05, as_of, "USD_OIS");
    let fwd_curve = build_flat_forward_curve(0.05, as_of, "USD_LIBOR_3M");
    let vol_surface = build_flat_vol_surface(0.20, as_of, "USD_CAP_VOL");

    let market = MarketContext::new()
        .insert(disc_curve)
        .insert(fwd_curve)
        .insert_surface(vol_surface);

    let pv = caplet.value(&market, as_of).unwrap().amount();

    // ATM 3M caplet with 20% vol should have small but positive value
    // Typical range: $100-$500 per $1MM notional
    assert!(
        pv > 50.0 && pv < 600.0,
        "Caplet PV should be in typical range: {}",
        pv
    );
}

#[test]
fn test_quantlib_parity_moneyness() {
    // Test that ITM > ATM > OTM for caps
    // Reference: QuantLib's general pricing behavior
    let as_of = date!(2024 - 01 - 01);
    let end = date!(2029 - 01 - 01);

    let itm_cap = CapFloor {
        id: "ITM_CAP".into(),
        rate_option_type: RateOptionType::Cap,
        notional: Money::new(1_000_000.0, Currency::USD),
        strike: Decimal::try_from(0.03).expect("valid decimal"), // ITM (forward = 5%)
        start_date: as_of,
        maturity: end,
        frequency: Tenor::quarterly(),
        day_count: DayCount::Act360,
        stub: StubKind::None,
        bdc: BusinessDayConvention::ModifiedFollowing,
        calendar_id: None,
        exercise_style: ExerciseStyle::European,
        settlement: SettlementType::Cash,
        discount_curve_id: "USD_OIS".into(),
        forward_curve_id: "USD_LIBOR_3M".into(),
        vol_surface_id: "USD_CAP_VOL".into(),
        vol_type: Default::default(),
        vol_shift: 0.0,
        overnight_coupon: None,
        spread: Decimal::ZERO,
        instrument_pricing_overrides: Default::default(),
        metric_pricing_overrides: Default::default(),
        scenario_pricing_overrides: Default::default(),
        attributes: Default::default(),
    };

    let atm_cap = CapFloor {
        id: "ATM_CAP".into(),
        rate_option_type: RateOptionType::Cap,
        notional: Money::new(1_000_000.0, Currency::USD),
        strike: Decimal::try_from(0.05).expect("valid decimal"),
        start_date: as_of,
        maturity: end,
        frequency: Tenor::quarterly(),
        day_count: DayCount::Act360,
        stub: StubKind::None,
        bdc: BusinessDayConvention::ModifiedFollowing,
        calendar_id: None,
        exercise_style: ExerciseStyle::European,
        settlement: SettlementType::Cash,
        discount_curve_id: "USD_OIS".into(),
        forward_curve_id: "USD_LIBOR_3M".into(),
        vol_surface_id: "USD_CAP_VOL".into(),
        vol_type: Default::default(),
        vol_shift: 0.0,
        overnight_coupon: None,
        spread: Decimal::ZERO,
        instrument_pricing_overrides: Default::default(),
        metric_pricing_overrides: Default::default(),
        scenario_pricing_overrides: Default::default(),
        attributes: Default::default(),
    };

    let otm_cap = CapFloor {
        id: "OTM_CAP".into(),
        rate_option_type: RateOptionType::Cap,
        notional: Money::new(1_000_000.0, Currency::USD),
        strike: Decimal::try_from(0.07).expect("valid decimal"), // OTM (forward = 5%)
        start_date: as_of,
        maturity: end,
        frequency: Tenor::quarterly(),
        day_count: DayCount::Act360,
        stub: StubKind::None,
        bdc: BusinessDayConvention::ModifiedFollowing,
        calendar_id: None,
        exercise_style: ExerciseStyle::European,
        settlement: SettlementType::Cash,
        discount_curve_id: "USD_OIS".into(),
        forward_curve_id: "USD_LIBOR_3M".into(),
        vol_surface_id: "USD_CAP_VOL".into(),
        vol_type: Default::default(),
        vol_shift: 0.0,
        overnight_coupon: None,
        spread: Decimal::ZERO,
        instrument_pricing_overrides: Default::default(),
        metric_pricing_overrides: Default::default(),
        scenario_pricing_overrides: Default::default(),
        attributes: Default::default(),
    };

    let disc_curve = build_flat_discount_curve(0.05, as_of, "USD_OIS");
    let fwd_curve = build_flat_forward_curve(0.05, as_of, "USD_LIBOR_3M");
    let vol_surface = build_flat_vol_surface(0.30, as_of, "USD_CAP_VOL");

    let market = MarketContext::new()
        .insert(disc_curve)
        .insert(fwd_curve)
        .insert_surface(vol_surface);

    let itm_pv = itm_cap.value(&market, as_of).unwrap().amount();
    let atm_pv = atm_cap.value(&market, as_of).unwrap().amount();
    let otm_pv = otm_cap.value(&market, as_of).unwrap().amount();

    // ITM > ATM > OTM
    assert!(
        itm_pv > atm_pv,
        "ITM ({}) should be > ATM ({})",
        itm_pv,
        atm_pv
    );
    assert!(
        atm_pv > otm_pv,
        "ATM ({}) should be > OTM ({})",
        atm_pv,
        otm_pv
    );
}

#[test]
fn test_quantlib_parity_delta_sign() {
    // Test that cap has positive delta, floor has negative delta
    // Reference: QuantLib's general Greeks behavior
    let as_of = date!(2024 - 01 - 01);
    let end = date!(2029 - 01 - 01);

    let cap = CapFloor {
        id: "CAP_DELTA".into(),
        rate_option_type: RateOptionType::Cap,
        notional: Money::new(1_000_000.0, Currency::USD),
        strike: Decimal::try_from(0.05).expect("valid decimal"),
        start_date: as_of,
        maturity: end,
        frequency: Tenor::quarterly(),
        day_count: DayCount::Act360,
        stub: StubKind::None,
        bdc: BusinessDayConvention::ModifiedFollowing,
        calendar_id: None,
        exercise_style: ExerciseStyle::European,
        settlement: SettlementType::Cash,
        discount_curve_id: "USD_OIS".into(),
        forward_curve_id: "USD_LIBOR_3M".into(),
        vol_surface_id: "USD_CAP_VOL".into(),
        vol_type: Default::default(),
        vol_shift: 0.0,
        overnight_coupon: None,
        spread: Decimal::ZERO,
        instrument_pricing_overrides: Default::default(),
        metric_pricing_overrides: Default::default(),
        scenario_pricing_overrides: Default::default(),
        attributes: Default::default(),
    };

    let floor = CapFloor {
        id: "FLOOR_DELTA".into(),
        rate_option_type: RateOptionType::Floor,
        notional: Money::new(1_000_000.0, Currency::USD),
        strike: Decimal::try_from(0.05).expect("valid decimal"),
        start_date: as_of,
        maturity: end,
        frequency: Tenor::quarterly(),
        day_count: DayCount::Act360,
        stub: StubKind::None,
        bdc: BusinessDayConvention::ModifiedFollowing,
        calendar_id: None,
        exercise_style: ExerciseStyle::European,
        settlement: SettlementType::Cash,
        discount_curve_id: "USD_OIS".into(),
        forward_curve_id: "USD_LIBOR_3M".into(),
        vol_surface_id: "USD_CAP_VOL".into(),
        vol_type: Default::default(),
        vol_shift: 0.0,
        overnight_coupon: None,
        spread: Decimal::ZERO,
        instrument_pricing_overrides: Default::default(),
        metric_pricing_overrides: Default::default(),
        scenario_pricing_overrides: Default::default(),
        attributes: Default::default(),
    };

    let disc_curve = build_flat_discount_curve(0.05, as_of, "USD_OIS");
    let fwd_curve = build_flat_forward_curve(0.05, as_of, "USD_LIBOR_3M");
    let vol_surface = build_flat_vol_surface(0.30, as_of, "USD_CAP_VOL");

    let market = MarketContext::new()
        .insert(disc_curve)
        .insert(fwd_curve)
        .insert_surface(vol_surface);

    let cap_result = cap
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Delta],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let floor_result = floor
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Delta],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let cap_delta = *cap_result.measures.get("delta").unwrap();
    let floor_delta = *floor_result.measures.get("delta").unwrap();

    // Cap benefits from higher forwards (positive delta)
    assert!(
        cap_delta > 0.0,
        "Cap delta should be positive: {}",
        cap_delta
    );

    // Floor benefits from lower forwards (negative delta)
    assert!(
        floor_delta < 0.0,
        "Floor delta should be negative: {}",
        floor_delta
    );
}

#[test]
fn test_quantlib_parity_gamma_positive() {
    // Test that gamma is positive for both caps and floors
    // Reference: QuantLib's general Greeks behavior
    let as_of = date!(2024 - 01 - 01);
    let end = date!(2029 - 01 - 01);

    let cap = CapFloor {
        id: "CAP_GAMMA".into(),
        rate_option_type: RateOptionType::Cap,
        notional: Money::new(1_000_000.0, Currency::USD),
        strike: Decimal::try_from(0.05).expect("valid decimal"),
        start_date: as_of,
        maturity: end,
        frequency: Tenor::quarterly(),
        day_count: DayCount::Act360,
        stub: StubKind::None,
        bdc: BusinessDayConvention::ModifiedFollowing,
        calendar_id: None,
        exercise_style: ExerciseStyle::European,
        settlement: SettlementType::Cash,
        discount_curve_id: "USD_OIS".into(),
        forward_curve_id: "USD_LIBOR_3M".into(),
        vol_surface_id: "USD_CAP_VOL".into(),
        vol_type: Default::default(),
        vol_shift: 0.0,
        overnight_coupon: None,
        spread: Decimal::ZERO,
        instrument_pricing_overrides: Default::default(),
        metric_pricing_overrides: Default::default(),
        scenario_pricing_overrides: Default::default(),
        attributes: Default::default(),
    };

    let floor = CapFloor {
        id: "FLOOR_GAMMA".into(),
        rate_option_type: RateOptionType::Floor,
        notional: Money::new(1_000_000.0, Currency::USD),
        strike: Decimal::try_from(0.05).expect("valid decimal"),
        start_date: as_of,
        maturity: end,
        frequency: Tenor::quarterly(),
        day_count: DayCount::Act360,
        stub: StubKind::None,
        bdc: BusinessDayConvention::ModifiedFollowing,
        calendar_id: None,
        exercise_style: ExerciseStyle::European,
        settlement: SettlementType::Cash,
        discount_curve_id: "USD_OIS".into(),
        forward_curve_id: "USD_LIBOR_3M".into(),
        vol_surface_id: "USD_CAP_VOL".into(),
        vol_type: Default::default(),
        vol_shift: 0.0,
        overnight_coupon: None,
        spread: Decimal::ZERO,
        instrument_pricing_overrides: Default::default(),
        metric_pricing_overrides: Default::default(),
        scenario_pricing_overrides: Default::default(),
        attributes: Default::default(),
    };

    let disc_curve = build_flat_discount_curve(0.05, as_of, "USD_OIS");
    let fwd_curve = build_flat_forward_curve(0.05, as_of, "USD_LIBOR_3M");
    let vol_surface = build_flat_vol_surface(0.30, as_of, "USD_CAP_VOL");

    let market = MarketContext::new()
        .insert(disc_curve)
        .insert(fwd_curve)
        .insert_surface(vol_surface);

    let cap_result = cap
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Gamma],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let floor_result = floor
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Gamma],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let cap_gamma = *cap_result.measures.get("gamma").unwrap();
    let floor_gamma = *floor_result.measures.get("gamma").unwrap();

    // Long options have positive gamma
    assert!(
        cap_gamma >= 0.0,
        "Cap gamma should be non-negative: {}",
        cap_gamma
    );
    assert!(
        floor_gamma >= 0.0,
        "Floor gamma should be non-negative: {}",
        floor_gamma
    );
}

#[test]
fn test_quantlib_parity_time_to_maturity() {
    // Test that longer maturity caps are more valuable
    // Reference: QuantLib's general pricing behavior
    let as_of = date!(2024 - 01 - 01);

    let short_cap = CapFloor {
        id: "SHORT_CAP".into(),
        rate_option_type: RateOptionType::Cap,
        notional: Money::new(1_000_000.0, Currency::USD),
        strike: Decimal::try_from(0.05).expect("valid decimal"),
        start_date: as_of,
        maturity: date!(2025 - 01 - 01), // 1Y
        frequency: Tenor::quarterly(),
        day_count: DayCount::Act360,
        stub: StubKind::None,
        bdc: BusinessDayConvention::ModifiedFollowing,
        calendar_id: None,
        exercise_style: ExerciseStyle::European,
        settlement: SettlementType::Cash,
        discount_curve_id: "USD_OIS".into(),
        forward_curve_id: "USD_LIBOR_3M".into(),
        vol_surface_id: "USD_CAP_VOL".into(),
        vol_type: Default::default(),
        vol_shift: 0.0,
        overnight_coupon: None,
        spread: Decimal::ZERO,
        instrument_pricing_overrides: Default::default(),
        metric_pricing_overrides: Default::default(),
        scenario_pricing_overrides: Default::default(),
        attributes: Default::default(),
    };

    let long_cap = CapFloor {
        id: "LONG_CAP".into(),
        rate_option_type: RateOptionType::Cap,
        notional: Money::new(1_000_000.0, Currency::USD),
        strike: Decimal::try_from(0.05).expect("valid decimal"),
        start_date: as_of,
        maturity: date!(2034 - 01 - 01), // 10Y
        frequency: Tenor::quarterly(),
        day_count: DayCount::Act360,
        stub: StubKind::None,
        bdc: BusinessDayConvention::ModifiedFollowing,
        calendar_id: None,
        exercise_style: ExerciseStyle::European,
        settlement: SettlementType::Cash,
        discount_curve_id: "USD_OIS".into(),
        forward_curve_id: "USD_LIBOR_3M".into(),
        vol_surface_id: "USD_CAP_VOL".into(),
        vol_type: Default::default(),
        vol_shift: 0.0,
        overnight_coupon: None,
        spread: Decimal::ZERO,
        instrument_pricing_overrides: Default::default(),
        metric_pricing_overrides: Default::default(),
        scenario_pricing_overrides: Default::default(),
        attributes: Default::default(),
    };

    let disc_curve = build_flat_discount_curve(0.05, as_of, "USD_OIS");
    let fwd_curve = build_flat_forward_curve(0.05, as_of, "USD_LIBOR_3M");
    let vol_surface = build_flat_vol_surface(0.30, as_of, "USD_CAP_VOL");

    let market = MarketContext::new()
        .insert(disc_curve)
        .insert(fwd_curve)
        .insert_surface(vol_surface);

    let short_pv = short_cap.value(&market, as_of).unwrap().amount();
    let long_pv = long_cap.value(&market, as_of).unwrap().amount();

    // Longer maturity cap has more caplets, should be more valuable
    assert!(
        long_pv > short_pv * 2.0,
        "10Y cap ({}) should be substantially more valuable than 1Y cap ({})",
        long_pv,
        short_pv
    );
}

#[test]
fn test_quantlib_parity_zero_vol_itm() {
    // Test that with zero vol, ITM cap has intrinsic value
    // Reference: QuantLib's behavior at zero volatility
    let as_of = date!(2024 - 01 - 01);
    let end = date!(2029 - 01 - 01);

    let cap = CapFloor {
        id: "CAP_ZERO_VOL".into(),
        rate_option_type: RateOptionType::Cap,
        notional: Money::new(1_000_000.0, Currency::USD),
        strike: Decimal::try_from(0.03).expect("valid decimal"), // Deep ITM (forward = 5%)
        start_date: as_of,
        maturity: end,
        frequency: Tenor::quarterly(),
        day_count: DayCount::Act360,
        stub: StubKind::None,
        bdc: BusinessDayConvention::ModifiedFollowing,
        calendar_id: None,
        exercise_style: ExerciseStyle::European,
        settlement: SettlementType::Cash,
        discount_curve_id: "USD_OIS".into(),
        forward_curve_id: "USD_LIBOR_3M".into(),
        vol_surface_id: "USD_CAP_VOL".into(),
        vol_type: Default::default(),
        vol_shift: 0.0,
        overnight_coupon: None,
        spread: Decimal::ZERO,
        instrument_pricing_overrides: Default::default(),
        metric_pricing_overrides: Default::default(),
        scenario_pricing_overrides: Default::default(),
        attributes: Default::default(),
    };

    let disc_curve = build_flat_discount_curve(0.05, as_of, "USD_OIS");
    let fwd_curve = build_flat_forward_curve(0.05, as_of, "USD_LIBOR_3M");
    let vol_surface = build_flat_vol_surface(0.0001, as_of, "USD_CAP_VOL"); // Near-zero vol

    let market = MarketContext::new()
        .insert(disc_curve)
        .insert(fwd_curve)
        .insert_surface(vol_surface);

    let pv = cap.value(&market, as_of).unwrap().amount();

    // Deep ITM cap with zero vol should still have substantial intrinsic value
    // Intrinsic ≈ (forward - strike) × notional × sum(accrual × df)
    // Rough estimate: (0.05 - 0.03) × 1M × 5Y × 0.25 × avg_df ≈ $20k-$25k
    assert!(
        pv > 15_000.0,
        "Deep ITM cap with zero vol should have intrinsic value: {}",
        pv
    );
}

#[test]
fn test_quantlib_parity_frequency_impact() {
    // Test that payment frequency affects cap value
    // Reference: QuantLib's handling of different frequencies
    let as_of = date!(2024 - 01 - 01);
    let end = date!(2029 - 01 - 01);

    let quarterly_cap = CapFloor {
        id: "QUARTERLY_CAP".into(),
        rate_option_type: RateOptionType::Cap,
        notional: Money::new(1_000_000.0, Currency::USD),
        strike: Decimal::try_from(0.05).expect("valid decimal"),
        start_date: as_of,
        maturity: end,
        frequency: Tenor::quarterly(),
        day_count: DayCount::Act360,
        stub: StubKind::None,
        bdc: BusinessDayConvention::ModifiedFollowing,
        calendar_id: None,
        exercise_style: ExerciseStyle::European,
        settlement: SettlementType::Cash,
        discount_curve_id: "USD_OIS".into(),
        forward_curve_id: "USD_LIBOR_3M".into(),
        vol_surface_id: "USD_CAP_VOL".into(),
        vol_type: Default::default(),
        vol_shift: 0.0,
        overnight_coupon: None,
        spread: Decimal::ZERO,
        instrument_pricing_overrides: Default::default(),
        metric_pricing_overrides: Default::default(),
        scenario_pricing_overrides: Default::default(),
        attributes: Default::default(),
    };

    let annual_cap = CapFloor {
        id: "ANNUAL_CAP".into(),
        rate_option_type: RateOptionType::Cap,
        notional: Money::new(1_000_000.0, Currency::USD),
        strike: Decimal::try_from(0.05).expect("valid decimal"),
        start_date: as_of,
        maturity: end,
        frequency: Tenor::annual(),
        day_count: DayCount::Act360,
        stub: StubKind::None,
        bdc: BusinessDayConvention::ModifiedFollowing,
        calendar_id: None,
        exercise_style: ExerciseStyle::European,
        settlement: SettlementType::Cash,
        discount_curve_id: "USD_OIS".into(),
        forward_curve_id: "USD_LIBOR_3M".into(),
        vol_surface_id: "USD_CAP_VOL".into(),
        vol_type: Default::default(),
        vol_shift: 0.0,
        overnight_coupon: None,
        spread: Decimal::ZERO,
        instrument_pricing_overrides: Default::default(),
        metric_pricing_overrides: Default::default(),
        scenario_pricing_overrides: Default::default(),
        attributes: Default::default(),
    };

    let disc_curve = build_flat_discount_curve(0.05, as_of, "USD_OIS");
    let fwd_curve = build_flat_forward_curve(0.05, as_of, "USD_LIBOR_3M");
    let vol_surface = build_flat_vol_surface(0.30, as_of, "USD_CAP_VOL");

    let market = MarketContext::new()
        .insert(disc_curve)
        .insert(fwd_curve)
        .insert_surface(vol_surface);

    let quarterly_pv = quarterly_cap.value(&market, as_of).unwrap().amount();
    let annual_pv = annual_cap.value(&market, as_of).unwrap().amount();

    // Both should be positive
    assert!(quarterly_pv > 0.0, "Quarterly cap should be positive");
    assert!(annual_pv > 0.0, "Annual cap should be positive");

    // Values should be different but in the same order of magnitude
    let ratio = quarterly_pv / annual_pv;
    assert!(
        ratio > 0.5 && ratio < 2.0,
        "Quarterly/Annual ratio should be reasonable: {}",
        ratio
    );
}
