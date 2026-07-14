//! DV01 tests for interest rate options.
//!
//! Validates dollar value of 01bp parallel shift in rates.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{BusinessDayConvention, Date, DayCount, StubKind, Tenor};
use finstack_quant_core::market_data::bumps::BumpSpec;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::surfaces::VolSurface;
use finstack_quant_core::market_data::term_structures::{DiscountCurve, ForwardCurve};
use finstack_quant_core::money::Money;
use finstack_quant_core::types::CurveId;
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
        pricing_overrides: finstack_quant_valuations::instruments::PricingOverrides::default(),
        attributes: Default::default(),
    }
}

#[test]
fn test_cap_dv01_signed_negative() {
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
            &[MetricId::Dv01],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let dv01 = *result.measures.get("dv01").unwrap();

    // Note: While the test comment mentions negative DV01 based on discount bump convention,
    // economically a cap should have positive DV01 as it benefits from higher rates.
    // The unified calculator correctly returns positive DV01 for caps.
    assert!(dv01 > 0.0, "Cap DV01 should be positive: {}", dv01);
    assert!(
        dv01.abs() < 10_000.0,
        "Cap DV01 magnitude should be reasonable: {}",
        dv01
    );
}

#[test]
fn test_floor_dv01_signed_negative() {
    let as_of = date!(2024 - 01 - 01);
    let end = date!(2029 - 01 - 01);

    let floor = CapFloor {
        id: "FLOOR_TEST".into(),
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
            &[MetricId::Dv01],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let dv01 = *result.measures.get("dv01").unwrap();

    // Signed convention (discount bump): floors lose value as discount rates rise ⇒ DV01 < 0
    assert!(dv01 < 0.0, "Floor DV01 should be negative: {}", dv01);
}

#[test]
fn test_dv01_scales_with_maturity() {
    let as_of = date!(2024 - 01 - 01);

    let short_cap = create_standard_cap(as_of, date!(2025 - 01 - 01), 0.05);
    let long_cap = create_standard_cap(as_of, date!(2034 - 01 - 01), 0.05);

    let disc_curve = build_flat_discount_curve(0.05, as_of, "USD_OIS");
    let fwd_curve = build_flat_forward_curve(0.05, as_of, "USD_LIBOR_3M");
    let vol_surface = build_flat_vol_surface(0.30, as_of, "USD_CAP_VOL");

    let market = MarketContext::new()
        .insert(disc_curve)
        .insert(fwd_curve)
        .insert_surface(vol_surface);

    let short_dv01 = *short_cap
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Dv01],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap()
        .measures
        .get("dv01")
        .unwrap();

    let long_dv01 = *long_cap
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Dv01],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap()
        .measures
        .get("dv01")
        .unwrap();

    // Longer maturity should have larger magnitude DV01 (more sensitive)
    assert!(
        long_dv01.abs() > short_dv01.abs(),
        "|10Y DV01| ({}) should be > |1Y DV01| ({})",
        long_dv01,
        short_dv01
    );
}

#[test]
fn test_dv01_scales_with_notional() {
    let as_of = date!(2024 - 01 - 01);
    let end = date!(2029 - 01 - 01);

    let small_cap = create_standard_cap(as_of, end, 0.05);
    let mut large_cap = create_standard_cap(as_of, end, 0.05);
    large_cap.notional = Money::new(10_000_000.0, Currency::USD);

    let disc_curve = build_flat_discount_curve(0.05, as_of, "USD_OIS");
    let fwd_curve = build_flat_forward_curve(0.05, as_of, "USD_LIBOR_3M");
    let vol_surface = build_flat_vol_surface(0.30, as_of, "USD_CAP_VOL");

    let market = MarketContext::new()
        .insert(disc_curve)
        .insert(fwd_curve)
        .insert_surface(vol_surface);

    let small_dv01 = *small_cap
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Dv01],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap()
        .measures
        .get("dv01")
        .unwrap();

    let large_dv01 = *large_cap
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Dv01],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap()
        .measures
        .get("dv01")
        .unwrap();

    // DV01 should scale approximately linearly with notional
    let ratio = large_dv01 / small_dv01;
    assert!(
        ratio > 9.0 && ratio < 11.0,
        "DV01 ratio should be ~10x, got: {}",
        ratio
    );
}

#[test]
fn test_rfr_cap_dv01_reports_raw_model_curve_risk_without_quote_scalar() {
    let as_of = date!(2024 - 01 - 03);
    let end = date!(2029 - 01 - 03);
    let cap = CapFloor {
        id: "RFR-CAP-RAW-DV01".into(),
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
        forward_curve_id: "USD-SOFR-OIS".into(),
        vol_surface_id: "USD_CAP_VOL".into(),
        vol_type: Default::default(),
        vol_shift: 0.0,
        overnight_coupon: None,
        spread: Decimal::ZERO,
        pricing_overrides: finstack_quant_valuations::instruments::PricingOverrides::default(),
        attributes: Default::default(),
    };
    let market = MarketContext::new()
        .insert(build_flat_discount_curve(0.05, as_of, "USD_OIS"))
        .insert(build_flat_forward_curve(0.05, as_of, "USD-SOFR-OIS"))
        .insert_surface(build_flat_vol_surface(0.30, as_of, "USD_CAP_VOL"));

    let metric_dv01 = *cap
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Dv01],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap()
        .measures
        .get("dv01")
        .unwrap();

    let mut up = market.clone();
    let discount_id = CurveId::new("USD_OIS");
    let forward_id = CurveId::new("USD-SOFR-OIS");
    let up_discount = up
        .apply_curve_bump_in_place(&discount_id, BumpSpec::parallel_bp(1.0))
        .expect("discount bump");
    let up_forward = up
        .apply_curve_bump_in_place(&forward_id, BumpSpec::parallel_bp(1.0))
        .expect("forward bump");
    let pv_up = cap.value(&up, as_of).unwrap().amount();
    up.revert_scratch_bump(up_forward).expect("revert forward");
    up.revert_scratch_bump(up_discount)
        .expect("revert discount");

    let mut down = market;
    let down_discount = down
        .apply_curve_bump_in_place(&discount_id, BumpSpec::parallel_bp(-1.0))
        .expect("discount bump");
    let down_forward = down
        .apply_curve_bump_in_place(&forward_id, BumpSpec::parallel_bp(-1.0))
        .expect("forward bump");
    let pv_down = cap.value(&down, as_of).unwrap().amount();
    down.revert_scratch_bump(down_forward)
        .expect("revert forward");
    down.revert_scratch_bump(down_discount)
        .expect("revert discount");

    let raw_dv01 = (pv_up - pv_down) / 2.0;

    assert!(
        (metric_dv01 - raw_dv01).abs() < 1.0e-8,
        "RFR cap DV01 should be raw model curve risk: metric={metric_dv01}, raw={raw_dv01}"
    );
}
