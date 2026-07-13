//! Tests for FloatingRateFallback policy wired into emission.
//!
//! Covers three fallback variants when no forward curve is available:
//! - `Error`: build should fail with an error
//! - `SpreadOnly`: build succeeds, rate == spread (legacy behavior)
//! - `FixedRate(r)`: build succeeds, rate == r + spread (through params pipeline)

use finstack_quant_cashflows::builder::specs::{
    CouponType, FloatingCouponSpec, FloatingRateFallback, FloatingRateSpec,
    OvernightIndexConstraintApplication,
};
use finstack_quant_cashflows::builder::CashFlowSchedule;
use finstack_quant_core::cashflow::CFKind;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{
    BusinessDayConvention, DayCount, DayCountContext, StubKind, Tenor,
};
use finstack_quant_core::money::Money;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use time::Month;

type Date = finstack_quant_core::dates::Date;

/// Build a minimal floating coupon spec with the given fallback policy and spread.
fn make_float_spec(fallback: FloatingRateFallback, spread_bp: Decimal) -> FloatingCouponSpec {
    FloatingCouponSpec {
        rate_spec: FloatingRateSpec {
            index_id: "USD-SOFR-3M".into(),
            spread_bp,
            gearing: Decimal::ONE,
            gearing_includes_spread: true,
            index_floor_bp: None,
            all_in_cap_bp: None,
            all_in_floor_bp: None,
            index_cap_bp: None,
            overnight_index_constraints: OvernightIndexConstraintApplication::Daily,
            reset_freq: Tenor::quarterly(),
            index_tenor: None,
            reset_lag_days: 0,
            fixing_calendar_id: None,
            overnight_compounding: None,
            overnight_basis: None,
            fallback,
        },
        coupon_type: CouponType::Cash,
        schedule: finstack_quant_cashflows::builder::ScheduleParams {
            freq: Tenor::quarterly(),
            dc: DayCount::Act360,
            bdc: BusinessDayConvention::Following,
            calendar_id: "weekends_only".to_string(),
            stub: StubKind::None,
            end_of_month: false,
            payment_lag_days: 0,
            adjust_accrual_dates: false,
        },
    }
}

#[test]
fn term_coupon_uses_the_actual_reset_date_fixing() {
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::term_structures::ForwardCurve;

    let base = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let issue = Date::from_calendar_date(2025, Month::January, 15).unwrap();
    let maturity = Date::from_calendar_date(2025, Month::April, 15).unwrap();
    let mut spec = make_float_spec(FloatingRateFallback::Error, dec!(0.0));
    spec.rate_spec.reset_lag_days = 2;

    let fwd = ForwardCurve::builder("USD-SOFR-3M", 0.25)
        .base_date(base)
        .day_count(DayCount::Act360)
        .knots([(0.0, 0.01), (1.0, 0.21)])
        .build()
        .expect("ForwardCurve builder should succeed");
    let market = MarketContext::new().insert(fwd);

    let mut builder = CashFlowSchedule::builder();
    let _ = builder
        .principal(Money::new(1_000_000.0, Currency::USD), issue, maturity)
        .floating_cf(spec);
    let schedule = builder
        .build_with_curves(Some(&market))
        .expect("floating schedule should build");
    let first_float = schedule
        .flows
        .iter()
        .find(|cf| cf.kind == CFKind::FloatReset)
        .expect("expected a floating coupon");
    let reset_date = first_float
        .reset_date
        .expect("floating coupon should have a reset date");
    let fwd = market
        .get_forward("USD-SOFR-3M")
        .expect("forward curve should exist");
    let reset_t = fwd
        .day_count()
        .year_fraction(base, reset_date, DayCountContext::default())
        .expect("valid reset year fraction");
    let payment_t = fwd
        .day_count()
        .year_fraction(base, first_float.date, DayCountContext::default())
        .expect("valid payment year fraction");
    let reset_fixing = fwd.rate(reset_t);
    let integrated_average = fwd.rate_period(reset_t, payment_t);
    let built_rate = first_float
        .rate
        .expect("floating coupon should store its rate");

    assert_eq!(
        reset_date,
        Date::from_calendar_date(2025, Month::January, 13).expect("valid reset date")
    );
    assert!((reset_fixing - integrated_average).abs() > 1e-6);
    assert!((built_rate - reset_fixing).abs() < RATE_TOLERANCE);
}

#[test]
fn term_index_rate_is_invariant_to_payment_frequency() {
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::term_structures::ForwardCurve;

    let issue = Date::from_calendar_date(2025, Month::January, 15).unwrap();
    let maturity = Date::from_calendar_date(2026, Month::January, 15).unwrap();
    let init = Money::new(1_000_000.0, Currency::USD);

    let mut spec = make_float_spec(FloatingRateFallback::Error, dec!(0.0));
    spec.schedule.freq = Tenor::semi_annual();
    spec.rate_spec.reset_freq = Tenor::quarterly();

    let fwd = ForwardCurve::builder("USD-SOFR-3M", 0.25)
        .base_date(issue)
        .day_count(DayCount::Act360)
        .knots([(0.0, 0.02), (0.25, 0.03), (0.5, 0.05), (1.0, 0.07)])
        .build()
        .expect("ForwardCurve builder should succeed");
    let market = MarketContext::new().insert(fwd);

    let mut b = CashFlowSchedule::builder();
    let _ = b.principal(init, issue, maturity).floating_cf(spec);
    let schedule = b
        .build_with_curves(Some(&market))
        .expect("mixed tenor floating schedule should build");

    let first_float = schedule
        .flows
        .iter()
        .find(|cf| cf.kind == CFKind::FloatReset)
        .expect("expected at least one floating coupon");

    let reset_date = first_float
        .reset_date
        .expect("floating coupon should have reset date");
    let fwd_curve = market
        .get_forward("USD-SOFR-3M")
        .expect("curve should exist");

    // A term-index coupon fixes its curve tenor at the reset date. The payment
    // frequency has no effect on that fixing.
    let reset_t = fwd_curve
        .day_count()
        .year_fraction(issue, reset_date, DayCountContext::default())
        .expect("valid reset year fraction");
    let expected_reset_fixing = fwd_curve.rate(reset_t);

    let built_rate = first_float
        .rate
        .expect("floating coupon should store built rate");
    assert!(
        (built_rate - expected_reset_fixing).abs() < RATE_TOLERANCE,
        "built rate should use the reset-date fixing: expected {}, got {}",
        expected_reset_fixing,
        built_rate
    );
}

// =============================================================================
// Test 1: FloatingRateFallback::Error + no curve => Err
// =============================================================================

#[test]
fn test_floating_rate_fallback_error_no_curve() {
    let issue = Date::from_calendar_date(2025, Month::January, 15).unwrap();
    let maturity = Date::from_calendar_date(2026, Month::January, 15).unwrap();
    let init = Money::new(1_000_000.0, Currency::USD);

    let spec = make_float_spec(FloatingRateFallback::Error, dec!(200.0));

    let mut b = CashFlowSchedule::builder();
    let _ = b.principal(init, issue, maturity).floating_cf(spec);

    // No market context => no forward curve => should error
    let result = b.build_with_curves(None);
    assert!(
        result.is_err(),
        "build_with_curves(None) should fail when fallback is Error"
    );
}

// =============================================================================
// Test 2: FloatingRateFallback::SpreadOnly + no curve => spread-only rate
// =============================================================================

#[test]
fn test_floating_rate_fallback_spread_only_no_curve() {
    let issue = Date::from_calendar_date(2025, Month::January, 15).unwrap();
    let maturity = Date::from_calendar_date(2026, Month::January, 15).unwrap();
    let init = Money::new(1_000_000.0, Currency::USD);

    // 200 bps spread => 0.02 rate when index is 0
    let spec = make_float_spec(FloatingRateFallback::SpreadOnly, dec!(200.0));

    let mut b = CashFlowSchedule::builder();
    let _ = b.principal(init, issue, maturity).floating_cf(spec);

    let schedule = b
        .build_with_curves(None)
        .expect("SpreadOnly fallback should succeed without a curve");

    // Find all FloatReset flows
    let float_flows: Vec<_> = schedule
        .flows
        .iter()
        .filter(|cf| cf.kind == CFKind::FloatReset)
        .collect();

    assert!(
        !float_flows.is_empty(),
        "Should have at least one FloatReset flow"
    );

    // With gearing=1 and index=0, rate should equal spread = 200bp = 0.02
    for cf in &float_flows {
        let rate = cf.rate.expect("FloatReset should have a rate");
        assert!(
            (rate - 0.02).abs() < 1e-10,
            "Rate should be 0.02 (spread-only), got {}",
            rate
        );
        assert_eq!(
            cf.accrual.and_then(|accrual| accrual.projected_index_rate),
            Some(0.0)
        );
    }
}

// =============================================================================
// Test 3: FloatingRateFallback::FixedRate(0.045) + no curve => 0.045 + spread
// =============================================================================

#[test]
fn test_floating_rate_fallback_fixed_rate() {
    let issue = Date::from_calendar_date(2025, Month::January, 15).unwrap();
    let maturity = Date::from_calendar_date(2026, Month::January, 15).unwrap();
    let init = Money::new(1_000_000.0, Currency::USD);

    // 200 bps spread + fixed index of 4.5%
    // Expected all-in rate: (0.045 + 0.02) * 1.0 = 0.065
    let spec = make_float_spec(FloatingRateFallback::FixedRate(dec!(0.045)), dec!(200.0));

    let mut b = CashFlowSchedule::builder();
    let _ = b.principal(init, issue, maturity).floating_cf(spec);

    let schedule = b
        .build_with_curves(None)
        .expect("FixedRate fallback should succeed without a curve");

    // Find all FloatReset flows
    let float_flows: Vec<_> = schedule
        .flows
        .iter()
        .filter(|cf| cf.kind == CFKind::FloatReset)
        .collect();

    assert!(
        !float_flows.is_empty(),
        "Should have at least one FloatReset flow"
    );

    // With gearing=1, index=0.045, spread=200bp=0.02
    // rate = (0.045 + 0.02) * 1.0 = 0.065
    for cf in &float_flows {
        let rate = cf.rate.expect("FloatReset should have a rate");
        assert!(
            (rate - 0.065).abs() < 1e-10,
            "Rate should be 0.065 (fixed index 4.5% + 200bp spread), got {}",
            rate
        );
        assert_eq!(
            cf.accrual.and_then(|accrual| accrual.projected_index_rate),
            Some(0.045)
        );
    }
}

// =============================================================================
// Test 4: FixedRate fallback respects floor/cap
// =============================================================================

#[test]
fn test_floating_rate_fallback_fixed_rate_with_floor_cap() {
    let issue = Date::from_calendar_date(2025, Month::January, 15).unwrap();
    let maturity = Date::from_calendar_date(2026, Month::January, 15).unwrap();
    let init = Money::new(1_000_000.0, Currency::USD);

    // Fixed index of 4.5% + 200bp spread = 6.5%, but cap at 5%
    let mut spec = make_float_spec(FloatingRateFallback::FixedRate(dec!(0.045)), dec!(200.0));
    spec.rate_spec.all_in_cap_bp = Some(dec!(500.0)); // all-in cap at 5%

    let mut b = CashFlowSchedule::builder();
    let _ = b.principal(init, issue, maturity).floating_cf(spec);

    let schedule = b
        .build_with_curves(None)
        .expect("FixedRate fallback with cap should succeed");

    let float_flows: Vec<_> = schedule
        .flows
        .iter()
        .filter(|cf| cf.kind == CFKind::FloatReset)
        .collect();

    assert!(
        !float_flows.is_empty(),
        "Should have at least one FloatReset flow"
    );

    // Uncapped rate would be 0.065, but all-in cap = 5% = 0.05
    for cf in &float_flows {
        let rate = cf.rate.expect("FloatReset should have a rate");
        assert!(
            (rate - 0.05).abs() < 1e-10,
            "Rate should be capped at 0.05 (all-in cap 500bp), got {}",
            rate
        );
    }
}

// =============================================================================
// Test 5: Default fallback (Error) still works when curve IS present
// =============================================================================

#[test]
fn test_floating_rate_default_fallback_with_curve() {
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::term_structures::ForwardCurve;

    let issue = Date::from_calendar_date(2025, Month::January, 15).unwrap();
    let maturity = Date::from_calendar_date(2026, Month::January, 15).unwrap();
    let init = Money::new(1_000_000.0, Currency::USD);

    // Default fallback (Error) but with a curve present => should succeed
    let spec = make_float_spec(FloatingRateFallback::Error, dec!(200.0));

    let fwd = ForwardCurve::builder("USD-SOFR-3M", 0.25)
        .base_date(issue)
        .day_count(DayCount::Act360)
        .knots([(0.0, 0.03), (1.0, 0.035), (5.0, 0.04)])
        .build()
        .expect("ForwardCurve builder should succeed");
    let market = MarketContext::new().insert(fwd);

    let mut b = CashFlowSchedule::builder();
    let _ = b.principal(init, issue, maturity).floating_cf(spec);

    let schedule = b
        .build_with_curves(Some(&market))
        .expect("Error fallback should succeed when curve is present");

    let float_flows: Vec<_> = schedule
        .flows
        .iter()
        .filter(|cf| cf.kind == CFKind::FloatReset)
        .collect();

    assert!(
        !float_flows.is_empty(),
        "Should have FloatReset flows when curve is present"
    );

    // Rate should be ~3% index + 2% spread = ~5%
    for cf in &float_flows {
        let rate = cf.rate.expect("FloatReset should have a rate");
        let index_rate = cf
            .accrual
            .and_then(|accrual| accrual.projected_index_rate)
            .expect("projected index rate should be stored on the flow");
        assert!(
            rate > 0.04 && rate < 0.06,
            "Rate should be ~5% (index + spread), got {}",
            rate
        );
        assert!((rate - index_rate - 0.02).abs() < 1e-10);
    }
}

// =============================================================================
// Test 6: PIK flows carry rate and accrual_factor from parent coupon
// =============================================================================

/// PIK flows should carry rate and accrual_factor from the parent coupon.
#[test]
fn test_pik_flow_metadata() {
    // Build a 100% PIK floating rate bond with SpreadOnly fallback (no curve needed).
    // With CouponType::PIK, the full coupon goes to PIK flows.
    let issue = Date::from_calendar_date(2025, Month::January, 15).unwrap();
    let maturity = Date::from_calendar_date(2026, Month::January, 15).unwrap();
    let init = Money::new(1_000_000.0, Currency::USD);

    // 200 bps spread => 0.02 rate when index is 0 (SpreadOnly)
    let mut spec = make_float_spec(FloatingRateFallback::SpreadOnly, dec!(200.0));
    spec.coupon_type = CouponType::PIK;

    let mut b = CashFlowSchedule::builder();
    let _ = b.principal(init, issue, maturity).floating_cf(spec);

    let schedule = b
        .build_with_curves(None)
        .expect("PIK with SpreadOnly fallback should succeed without a curve");

    // Find all PIK flows
    let pik_flows: Vec<_> = schedule
        .flows
        .iter()
        .filter(|cf| cf.kind == CFKind::PIK)
        .collect();

    assert!(
        !pik_flows.is_empty(),
        "Should have at least one PIK flow for a 100% PIK coupon"
    );

    // Verify that all PIK flows carry rate and accrual_factor from parent coupon
    for cf in &pik_flows {
        let rate = cf
            .rate
            .expect("PIK flow should carry rate from parent coupon");
        assert!(
            (rate - 0.02).abs() < 1e-10,
            "PIK flow rate should be 0.02 (spread-only), got {}",
            rate
        );
        assert!(
            cf.accrual_factor > 0.0,
            "PIK flow accrual_factor should be > 0.0, got {}",
            cf.accrual_factor
        );
        assert_eq!(
            cf.accrual.and_then(|accrual| accrual.projected_index_rate),
            Some(0.0)
        );
    }

    // Also verify there are no FloatReset flows (100% PIK means no cash coupons)
    let float_flows: Vec<_> = schedule
        .flows
        .iter()
        .filter(|cf| cf.kind == CFKind::FloatReset)
        .collect();
    assert!(
        float_flows.is_empty(),
        "100% PIK coupon should have no FloatReset (cash) flows"
    );
}

// =============================================================================
// Golden Value Tests
// =============================================================================

const RATE_TOLERANCE: f64 = 1e-10;

/// Helper: create a flat forward curve at `flat_rate` for "USD-SOFR-3M" with
/// base date `base` and Act/360.
fn make_flat_forward_market(
    base: Date,
    flat_rate: f64,
) -> finstack_quant_core::market_data::context::MarketContext {
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::term_structures::ForwardCurve;

    let fwd = ForwardCurve::builder("USD-SOFR-3M", 0.25)
        .base_date(base)
        .day_count(DayCount::Act360)
        .knots([(0.0, flat_rate), (5.0, flat_rate)])
        .build()
        .expect("Flat ForwardCurve builder should succeed");
    MarketContext::new().insert(fwd)
}

// =============================================================================
// Golden Value Test 1: SOFR + 200bp flat curve
// =============================================================================

/// Golden value: SOFR + 200bp, quarterly, Act/360, $1M notional.
/// Flat forward curve at 4.5%.
/// All-in rate = 4.5% + 2.0% = 6.5%
/// Each quarterly coupon ~ $1M x 0.065 x (days/360)
#[test]
fn test_floating_rate_golden_sofr_200bp() {
    let issue = Date::from_calendar_date(2024, Month::January, 15).unwrap();
    let maturity = Date::from_calendar_date(2025, Month::January, 15).unwrap();
    let notional = 1_000_000.0;
    let init = Money::new(notional, Currency::USD);

    let spec = make_float_spec(FloatingRateFallback::Error, dec!(200.0));
    let market = make_flat_forward_market(issue, 0.045);

    let mut b = CashFlowSchedule::builder();
    let _ = b.principal(init, issue, maturity).floating_cf(spec);

    let schedule = b
        .build_with_curves(Some(&market))
        .expect("Golden SOFR+200bp build should succeed with flat curve");

    let float_flows: Vec<_> = schedule
        .flows
        .iter()
        .filter(|cf| cf.kind == CFKind::FloatReset)
        .collect();

    // Expect 4 quarterly FloatReset flows for a 1-year bond
    assert_eq!(
        float_flows.len(),
        4,
        "Expected 4 quarterly FloatReset flows, got {}",
        float_flows.len()
    );

    let expected_rate = 0.065; // 4.5% index + 2.0% spread

    for (i, cf) in float_flows.iter().enumerate() {
        let rate = cf.rate.expect("FloatReset should have a rate");
        assert!(
            (rate - expected_rate).abs() < RATE_TOLERANCE,
            "Flow {}: rate should be {} (SOFR 4.5% + 200bp), got {}",
            i,
            expected_rate,
            rate
        );

        // Coupon amount = notional * rate * accrual_factor
        // For quarterly Act/360, accrual_factor ~ days_in_period / 360
        // Each quarter has ~90-92 days, so accrual ~ 0.25
        // Expected coupon ~ 1_000_000 * 0.065 * 0.25 ~ $16,250
        let amount = cf.amount.amount().abs();
        assert!(
            amount > 15_000.0 && amount < 18_500.0,
            "Flow {}: coupon amount should be ~$16,250 (within bounds), got {:.2}",
            i,
            amount
        );

        // Verify the amount is consistent with rate * notional * accrual_factor
        let expected_amount = notional * expected_rate * cf.accrual_factor;
        assert!(
            (amount - expected_amount).abs() < 1.0,
            "Flow {}: amount {:.2} should match rate * notional * accrual ({:.2})",
            i,
            amount,
            expected_amount
        );
    }
}

// =============================================================================
// Golden Value Test 2: Zero spread (index only)
// =============================================================================

/// Golden value: SOFR + 0bp. Rate should equal index rate (4.5%).
#[test]
fn test_floating_rate_golden_zero_spread() {
    let issue = Date::from_calendar_date(2024, Month::January, 15).unwrap();
    let maturity = Date::from_calendar_date(2025, Month::January, 15).unwrap();
    let notional = 1_000_000.0;
    let init = Money::new(notional, Currency::USD);

    // Zero spread
    let spec = make_float_spec(FloatingRateFallback::Error, dec!(0.0));
    let market = make_flat_forward_market(issue, 0.045);

    let mut b = CashFlowSchedule::builder();
    let _ = b.principal(init, issue, maturity).floating_cf(spec);

    let schedule = b
        .build_with_curves(Some(&market))
        .expect("Golden zero-spread build should succeed with flat curve");

    let float_flows: Vec<_> = schedule
        .flows
        .iter()
        .filter(|cf| cf.kind == CFKind::FloatReset)
        .collect();

    assert_eq!(
        float_flows.len(),
        4,
        "Expected 4 quarterly FloatReset flows, got {}",
        float_flows.len()
    );

    let expected_rate = 0.045; // Index rate only, no spread

    for (i, cf) in float_flows.iter().enumerate() {
        let rate = cf.rate.expect("FloatReset should have a rate");
        assert!(
            (rate - expected_rate).abs() < RATE_TOLERANCE,
            "Flow {}: rate should be exactly {} (index only, zero spread), got {}",
            i,
            expected_rate,
            rate
        );

        // Coupon amount = notional * 0.045 * accrual ~ $11,250 per quarter
        let amount = cf.amount.amount().abs();
        let expected_amount = notional * expected_rate * cf.accrual_factor;
        assert!(
            (amount - expected_amount).abs() < 1.0,
            "Flow {}: amount {:.2} should match rate * notional * accrual ({:.2})",
            i,
            amount,
            expected_amount
        );
    }
}

// =============================================================================
// Golden Value Test 3: Gearing (gearing_includes_spread = true)
// =============================================================================

/// Golden value: gearing=1.5 on 4.5% SOFR + 200bp.
/// With gearing_includes_spread=true: rate = 1.5 * (4.5% + 2.0%) = 9.75%
#[test]
fn test_floating_rate_golden_gearing_includes_spread() {
    let issue = Date::from_calendar_date(2024, Month::January, 15).unwrap();
    let maturity = Date::from_calendar_date(2025, Month::January, 15).unwrap();
    let notional = 1_000_000.0;
    let init = Money::new(notional, Currency::USD);

    let mut spec = make_float_spec(FloatingRateFallback::Error, dec!(200.0));
    spec.rate_spec.gearing = dec!(1.5);
    spec.rate_spec.gearing_includes_spread = true;

    let market = make_flat_forward_market(issue, 0.045);

    let mut b = CashFlowSchedule::builder();
    let _ = b.principal(init, issue, maturity).floating_cf(spec);

    let schedule = b
        .build_with_curves(Some(&market))
        .expect("Golden gearing (includes spread) build should succeed");

    let float_flows: Vec<_> = schedule
        .flows
        .iter()
        .filter(|cf| cf.kind == CFKind::FloatReset)
        .collect();

    assert_eq!(
        float_flows.len(),
        4,
        "Expected 4 quarterly FloatReset flows, got {}",
        float_flows.len()
    );

    // gearing_includes_spread=true: (index + spread) * gearing
    // = (0.045 + 0.02) * 1.5 = 0.065 * 1.5 = 0.0975
    let expected_rate = 0.0975;

    for (i, cf) in float_flows.iter().enumerate() {
        let rate = cf.rate.expect("FloatReset should have a rate");
        assert!(
            (rate - expected_rate).abs() < RATE_TOLERANCE,
            "Flow {}: rate should be {} (1.5 * (4.5% + 2%)), got {}",
            i,
            expected_rate,
            rate
        );

        let amount = cf.amount.amount().abs();
        let expected_amount = notional * expected_rate * cf.accrual_factor;
        assert!(
            (amount - expected_amount).abs() < 1.0,
            "Flow {}: amount {:.2} should match rate * notional * accrual ({:.2})",
            i,
            amount,
            expected_amount
        );
    }
}

// =============================================================================
// Golden Value Test 3b: Gearing (gearing_includes_spread = false, affine)
// =============================================================================

/// Golden value: gearing=1.5 on 4.5% SOFR + 200bp.
/// With gearing_includes_spread=false: rate = (1.5 * 4.5%) + 2.0% = 6.75% + 2.0% = 8.75%
#[test]
fn test_floating_rate_golden_gearing_excludes_spread() {
    let issue = Date::from_calendar_date(2024, Month::January, 15).unwrap();
    let maturity = Date::from_calendar_date(2025, Month::January, 15).unwrap();
    let notional = 1_000_000.0;
    let init = Money::new(notional, Currency::USD);

    let mut spec = make_float_spec(FloatingRateFallback::Error, dec!(200.0));
    spec.rate_spec.gearing = dec!(1.5);
    spec.rate_spec.gearing_includes_spread = false;

    let market = make_flat_forward_market(issue, 0.045);

    let mut b = CashFlowSchedule::builder();
    let _ = b.principal(init, issue, maturity).floating_cf(spec);

    let schedule = b
        .build_with_curves(Some(&market))
        .expect("Golden gearing (excludes spread) build should succeed");

    let float_flows: Vec<_> = schedule
        .flows
        .iter()
        .filter(|cf| cf.kind == CFKind::FloatReset)
        .collect();

    assert_eq!(
        float_flows.len(),
        4,
        "Expected 4 quarterly FloatReset flows, got {}",
        float_flows.len()
    );

    // gearing_includes_spread=false (affine): (index * gearing) + spread
    // = (0.045 * 1.5) + 0.02 = 0.0675 + 0.02 = 0.0875
    let expected_rate = 0.0875;

    for (i, cf) in float_flows.iter().enumerate() {
        let rate = cf.rate.expect("FloatReset should have a rate");
        assert!(
            (rate - expected_rate).abs() < RATE_TOLERANCE,
            "Flow {}: rate should be {} (1.5 * 4.5% + 2%), got {}",
            i,
            expected_rate,
            rate
        );

        let amount = cf.amount.amount().abs();
        let expected_amount = notional * expected_rate * cf.accrual_factor;
        assert!(
            (amount - expected_amount).abs() < 1.0,
            "Flow {}: amount {:.2} should match rate * notional * accrual ({:.2})",
            i,
            amount,
            expected_amount
        );
    }

    // Additionally verify the difference between the two gearing modes:
    // Standard (includes spread): 0.0975
    // Affine (excludes spread): 0.0875
    // Difference = spread * (gearing - 1) = 0.02 * 0.5 = 0.01
    let standard_rate = 0.0975;
    let affine_rate = expected_rate;
    let expected_diff = 0.02 * (1.5 - 1.0); // spread * (gearing - 1) = 0.01
    assert!(
        ((standard_rate - affine_rate) - expected_diff).abs() < RATE_TOLERANCE,
        "Difference between standard and affine should be spread*(gearing-1) = {}, got {}",
        expected_diff,
        standard_rate - affine_rate
    );
}

// =============================================================================
// Cap/Floor and Negative Rate Tests
// =============================================================================

/// Index floor at 0%: negative index rates are clamped to 0.
/// Flat curve at -0.4% with floor at 0 -> all-in rate = 0% + spread.
#[test]
fn test_floating_rate_index_floor_zero() {
    let issue = Date::from_calendar_date(2024, Month::January, 15).unwrap();
    let maturity = Date::from_calendar_date(2025, Month::January, 15).unwrap();
    let notional = 1_000_000.0;
    let init = Money::new(notional, Currency::USD);

    // Build FloatingRateSpec with index floor at 0% and flat curve at -0.4%
    let mut spec = make_float_spec(FloatingRateFallback::Error, dec!(300.0)); // 3% spread
    spec.rate_spec.index_floor_bp = Some(dec!(0)); // index floored at 0%

    let market = make_flat_forward_market(issue, -0.004); // -0.4% flat curve

    let mut b = CashFlowSchedule::builder();
    let _ = b.principal(init, issue, maturity).floating_cf(spec);

    let schedule = b
        .build_with_curves(Some(&market))
        .expect("Index floor test should succeed with flat curve");

    let float_flows: Vec<_> = schedule
        .flows
        .iter()
        .filter(|cf| cf.kind == CFKind::FloatReset)
        .collect();

    assert!(
        !float_flows.is_empty(),
        "Should have at least one FloatReset flow"
    );

    // Index = -0.4%, floored at 0% -> eff_index = 0%
    // all-in = (0% + 3%) * 1.0 = 3% = 0.03
    let expected_rate = 0.03;
    for (i, cf) in float_flows.iter().enumerate() {
        let rate = cf.rate.expect("FloatReset should have a rate");
        assert!(
            (rate - expected_rate).abs() < RATE_TOLERANCE,
            "Flow {}: rate should be {} (index floored at 0% + 300bp spread), got {}",
            i,
            expected_rate,
            rate
        );
    }
}

/// Index cap at 5%: index rate clamped to cap.
/// Flat curve at 6% with index_cap at 5% -> all-in = 5% + spread.
#[test]
fn test_floating_rate_index_cap() {
    let issue = Date::from_calendar_date(2024, Month::January, 15).unwrap();
    let maturity = Date::from_calendar_date(2025, Month::January, 15).unwrap();
    let notional = 1_000_000.0;
    let init = Money::new(notional, Currency::USD);

    // Build FloatingRateSpec with index cap at 5% and flat curve at 6%
    let mut spec = make_float_spec(FloatingRateFallback::Error, dec!(200.0)); // 2% spread
    spec.rate_spec.index_cap_bp = Some(dec!(500)); // 5% cap on index

    let market = make_flat_forward_market(issue, 0.06); // 6% flat curve

    let mut b = CashFlowSchedule::builder();
    let _ = b.principal(init, issue, maturity).floating_cf(spec);

    let schedule = b
        .build_with_curves(Some(&market))
        .expect("Index cap test should succeed with flat curve");

    let float_flows: Vec<_> = schedule
        .flows
        .iter()
        .filter(|cf| cf.kind == CFKind::FloatReset)
        .collect();

    assert!(
        !float_flows.is_empty(),
        "Should have at least one FloatReset flow"
    );

    // Index = 6%, capped at 5% -> eff_index = 5%
    // all-in = (5% + 2%) * 1.0 = 7% = 0.07
    let expected_rate = 0.07;
    for (i, cf) in float_flows.iter().enumerate() {
        let rate = cf.rate.expect("FloatReset should have a rate");
        assert!(
            (rate - expected_rate).abs() < RATE_TOLERANCE,
            "Flow {}: rate should be {} (index capped at 5% + 200bp spread), got {}",
            i,
            expected_rate,
            rate
        );
    }
}

/// All-in cap at 7%: total rate clamped after adding spread.
/// Flat curve at 6%, spread 200bp, cap at 7% -> uncapped = 8%, capped = 7%.
#[test]
fn test_floating_rate_all_in_cap() {
    let issue = Date::from_calendar_date(2024, Month::January, 15).unwrap();
    let maturity = Date::from_calendar_date(2025, Month::January, 15).unwrap();
    let notional = 1_000_000.0;
    let init = Money::new(notional, Currency::USD);

    // Build FloatingRateSpec with all-in cap at 7%
    let mut spec = make_float_spec(FloatingRateFallback::Error, dec!(200.0)); // 2% spread
    spec.rate_spec.all_in_cap_bp = Some(dec!(700)); // 7% all-in cap

    let market = make_flat_forward_market(issue, 0.06); // 6% flat curve

    let mut b = CashFlowSchedule::builder();
    let _ = b.principal(init, issue, maturity).floating_cf(spec);

    let schedule = b
        .build_with_curves(Some(&market))
        .expect("All-in cap test should succeed with flat curve");

    let float_flows: Vec<_> = schedule
        .flows
        .iter()
        .filter(|cf| cf.kind == CFKind::FloatReset)
        .collect();

    assert!(
        !float_flows.is_empty(),
        "Should have at least one FloatReset flow"
    );

    // Uncapped = 6% + 2% = 8%, but all-in cap at 7% -> rate = 0.07
    let expected_rate = 0.07;
    for (i, cf) in float_flows.iter().enumerate() {
        let rate = cf.rate.expect("FloatReset should have a rate");
        assert!(
            (rate - expected_rate).abs() < RATE_TOLERANCE,
            "Flow {}: rate should be {} (all-in capped at 7%), got {}",
            i,
            expected_rate,
            rate
        );
    }
}

/// Negative rate: EUR EURIBOR at -0.40% + 300bp spread, no floor.
/// All-in rate = -0.004 + 0.03 = 0.026.
#[test]
fn test_floating_rate_negative_index_no_floor() {
    let issue = Date::from_calendar_date(2024, Month::January, 15).unwrap();
    let maturity = Date::from_calendar_date(2025, Month::January, 15).unwrap();
    let notional = 1_000_000.0;
    let init = Money::new(notional, Currency::USD);

    // No floor, negative index rate
    let spec = make_float_spec(FloatingRateFallback::Error, dec!(300.0)); // 3% spread, no floor
    let market = make_flat_forward_market(issue, -0.004); // -0.4% flat curve

    let mut b = CashFlowSchedule::builder();
    let _ = b.principal(init, issue, maturity).floating_cf(spec);

    let schedule = b
        .build_with_curves(Some(&market))
        .expect("Negative index no-floor test should succeed with flat curve");

    let float_flows: Vec<_> = schedule
        .flows
        .iter()
        .filter(|cf| cf.kind == CFKind::FloatReset)
        .collect();

    assert!(
        !float_flows.is_empty(),
        "Should have at least one FloatReset flow"
    );

    // No floor: index = -0.4%, spread = 3%
    // all-in = (-0.004 + 0.03) * 1.0 = 0.026
    let expected_rate = 0.026;
    for (i, cf) in float_flows.iter().enumerate() {
        let rate = cf.rate.expect("FloatReset should have a rate");
        assert!(
            (rate - expected_rate).abs() < RATE_TOLERANCE,
            "Flow {}: rate should be {} (negative index -0.4% + 300bp spread, no floor), got {}",
            i,
            expected_rate,
            rate
        );
    }
}

/// All-in floor at 1%: total rate floored after adding spread.
/// Flat curve at -2%, spread 200bp -> uncapped = -2% + 2% = 0%, but all-in floor at 1% -> rate = 1%.
#[test]
fn test_floating_rate_all_in_floor() {
    let issue = Date::from_calendar_date(2024, Month::January, 15).unwrap();
    let maturity = Date::from_calendar_date(2025, Month::January, 15).unwrap();
    let notional = 1_000_000.0;
    let init = Money::new(notional, Currency::USD);

    // Build FloatingRateSpec with all-in floor at 1%
    let mut spec = make_float_spec(FloatingRateFallback::Error, dec!(200.0)); // 2% spread
    spec.rate_spec.all_in_floor_bp = Some(dec!(100)); // 1% all-in floor

    let market = make_flat_forward_market(issue, -0.02); // -2% flat curve

    let mut b = CashFlowSchedule::builder();
    let _ = b.principal(init, issue, maturity).floating_cf(spec);

    let schedule = b
        .build_with_curves(Some(&market))
        .expect("All-in floor test should succeed with flat curve");

    let float_flows: Vec<_> = schedule
        .flows
        .iter()
        .filter(|cf| cf.kind == CFKind::FloatReset)
        .collect();

    assert!(
        !float_flows.is_empty(),
        "Should have at least one FloatReset flow"
    );

    // Unfloored = -2% + 2% = 0%, but all-in floor at 1% -> rate = 0.01
    let expected_rate = 0.01;
    for (i, cf) in float_flows.iter().enumerate() {
        let rate = cf.rate.expect("FloatReset should have a rate");
        assert!(
            (rate - expected_rate).abs() < RATE_TOLERANCE,
            "Flow {}: rate should be {} (all-in floored at 1%), got {}",
            i,
            expected_rate,
            rate
        );
    }
}

// =============================================================================
// Overnight Compounding Tests
// =============================================================================

use finstack_quant_cashflows::builder::specs::OvernightCompoundingMethod;

/// Helper: create a floating coupon spec with overnight compounding enabled.
fn make_overnight_float_spec(
    method: OvernightCompoundingMethod,
    fallback: FloatingRateFallback,
    spread_bp: Decimal,
) -> FloatingCouponSpec {
    FloatingCouponSpec {
        rate_spec: FloatingRateSpec {
            index_id: "USD-SOFR-3M".into(),
            spread_bp,
            gearing: Decimal::ONE,
            gearing_includes_spread: true,
            index_floor_bp: None,
            all_in_cap_bp: None,
            all_in_floor_bp: None,
            index_cap_bp: None,
            overnight_index_constraints: OvernightIndexConstraintApplication::Daily,
            reset_freq: Tenor::quarterly(),
            index_tenor: None,
            reset_lag_days: 0,
            fixing_calendar_id: None,
            overnight_compounding: Some(method),
            overnight_basis: None,
            fallback,
        },
        coupon_type: CouponType::Cash,
        schedule: finstack_quant_cashflows::builder::ScheduleParams {
            freq: Tenor::quarterly(),
            dc: DayCount::Act360,
            bdc: BusinessDayConvention::Following,
            calendar_id: "weekends_only".to_string(),
            stub: StubKind::None,
            end_of_month: false,
            payment_lag_days: 0,
            adjust_accrual_dates: false,
        },
    }
}

/// Overnight compounding (CompoundedInArrears) with a flat curve should produce
/// approximately the same rate as the flat forward rate.
///
/// With a flat curve at 4.5%, daily compounding produces a rate very close to 4.5%
/// (the compounding effect on such small daily increments is negligible).
/// All-in rate = ~4.5% + 2% spread = ~6.5%.
#[test]
fn test_overnight_compounding_flat_curve() {
    let issue = Date::from_calendar_date(2024, Month::January, 15).unwrap();
    let maturity = Date::from_calendar_date(2025, Month::January, 15).unwrap();
    let notional = 1_000_000.0;
    let init = Money::new(notional, Currency::USD);

    let spec = make_overnight_float_spec(
        OvernightCompoundingMethod::CompoundedInArrears,
        FloatingRateFallback::Error,
        dec!(200.0), // 200 bps spread
    );
    let market = make_flat_forward_market(issue, 0.045);

    let mut b = CashFlowSchedule::builder();
    let _ = b.principal(init, issue, maturity).floating_cf(spec);

    let schedule = b
        .build_with_curves(Some(&market))
        .expect("Overnight compounding with flat curve should succeed");

    let float_flows: Vec<_> = schedule
        .flows
        .iter()
        .filter(|cf| cf.kind == CFKind::FloatReset)
        .collect();

    assert_eq!(
        float_flows.len(),
        4,
        "Expected 4 quarterly FloatReset flows, got {}",
        float_flows.len()
    );

    // Flat curve: compounded rate is very close to the flat rate (4.5%).
    // All-in = ~4.5% + 2% = ~6.5%.
    // Allow a small tolerance for compounding effects on flat curve.
    let expected_rate = 0.065;
    for (i, cf) in float_flows.iter().enumerate() {
        let rate = cf.rate.expect("FloatReset should have a rate");
        assert!(
            (rate - expected_rate).abs() < 0.001,
            "Flow {}: overnight compounded rate should be ~{} (flat 4.5% + 200bp), got {}",
            i,
            expected_rate,
            rate
        );

        // Verify the amount is consistent with rate * notional * accrual_factor
        let amount = cf.amount.amount().abs();
        let expected_amount = notional * rate * cf.accrual_factor;
        assert!(
            (amount - expected_amount).abs() < 1.0,
            "Flow {}: amount {:.2} should match rate * notional * accrual ({:.2})",
            i,
            amount,
            expected_amount
        );
    }
}

/// Simple average should produce an identical rate to the flat forward rate.
///
/// With a flat curve at 4.5%, the simple average of daily rates is exactly 4.5%.
/// All-in rate = 4.5% + 2% spread = 6.5%.
#[test]
fn test_overnight_simple_average_flat() {
    let issue = Date::from_calendar_date(2024, Month::January, 15).unwrap();
    let maturity = Date::from_calendar_date(2025, Month::January, 15).unwrap();
    let notional = 1_000_000.0;
    let init = Money::new(notional, Currency::USD);

    let spec = make_overnight_float_spec(
        OvernightCompoundingMethod::SimpleAverage,
        FloatingRateFallback::Error,
        dec!(200.0), // 200 bps spread
    );
    let market = make_flat_forward_market(issue, 0.045);

    let mut b = CashFlowSchedule::builder();
    let _ = b.principal(init, issue, maturity).floating_cf(spec);

    let schedule = b
        .build_with_curves(Some(&market))
        .expect("Overnight simple average with flat curve should succeed");

    let float_flows: Vec<_> = schedule
        .flows
        .iter()
        .filter(|cf| cf.kind == CFKind::FloatReset)
        .collect();

    assert_eq!(
        float_flows.len(),
        4,
        "Expected 4 quarterly FloatReset flows, got {}",
        float_flows.len()
    );

    // Simple average of a flat rate == that flat rate.
    // All-in = 4.5% + 2% = 6.5%.
    let expected_rate = 0.065;
    for (i, cf) in float_flows.iter().enumerate() {
        let rate = cf.rate.expect("FloatReset should have a rate");
        assert!(
            (rate - expected_rate).abs() < 0.001,
            "Flow {}: simple average rate should be ~{} (flat 4.5% + 200bp), got {}",
            i,
            expected_rate,
            rate
        );
    }
}

/// Overnight compounding with lockout on a flat curve should produce
/// approximately the same rate as the flat forward rate.
#[test]
fn test_overnight_lockout_flat_curve() {
    let issue = Date::from_calendar_date(2024, Month::January, 15).unwrap();
    let maturity = Date::from_calendar_date(2025, Month::January, 15).unwrap();
    let notional = 1_000_000.0;
    let init = Money::new(notional, Currency::USD);

    let spec = make_overnight_float_spec(
        OvernightCompoundingMethod::CompoundedWithLockout { lockout_days: 2 },
        FloatingRateFallback::Error,
        dec!(200.0),
    );
    let market = make_flat_forward_market(issue, 0.045);

    let mut b = CashFlowSchedule::builder();
    let _ = b.principal(init, issue, maturity).floating_cf(spec);

    let schedule = b
        .build_with_curves(Some(&market))
        .expect("Overnight lockout with flat curve should succeed");

    let float_flows: Vec<_> = schedule
        .flows
        .iter()
        .filter(|cf| cf.kind == CFKind::FloatReset)
        .collect();

    assert_eq!(
        float_flows.len(),
        4,
        "Expected 4 quarterly FloatReset flows, got {}",
        float_flows.len()
    );

    // Lockout on a flat curve has no effect — rate is still ~4.5% + 2% = 6.5%.
    let expected_rate = 0.065;
    for (i, cf) in float_flows.iter().enumerate() {
        let rate = cf.rate.expect("FloatReset should have a rate");
        assert!(
            (rate - expected_rate).abs() < 0.001,
            "Flow {}: lockout rate should be ~{} (flat 4.5% + 200bp), got {}",
            i,
            expected_rate,
            rate
        );
    }
}

/// ISDA 2021 observation-shift must move the observation window *before*
/// the accrual period, so rates sampled under a SONIA-style 5 BD shift
/// must differ from in-arrears rates on any non-flat forward curve.
///
/// Before the fix: `sample_overnight_rates` only ever sampled the accrual
/// window and the shift was applied post-hoc as index rewriting, which on a
/// rising curve produced identical rates to the accrual-window sample for the
/// earliest `shift_days` observations (falling back to `daily_rates[0]`).
///
/// After the fix: on a steeply-rising forward curve `shifted < arrears` for
/// every coupon because the observations genuinely come from `shift_days`
/// business days earlier in time.
#[test]
fn test_overnight_observation_shift_samples_pre_accrual_window() {
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::term_structures::ForwardCurve;

    // Base the forward curve well before issue so both the accrual and the
    // shifted observation windows fall strictly after base_date (otherwise the
    // sampler clamps t=0 and the two windows collapse to the same rate).
    let base = Date::from_calendar_date(2023, Month::January, 15).unwrap();
    let issue = Date::from_calendar_date(2024, Month::January, 15).unwrap();
    let maturity = Date::from_calendar_date(2024, Month::July, 15).unwrap();
    let notional = 1_000_000.0;
    let init = Money::new(notional, Currency::USD);

    // Steeply rising curve: 3% at base, 8% at 1Y. Any calendar-day shift
    // earlier yields a strictly lower sampled rate.
    let fwd = ForwardCurve::builder("USD-SOFR-3M", 0.25)
        .base_date(base)
        .day_count(DayCount::Act360)
        .knots([(0.0, 0.03), (2.0, 0.13)])
        .build()
        .expect("rising forward curve builds");
    let market = MarketContext::new().insert(fwd);

    // Zero spread so the emitted rate is exactly the compounded index rate.
    let arrears_spec = make_overnight_float_spec(
        OvernightCompoundingMethod::CompoundedInArrears,
        FloatingRateFallback::Error,
        dec!(0.0),
    );
    let shifted_spec = make_overnight_float_spec(
        OvernightCompoundingMethod::CompoundedWithObservationShift { shift_days: 5 },
        FloatingRateFallback::Error,
        dec!(0.0),
    );

    let arrears_schedule = {
        let mut b = CashFlowSchedule::builder();
        let _ = b.principal(init, issue, maturity).floating_cf(arrears_spec);
        b.build_with_curves(Some(&market))
            .expect("in-arrears schedule builds")
    };
    let shifted_schedule = {
        let mut b = CashFlowSchedule::builder();
        let _ = b.principal(init, issue, maturity).floating_cf(shifted_spec);
        b.build_with_curves(Some(&market))
            .expect("observation-shifted schedule builds")
    };

    let arrears_rates: Vec<f64> = arrears_schedule
        .flows
        .iter()
        .filter(|cf| cf.kind == CFKind::FloatReset)
        .map(|cf| cf.rate.expect("FloatReset has a rate"))
        .collect();
    let shifted_rates: Vec<f64> = shifted_schedule
        .flows
        .iter()
        .filter(|cf| cf.kind == CFKind::FloatReset)
        .map(|cf| cf.rate.expect("FloatReset has a rate"))
        .collect();

    assert_eq!(
        arrears_rates.len(),
        shifted_rates.len(),
        "both schedules produce the same number of FloatReset flows"
    );
    assert!(
        !arrears_rates.is_empty(),
        "schedule emits at least one coupon"
    );

    // On a rising curve, every shifted coupon must be strictly below the
    // in-arrears coupon for the same period. A non-trivial gap (> 1 bp)
    // proves the observation window actually moved — not just a floating
    // point rounding difference.
    for (i, (arr, sh)) in arrears_rates
        .iter()
        .copied()
        .zip(shifted_rates.iter().copied())
        .enumerate()
    {
        assert!(
            sh < arr,
            "flow {i}: shifted rate {sh:.6} must be < in-arrears rate {arr:.6} on a rising curve"
        );
        assert!(
            (arr - sh) > 1e-4,
            "flow {i}: shift gap {gap:.6} should exceed 1 bp (arrears {arr:.6}, shifted {sh:.6})",
            gap = arr - sh
        );
    }
}

/// ARRC 2020 §2 "Lookback" must sample rates from N business days before
/// each accrual date — the rate observation is shifted but the
/// accrual-period weight is preserved.
///
/// Before the follow-up: `compute_overnight_rate::CompoundedWithLookback`
/// did index-rewriting inside the accrual-window sample. For the first
/// `lookback_days` business days it fell back to `daily_rates[0]` (the
/// first rate of the accrual window), effectively muting the first week of
/// the lookback shift and producing a biased coupon rate.
///
/// After the follow-up: `sample_overnight_rates_with_lookback` in
/// coupons.rs looks up each rate via `add_business_days(-lookback)`, so on
/// a rising forward curve every accrual-business-day observation is
/// strictly earlier in time than its in-arrears counterpart.
#[test]
fn test_overnight_lookback_samples_pre_accrual_rates() {
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::term_structures::ForwardCurve;

    let base = Date::from_calendar_date(2023, Month::January, 15).unwrap();
    let issue = Date::from_calendar_date(2024, Month::January, 15).unwrap();
    let maturity = Date::from_calendar_date(2024, Month::July, 15).unwrap();
    let notional = 1_000_000.0;
    let init = Money::new(notional, Currency::USD);

    let fwd = ForwardCurve::builder("USD-SOFR-3M", 0.25)
        .base_date(base)
        .day_count(DayCount::Act360)
        .knots([(0.0, 0.03), (2.0, 0.13)])
        .build()
        .expect("rising forward curve builds");
    let market = MarketContext::new().insert(fwd);

    let arrears_spec = make_overnight_float_spec(
        OvernightCompoundingMethod::CompoundedInArrears,
        FloatingRateFallback::Error,
        dec!(0.0),
    );
    let lookback_spec = make_overnight_float_spec(
        OvernightCompoundingMethod::CompoundedWithLookback { lookback_days: 5 },
        FloatingRateFallback::Error,
        dec!(0.0),
    );

    let arrears_schedule = {
        let mut b = CashFlowSchedule::builder();
        let _ = b.principal(init, issue, maturity).floating_cf(arrears_spec);
        b.build_with_curves(Some(&market))
            .expect("in-arrears schedule builds")
    };
    let lookback_schedule = {
        let mut b = CashFlowSchedule::builder();
        let _ = b
            .principal(init, issue, maturity)
            .floating_cf(lookback_spec);
        b.build_with_curves(Some(&market))
            .expect("lookback schedule builds")
    };

    let arrears_rates: Vec<f64> = arrears_schedule
        .flows
        .iter()
        .filter(|cf| cf.kind == CFKind::FloatReset)
        .map(|cf| cf.rate.expect("FloatReset has a rate"))
        .collect();
    let lookback_rates: Vec<f64> = lookback_schedule
        .flows
        .iter()
        .filter(|cf| cf.kind == CFKind::FloatReset)
        .map(|cf| cf.rate.expect("FloatReset has a rate"))
        .collect();

    assert_eq!(arrears_rates.len(), lookback_rates.len(), "same schedule");
    assert!(
        !arrears_rates.is_empty(),
        "schedule emits at least one coupon"
    );

    for (i, (arr, lb)) in arrears_rates
        .iter()
        .copied()
        .zip(lookback_rates.iter().copied())
        .enumerate()
    {
        assert!(
            lb < arr,
            "flow {i}: lookback rate {lb:.6} must be < in-arrears rate {arr:.6} on a rising curve"
        );
        assert!(
            (arr - lb) > 1e-4,
            "flow {i}: lookback gap {gap:.6} should exceed 1 bp (arrears {arr:.6}, lookback {lb:.6})",
            gap = arr - lb
        );
    }
}

/// Overnight compounding with no curve and Error fallback should fail.
#[test]
fn test_overnight_compounding_no_curve_error_fallback() {
    let issue = Date::from_calendar_date(2024, Month::January, 15).unwrap();
    let maturity = Date::from_calendar_date(2025, Month::January, 15).unwrap();
    let init = Money::new(1_000_000.0, Currency::USD);

    let spec = make_overnight_float_spec(
        OvernightCompoundingMethod::CompoundedInArrears,
        FloatingRateFallback::Error,
        dec!(200.0),
    );

    let mut b = CashFlowSchedule::builder();
    let _ = b.principal(init, issue, maturity).floating_cf(spec);

    let result = b.build_with_curves(None);
    assert!(
        result.is_err(),
        "Overnight compounding with no curve and Error fallback should fail"
    );
}

/// Overnight compounding with no curve and SpreadOnly fallback should succeed.
#[test]
fn test_overnight_compounding_no_curve_spread_only_fallback() {
    let issue = Date::from_calendar_date(2024, Month::January, 15).unwrap();
    let maturity = Date::from_calendar_date(2025, Month::January, 15).unwrap();
    let init = Money::new(1_000_000.0, Currency::USD);

    let spec = make_overnight_float_spec(
        OvernightCompoundingMethod::CompoundedInArrears,
        FloatingRateFallback::SpreadOnly,
        dec!(200.0),
    );

    let mut b = CashFlowSchedule::builder();
    let _ = b.principal(init, issue, maturity).floating_cf(spec);

    let schedule = b
        .build_with_curves(None)
        .expect("Overnight compounding with SpreadOnly fallback should succeed");

    let float_flows: Vec<_> = schedule
        .flows
        .iter()
        .filter(|cf| cf.kind == CFKind::FloatReset)
        .collect();

    assert!(!float_flows.is_empty(), "Should have FloatReset flows");

    // SpreadOnly: index=0, all-in = spread = 200bp = 0.02
    for cf in &float_flows {
        let rate = cf.rate.expect("FloatReset should have a rate");
        assert!(
            (rate - 0.02).abs() < 1e-10,
            "Rate should be 0.02 (spread-only), got {}",
            rate
        );
    }
}

/// Overnight compounding should produce the same result as the term rate path
/// when the curve is flat (verifying both paths converge for simple cases).
#[test]
fn test_overnight_vs_term_rate_flat_curve_equivalence() {
    let issue = Date::from_calendar_date(2024, Month::January, 15).unwrap();
    let maturity = Date::from_calendar_date(2025, Month::January, 15).unwrap();
    let notional = 1_000_000.0;
    let init = Money::new(notional, Currency::USD);
    let market = make_flat_forward_market(issue, 0.045);

    // Build with overnight compounding
    let overnight_spec = make_overnight_float_spec(
        OvernightCompoundingMethod::SimpleAverage,
        FloatingRateFallback::Error,
        dec!(200.0),
    );
    let mut b1 = CashFlowSchedule::builder();
    let _ = b1
        .principal(init, issue, maturity)
        .floating_cf(overnight_spec);
    let overnight_schedule = b1
        .build_with_curves(Some(&market))
        .expect("Overnight build should succeed");

    // Build with standard term rate
    let term_spec = make_float_spec(FloatingRateFallback::Error, dec!(200.0));
    let mut b2 = CashFlowSchedule::builder();
    let _ = b2.principal(init, issue, maturity).floating_cf(term_spec);
    let term_schedule = b2
        .build_with_curves(Some(&market))
        .expect("Term rate build should succeed");

    let overnight_flows: Vec<_> = overnight_schedule
        .flows
        .iter()
        .filter(|cf| cf.kind == CFKind::FloatReset)
        .collect();
    let term_flows: Vec<_> = term_schedule
        .flows
        .iter()
        .filter(|cf| cf.kind == CFKind::FloatReset)
        .collect();

    assert_eq!(
        overnight_flows.len(),
        term_flows.len(),
        "Both paths should produce the same number of flows"
    );

    for (i, (on, term)) in overnight_flows.iter().zip(term_flows.iter()).enumerate() {
        let on_rate = on.rate.expect("Overnight flow should have a rate");
        let term_rate = term.rate.expect("Term flow should have a rate");
        assert!(
            (on_rate - term_rate).abs() < 0.001,
            "Flow {}: overnight rate ({}) and term rate ({}) should be approximately equal for flat curve",
            i,
            on_rate,
            term_rate
        );
    }
}

// =============================================================================
// Test: Overnight compounding accrual starts on a weekend
// =============================================================================

/// Verifies that overnight compounding correctly accounts for non-business days
/// at the start of an accrual period (e.g., accrual_start on a Saturday).
///
/// Jan 4, 2025 is a Saturday. Using Unadjusted BDC preserves this as the raw
/// accrual start. The fix ensures the Saturday and Sunday (2 days) before the
/// first business day (Monday Jan 6) are assigned to Monday's fixing weight,
/// so no accrual days are lost.
///
/// We build two schedules — one starting Saturday (Unadjusted), one starting
/// Monday (Following) — and verify they produce approximately equal coupons.
/// Without the fix, the Saturday-start schedule would lose 2 days of accrual.
#[test]
fn test_overnight_compounding_weekend_start_no_lost_days() {
    use finstack_quant_cashflows::builder::specs::OvernightCompoundingMethod;
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::term_structures::ForwardCurve;
    use finstack_quant_core::math::interp::InterpStyle;

    // Jan 4 2025 = Saturday, Jan 6 2025 = Monday.
    // Maturity is a Tuesday so the Monday-start schedule's quarterly anchor
    // (Sun Apr 6, rolled to Mon Apr 7 by Following) does not collide with the
    // stub period's payment date — duplicate adjusted payment dates are now a
    // hard validation error instead of silently dropping a period.
    let saturday = Date::from_calendar_date(2025, Month::January, 4).unwrap();
    let monday = Date::from_calendar_date(2025, Month::January, 6).unwrap();
    let maturity = Date::from_calendar_date(2025, Month::April, 8).unwrap();
    let init = Money::new(1_000_000.0, Currency::USD);

    let friday = Date::from_calendar_date(2025, Month::January, 3).unwrap();
    let fwd = ForwardCurve::builder("USD-SOFR-ON", 1.0 / 360.0)
        .base_date(friday)
        .day_count(DayCount::Act360)
        .knots([(0.0, 0.05), (1.0, 0.05)])
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();
    let market = MarketContext::new().insert(fwd);

    let make_spec = |bdc| FloatingCouponSpec {
        rate_spec: FloatingRateSpec {
            index_id: "USD-SOFR-ON".into(),
            spread_bp: dec!(0),
            gearing: Decimal::ONE,
            gearing_includes_spread: true,
            index_floor_bp: None,
            all_in_cap_bp: None,
            all_in_floor_bp: None,
            index_cap_bp: None,
            overnight_index_constraints: OvernightIndexConstraintApplication::Daily,
            reset_freq: Tenor::quarterly(),
            index_tenor: None,
            reset_lag_days: 0,
            fixing_calendar_id: None,
            overnight_compounding: Some(OvernightCompoundingMethod::CompoundedInArrears),
            overnight_basis: None,
            fallback: FloatingRateFallback::Error,
        },
        coupon_type: CouponType::Cash,
        schedule: finstack_quant_cashflows::builder::ScheduleParams {
            freq: Tenor::quarterly(),
            dc: DayCount::Act360,
            bdc,
            calendar_id: "weekends_only".to_string(),
            stub: StubKind::ShortBack,
            end_of_month: false,
            payment_lag_days: 0,
            adjust_accrual_dates: false,
        },
    };

    // Schedule 1: Saturday start with Unadjusted BDC (accrual_start = Saturday)
    let sat_schedule = CashFlowSchedule::builder()
        .principal(init, saturday, maturity)
        .floating_cf(make_spec(BusinessDayConvention::Unadjusted))
        .build_with_curves(Some(&market))
        .expect("Unadjusted Saturday start should build");

    // Schedule 2: Monday start with Following BDC (baseline)
    let mon_schedule = CashFlowSchedule::builder()
        .principal(init, monday, maturity)
        .floating_cf(make_spec(BusinessDayConvention::Following))
        .build_with_curves(Some(&market))
        .expect("Following Monday start should build");

    let sat_floats: Vec<_> = sat_schedule
        .flows
        .iter()
        .filter(|cf| cf.kind == CFKind::FloatReset)
        .collect();
    let mon_floats: Vec<_> = mon_schedule
        .flows
        .iter()
        .filter(|cf| cf.kind == CFKind::FloatReset)
        .collect();

    assert!(
        !sat_floats.is_empty(),
        "Saturday schedule should have float flows"
    );
    assert!(
        !mon_floats.is_empty(),
        "Monday schedule should have float flows"
    );

    // Both should produce ~5% rate on a flat curve
    for cf in &sat_floats {
        let rate = cf.rate.expect("FloatReset should have a rate");
        assert!(
            (rate - 0.05).abs() < 0.002,
            "Saturday-start overnight rate should be ~5%, got {:.6}",
            rate
        );
    }

    // The Saturday schedule covers 2 extra calendar days (Sat+Sun) at the start.
    // With the fix, these days are assigned to Monday's fixing, so the total
    // coupon for the Saturday schedule should be >= the Monday schedule.
    let sat_total: f64 = sat_floats.iter().map(|cf| cf.amount.amount()).sum();
    let mon_total: f64 = mon_floats.iter().map(|cf| cf.amount.amount()).sum();

    assert!(
        sat_total >= mon_total * 0.99,
        "Saturday-start total ({:.2}) should not be materially less than Monday-start ({:.2}); \
         lost weekend days would cause a shortfall",
        sat_total,
        mon_total,
    );
}

// =============================================================================
// Empty overnight observation window must fail loudly
// =============================================================================

/// An accrual period containing no business-day fixings (Unadjusted BDC, stub
/// entirely on a weekend) must be a validation error, not a silent 0% index
/// with spread-only accrual.
#[test]
fn test_overnight_empty_fixing_window_errors() {
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::term_structures::ForwardCurve;

    // [Sat 2025-01-04, Sun 2025-01-05): single stub period with no business days.
    let issue = Date::from_calendar_date(2025, Month::January, 4).unwrap();
    let maturity = Date::from_calendar_date(2025, Month::January, 5).unwrap();
    let init = Money::new(1_000_000.0, Currency::USD);

    let fwd = ForwardCurve::builder("USD-SOFR-ON", 1.0 / 360.0)
        .base_date(issue)
        .day_count(DayCount::Act360)
        .knots([(0.0, 0.05), (1.0, 0.05)])
        .build()
        .unwrap();
    let market = MarketContext::new().insert(fwd);

    let spec = FloatingCouponSpec {
        rate_spec: FloatingRateSpec {
            index_id: "USD-SOFR-ON".into(),
            spread_bp: dec!(100),
            gearing: Decimal::ONE,
            gearing_includes_spread: true,
            index_floor_bp: None,
            all_in_cap_bp: None,
            all_in_floor_bp: None,
            index_cap_bp: None,
            overnight_index_constraints: OvernightIndexConstraintApplication::Daily,
            reset_freq: Tenor::quarterly(),
            index_tenor: None,
            reset_lag_days: 0,
            fixing_calendar_id: None,
            overnight_compounding: Some(OvernightCompoundingMethod::CompoundedInArrears),
            overnight_basis: None,
            fallback: FloatingRateFallback::Error,
        },
        coupon_type: CouponType::Cash,
        schedule: finstack_quant_cashflows::builder::ScheduleParams {
            freq: Tenor::quarterly(),
            dc: DayCount::Act360,
            bdc: BusinessDayConvention::Unadjusted,
            calendar_id: "weekends_only".to_string(),
            stub: StubKind::ShortFront,
            end_of_month: false,
            payment_lag_days: 0,
            adjust_accrual_dates: false,
        },
    };

    let mut b = CashFlowSchedule::builder();
    let _ = b.principal(init, issue, maturity).floating_cf(spec);
    let err = b
        .build_with_curves(Some(&market))
        .expect_err("empty fixing window must not silently accrue at 0% index");
    assert!(
        err.to_string().contains("no business-day fixings"),
        "error should describe the empty fixing window: {err}"
    );
}

// =============================================================================
// Strictly-past observations route through the fallback policy
// =============================================================================

/// With the default `Error` fallback, a coupon whose projection start is
/// strictly before the curve base date fails with a descriptive error naming
/// the date and index (historical fixings are not supported).
#[test]
fn test_seasoned_coupon_before_curve_base_errors_by_default() {
    let issue = Date::from_calendar_date(2025, Month::January, 15).unwrap();
    let maturity = Date::from_calendar_date(2026, Month::January, 15).unwrap();
    let init = Money::new(1_000_000.0, Currency::USD);

    // Curve based mid-life: the first accrual periods start strictly before it.
    let curve_base = Date::from_calendar_date(2025, Month::June, 15).unwrap();
    let market = make_flat_forward_market(curve_base, 0.03);

    let spec = make_float_spec(FloatingRateFallback::Error, dec!(0.0));
    let mut b = CashFlowSchedule::builder();
    let _ = b.principal(init, issue, maturity).floating_cf(spec);

    let err = b
        .build_with_curves(Some(&market))
        .expect_err("strictly-past observation with Error fallback must fail");
    let msg = err.to_string();
    assert!(
        msg.contains("USD-SOFR-3M") && msg.contains("requires fixings"),
        "error should name the index and require its historical fixing: {msg}"
    );
    assert!(
        msg.contains("ScalarTimeSeries"),
        "error should identify the required fixing series: {msg}"
    );
}

/// With `FixedRate(r)`, coupons whose observations are strictly before the
/// curve base use `r` as the index rate; later coupons still project from the
/// curve.
#[test]
fn test_seasoned_coupon_before_curve_base_uses_fixed_rate_fallback() {
    let issue = Date::from_calendar_date(2025, Month::January, 15).unwrap();
    let maturity = Date::from_calendar_date(2026, Month::January, 15).unwrap();
    let init = Money::new(1_000_000.0, Currency::USD);

    let curve_base = Date::from_calendar_date(2025, Month::June, 15).unwrap();
    let market = make_flat_forward_market(curve_base, 0.03);

    let spec = make_float_spec(FloatingRateFallback::FixedRate(dec!(0.045)), dec!(0.0));
    let mut b = CashFlowSchedule::builder();
    let _ = b.principal(init, issue, maturity).floating_cf(spec);
    let schedule = b
        .build_with_curves(Some(&market))
        .expect("FixedRate fallback should cover pre-base coupons");

    let rates: Vec<(Date, f64)> = schedule
        .flows
        .iter()
        .filter(|cf| cf.kind == CFKind::FloatReset)
        .map(|cf| (cf.date, cf.rate.expect("rate")))
        .collect();
    assert_eq!(rates.len(), 4, "quarterly coupons over one year");

    // Coupons accruing from Jan-15 and Apr-15 (strictly before the Jun-15
    // curve base) take the fixed fallback rate; later coupons project the
    // flat 3% curve.
    assert!(
        (rates[0].1 - 0.045).abs() < 1e-10,
        "pre-base coupon should use the FixedRate fallback: {:?}",
        rates[0]
    );
    assert!(
        (rates[1].1 - 0.045).abs() < 1e-10,
        "pre-base coupon should use the FixedRate fallback: {:?}",
        rates[1]
    );
    assert!(
        (rates[3].1 - 0.03).abs() < 1e-6,
        "post-base coupon should project from the curve: {:?}",
        rates[3]
    );
}

// =============================================================================
// Fixing calendar is used for overnight sampling
// =============================================================================

/// Overnight fixings must be sampled on the index's fixing calendar, not the
/// accrual calendar. With a rising curve, skipping the 2025-07-04 US holiday
/// (usny fixing calendar) produces a different compounded rate than treating
/// it as a fixing day (weekends_only).
#[test]
fn test_overnight_sampling_uses_fixing_calendar() {
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::term_structures::ForwardCurve;

    let base = Date::from_calendar_date(2024, Month::January, 15).unwrap();
    let issue = Date::from_calendar_date(2025, Month::April, 15).unwrap();
    let maturity = Date::from_calendar_date(2025, Month::October, 15).unwrap();
    let init = Money::new(1_000_000.0, Currency::USD);

    let fwd = ForwardCurve::builder("USD-SOFR-3M", 0.25)
        .base_date(base)
        .day_count(DayCount::Act360)
        .knots([(0.0, 0.03), (2.0, 0.13)])
        .build()
        .expect("rising forward curve builds");
    let market = MarketContext::new().insert(fwd);

    let build_with_fixing_cal = |fixing_calendar_id: Option<String>| {
        let mut spec = make_overnight_float_spec(
            OvernightCompoundingMethod::CompoundedInArrears,
            FloatingRateFallback::Error,
            dec!(0.0),
        );
        spec.rate_spec.fixing_calendar_id = fixing_calendar_id;
        let mut b = CashFlowSchedule::builder();
        let _ = b.principal(init, issue, maturity).floating_cf(spec);
        b.build_with_curves(Some(&market)).expect("schedule builds")
    };

    let weekends_only = build_with_fixing_cal(None);
    let usny = build_with_fixing_cal(Some("usny".to_string()));

    let first_rate = |s: &CashFlowSchedule| {
        s.flows
            .iter()
            .find(|cf| cf.kind == CFKind::FloatReset)
            .and_then(|cf| cf.rate)
            .expect("first floating coupon rate")
    };

    // The first accrual period [Apr-15, Jul-15) contains 2025-07-04 (a usny
    // holiday but a weekends_only business day); the holiday's weight folds
    // into the preceding fixing under usny, changing the compounded rate on a
    // non-flat curve.
    let r_weekends = first_rate(&weekends_only);
    let r_usny = first_rate(&usny);
    assert!(
        (r_weekends - r_usny).abs() > 1e-9,
        "fixing calendar must affect overnight sampling: weekends_only {r_weekends:.9} vs \
         usny {r_usny:.9}"
    );
}

// =============================================================================
// Historical fixings for seasoned instruments (FIXING:{index} series)
// =============================================================================

use finstack_quant_core::market_data::scalars::ScalarTimeSeries;

/// Helper: flat "USD-SOFR-3M" forward market plus an optional
/// `FIXING:USD-SOFR-3M` historical fixing series.
fn make_market_with_fixings(
    curve_base: Date,
    flat_rate: f64,
    fixings: &[(Date, f64)],
) -> finstack_quant_core::market_data::context::MarketContext {
    let mut market = make_flat_forward_market(curve_base, flat_rate);
    if !fixings.is_empty() {
        let series = ScalarTimeSeries::new("FIXING:USD-SOFR-3M", fixings.to_vec(), None)
            .expect("fixing series builds");
        market = market.insert_series(series);
    }
    market
}

/// Helper: ISDA 2021 compounded-in-arrears expectation over `(rate, days)`
/// pairs with an Act/360 compounding basis.
fn expected_compounded(rates_weights: &[(f64, u32)], total_days: u32) -> f64 {
    let mut product = 1.0;
    for &(rate, days) in rates_weights {
        product *= 1.0 + rate * f64::from(days) / 360.0;
    }
    (product - 1.0) * 360.0 / f64::from(total_days)
}

/// Helper: single-period overnight schedule [issue, maturity) and the emitted
/// FloatReset rate.
fn build_single_overnight_coupon(
    method: OvernightCompoundingMethod,
    issue: Date,
    maturity: Date,
    market: &finstack_quant_core::market_data::context::MarketContext,
) -> finstack_quant_core::Result<f64> {
    let mut spec = make_overnight_float_spec(method, FloatingRateFallback::Error, dec!(0.0));
    spec.schedule.stub = StubKind::ShortFront;

    let mut b = CashFlowSchedule::builder();
    let _ = b
        .principal(Money::new(1_000_000.0, Currency::USD), issue, maturity)
        .floating_cf(spec);
    let schedule = b.build_with_curves(Some(market))?;
    Ok(schedule
        .flows
        .iter()
        .find(|cf| cf.kind == CFKind::FloatReset)
        .and_then(|cf| cf.rate)
        .expect("single overnight FloatReset coupon with a rate"))
}

/// Partially seasoned compounded-in-arrears coupon: the curve is based
/// mid-accrual-period and the realized stretch resolves from the
/// `FIXING:USD-SOFR-3M` series. Golden value mixes hand-compounded realized
/// fixings (distinct from the curve rate so mixing errors are detectable)
/// with curve-projected forwards under identical `(rate, days)` weighting.
#[test]
fn test_seasoned_overnight_coupon_mixes_fixings_and_curve() {
    // Accrual [Mon 2025-06-02, Mon 2025-06-16); curve based Mon 2025-06-09.
    let issue = Date::from_calendar_date(2025, Month::June, 2).unwrap();
    let curve_base = Date::from_calendar_date(2025, Month::June, 9).unwrap();
    let maturity = Date::from_calendar_date(2025, Month::June, 16).unwrap();

    let fixings = [
        (
            Date::from_calendar_date(2025, Month::June, 2).unwrap(),
            0.020,
        ),
        (
            Date::from_calendar_date(2025, Month::June, 3).unwrap(),
            0.021,
        ),
        (
            Date::from_calendar_date(2025, Month::June, 4).unwrap(),
            0.022,
        ),
        (
            Date::from_calendar_date(2025, Month::June, 5).unwrap(),
            0.023,
        ),
        (
            Date::from_calendar_date(2025, Month::June, 6).unwrap(),
            0.024,
        ),
    ];
    let market = make_market_with_fixings(curve_base, 0.05, &fixings);

    let rate = build_single_overnight_coupon(
        OvernightCompoundingMethod::CompoundedInArrears,
        issue,
        maturity,
        &market,
    )
    .expect("partially seasoned compounded coupon builds from fixings + curve");

    // Realized: Mon-Thu weight 1, Fri weight 3 (weekend). Projected: curve
    // base Mon 06-09 onwards at the flat 5% (06-09 has no published fixing,
    // so t = 0 projection applies).
    let expected = expected_compounded(
        &[
            (0.020, 1),
            (0.021, 1),
            (0.022, 1),
            (0.023, 1),
            (0.024, 3),
            (0.05, 1),
            (0.05, 1),
            (0.05, 1),
            (0.05, 1),
            (0.05, 3),
        ],
        14,
    );
    assert!(
        (rate - expected).abs() < RATE_TOLERANCE,
        "seasoned compounded rate should mix realized fixings with projected forwards: \
         got {rate:.12}, expected {expected:.12}"
    );
}

/// Fully seasoned coupon: every observation precedes the curve base, so the
/// entire compounding window resolves from the fixing series.
#[test]
fn test_fully_seasoned_overnight_coupon_from_fixings_only() {
    let issue = Date::from_calendar_date(2025, Month::June, 2).unwrap();
    let maturity = Date::from_calendar_date(2025, Month::June, 16).unwrap();
    // Curve based on maturity: all observations are realized history.
    let curve_base = maturity;

    let fixings = [
        (
            Date::from_calendar_date(2025, Month::June, 2).unwrap(),
            0.020,
        ),
        (
            Date::from_calendar_date(2025, Month::June, 3).unwrap(),
            0.021,
        ),
        (
            Date::from_calendar_date(2025, Month::June, 4).unwrap(),
            0.022,
        ),
        (
            Date::from_calendar_date(2025, Month::June, 5).unwrap(),
            0.023,
        ),
        (
            Date::from_calendar_date(2025, Month::June, 6).unwrap(),
            0.024,
        ),
        (
            Date::from_calendar_date(2025, Month::June, 9).unwrap(),
            0.025,
        ),
        (
            Date::from_calendar_date(2025, Month::June, 10).unwrap(),
            0.026,
        ),
        (
            Date::from_calendar_date(2025, Month::June, 11).unwrap(),
            0.027,
        ),
        (
            Date::from_calendar_date(2025, Month::June, 12).unwrap(),
            0.028,
        ),
        (
            Date::from_calendar_date(2025, Month::June, 13).unwrap(),
            0.029,
        ),
    ];
    // Curve flat at 9% — far from every fixing, so any curve leakage is loud.
    let market = make_market_with_fixings(curve_base, 0.09, &fixings);

    let rate = build_single_overnight_coupon(
        OvernightCompoundingMethod::CompoundedInArrears,
        issue,
        maturity,
        &market,
    )
    .expect("fully seasoned compounded coupon builds from fixings only");

    let expected = expected_compounded(
        &[
            (0.020, 1),
            (0.021, 1),
            (0.022, 1),
            (0.023, 1),
            (0.024, 3),
            (0.025, 1),
            (0.026, 1),
            (0.027, 1),
            (0.028, 1),
            (0.029, 3),
        ],
        14,
    );
    assert!(
        (rate - expected).abs() < RATE_TOLERANCE,
        "fully seasoned compounded rate should come from fixings only: \
         got {rate:.12}, expected {expected:.12}"
    );
}

/// A missing historical business-day fixing is rejected. Weekend and holiday
/// carry is encoded by observation weights, not by carrying across publication
/// gaps on dates where the benchmark should have fixed.
#[test]
fn test_seasoned_overnight_coupon_rejects_missing_business_day_fixing() {
    let issue = Date::from_calendar_date(2025, Month::June, 2).unwrap();
    let maturity = Date::from_calendar_date(2025, Month::June, 16).unwrap();
    let curve_base = maturity;

    // 2025-06-11 is intentionally absent.
    let fixings = [
        (
            Date::from_calendar_date(2025, Month::June, 2).unwrap(),
            0.020,
        ),
        (
            Date::from_calendar_date(2025, Month::June, 3).unwrap(),
            0.021,
        ),
        (
            Date::from_calendar_date(2025, Month::June, 4).unwrap(),
            0.022,
        ),
        (
            Date::from_calendar_date(2025, Month::June, 5).unwrap(),
            0.023,
        ),
        (
            Date::from_calendar_date(2025, Month::June, 6).unwrap(),
            0.024,
        ),
        (
            Date::from_calendar_date(2025, Month::June, 9).unwrap(),
            0.025,
        ),
        (
            Date::from_calendar_date(2025, Month::June, 10).unwrap(),
            0.026,
        ),
        (
            Date::from_calendar_date(2025, Month::June, 12).unwrap(),
            0.028,
        ),
        (
            Date::from_calendar_date(2025, Month::June, 13).unwrap(),
            0.029,
        ),
    ];
    let market = make_market_with_fixings(curve_base, 0.09, &fixings);

    let err = build_single_overnight_coupon(
        OvernightCompoundingMethod::CompoundedInArrears,
        issue,
        maturity,
        &market,
    )
    .expect_err("missing historical business-day fixing must fail");
    assert!(
        err.to_string().contains("2025-06-11"),
        "error should identify the missing fixing date: {err}"
    );
}

/// An observation exactly on the curve base date prefers a published fixing
/// over the curve's t = 0 projection.
#[test]
fn test_seasoned_overnight_coupon_prefers_fixing_on_curve_base_date() {
    let issue = Date::from_calendar_date(2025, Month::June, 2).unwrap();
    let curve_base = Date::from_calendar_date(2025, Month::June, 9).unwrap();
    let maturity = Date::from_calendar_date(2025, Month::June, 16).unwrap();

    // The 06-09 fixing (3%) is published and differs from the curve (5%).
    let fixings = [
        (
            Date::from_calendar_date(2025, Month::June, 2).unwrap(),
            0.020,
        ),
        (
            Date::from_calendar_date(2025, Month::June, 3).unwrap(),
            0.021,
        ),
        (
            Date::from_calendar_date(2025, Month::June, 4).unwrap(),
            0.022,
        ),
        (
            Date::from_calendar_date(2025, Month::June, 5).unwrap(),
            0.023,
        ),
        (
            Date::from_calendar_date(2025, Month::June, 6).unwrap(),
            0.024,
        ),
        (
            Date::from_calendar_date(2025, Month::June, 9).unwrap(),
            0.030,
        ),
    ];
    let market = make_market_with_fixings(curve_base, 0.05, &fixings);

    let rate = build_single_overnight_coupon(
        OvernightCompoundingMethod::CompoundedInArrears,
        issue,
        maturity,
        &market,
    )
    .expect("published same-day fixing is preferred over the curve");

    let expected = expected_compounded(
        &[
            (0.020, 1),
            (0.021, 1),
            (0.022, 1),
            (0.023, 1),
            (0.024, 3),
            (0.030, 1), // published fixing on the curve base date
            (0.05, 1),
            (0.05, 1),
            (0.05, 1),
            (0.05, 3),
        ],
        14,
    );
    assert!(
        (rate - expected).abs() < RATE_TOLERANCE,
        "the published base-date fixing should beat the t=0 projection: \
         got {rate:.12}, expected {expected:.12}"
    );
}

/// Lockout (ISDA 2021 rate cut-off) over a fully seasoned window: the cut-off
/// observation dates resolve from fixings. With constant fixings equal to a
/// flat curve, the seasoned build must match an unseasoned build of the same
/// instrument.
#[test]
fn test_seasoned_overnight_lockout_resolves_from_fixings() {
    let issue = Date::from_calendar_date(2025, Month::June, 2).unwrap();
    let maturity = Date::from_calendar_date(2025, Month::June, 16).unwrap();
    let method = OvernightCompoundingMethod::CompoundedWithLockout { lockout_days: 2 };

    // All ten business-day fixings at the constant 4%.
    let fixings: Vec<(Date, f64)> = [2, 3, 4, 5, 6, 9, 10, 11, 12, 13]
        .into_iter()
        .map(|day| {
            (
                Date::from_calendar_date(2025, Month::June, day).unwrap(),
                0.04,
            )
        })
        .collect();

    // Seasoned: curve based on maturity, whole window realized from fixings.
    let seasoned_market = make_market_with_fixings(maturity, 0.04, &fixings);
    let seasoned = build_single_overnight_coupon(method, issue, maturity, &seasoned_market)
        .expect("seasoned lockout coupon resolves from fixings");

    // Unseasoned reference: same flat 4% curve based at issue, no fixings.
    let reference_market = make_flat_forward_market(issue, 0.04);
    let reference = build_single_overnight_coupon(method, issue, maturity, &reference_market)
        .expect("unseasoned lockout coupon projects from the curve");

    assert!(
        (seasoned - reference).abs() < RATE_TOLERANCE,
        "seasoned lockout rate {seasoned:.12} should equal the unseasoned reference \
         {reference:.12} when fixings equal the flat curve"
    );
}

/// Lookback (ARRC 2020 §2) with a seasoned window: shifted observation dates
/// before the curve base resolve from fixings. With constant fixings equal to
/// a flat curve, the seasoned build must match an unseasoned build whose
/// curve covers the full shifted window.
#[test]
fn test_seasoned_overnight_lookback_resolves_from_fixings() {
    let issue = Date::from_calendar_date(2025, Month::June, 2).unwrap();
    let curve_base = Date::from_calendar_date(2025, Month::June, 9).unwrap();
    let maturity = Date::from_calendar_date(2025, Month::June, 16).unwrap();
    let method = OvernightCompoundingMethod::CompoundedWithLookback { lookback_days: 2 };

    // 2 BD lookback shifts observations back to [Thu 2025-05-29, Wed 2025-06-11];
    // pre-base dates (05-29 .. 06-06) must resolve from fixings.
    let fixings: Vec<(Date, f64)> = [
        Date::from_calendar_date(2025, Month::May, 29).unwrap(),
        Date::from_calendar_date(2025, Month::May, 30).unwrap(),
        Date::from_calendar_date(2025, Month::June, 2).unwrap(),
        Date::from_calendar_date(2025, Month::June, 3).unwrap(),
        Date::from_calendar_date(2025, Month::June, 4).unwrap(),
        Date::from_calendar_date(2025, Month::June, 5).unwrap(),
        Date::from_calendar_date(2025, Month::June, 6).unwrap(),
    ]
    .into_iter()
    .map(|d| (d, 0.04))
    .collect();

    let seasoned_market = make_market_with_fixings(curve_base, 0.04, &fixings);
    let seasoned = build_single_overnight_coupon(method, issue, maturity, &seasoned_market)
        .expect("seasoned lookback coupon resolves shifted observations from fixings");

    // Unseasoned reference: flat 4% curve based before the first shifted
    // observation, no fixings required.
    let reference_market = make_flat_forward_market(
        Date::from_calendar_date(2025, Month::May, 29).unwrap(),
        0.04,
    );
    let reference = build_single_overnight_coupon(method, issue, maturity, &reference_market)
        .expect("unseasoned lookback coupon projects from the curve");

    assert!(
        (seasoned - reference).abs() < RATE_TOLERANCE,
        "seasoned lookback rate {seasoned:.12} should equal the unseasoned reference \
         {reference:.12} when fixings equal the flat curve"
    );
}

/// A seasoned term-rate reset resolves from the exact-date fixing, and
/// gearing/spread are applied on top of the historical index rate exactly as
/// for projected rates.
#[test]
fn test_seasoned_term_reset_resolves_from_exact_fixing_with_spread_and_gearing() {
    let issue = Date::from_calendar_date(2025, Month::January, 15).unwrap();
    let maturity = Date::from_calendar_date(2026, Month::January, 15).unwrap();
    let init = Money::new(1_000_000.0, Currency::USD);

    // Curve based mid-life: the Jan-15 and Apr-15 resets are realized history.
    let curve_base = Date::from_calendar_date(2025, Month::June, 15).unwrap();
    let fixings = [
        (
            Date::from_calendar_date(2025, Month::January, 15).unwrap(),
            0.040,
        ),
        (
            Date::from_calendar_date(2025, Month::April, 15).unwrap(),
            0.042,
        ),
    ];
    let market = make_market_with_fixings(curve_base, 0.03, &fixings);

    // SOFR + 100 bps, 2x gearing including spread: rate = (index + 1%) * 2.
    let mut spec = make_float_spec(FloatingRateFallback::Error, dec!(100.0));
    spec.rate_spec.gearing = dec!(2.0);

    let mut b = CashFlowSchedule::builder();
    let _ = b.principal(init, issue, maturity).floating_cf(spec);
    let schedule = b
        .build_with_curves(Some(&market))
        .expect("seasoned term resets should resolve from exact-date fixings");

    let rates: Vec<f64> = schedule
        .flows
        .iter()
        .filter(|cf| cf.kind == CFKind::FloatReset)
        .map(|cf| cf.rate.expect("rate"))
        .collect();
    assert_eq!(rates.len(), 4, "quarterly coupons over one year");

    // Historical resets: (fixing + spread) * gearing.
    assert!(
        (rates[0] - (0.040 + 0.01) * 2.0).abs() < RATE_TOLERANCE,
        "Jan-15 reset should be (4.0% fixing + 100bp) * 2: {}",
        rates[0]
    );
    assert!(
        (rates[1] - (0.042 + 0.01) * 2.0).abs() < RATE_TOLERANCE,
        "Apr-15 reset should be (4.2% fixing + 100bp) * 2: {}",
        rates[1]
    );
    // Post-base resets: projected from the flat 3% curve with the same
    // spread/gearing treatment.
    assert!(
        (rates[3] - (0.03 + 0.01) * 2.0).abs() < 1e-6,
        "post-base reset should project from the curve: {}",
        rates[3]
    );
}

#[test]
fn test_term_reset_before_curve_base_uses_fixing_when_accrual_starts_on_base() {
    let issue = Date::from_calendar_date(2025, Month::June, 16).unwrap();
    let maturity = Date::from_calendar_date(2025, Month::September, 16).unwrap();
    let init = Money::new(1_000_000.0, Currency::USD);
    let curve_base = issue;
    let reset_date = Date::from_calendar_date(2025, Month::June, 12).unwrap();
    let market = make_market_with_fixings(curve_base, 0.03, &[(reset_date, 0.0475)]);

    let mut spec = make_float_spec(FloatingRateFallback::Error, dec!(125.0));
    spec.rate_spec.reset_lag_days = 2;

    let mut b = CashFlowSchedule::builder();
    let _ = b.principal(init, issue, maturity).floating_cf(spec);
    let schedule = b
        .build_with_curves(Some(&market))
        .expect("reset before curve base should use exact fixing");

    let first_float = schedule
        .flows
        .iter()
        .find(|cf| cf.kind == CFKind::FloatReset)
        .expect("floating coupon");
    assert_eq!(first_float.reset_date, Some(reset_date));
    let rate = first_float.rate.expect("rate");
    assert!(
        (rate - 0.0600).abs() < RATE_TOLERANCE,
        "expected fixed index 4.75% + 125bp spread, got {rate}"
    );
}

#[test]
fn test_overnight_index_floor_defaults_to_daily_fixing_application() {
    let issue = Date::from_calendar_date(2025, Month::June, 2).unwrap();
    let maturity = Date::from_calendar_date(2025, Month::June, 5).unwrap();
    let curve_base = maturity;
    let fixings = [
        (
            Date::from_calendar_date(2025, Month::June, 2).unwrap(),
            -0.010,
        ),
        (
            Date::from_calendar_date(2025, Month::June, 3).unwrap(),
            0.020,
        ),
        (
            Date::from_calendar_date(2025, Month::June, 4).unwrap(),
            -0.010,
        ),
    ];
    let market = make_market_with_fixings(curve_base, 0.00, &fixings);

    let mut spec = make_overnight_float_spec(
        OvernightCompoundingMethod::CompoundedInArrears,
        FloatingRateFallback::Error,
        dec!(0.0),
    );
    spec.rate_spec.index_floor_bp = Some(dec!(0.0));
    spec.schedule.stub = StubKind::ShortFront;

    let mut b = CashFlowSchedule::builder();
    let _ = b
        .principal(Money::new(1_000_000.0, Currency::USD), issue, maturity)
        .floating_cf(spec);
    let schedule = b
        .build_with_curves(Some(&market))
        .expect("overnight coupon");
    let rate = schedule
        .flows
        .iter()
        .find(|cf| cf.kind == CFKind::FloatReset)
        .and_then(|cf| cf.rate)
        .expect("rate");

    let expected_daily_floor = expected_compounded(&[(0.0, 1), (0.020, 1), (0.0, 1)], 3);
    assert!(
        (rate - expected_daily_floor).abs() < RATE_TOLERANCE,
        "daily index floor expected {expected_daily_floor}, got {rate}"
    );
    assert!(
        rate > 0.0,
        "period-level floor would collapse this fixture to zero"
    );
}

#[test]
fn test_term_reset_on_curve_base_projects_without_same_day_fixing() {
    let issue = Date::from_calendar_date(2025, Month::June, 16).unwrap();
    let maturity = Date::from_calendar_date(2025, Month::September, 16).unwrap();
    let init = Money::new(1_000_000.0, Currency::USD);
    let prior_date = Date::from_calendar_date(2025, Month::June, 13).unwrap();
    let market = make_market_with_fixings(issue, 0.03, &[(prior_date, 0.0475)]);

    let spec = make_float_spec(FloatingRateFallback::Error, dec!(0.0));
    let mut builder = CashFlowSchedule::builder();
    let _ = builder.principal(init, issue, maturity).floating_cf(spec);
    let schedule = builder
        .build_with_curves(Some(&market))
        .expect("a reset on the curve base should remain projectable");

    let rate = schedule
        .flows
        .iter()
        .find(|cf| cf.kind == CFKind::FloatReset)
        .and_then(|cf| cf.rate)
        .expect("floating rate");
    assert!(
        (rate - 0.03).abs() < RATE_TOLERANCE,
        "same-day reset should project at 3%, got {rate}"
    );
}

/// A seasoned term-rate reset with a fixing series that lacks the exact reset
/// date fails with a descriptive error (term resets do not carry forward).
#[test]
fn test_seasoned_term_reset_missing_exact_fixing_errors() {
    let issue = Date::from_calendar_date(2025, Month::January, 15).unwrap();
    let maturity = Date::from_calendar_date(2026, Month::January, 15).unwrap();
    let init = Money::new(1_000_000.0, Currency::USD);

    let curve_base = Date::from_calendar_date(2025, Month::June, 15).unwrap();
    // Only the Jan-15 fixing is present; the Apr-15 reset has no observation.
    let fixings = [(
        Date::from_calendar_date(2025, Month::January, 15).unwrap(),
        0.040,
    )];
    let market = make_market_with_fixings(curve_base, 0.03, &fixings);

    let spec = make_float_spec(FloatingRateFallback::Error, dec!(0.0));
    let mut b = CashFlowSchedule::builder();
    let _ = b.principal(init, issue, maturity).floating_cf(spec);

    let err = b
        .build_with_curves(Some(&market))
        .expect_err("missing exact-date term fixing must fail");
    let msg = err.to_string();
    assert!(
        msg.contains("USD-SOFR-3M") && msg.contains("2025-04-15"),
        "error should name the index and the missing fixing date: {msg}"
    );
}
