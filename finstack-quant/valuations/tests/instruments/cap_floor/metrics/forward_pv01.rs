//! Forward PV01 tests for interest rate options.
//!
//! Validates sensitivity to forward curve shifts.

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

fn create_standard_cap(_as_of: Date, start: Date, end: Date, strike: f64) -> CapFloor {
    CapFloor {
        id: "CAP_TEST".into(),
        rate_option_type: RateOptionType::Cap,
        notional: Money::new(1_000_000.0, Currency::USD),
        strike: Decimal::try_from(strike).expect("valid decimal"),
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
        pricing_overrides: finstack_quant_valuations::instruments::PricingOverrides::default(),
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
            finstack_quant_valuations::instruments::PricingOptions::default(),
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
        pricing_overrides: finstack_quant_valuations::instruments::PricingOverrides::default(),
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
            finstack_quant_valuations::instruments::PricingOptions::default(),
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
            finstack_quant_valuations::instruments::PricingOptions::default(),
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
            finstack_quant_valuations::instruments::PricingOptions::default(),
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
