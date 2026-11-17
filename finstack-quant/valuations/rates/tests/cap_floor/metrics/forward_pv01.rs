//! Forward PV01 tests for interest rate options.
//!
//! Validates sensitivity to forward curve shifts.

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
use finstack_quant_valuations_core::metrics::MetricId;
use finstack_quant_valuations_rates::cap_floor::{CapFloor, CapFloorVolType, RateOptionType};
use rust_decimal::Decimal;
use time::macros::date;

fn build_flat_forward_curve(rate: f64, base_date: Date, curve_id: &str) -> ForwardCurve {
    ForwardCurve::flat(curve_id, base_date, rate, DayCount::Act360).unwrap()
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
        .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .unwrap()
}

fn build_flat_vol_surface(vol: f64, _base_date: Date, vol_surface_id: &str) -> VolSurface {
    VolSurface::builder(vol_surface_id)
        .secondary_axis(VolSurfaceAxis::Strike)
        .quote_type(VolQuoteType::BlackLognormal)
        .interpolation_mode(VolInterpolationMode::Vol)
        .expiries(&[0.25, 1.0, 5.0, 10.0])
        .secondary_values(&[0.01, 0.03, 0.05, 0.07, 0.10])
        .row(&[vol, vol, vol, vol, vol])
        .row(&[vol, vol, vol, vol, vol])
        .row(&[vol, vol, vol, vol, vol])
        .row(&[vol, vol, vol, vol, vol])
        .build()
        .unwrap()
}

fn create_standard_cap(_as_of: Date, start: Date, end: Date, strike: f64) -> CapFloor {
    CapFloor {
        id: "CAP_TEST".into(),
        rate_option_type: RateOptionType::Cap,
        notional: Money::new(1_000_000.0, Currency::USD).expect("valid money fixture"),
        strike: Decimal::try_from(strike).expect("valid decimal"),
        start_date: start,
        maturity: end,
        frequency: Tenor::quarterly(),
        day_count: DayCount::Act360,
        stub: StubKind::None,
        business_day_convention: BusinessDayConvention::ModifiedFollowing,
        calendar_id: None,
        exercise_style: ExerciseStyle::European,
        settlement: SettlementType::Cash,
        discount_curve_id: "USD_OIS".into(),
        forward_curve_id: "USD_LIBOR_3M".into(),
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
    }
}

#[test]
fn test_cap_forward_pv01() {
    let as_of = date!(2024 - 01 - 01);
    let start = date!(2024 - 03 - 01); // Forward-starting
    let end = date!(2029 - 03 - 01);

    let cap = create_standard_cap(as_of, start, end, 0.05);

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
            &[MetricId::ForwardPv01],
            finstack_quant_valuations_core::instruments::PricingOptions::default(),
        )
        .unwrap();

    let forward_pv01 = *result.measures.get("forward_pv01").unwrap();

    // Cap benefits from higher forwards (positive delta)
    assert!(
        forward_pv01 > 0.0,
        "Cap forward PV01 should be positive: {}",
        forward_pv01
    );
    assert!(
        forward_pv01 < 100_000.0,
        "Forward PV01 should be reasonable: {}",
        forward_pv01
    );
}

#[test]
fn test_floor_forward_pv01() {
    let as_of = date!(2024 - 01 - 01);
    let start = date!(2024 - 03 - 01); // Forward-starting
    let end = date!(2029 - 03 - 01);

    let floor = CapFloor {
        id: "FLOOR_TEST".into(),
        rate_option_type: RateOptionType::Floor,
        notional: Money::new(1_000_000.0, Currency::USD).expect("valid money fixture"),
        strike: Decimal::try_from(0.05).expect("valid decimal"),
        start_date: start,
        maturity: end,
        frequency: Tenor::quarterly(),
        day_count: DayCount::Act360,
        stub: StubKind::None,
        business_day_convention: BusinessDayConvention::ModifiedFollowing,
        calendar_id: None,
        exercise_style: ExerciseStyle::European,
        settlement: SettlementType::Cash,
        discount_curve_id: "USD_OIS".into(),
        forward_curve_id: "USD_LIBOR_3M".into(),
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
            &[MetricId::ForwardPv01],
            finstack_quant_valuations_core::instruments::PricingOptions::default(),
        )
        .unwrap();

    let forward_pv01 = *result.measures.get("forward_pv01").unwrap();

    // Floor benefits from lower forwards (negative delta)
    assert!(
        forward_pv01 < 0.0,
        "Floor forward PV01 should be negative: {}",
        forward_pv01
    );
}

#[test]
fn test_forward_pv01_scales_with_maturity() {
    let as_of = date!(2024 - 01 - 01);
    let start = date!(2024 - 03 - 01); // Forward-starting

    let short_cap = create_standard_cap(as_of, start, date!(2025 - 03 - 01), 0.05);
    let long_cap = create_standard_cap(as_of, start, date!(2034 - 03 - 01), 0.05);

    let disc_curve = build_flat_discount_curve(0.05, as_of, "USD_OIS");
    let fwd_curve = build_flat_forward_curve(0.05, as_of, "USD_LIBOR_3M");
    let vol_surface = build_flat_vol_surface(0.30, as_of, "USD_CAP_VOL");

    let market = MarketContext::new()
        .insert(disc_curve)
        .insert(fwd_curve)
        .insert_surface(vol_surface);

    let short_fpv01 = *short_cap
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::ForwardPv01],
            finstack_quant_valuations_core::instruments::PricingOptions::default(),
        )
        .unwrap()
        .measures
        .get("forward_pv01")
        .unwrap();

    let long_fpv01 = *long_cap
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::ForwardPv01],
            finstack_quant_valuations_core::instruments::PricingOptions::default(),
        )
        .unwrap()
        .measures
        .get("forward_pv01")
        .unwrap();

    // Longer maturity has more caplets, higher forward PV01
    assert!(
        long_fpv01 > short_fpv01,
        "10Y forward PV01 ({}) should be > 1Y forward PV01 ({})",
        long_fpv01,
        short_fpv01
    );
}

#[test]
fn test_forward_pv01_keeps_the_forward_curve_interpolation() {
    use finstack_quant_core::math::interp::InterpStyle;

    let as_of = date!(2024 - 01 - 01);
    let cap = create_standard_cap(as_of, date!(2024 - 03 - 01), date!(2029 - 03 - 01), 0.04);
    let knots = [
        (0.0, 0.030),
        (1.0, 0.036),
        (2.0, 0.045),
        (5.0, 0.041),
        (10.0, 0.048),
    ];

    // Under factor-based LogLinear curves, shifting every continuous zero-rate
    // knot by `shift` is algebraically identical to ForwardCurve's continuous
    // zero-rate bump (`P_b(t) = P(t) * exp(-shift * t)`).
    let make_curve = |style: InterpStyle, shift: f64| {
        let shifted = knots.map(|(t, rate)| (t, rate + shift));
        ForwardCurve::from_zero_rates(
            "USD_LIBOR_3M",
            as_of,
            &shifted,
            finstack_quant_core::math::Compounding::Continuous,
            DayCount::Act360,
            style,
            finstack_quant_core::math::interp::ExtrapolationPolicy::Extend,
        )
        .unwrap()
    };
    let make_market = |style: InterpStyle, shift: f64| {
        MarketContext::new()
            .insert(build_flat_discount_curve(0.04, as_of, "USD_OIS"))
            .insert(make_curve(style, shift))
            .insert_surface(build_flat_vol_surface(0.30, as_of, "USD_CAP_VOL"))
    };
    let fpv01 = |style: InterpStyle| {
        *cap.price_with_metrics(
            &make_market(style, 0.0),
            as_of,
            &[MetricId::ForwardPv01],
            finstack_quant_valuations_core::instruments::PricingOptions::default(),
        )
        .unwrap()
        .measures
        .get("forward_pv01")
        .unwrap()
    };

    let fpv01_log_linear = fpv01(InterpStyle::LogLinear);
    let pv = |shift: f64| {
        cap.value(&make_market(InterpStyle::LogLinear, shift), as_of)
            .unwrap()
            .amount()
    };
    let expected = (pv(1e-4) - pv(-1e-4)) / 2.0;
    assert!(
        (fpv01_log_linear - expected).abs() < 1e-6 * expected.abs().max(1.0),
        "LogLinear: forward PV01 {fpv01_log_linear} should equal the same-interpolation \
         revaluation {expected}"
    );

    // Distinct interpolation styles on the same factor pillars remain preserved by ForwardPv01.
    let fpv01_linear = fpv01(InterpStyle::Linear);
    assert!(
        (fpv01_linear - fpv01_log_linear).abs() > 1e-2,
        "Linear forward PV01 ({fpv01_linear}) should differ from LogLinear ({fpv01_log_linear})"
    );
}
