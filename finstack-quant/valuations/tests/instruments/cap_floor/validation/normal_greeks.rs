//! Bachelier (Normal) model Greeks validation for interest rate options.
//!
//! Validates that:
//! 1. Normal-vol Greeks have correct signs (cap delta > 0, floor delta < 0, etc.)
//! 2. Normal-vol delta matches finite-difference approximation
//! 3. Normal-vol vega matches finite-difference approximation
//! 4. Normal-vol gamma is non-negative
//! 5. Normal-vol Greeks handle negative forwards correctly

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{BusinessDayConvention, Date, DayCount, StubKind, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::surfaces::VolSurface;
use finstack_quant_core::market_data::term_structures::{DiscountCurve, ForwardCurve};
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::rates::cap_floor::{
    CapFloor, CapFloorVolType, RateOptionType,
};
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

fn build_normal_vol_surface(vol: f64, _base_date: Date, surface_id: &str) -> VolSurface {
    VolSurface::builder(surface_id)
        .expiries(&[0.25, 1.0, 5.0, 10.0])
        .strikes(&[-0.02, 0.0, 0.03, 0.05, 0.10])
        .row(&[vol, vol, vol, vol, vol])
        .row(&[vol, vol, vol, vol, vol])
        .row(&[vol, vol, vol, vol, vol])
        .row(&[vol, vol, vol, vol, vol])
        .build()
        .unwrap()
}

fn create_normal_cap(as_of: Date, end: Date, strike: f64) -> CapFloor {
    CapFloor {
        id: "CAP_NORMAL".into(),
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
        forward_curve_id: "USD_FWD".into(),
        vol_surface_id: "NORMAL_VOL".into(),
        vol_type: CapFloorVolType::Normal,
        vol_shift: 0.0,
        pricing_overrides: finstack_quant_valuations::instruments::PricingOverrides::default(),
        attributes: Default::default(),
    }
}

fn create_normal_floor(as_of: Date, end: Date, strike: f64) -> CapFloor {
    CapFloor {
        id: "FLOOR_NORMAL".into(),
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
        forward_curve_id: "USD_FWD".into(),
        vol_surface_id: "NORMAL_VOL".into(),
        vol_type: CapFloorVolType::Normal,
        vol_shift: 0.0,
        pricing_overrides: finstack_quant_valuations::instruments::PricingOverrides::default(),
        attributes: Default::default(),
    }
}

fn normal_market(as_of: Date, fwd_rate: f64) -> MarketContext {
    let disc = build_flat_discount_curve(0.05, as_of, "USD_OIS");
    let fwd = build_flat_forward_curve(fwd_rate, as_of, "USD_FWD");
    let vol = build_normal_vol_surface(0.005, as_of, "NORMAL_VOL");
    MarketContext::new()
        .insert(disc)
        .insert(fwd)
        .insert_surface(vol)
}

#[test]
fn normal_cap_delta_positive() {
    let as_of = date!(2024 - 01 - 01);
    let cap = create_normal_cap(as_of, date!(2027 - 01 - 01), 0.05);
    let ctx = normal_market(as_of, 0.05);

    let result = cap
        .price_with_metrics(
            &ctx,
            as_of,
            &[MetricId::Delta],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let delta = *result.measures.get("delta").unwrap();

    assert!(
        delta > 0.0,
        "Normal cap delta should be positive: {}",
        delta
    );
    assert!(delta.is_finite(), "Normal cap delta should be finite");
}

#[test]
fn normal_floor_delta_negative() {
    let as_of = date!(2024 - 01 - 01);
    let floor = create_normal_floor(as_of, date!(2027 - 01 - 01), 0.05);
    let ctx = normal_market(as_of, 0.05);

    let result = floor
        .price_with_metrics(
            &ctx,
            as_of,
            &[MetricId::Delta],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let delta = *result.measures.get("delta").unwrap();

    assert!(
        delta < 0.0,
        "Normal floor delta should be negative: {}",
        delta
    );
}

#[test]
fn normal_gamma_non_negative() {
    let as_of = date!(2024 - 01 - 01);
    let cap = create_normal_cap(as_of, date!(2027 - 01 - 01), 0.05);
    let floor = create_normal_floor(as_of, date!(2027 - 01 - 01), 0.05);
    let ctx = normal_market(as_of, 0.05);

    let cap_gamma = *cap
        .price_with_metrics(
            &ctx,
            as_of,
            &[MetricId::Gamma],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap()
        .measures
        .get("gamma")
        .unwrap();

    let floor_gamma = *floor
        .price_with_metrics(
            &ctx,
            as_of,
            &[MetricId::Gamma],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap()
        .measures
        .get("gamma")
        .unwrap();

    assert!(
        cap_gamma >= 0.0,
        "Normal cap gamma non-negative: {}",
        cap_gamma
    );
    assert!(
        floor_gamma >= 0.0,
        "Normal floor gamma non-negative: {}",
        floor_gamma
    );
}

#[test]
fn normal_vega_positive() {
    let as_of = date!(2024 - 01 - 01);
    let cap = create_normal_cap(as_of, date!(2027 - 01 - 01), 0.05);
    let ctx = normal_market(as_of, 0.05);

    let vega = *cap
        .price_with_metrics(
            &ctx,
            as_of,
            &[MetricId::Vega],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap()
        .measures
        .get("vega")
        .unwrap();

    assert!(vega > 0.0, "Normal cap vega should be positive: {}", vega);
    assert!(vega.is_finite(), "Normal cap vega should be finite");
}

#[test]
fn normal_delta_matches_finite_difference() {
    let as_of = date!(2024 - 01 - 01);
    let start = date!(2024 - 03 - 01);
    let end = date!(2024 - 06 - 01);

    let caplet = CapFloor {
        id: "CAPLET_NORMAL_FD".into(),
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
        forward_curve_id: "USD_FWD".into(),
        vol_surface_id: "NORMAL_VOL".into(),
        vol_type: CapFloorVolType::Normal,
        vol_shift: 0.0,
        pricing_overrides: finstack_quant_valuations::instruments::PricingOverrides::default(),
        attributes: Default::default(),
    };

    let bump = 0.0001;
    let base_rate = 0.05;

    let ctx_down = normal_market(as_of, base_rate - bump);
    let ctx_base = normal_market(as_of, base_rate);
    let ctx_up = normal_market(as_of, base_rate + bump);

    let pv_down = caplet.value(&ctx_down, as_of).unwrap().amount();
    let pv_up = caplet.value(&ctx_up, as_of).unwrap().amount();

    let fd_delta = (pv_up - pv_down) / (2.0 * bump);

    let analytic_delta = *caplet
        .price_with_metrics(
            &ctx_base,
            as_of,
            &[MetricId::Delta],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap()
        .measures
        .get("delta")
        .unwrap();

    let abs_diff = (fd_delta - analytic_delta).abs();
    let within_relative = analytic_delta.abs() > 100.0 && abs_diff / analytic_delta.abs() < 0.05;
    let within_absolute = abs_diff < 10.0;

    assert!(
        within_relative || within_absolute,
        "Normal FD delta ({:.4}) vs analytic ({:.4}): abs_diff={:.4}",
        fd_delta,
        analytic_delta,
        abs_diff
    );
}

#[test]
fn normal_vega_matches_finite_difference() {
    let as_of = date!(2024 - 01 - 01);
    let start = date!(2024 - 01 - 01);
    let end = date!(2024 - 04 - 01);

    let caplet = CapFloor {
        id: "CAPLET_NORMAL_VEGA_FD".into(),
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
        forward_curve_id: "USD_FWD".into(),
        vol_surface_id: "NORMAL_VOL".into(),
        vol_type: CapFloorVolType::Normal,
        vol_shift: 0.0,
        pricing_overrides: finstack_quant_valuations::instruments::PricingOverrides::default(),
        attributes: Default::default(),
    };

    let base_vol = 0.005;
    let vol_bump = 0.0001;

    let vol_down = build_normal_vol_surface(base_vol - vol_bump, as_of, "NORMAL_VOL");
    let vol_mid = build_normal_vol_surface(base_vol, as_of, "NORMAL_VOL");
    let vol_up = build_normal_vol_surface(base_vol + vol_bump, as_of, "NORMAL_VOL");

    let ctx_down = MarketContext::new()
        .insert(build_flat_discount_curve(0.05, as_of, "USD_OIS"))
        .insert(build_flat_forward_curve(0.05, as_of, "USD_FWD"))
        .insert_surface(vol_down);
    let ctx_mid = MarketContext::new()
        .insert(build_flat_discount_curve(0.05, as_of, "USD_OIS"))
        .insert(build_flat_forward_curve(0.05, as_of, "USD_FWD"))
        .insert_surface(vol_mid);
    let ctx_up = MarketContext::new()
        .insert(build_flat_discount_curve(0.05, as_of, "USD_OIS"))
        .insert(build_flat_forward_curve(0.05, as_of, "USD_FWD"))
        .insert_surface(vol_up);

    let pv_down = caplet.value(&ctx_down, as_of).unwrap().amount();
    let pv_up = caplet.value(&ctx_up, as_of).unwrap().amount();

    let fd_vega = (pv_up - pv_down) / (2.0 * vol_bump * 100.0);

    let analytic_vega = *caplet
        .price_with_metrics(
            &ctx_mid,
            as_of,
            &[MetricId::Vega],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap()
        .measures
        .get("vega")
        .unwrap();

    let abs_diff = (fd_vega - analytic_vega).abs();
    let within_relative = analytic_vega.abs() > 1.0 && abs_diff / analytic_vega.abs() < 0.05;
    let within_absolute = abs_diff < 1.0;

    assert!(
        within_relative || within_absolute,
        "Normal FD vega ({:.6}) vs analytic ({:.6}): abs_diff={:.6}",
        fd_vega,
        analytic_vega,
        abs_diff
    );
}

#[test]
fn normal_greeks_with_negative_forward() {
    let as_of = date!(2024 - 01 - 01);
    let start = date!(2024 - 03 - 01);
    let end = date!(2027 - 03 - 01);

    let cap = CapFloor {
        id: "CAP_NEG_FWD".into(),
        rate_option_type: RateOptionType::Cap,
        notional: Money::new(1_000_000.0, Currency::USD),
        strike: Decimal::try_from(0.0).expect("valid decimal"),
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
        forward_curve_id: "USD_FWD".into(),
        vol_surface_id: "NORMAL_VOL".into(),
        vol_type: CapFloorVolType::Normal,
        vol_shift: 0.0,
        pricing_overrides: finstack_quant_valuations::instruments::PricingOverrides::default(),
        attributes: Default::default(),
    };

    let disc = build_flat_discount_curve(0.02, as_of, "USD_OIS");
    let fwd = build_flat_forward_curve(-0.005, as_of, "USD_FWD");
    let vol = build_normal_vol_surface(0.005, as_of, "NORMAL_VOL");
    let ctx = MarketContext::new()
        .insert(disc)
        .insert(fwd)
        .insert_surface(vol);

    let pv = cap.value(&ctx, as_of).unwrap();
    assert!(
        pv.amount().is_finite(),
        "Normal PV should be finite with negative fwd"
    );

    let metrics = [MetricId::Delta, MetricId::Gamma, MetricId::Vega];
    for metric in &metrics {
        let result = cap.price_with_metrics(
            &ctx,
            as_of,
            std::slice::from_ref(metric),
            finstack_quant_valuations::instruments::PricingOptions::default(),
        );
        assert!(
            result.is_ok(),
            "Normal {:?} should succeed with negative fwd",
            metric
        );
        let val = result.unwrap();
        let metric_name = format!("{metric:?}").to_lowercase();
        if let Some(&v) = val.measures.get(metric_name.as_str()) {
            assert!(
                v.is_finite(),
                "Normal {:?} should be finite with negative fwd, got: {v}",
                metric
            );
        }
    }
}

#[test]
fn normal_delta_increases_with_moneyness() {
    let as_of = date!(2024 - 01 - 01);
    let end = date!(2027 - 01 - 01);
    let ctx = normal_market(as_of, 0.05);

    let otm_cap = create_normal_cap(as_of, end, 0.08);
    let atm_cap = create_normal_cap(as_of, end, 0.05);
    let itm_cap = create_normal_cap(as_of, end, 0.02);

    let otm_delta = *otm_cap
        .price_with_metrics(
            &ctx,
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
            &ctx,
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
            &ctx,
            as_of,
            &[MetricId::Delta],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap()
        .measures
        .get("delta")
        .unwrap();

    assert!(
        itm_delta > atm_delta,
        "Normal ITM delta ({}) > ATM ({})",
        itm_delta,
        atm_delta
    );
    assert!(
        atm_delta > otm_delta,
        "Normal ATM delta ({}) > OTM ({})",
        atm_delta,
        otm_delta
    );
}
