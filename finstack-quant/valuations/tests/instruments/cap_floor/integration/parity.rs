//! Cap-floor parity tests.
//!
//! Validates the fundamental relationship: Cap - Floor ≈ Swap at strike.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{BusinessDayConvention, Date, DayCount, StubKind, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::surfaces::VolSurface;
use finstack_quant_core::market_data::term_structures::{DiscountCurve, ForwardCurve};
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::rates::cap_floor::{CapFloor, RateOptionType};
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::instruments::{ExerciseStyle, SettlementType};
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

fn create_cap(as_of: Date, end: Date, strike: f64) -> CapFloor {
    CapFloor {
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
        pricing_overrides: finstack_quant_valuations::instruments::PricingOverrides::default(),
        attributes: Default::default(),
    }
}

fn create_floor(as_of: Date, end: Date, strike: f64) -> CapFloor {
    CapFloor {
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
        pricing_overrides: finstack_quant_valuations::instruments::PricingOverrides::default(),
        attributes: Default::default(),
    }
}

#[test]
fn test_cap_floor_parity_atm() {
    // Cap - Floor ≈ 0 when strike = forward (ATM)
    let as_of = date!(2024 - 01 - 01);
    let end = date!(2029 - 01 - 01);
    let strike = 0.05; // Equal to forward

    let cap = create_cap(as_of, end, strike);
    let floor = create_floor(as_of, end, strike);

    let disc_curve = build_flat_discount_curve(0.05, as_of, "USD_OIS");
    let fwd_curve = build_flat_forward_curve(0.05, as_of, "USD_LIBOR_3M");
    let vol_surface = build_flat_vol_surface(0.30, as_of, "USD_CAP_VOL");

    let market = MarketContext::new()
        .insert(disc_curve)
        .insert(fwd_curve)
        .insert_surface(vol_surface);

    let cap_pv = cap.value(&market, as_of).unwrap().amount();
    let floor_pv = floor.value(&market, as_of).unwrap().amount();

    // At ATM, cap and floor should have similar values (Cap - Floor ≈ 0)
    let diff = (cap_pv - floor_pv).abs();
    let avg = (cap_pv + floor_pv) / 2.0;
    let relative_diff = diff / avg;

    assert!(
        relative_diff < 0.05,
        "ATM cap-floor parity: diff={}, relative={}",
        diff,
        relative_diff
    );
}

#[test]
fn test_cap_floor_parity_itm_cap() {
    // Cap - Floor > 0 when strike < forward (ITM cap)
    let as_of = date!(2024 - 01 - 01);
    let end = date!(2029 - 01 - 01);
    let strike = 0.03; // Below forward (5%)

    let cap = create_cap(as_of, end, strike);
    let floor = create_floor(as_of, end, strike);

    let disc_curve = build_flat_discount_curve(0.05, as_of, "USD_OIS");
    let fwd_curve = build_flat_forward_curve(0.05, as_of, "USD_LIBOR_3M");
    let vol_surface = build_flat_vol_surface(0.30, as_of, "USD_CAP_VOL");

    let market = MarketContext::new()
        .insert(disc_curve)
        .insert(fwd_curve)
        .insert_surface(vol_surface);

    let cap_pv = cap.value(&market, as_of).unwrap().amount();
    let floor_pv = floor.value(&market, as_of).unwrap().amount();

    // ITM cap should be worth more than OTM floor
    assert!(
        cap_pv > floor_pv,
        "ITM cap ({}) should be > OTM floor ({})",
        cap_pv,
        floor_pv
    );
}

#[test]
fn test_cap_floor_parity_otm_cap() {
    // Cap - Floor < 0 when strike > forward (OTM cap, ITM floor)
    let as_of = date!(2024 - 01 - 01);
    let end = date!(2029 - 01 - 01);
    let strike = 0.07; // Above forward (5%)

    let cap = create_cap(as_of, end, strike);
    let floor = create_floor(as_of, end, strike);

    let disc_curve = build_flat_discount_curve(0.05, as_of, "USD_OIS");
    let fwd_curve = build_flat_forward_curve(0.05, as_of, "USD_LIBOR_3M");
    let vol_surface = build_flat_vol_surface(0.30, as_of, "USD_CAP_VOL");

    let market = MarketContext::new()
        .insert(disc_curve)
        .insert(fwd_curve)
        .insert_surface(vol_surface);

    let cap_pv = cap.value(&market, as_of).unwrap().amount();
    let floor_pv = floor.value(&market, as_of).unwrap().amount();

    // OTM cap should be worth less than ITM floor
    assert!(
        cap_pv < floor_pv,
        "OTM cap ({}) should be < ITM floor ({})",
        cap_pv,
        floor_pv
    );
}

#[test]
fn test_parity_with_different_vols() {
    // Parity should hold regardless of vol level
    let as_of = date!(2024 - 01 - 01);
    let end = date!(2029 - 01 - 01);
    let strike = 0.05;

    let cap = create_cap(as_of, end, strike);
    let floor = create_floor(as_of, end, strike);

    // Test with low and high vol
    for vol in [0.10, 0.50] {
        let vol_surface = build_flat_vol_surface(vol, as_of, "USD_CAP_VOL");
        let disc_curve_vol = build_flat_discount_curve(0.05, as_of, "USD_OIS");
        let fwd_curve_vol = build_flat_forward_curve(0.05, as_of, "USD_LIBOR_3M");

        let market = MarketContext::new()
            .insert(disc_curve_vol)
            .insert(fwd_curve_vol)
            .insert_surface(vol_surface);

        let cap_pv = cap.value(&market, as_of).unwrap().amount();
        let floor_pv = floor.value(&market, as_of).unwrap().amount();

        let diff = (cap_pv - floor_pv).abs();
        let avg = (cap_pv + floor_pv) / 2.0;
        let relative_diff = diff / avg;

        assert!(
            relative_diff < 0.10,
            "Parity should hold at vol={}: relative diff={}",
            vol,
            relative_diff
        );
    }
}

#[test]
fn test_caplet_floorlet_parity() {
    let as_of = date!(2024 - 01 - 01);
    let start = date!(2024 - 04 - 01);
    let end = date!(2024 - 07 - 01);
    let strike = 0.05;

    let caplet = CapFloor {
        id: "CAPLET".into(),
        rate_option_type: RateOptionType::Caplet,
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
    };

    let floorlet = CapFloor {
        id: "FLOORLET".into(),
        rate_option_type: RateOptionType::Floorlet,
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
    };

    let disc_curve = build_flat_discount_curve(0.05, as_of, "USD_OIS");
    let fwd_curve = build_flat_forward_curve(0.05, as_of, "USD_LIBOR_3M");
    let vol_surface = build_flat_vol_surface(0.30, as_of, "USD_CAP_VOL");

    let market = MarketContext::new()
        .insert(disc_curve)
        .insert(fwd_curve)
        .insert_surface(vol_surface);

    let caplet_pv = caplet.value(&market, as_of).unwrap().amount();
    let floorlet_pv = floorlet.value(&market, as_of).unwrap().amount();

    // ATM caplet and floorlet should have similar values
    let diff = (caplet_pv - floorlet_pv).abs();
    let avg = (caplet_pv + floorlet_pv) / 2.0;
    let relative_diff = if avg > 0.0 { diff / avg } else { 0.0 };

    assert!(
        relative_diff < 0.05,
        "ATM caplet-floorlet parity: diff={}, relative={}",
        diff,
        relative_diff
    );
}
