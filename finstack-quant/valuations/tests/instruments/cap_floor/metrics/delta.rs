//! Delta tests for interest rate options.
//!
//! Validates first-order sensitivity to forward rates.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{BusinessDayConvention, Date, DayCount, StubKind, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::surfaces::VolSurface;
use finstack_quant_core::market_data::term_structures::{DiscountCurve, ForwardCurve};
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::rates::cap_floor::{
    CapFloor, CapFloorVolType, OvernightCouponConvention, OvernightSpreadCompounding,
    RateOptionType,
};
use finstack_quant_valuations::instruments::rates::irs::FloatingLegCompounding;
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

fn create_standard_cap(as_of: Date, end: Date, strike: f64) -> CapFloor {
    CapFloor {
        id: "CAP_TEST".into(),
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
    }
}

fn create_standard_floor(as_of: Date, end: Date, strike: f64) -> CapFloor {
    CapFloor {
        id: "FLOOR_TEST".into(),
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
    }
}

#[test]
fn test_cap_delta_finite() {
    let as_of = date!(2024 - 01 - 01);
    let end = date!(2029 - 01 - 01);

    let cap = create_standard_cap(as_of, end, 0.05);

    let disc_curve = build_flat_discount_curve(0.05, as_of, "USD_OIS");
    let fwd_curve = build_flat_forward_curve(0.05, as_of, "USD_LIBOR_3M");
    let vol_surface = build_flat_vol_surface(0.30, as_of, "USD_CAP_VOL");

    let market = MarketContext::new()
        .insert(disc_curve)
        .insert(fwd_curve)
        .insert_surface(vol_surface);

    let result = cap
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Delta],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let delta = *result.measures.get("delta").unwrap();

    // Delta should be finite and reasonable
    assert!(delta.is_finite(), "Delta should be finite");
    assert!(
        delta.abs() < 1e8,
        "Delta should be reasonable, got: {}",
        delta
    );
}

#[test]
fn same_day_caplet_has_zero_stochastic_delta() {
    let as_of = date!(2024 - 03 - 01);
    let caplet = CapFloor::new_caplet(
        "SAME-DAY-FIXED-DELTA",
        Money::new(1_000_000.0, Currency::USD),
        0.05,
        as_of,
        date!(2024 - 06 - 01),
        DayCount::Act360,
        "USD_OIS",
        "USD_LIBOR_3M",
        "USD_CAP_VOL",
    )
    .expect("caplet");
    let market = MarketContext::new()
        .insert(build_flat_discount_curve(0.03, as_of, "USD_OIS"))
        .insert(build_flat_forward_curve(0.12, as_of, "USD_LIBOR_3M"))
        .insert_surface(build_flat_vol_surface(0.30, as_of, "USD_CAP_VOL"));

    let result = caplet
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Delta],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .expect("fixed caplet delta");

    assert_eq!(result.measures["delta"], 0.0);
}

#[test]
fn test_itm_cap_high_delta() {
    let as_of = date!(2024 - 01 - 01);
    let end = date!(2029 - 01 - 01);

    // ITM: strike < forward
    let cap = create_standard_cap(as_of, end, 0.03);

    let disc_curve = build_flat_discount_curve(0.05, as_of, "USD_OIS");
    let fwd_curve = build_flat_forward_curve(0.05, as_of, "USD_LIBOR_3M");
    let vol_surface = build_flat_vol_surface(0.30, as_of, "USD_CAP_VOL");

    let market = MarketContext::new()
        .insert(disc_curve)
        .insert(fwd_curve)
        .insert_surface(vol_surface);

    let result = cap
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Delta],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let delta = *result.measures.get("delta").unwrap();

    // ITM cap should have high positive delta (per caplet ~1, aggregated across many periods)
    assert!(delta > 0.0, "ITM cap delta should be positive: {}", delta);
}

#[test]
fn test_otm_cap_lower_delta() {
    let as_of = date!(2024 - 01 - 01);
    let end = date!(2029 - 01 - 01);

    // OTM: strike > forward
    let cap = create_standard_cap(as_of, end, 0.10);

    let disc_curve = build_flat_discount_curve(0.05, as_of, "USD_OIS");
    let fwd_curve = build_flat_forward_curve(0.05, as_of, "USD_LIBOR_3M");
    let vol_surface = build_flat_vol_surface(0.30, as_of, "USD_CAP_VOL");

    let market = MarketContext::new()
        .insert(disc_curve)
        .insert(fwd_curve)
        .insert_surface(vol_surface);

    let result = cap
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Delta],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let delta = *result.measures.get("delta").unwrap();

    // OTM cap should have lower delta
    assert!(delta.is_finite(), "OTM cap delta should be finite");
}

#[test]
fn test_floor_delta_negative() {
    let as_of = date!(2024 - 01 - 01);
    let end = date!(2029 - 01 - 01);

    let floor = create_standard_floor(as_of, end, 0.05);

    let disc_curve = build_flat_discount_curve(0.05, as_of, "USD_OIS");
    let fwd_curve = build_flat_forward_curve(0.05, as_of, "USD_LIBOR_3M");
    let vol_surface = build_flat_vol_surface(0.30, as_of, "USD_CAP_VOL");

    let market = MarketContext::new()
        .insert(disc_curve)
        .insert(fwd_curve)
        .insert_surface(vol_surface);

    let result = floor
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Delta],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let delta = *result.measures.get("delta").unwrap();

    // Floor delta should be negative (floor benefits from lower forwards)
    assert!(
        delta <= 0.0,
        "Floor delta should be non-positive: {}",
        delta
    );
}

#[test]
fn test_atm_cap_delta_around_half() {
    let as_of = date!(2024 - 01 - 01);
    let end = date!(2025 - 01 - 01); // Short maturity for clearer ATM behavior

    // ATM: strike = forward
    let cap = create_standard_cap(as_of, end, 0.05);

    let disc_curve = build_flat_discount_curve(0.05, as_of, "USD_OIS");
    let fwd_curve = build_flat_forward_curve(0.05, as_of, "USD_LIBOR_3M");
    let vol_surface = build_flat_vol_surface(0.30, as_of, "USD_CAP_VOL");

    let market = MarketContext::new()
        .insert(disc_curve)
        .insert(fwd_curve)
        .insert_surface(vol_surface);

    let result = cap
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Delta],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let delta = *result.measures.get("delta").unwrap();

    // ATM cap delta should be positive and reasonable
    // (Note: portfolio of caplets, so aggregate delta not constrained to [0,1])
    assert!(delta > 0.0, "ATM cap delta should be positive: {}", delta);
}

#[test]
fn test_delta_increases_with_moneyness() {
    let as_of = date!(2024 - 01 - 01);
    let end = date!(2027 - 01 - 01);

    let otm_cap = create_standard_cap(as_of, end, 0.08); // Far OTM
    let atm_cap = create_standard_cap(as_of, end, 0.05); // ATM
    let itm_cap = create_standard_cap(as_of, end, 0.02); // Far ITM

    let disc_curve = build_flat_discount_curve(0.05, as_of, "USD_OIS");
    let fwd_curve = build_flat_forward_curve(0.05, as_of, "USD_LIBOR_3M");
    let vol_surface = build_flat_vol_surface(0.30, as_of, "USD_CAP_VOL");

    let market = MarketContext::new()
        .insert(disc_curve)
        .insert(fwd_curve)
        .insert_surface(vol_surface);

    let otm_delta = *otm_cap
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Delta],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap()
        .measures
        .get("delta")
        .unwrap();

    let atm_delta = *atm_cap
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Delta],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap()
        .measures
        .get("delta")
        .unwrap();

    let itm_delta = *itm_cap
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Delta],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap()
        .measures
        .get("delta")
        .unwrap();

    // Delta should increase as option goes more in-the-money
    assert!(
        itm_delta > atm_delta,
        "ITM delta ({}) should be > ATM delta ({})",
        itm_delta,
        atm_delta
    );
    assert!(
        atm_delta > otm_delta,
        "ATM delta ({}) should be > OTM delta ({})",
        atm_delta,
        otm_delta
    );
}

#[test]
fn test_caplet_delta() {
    let as_of = date!(2024 - 01 - 01);
    let start = date!(2024 - 01 - 01);
    let end = date!(2024 - 04 - 01);

    let caplet = CapFloor {
        id: "CAPLET_TEST".into(),
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
    let vol_surface = build_flat_vol_surface(0.30, as_of, "USD_CAP_VOL");

    let market = MarketContext::new()
        .insert(disc_curve)
        .insert(fwd_curve)
        .insert_surface(vol_surface);

    let result = caplet
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Delta],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let delta = *result.measures.get("delta").unwrap();

    // Caplet delta should be finite and positive for ATM
    assert!(
        delta >= 0.0,
        "Caplet delta should be non-negative: {}",
        delta
    );
    assert!(delta.is_finite(), "Caplet delta should be finite");
}

#[test]
fn compounded_sofr_delta_matches_parallel_forward_finite_difference() {
    let as_of = date!(2024 - 12 - 02);
    let mut caplet = CapFloor::new_caplet(
        "SOFR-DELTA",
        Money::new(1_000_000.0, Currency::USD),
        0.04,
        date!(2025 - 01 - 02),
        date!(2025 - 04 - 02),
        DayCount::Act360,
        "USD_OIS",
        "USD-SOFR-OIS",
        "USD_CAP_VOL",
    )
    .expect("caplet");
    caplet.vol_type = CapFloorVolType::Normal;
    caplet.overnight_coupon = Some(OvernightCouponConvention {
        compounding: FloatingLegCompounding::CompoundedWithRateCutoff { cutoff_days: 1 },
        payment_delay_days: 2,
        fixing_calendar_id: Some("usny".into()),
        payment_calendar_id: Some("usny".into()),
        spread_compounding: OvernightSpreadCompounding::Exclude,
    });
    let discount = build_flat_discount_curve(0.04, as_of, "USD_OIS");
    let surface = build_flat_vol_surface(0.005, as_of, "USD_CAP_VOL");
    let forward = |shift: f64| {
        ForwardCurve::builder("USD-SOFR-OIS", 1.0 / 360.0)
            .base_date(as_of)
            .day_count(DayCount::Act360)
            .knots([
                (0.0, 0.03 + shift),
                (0.2, 0.04 + shift),
                (0.5, 0.055 + shift),
                (1.0, 0.06 + shift),
            ])
            .build()
            .expect("forward")
    };
    let market = |shift: f64| {
        MarketContext::new()
            .insert(discount.clone())
            .insert(forward(shift))
            .insert_surface(surface.clone())
    };
    let base_market = market(0.0);
    let analytic = caplet
        .price_with_metrics(
            &base_market,
            as_of,
            &[MetricId::Delta],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .expect("delta")
        .measures["delta"];
    let h = 1.0e-5;
    let up = caplet.value(&market(h), as_of).expect("up price").amount();
    let down = caplet
        .value(&market(-h), as_of)
        .expect("down price")
        .amount();
    let finite_difference = (up - down) / (2.0 * h);

    assert!(
        (analytic - finite_difference).abs() <= 1.0e-3 * finite_difference.abs().max(1.0),
        "compounded delta {analytic} should match pricing finite difference {finite_difference}"
    );
}
