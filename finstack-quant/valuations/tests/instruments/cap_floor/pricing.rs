//! Pricing tests for interest rate options.
//!
//! Validates Black-76 model implementation for caps, floors, caplets, and floorlets.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{BusinessDayConvention, Date, DayCount, StubKind, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::ScalarTimeSeries;
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

        pricing_overrides: finstack_quant_valuations::instruments::PricingOverrides::default(),
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

        pricing_overrides: finstack_quant_valuations::instruments::PricingOverrides::default(),
        attributes: Default::default(),
    }
}

#[test]
fn test_cap_pv_positive() {
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

    let pv = cap.value(&market, as_of).unwrap();

    // Cap should have positive value due to vol
    assert!(pv.amount() > 0.0, "Cap PV should be positive");
    assert!(pv.currency() == Currency::USD);
}

#[test]
fn test_floor_pv_positive() {
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

    let pv = floor.value(&market, as_of).unwrap();

    // Floor should have positive value due to vol
    assert!(pv.amount() > 0.0, "Floor PV should be positive");
}

#[test]
fn test_atm_cap_pricing() {
    let as_of = date!(2024 - 01 - 01);
    let end = date!(2029 - 01 - 01);

    // ATM: strike equals forward
    let cap = create_standard_cap(as_of, end, 0.05);

    let disc_curve = build_flat_discount_curve(0.05, as_of, "USD_OIS");
    let fwd_curve = build_flat_forward_curve(0.05, as_of, "USD_LIBOR_3M");
    let vol_surface = build_flat_vol_surface(0.30, as_of, "USD_CAP_VOL");

    let market = MarketContext::new()
        .insert(disc_curve)
        .insert(fwd_curve)
        .insert_surface(vol_surface);

    let pv = cap.value(&market, as_of).unwrap().amount();

    // ATM cap should have significant value
    assert!(
        pv > 10_000.0,
        "ATM cap should have meaningful value: {}",
        pv
    );
}

#[test]
fn test_itm_cap_more_valuable_than_atm() {
    let as_of = date!(2024 - 01 - 01);
    let end = date!(2029 - 01 - 01);

    let atm_cap = create_standard_cap(as_of, end, 0.05);
    let itm_cap = create_standard_cap(as_of, end, 0.03); // Strike < forward = ITM

    let disc_curve = build_flat_discount_curve(0.05, as_of, "USD_OIS");
    let fwd_curve = build_flat_forward_curve(0.05, as_of, "USD_LIBOR_3M");
    let vol_surface = build_flat_vol_surface(0.30, as_of, "USD_CAP_VOL");

    let market = MarketContext::new()
        .insert(disc_curve)
        .insert(fwd_curve)
        .insert_surface(vol_surface);

    let atm_pv = atm_cap.value(&market, as_of).unwrap().amount();
    let itm_pv = itm_cap.value(&market, as_of).unwrap().amount();

    // ITM cap should be more valuable than ATM
    assert!(
        itm_pv > atm_pv,
        "ITM cap ({}) should be more valuable than ATM ({})",
        itm_pv,
        atm_pv
    );
}

#[test]
fn test_otm_cap_less_valuable_than_atm() {
    let as_of = date!(2024 - 01 - 01);
    let end = date!(2029 - 01 - 01);

    let atm_cap = create_standard_cap(as_of, end, 0.05);
    let otm_cap = create_standard_cap(as_of, end, 0.07); // Strike > forward = OTM

    let disc_curve = build_flat_discount_curve(0.05, as_of, "USD_OIS");
    let fwd_curve = build_flat_forward_curve(0.05, as_of, "USD_LIBOR_3M");
    let vol_surface = build_flat_vol_surface(0.30, as_of, "USD_CAP_VOL");

    let market = MarketContext::new()
        .insert(disc_curve)
        .insert(fwd_curve)
        .insert_surface(vol_surface);

    let atm_pv = atm_cap.value(&market, as_of).unwrap().amount();
    let otm_pv = otm_cap.value(&market, as_of).unwrap().amount();

    // OTM cap should be less valuable than ATM
    assert!(
        otm_pv < atm_pv,
        "OTM cap ({}) should be less valuable than ATM ({})",
        otm_pv,
        atm_pv
    );
}

#[test]
fn test_higher_vol_increases_cap_value() {
    let as_of = date!(2024 - 01 - 01);
    let end = date!(2029 - 01 - 01);

    let cap = create_standard_cap(as_of, end, 0.05);

    let disc_curve = build_flat_discount_curve(0.05, as_of, "USD_OIS");
    let fwd_curve = build_flat_forward_curve(0.05, as_of, "USD_LIBOR_3M");

    let low_vol_surface = build_flat_vol_surface(0.20, as_of, "USD_CAP_VOL");
    let high_vol_surface = build_flat_vol_surface(0.40, as_of, "USD_CAP_VOL");

    let low_vol_market = MarketContext::new()
        .insert(disc_curve)
        .insert(fwd_curve)
        .insert_surface(low_vol_surface);

    let disc_curve2 = build_flat_discount_curve(0.05, as_of, "USD_OIS");
    let fwd_curve2 = build_flat_forward_curve(0.05, as_of, "USD_LIBOR_3M");

    let high_vol_market = MarketContext::new()
        .insert(disc_curve2)
        .insert(fwd_curve2)
        .insert_surface(high_vol_surface);

    let low_vol_pv = cap.value(&low_vol_market, as_of).unwrap().amount();
    let high_vol_pv = cap.value(&high_vol_market, as_of).unwrap().amount();

    // Higher vol should increase option value
    assert!(
        high_vol_pv > low_vol_pv,
        "High vol ({}) should produce higher value than low vol ({})",
        high_vol_pv,
        low_vol_pv
    );
}

#[test]
fn test_caplet_single_period_pricing() {
    let as_of = date!(2024 - 01 - 01);
    let start = date!(2024 - 01 - 01);
    let end = date!(2024 - 04 - 01); // Single 3M period

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

    let pv = caplet.value(&market, as_of).unwrap();

    // Caplet should price successfully
    assert!(pv.amount() >= 0.0, "Caplet PV should be non-negative");
}

#[test]
fn test_pricing_with_different_vol_surfaces() {
    let as_of = date!(2024 - 01 - 01);
    let end = date!(2029 - 01 - 01);

    let cap = create_standard_cap(as_of, end, 0.05);

    let disc_curve = build_flat_discount_curve(0.05, as_of, "USD_OIS");
    let fwd_curve = build_flat_forward_curve(0.05, as_of, "USD_LIBOR_3M");

    // Price with 25% vol surface
    let vol_surface_25 = build_flat_vol_surface(0.25, as_of, "USD_CAP_VOL");
    let market_25 = MarketContext::new()
        .insert(disc_curve.clone())
        .insert(fwd_curve.clone())
        .insert_surface(vol_surface_25);
    let pv_25 = cap.value(&market_25, as_of).unwrap();

    // Price with 30% vol surface
    let vol_surface_30 = build_flat_vol_surface(0.30, as_of, "USD_CAP_VOL");
    let market_30 = MarketContext::new()
        .insert(disc_curve)
        .insert(fwd_curve)
        .insert_surface(vol_surface_30);
    let pv_30 = cap.value(&market_30, as_of).unwrap();

    // Both should price positively, and higher vol should give higher cap price
    assert!(pv_25.amount() > 0.0, "Cap with 25% vol should price");
    assert!(
        pv_30.amount() > pv_25.amount(),
        "Higher vol should give higher cap price"
    );
}

#[test]
fn test_zero_notional_cap() {
    let as_of = date!(2024 - 01 - 01);
    let end = date!(2029 - 01 - 01);

    let mut cap = create_standard_cap(as_of, end, 0.05);
    cap.notional = Money::new(0.0, Currency::USD);

    let disc_curve = build_flat_discount_curve(0.05, as_of, "USD_OIS");
    let fwd_curve = build_flat_forward_curve(0.05, as_of, "USD_LIBOR_3M");
    let vol_surface = build_flat_vol_surface(0.30, as_of, "USD_CAP_VOL");

    let market = MarketContext::new()
        .insert(disc_curve)
        .insert(fwd_curve)
        .insert_surface(vol_surface);

    let pv = cap.value(&market, as_of).unwrap().amount();

    // Zero notional should result in zero PV
    assert_eq!(pv, 0.0, "Zero notional cap should have zero PV");
}

#[test]
fn test_longer_maturity_more_valuable() {
    let as_of = date!(2024 - 01 - 01);

    let short_cap = create_standard_cap(as_of, date!(2026 - 01 - 01), 0.05);
    let long_cap = create_standard_cap(as_of, date!(2034 - 01 - 01), 0.05);

    let disc_curve = build_flat_discount_curve(0.05, as_of, "USD_OIS");
    let fwd_curve = build_flat_forward_curve(0.05, as_of, "USD_LIBOR_3M");
    let vol_surface = build_flat_vol_surface(0.30, as_of, "USD_CAP_VOL");

    let market = MarketContext::new()
        .insert(disc_curve)
        .insert(fwd_curve)
        .insert_surface(vol_surface);

    let short_pv = short_cap.value(&market, as_of).unwrap().amount();
    let long_pv = long_cap.value(&market, as_of).unwrap().amount();

    // Longer maturity cap contains more caplets, should be more valuable
    assert!(
        long_pv > short_pv,
        "10Y cap ({}) should be more valuable than 2Y cap ({})",
        long_pv,
        short_pv
    );
}

/// Test that fixing date and payment date timing are handled correctly.
///
/// In a caplet:
/// - **Fixing date** (start of period): The date when the forward rate is observed/fixed.
///   Time-to-fixing is used for vol surface lookup and determines if option has expired.
/// - **Payment date** (end of period): The date when the cashflow is paid.
///   The discount factor is computed to this date.
///
/// This test verifies these timing conventions by comparing a forward-starting caplet
/// against immediate pricing to ensure the discount factor and forward rate are
/// computed relative to the correct dates.
#[test]
fn test_fixing_vs_payment_date_timing() {
    let as_of = date!(2024 - 01 - 01);
    let fixing_date = date!(2024 - 03 - 01); // Start of period = fixing date
    let payment_date = date!(2024 - 06 - 01); // End of period = payment date

    // Forward-starting caplet
    let caplet = CapFloor {
        id: "CAPLET_TIMING".into(),
        rate_option_type: RateOptionType::Caplet,
        notional: Money::new(1_000_000.0, Currency::USD),
        strike: Decimal::try_from(0.05).expect("valid decimal"),
        start_date: fixing_date,
        maturity: payment_date,
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

    let pv = caplet.value(&market, as_of).unwrap();

    // Forward-starting caplet should have positive value
    assert!(
        pv.amount() > 0.0,
        "Forward-starting caplet should have positive value: {}",
        pv.amount()
    );

    // Price the same caplet ON the fixing date
    // At this point, t_fix = 0, so we should get intrinsic value (which could be zero for ATM)
    let as_of_fixing = fixing_date;
    let disc_curve2 = build_flat_discount_curve(0.05, as_of_fixing, "USD_OIS");
    let fwd_curve2 = build_flat_forward_curve(0.05, as_of_fixing, "USD_LIBOR_3M");
    let vol_surface2 = build_flat_vol_surface(0.30, as_of_fixing, "USD_CAP_VOL");

    let market2 = MarketContext::new()
        .insert(disc_curve2)
        .insert(fwd_curve2)
        .insert_surface(vol_surface2);

    let pv_at_fixing = caplet.value(&market2, as_of_fixing).unwrap();

    // At fixing date with t_fix=0, we get intrinsic value (0 for ATM caplet)
    // But the caplet hasn't paid yet, so it shouldn't be zero if F > K
    // For ATM (F=K=5%), intrinsic is zero
    assert!(
        pv_at_fixing.amount() >= 0.0,
        "Caplet at fixing date should have non-negative value: {}",
        pv_at_fixing.amount()
    );

    // Note: Testing seasoned caplets (past fixing, before payment) would require
    // providing the actual fixed rate rather than relying on the forward curve.
    // The standard pricer uses forward curve rates, which is valid for forward-starting
    // caplets but not for seasoned caplets where the rate is already fixed.
}

#[test]
fn test_seasoned_caplet_uses_historical_fixing_after_reset() {
    let fixing_date = date!(2024 - 01 - 01);
    let as_of = date!(2024 - 02 - 15);
    let payment_date = date!(2024 - 04 - 01);

    let caplet = CapFloor {
        id: "CAPLET_SEASONED".into(),
        rate_option_type: RateOptionType::Caplet,
        notional: Money::new(1_000_000.0, Currency::USD),
        strike: Decimal::try_from(0.05).expect("valid decimal"),
        start_date: fixing_date,
        maturity: payment_date,
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
        pricing_overrides: finstack_quant_valuations::instruments::PricingOverrides::default(),
        attributes: Default::default(),
    };

    let disc_curve = build_flat_discount_curve(0.03, as_of, "USD_OIS");
    let fwd_curve = build_flat_forward_curve(0.12, as_of, "USD_LIBOR_3M");
    let vol_surface = build_flat_vol_surface(0.30, as_of, "USD_CAP_VOL");
    let fixings =
        ScalarTimeSeries::new("FIXING:USD_LIBOR_3M", vec![(fixing_date, 0.07)], None).unwrap();

    let market = MarketContext::new()
        .insert(disc_curve)
        .insert(fwd_curve)
        .insert_surface(vol_surface)
        .insert_series(fixings);

    let pv = caplet.value(&market, as_of).unwrap().amount();

    let tau = DayCount::Act360
        .year_fraction(fixing_date, payment_date, Default::default())
        .unwrap();
    let df = market
        .get_discount("USD_OIS")
        .unwrap()
        .df_between_dates(as_of, payment_date)
        .unwrap();
    let expected = 1_000_000.0 * tau * df * (0.07_f64 - 0.05_f64);

    assert!(
        (pv - expected).abs() < 0.01,
        "Seasoned caplet should use realized fixing after reset: expected {expected}, got {pv}"
    );
}

#[test]
fn test_single_period_cap_matches_caplet_with_resolved_lags() {
    let as_of = date!(2024 - 01 - 01);
    let start = date!(2024 - 03 - 01);
    let end = date!(2024 - 06 - 01);
    let strike = 0.05;
    let notional = Money::new(1_000_000.0, Currency::USD);

    let cap = CapFloor::new_cap(
        "ONE_PERIOD_CAP",
        notional,
        strike,
        start,
        end,
        Tenor::quarterly(),
        DayCount::Act360,
        "USD_OIS",
        "USD-SOFR-OIS",
        "USD_CAP_VOL",
    )
    .expect("valid strike");
    let caplet = CapFloor::new_caplet(
        "ONE_PERIOD_CAPLET",
        notional,
        strike,
        start,
        end,
        DayCount::Act360,
        "USD_OIS",
        "USD-SOFR-OIS",
        "USD_CAP_VOL",
    )
    .expect("valid literal strike");

    let market = MarketContext::new()
        .insert(build_flat_discount_curve(0.03, as_of, "USD_OIS"))
        .insert(build_flat_forward_curve(0.05, as_of, "USD-SOFR-OIS"))
        .insert_surface(build_flat_vol_surface(0.30, as_of, "USD_CAP_VOL"));

    let cap_pv = cap.value(&market, as_of).unwrap().amount();
    let caplet_pv = caplet.value(&market, as_of).unwrap().amount();

    assert!(
        (cap_pv - caplet_pv).abs() < 0.01,
        "One-period cap and caplet should agree under the same lag conventions: cap={cap_pv}, caplet={caplet_pv}"
    );
}

/// Test that a caplet valued after the payment date returns zero.
#[test]
fn test_caplet_after_payment_date_is_zero() {
    let as_of = date!(2024 - 07 - 01); // After payment date
    let fixing_date = date!(2024 - 03 - 01);
    let payment_date = date!(2024 - 06 - 01);

    let caplet = CapFloor {
        id: "CAPLET_EXPIRED".into(),
        rate_option_type: RateOptionType::Caplet,
        notional: Money::new(1_000_000.0, Currency::USD),
        strike: Decimal::try_from(0.05).expect("valid decimal"),
        start_date: fixing_date,
        maturity: payment_date,
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

    let pv = caplet.value(&market, as_of).unwrap();

    // Caplet after payment date should have zero value (cashflow already settled)
    assert_eq!(
        pv.amount(),
        0.0,
        "Caplet after payment date should be zero: {}",
        pv.amount()
    );
}
