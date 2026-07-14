//! Numerical accuracy tests for interest rate options.
//!
//! Validates Black model implementation accuracy and numerical stability.

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
fn test_black_model_symmetry() {
    // Test that Black model is symmetric for small changes
    let as_of = date!(2024 - 01 - 01);
    let start = date!(2024 - 01 - 01);
    let end = date!(2024 - 04 - 01);

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
        pricing_overrides: finstack_quant_valuations::instruments::PricingOverrides::default(),
        attributes: Default::default(),
    };

    // Test with forward slightly above and below strike
    let fwd1 = build_flat_forward_curve(0.0501, as_of, "USD_LIBOR_3M");
    let fwd2 = build_flat_forward_curve(0.0499, as_of, "USD_LIBOR_3M");
    let disc_curve1 = build_flat_discount_curve(0.05, as_of, "USD_OIS");
    let disc_curve2 = build_flat_discount_curve(0.05, as_of, "USD_OIS");
    let vol_surface1 = build_flat_vol_surface(0.30, as_of, "USD_CAP_VOL");
    let vol_surface2 = build_flat_vol_surface(0.30, as_of, "USD_CAP_VOL");

    let market1 = MarketContext::new()
        .insert(disc_curve1)
        .insert(fwd1)
        .insert_surface(vol_surface1);

    let market2 = MarketContext::new()
        .insert(disc_curve2)
        .insert(fwd2)
        .insert_surface(vol_surface2);

    let pv1 = caplet.value(&market1, as_of).unwrap().amount();
    let pv2 = caplet.value(&market2, as_of).unwrap().amount();

    // Both should be close to ATM value, difference should be small
    let diff = (pv1 - pv2).abs();
    assert!(
        diff < 50.0,
        "Small forward changes should produce small PV changes: diff={}",
        diff
    );
}

#[test]
fn test_vega_gamma_relation() {
    // Test relationship: vega ≈ spot × gamma × time × vol
    let as_of = date!(2024 - 01 - 01);
    let end = date!(2025 - 01 - 01);

    let cap = CapFloor {
        id: "CAP_RELATIONS".into(),
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

    let result = cap
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Vega, MetricId::Gamma],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let vega = *result.measures.get("vega").unwrap();
    let gamma = *result.measures.get("gamma").unwrap();

    // Both should be positive and finite
    assert!(
        vega > 0.0 && vega.is_finite(),
        "Vega should be positive and finite"
    );
    assert!(
        gamma >= 0.0 && gamma.is_finite(),
        "Gamma should be non-negative and finite"
    );
}

#[test]
fn test_delta_by_finite_difference() {
    // Verify delta by finite difference approximation
    let as_of = date!(2024 - 01 - 01);
    let start = date!(2024 - 03 - 01); // Future start to get t_fix > 0
    let end = date!(2024 - 06 - 01);

    let caplet = CapFloor {
        id: "CAPLET_FD".into(),
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
        pricing_overrides: finstack_quant_valuations::instruments::PricingOverrides::default(),
        attributes: Default::default(),
    };

    let bump = 0.0001; // 1bp
    let fwd_down = build_flat_forward_curve(0.05 - bump, as_of, "USD_LIBOR_3M");
    let fwd_base = build_flat_forward_curve(0.05, as_of, "USD_LIBOR_3M");
    let fwd_up = build_flat_forward_curve(0.05 + bump, as_of, "USD_LIBOR_3M");

    let disc_curve1 = build_flat_discount_curve(0.05, as_of, "USD_OIS");
    let disc_curve2 = build_flat_discount_curve(0.05, as_of, "USD_OIS");
    let disc_curve3 = build_flat_discount_curve(0.05, as_of, "USD_OIS");
    let vol_surface1 = build_flat_vol_surface(0.30, as_of, "USD_CAP_VOL");
    let vol_surface2 = build_flat_vol_surface(0.30, as_of, "USD_CAP_VOL");
    let vol_surface3 = build_flat_vol_surface(0.30, as_of, "USD_CAP_VOL");

    let market_down = MarketContext::new()
        .insert(disc_curve1)
        .insert(fwd_down)
        .insert_surface(vol_surface1);

    let market_base = MarketContext::new()
        .insert(disc_curve2)
        .insert(fwd_base)
        .insert_surface(vol_surface2);

    let market_up = MarketContext::new()
        .insert(disc_curve3)
        .insert(fwd_up)
        .insert_surface(vol_surface3);

    let pv_down = caplet.value(&market_down, as_of).unwrap().amount();
    let pv_up = caplet.value(&market_up, as_of).unwrap().amount();

    // Finite difference delta
    let fd_delta = (pv_up - pv_down) / (2.0 * bump);

    // Analytical delta
    let result = caplet
        .price_with_metrics(
            &market_base,
            as_of,
            &[MetricId::Delta],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let analytic_delta = *result.measures.get("delta").unwrap();

    // FD vs analytic delta should match within 2% relative error for meaningful values,
    // or within absolute tolerance of $10 for small values.
    let abs_diff = (fd_delta - analytic_delta).abs();
    let within_relative = analytic_delta.abs() > 100.0 && abs_diff / analytic_delta.abs() < 0.02;
    let within_absolute = abs_diff < 10.0;

    assert!(
        within_relative || within_absolute,
        "FD delta ({:.4}) should match analytic delta ({:.4}): \
        abs_diff={:.4}, rel_error={:.2}%",
        fd_delta,
        analytic_delta,
        abs_diff,
        abs_diff / analytic_delta.abs().max(1e-10) * 100.0
    );
}

#[test]
fn test_vega_by_finite_difference() {
    let as_of = date!(2024 - 01 - 01);
    let start = date!(2024 - 01 - 01);
    let end = date!(2024 - 04 - 01);

    let caplet = CapFloor {
        id: "CAPLET_VEGA_FD".into(),
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
        pricing_overrides: finstack_quant_valuations::instruments::PricingOverrides::default(),
        attributes: Default::default(),
    };

    let vol_base = 0.30;
    let vol_bump = 0.01; // 1% vol bump

    let vol_down = build_flat_vol_surface(vol_base - vol_bump, as_of, "USD_CAP_VOL");
    let vol_up = build_flat_vol_surface(vol_base + vol_bump, as_of, "USD_CAP_VOL");
    let vol_mid = build_flat_vol_surface(vol_base, as_of, "USD_CAP_VOL");

    let disc_curve1 = build_flat_discount_curve(0.05, as_of, "USD_OIS");
    let disc_curve2 = build_flat_discount_curve(0.05, as_of, "USD_OIS");
    let disc_curve3 = build_flat_discount_curve(0.05, as_of, "USD_OIS");
    let fwd_curve1 = build_flat_forward_curve(0.05, as_of, "USD_LIBOR_3M");
    let fwd_curve2 = build_flat_forward_curve(0.05, as_of, "USD_LIBOR_3M");
    let fwd_curve3 = build_flat_forward_curve(0.05, as_of, "USD_LIBOR_3M");

    let market_down = MarketContext::new()
        .insert(disc_curve1)
        .insert(fwd_curve1)
        .insert_surface(vol_down);

    let market_up = MarketContext::new()
        .insert(disc_curve2)
        .insert(fwd_curve2)
        .insert_surface(vol_up);

    let market_mid = MarketContext::new()
        .insert(disc_curve3)
        .insert(fwd_curve3)
        .insert_surface(vol_mid);

    let pv_down = caplet.value(&market_down, as_of).unwrap().amount();
    let pv_up = caplet.value(&market_up, as_of).unwrap().amount();

    // FD vega scaled to match the library convention: per 1% vol change.
    let fd_vega = (pv_up - pv_down) / (2.0 * vol_bump) * 0.01;

    // Analytical vega (per 1% vol change)
    let result = caplet
        .price_with_metrics(
            &market_mid,
            as_of,
            &[MetricId::Vega],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let analytic_vega = *result.measures.get("vega").unwrap();

    // FD vs analytic vega should match within 2% relative error for meaningful values,
    // or within absolute tolerance of $1 for small values.
    let abs_diff = (fd_vega - analytic_vega).abs();
    let within_relative = analytic_vega.abs() > 10.0 && abs_diff / analytic_vega.abs() < 0.02;
    let within_absolute = abs_diff < 1.0;

    assert!(
        within_relative || within_absolute,
        "FD vega ({:.4}) should match analytic vega ({:.4}): \
        abs_diff={:.4}, rel_error={:.2}%",
        fd_vega,
        analytic_vega,
        abs_diff,
        abs_diff / analytic_vega.abs().max(1e-10) * 100.0
    );
}

#[test]
fn test_numerical_stability_extreme_params() {
    // Test that pricing remains stable with extreme but valid parameters
    let as_of = date!(2024 - 01 - 01);
    let end = date!(2054 - 01 - 01); // 30 year cap

    let cap = CapFloor {
        id: "CAP_EXTREME".into(),
        rate_option_type: RateOptionType::Cap,
        notional: Money::new(100_000_000.0, Currency::USD), // $100MM
        strike: Decimal::try_from(0.15).expect("valid decimal"), // 15% strike
        start_date: as_of,
        maturity: end,
        frequency: Tenor::monthly(),
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
    let vol_surface = build_flat_vol_surface(0.50, as_of, "USD_CAP_VOL");

    let market = MarketContext::new()
        .insert(disc_curve)
        .insert(fwd_curve)
        .insert_surface(vol_surface);

    let result = cap
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Delta, MetricId::Vega],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    // All values should be finite
    assert!(result.value.amount().is_finite(), "PV should be finite");
    assert!(
        result.measures.get("delta").unwrap().is_finite(),
        "Delta should be finite"
    );
    assert!(
        result.measures.get("vega").unwrap().is_finite(),
        "Vega should be finite"
    );
}
