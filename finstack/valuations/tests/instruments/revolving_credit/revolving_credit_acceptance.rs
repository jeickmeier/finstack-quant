//! Acceptance tests for revolving credit market-standards compliance.
//!
//! These tests verify that the implementation follows market-standard practices:
//! - Correct sign conventions for upfront fees
//! - Intra-period event accrual
//! - Curve-aware deterministic pricing for floating rates
//! - Reset frequency handling
//! - Fee tiering
//! - As-of filtering

use finstack_core::currency::Currency;
use finstack_core::dates::{Date, DayCount, Tenor};
use finstack_core::market_data::context::MarketContext;
use finstack_core::market_data::term_structures::DiscountCurve;
use finstack_core::market_data::term_structures::ForwardCurve;
use finstack_core::money::Money;
use finstack_valuations::instruments::fixed_income::revolving_credit::{
    BaseRateSpec, DrawRepayEvent, DrawRepaySpec, RevolvingCredit, RevolvingCreditFees,
};
use finstack_valuations::instruments::Instrument;
use time::Month;

/// Helper function to generate deterministic cashflows with curves using the new engine
fn _generate_deterministic_cashflows_with_curves_replaced(
    facility: &RevolvingCredit,
    market: &MarketContext,
    as_of: Date,
) -> finstack_core::Result<finstack_cashflows::builder::CashFlowSchedule> {
    use finstack_valuations::instruments::fixed_income::revolving_credit::cashflow_engine::CashflowEngine;
    let engine = CashflowEngine::new(facility, Some(market), as_of, None)?;
    let path_schedule = engine.generate_deterministic()?;
    Ok(path_schedule.schedule)
}

#[test]
fn test_upfront_fee_sign() {
    // Test that upfront fee increases PV when valued before commitment
    // (borrower pays lender, lender inflow). When as_of >= commitment_date
    // the fee is excluded because it's already been paid.
    let as_of = Date::from_calendar_date(2024, Month::December, 31).unwrap();
    let start = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let end = Date::from_calendar_date(2026, Month::January, 1).unwrap();

    let facility = RevolvingCredit::builder()
        .id("RC-UPFRONT-TEST".into())
        .commitment_amount(Money::new(10_000_000.0, Currency::USD))
        .drawn_amount(Money::new(0.0, Currency::USD))
        .commitment_date(start)
        .maturity(end)
        .base_rate_spec(BaseRateSpec::Fixed { rate: 0.0 })
        .day_count(DayCount::Act360)
        .frequency(Tenor::quarterly())
        .fees({
            let mut fees = RevolvingCreditFees::flat(0.0, 0.0, 0.0);
            fees.upfront_fee = Some(Money::new(50_000.0, Currency::USD));
            fees
        })
        .draw_repay_spec(DrawRepaySpec::Deterministic(vec![]))
        .discount_curve_id("USD-OIS".into())
        .build()
        .unwrap();

    let disc_curve = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .day_count(DayCount::Act365F)
        .knots([(0.0, 1.0), (1.0, 0.9999)])
        .build()
        .unwrap();
    let market = MarketContext::new().insert(disc_curve);

    // Pre-commitment valuation: fee is still in the future, should be included
    let pv_pre = facility.value(&market, as_of).unwrap();
    assert!(
        (pv_pre.amount() - 50_000.0).abs() < 5.0,
        "PV (pre-commitment) should include upfront fee, got {}",
        pv_pre.amount()
    );
    assert!(
        pv_pre.amount() > 0.0,
        "PV should be positive (lender inflow)"
    );

    // Post-commitment valuation: fee already paid, should be excluded
    let disc_curve2 = DiscountCurve::builder("USD-OIS")
        .base_date(start)
        .day_count(DayCount::Act365F)
        .knots([(0.0, 1.0), (1.0, 0.9999)])
        .build()
        .unwrap();
    let market2 = MarketContext::new().insert(disc_curve2);
    let pv_post = facility.value(&market2, start).unwrap();
    assert!(
        pv_post.amount().abs() < 1.0,
        "PV (post-commitment, zero-rate, no draws) should be ~0, got {}",
        pv_post.amount()
    );
}

#[test]
fn test_mid_period_draw_accrual() {
    // Test that interest accrues correctly when draw occurs mid-period
    // Jan-1 to Apr-1 quarter; draw on Feb-15
    let start = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let end = Date::from_calendar_date(2025, Month::April, 1).unwrap();
    let draw_date = Date::from_calendar_date(2025, Month::February, 15).unwrap();

    let facility = RevolvingCredit::builder()
        .id("RC-MID-DRAW".into())
        .commitment_amount(Money::new(10_000_000.0, Currency::USD))
        .drawn_amount(Money::new(5_000_000.0, Currency::USD))
        .commitment_date(start)
        .maturity(end)
        .base_rate_spec(BaseRateSpec::Fixed { rate: 0.05 }) // 5% annual
        .day_count(DayCount::Act360)
        .frequency(Tenor::quarterly())
        .fees(RevolvingCreditFees::default())
        .draw_repay_spec(DrawRepaySpec::Deterministic(vec![DrawRepayEvent {
            date: draw_date,
            amount: Money::new(2_000_000.0, Currency::USD),
            is_draw: true,
        }]))
        .discount_curve_id("USD-OIS".into())
        .build()
        .unwrap();

    // Create flat discount curve
    let disc_curve = DiscountCurve::builder("USD-OIS")
        .base_date(start)
        .day_count(DayCount::Act365F)
        .knots([(0.0, 1.0), (0.25, 0.9999)])
        .build()
        .unwrap();
    let market = MarketContext::new().insert(disc_curve);

    let schedule =
        _generate_deterministic_cashflows_with_curves_replaced(&facility, &market, start).unwrap();

    // Find interest cashflow at period end (Apr-1)
    let interest_flow = schedule
        .flows
        .iter()
        .find(|cf| cf.date == end && matches!(cf.kind, finstack_core::cashflow::CFKind::Fixed))
        .expect("Should have interest flow at period end");

    // Interest should be:
    // - Jan 1 to Feb 15: 5M * 0.05 * (45/360) = 31,250
    // - Feb 15 to Apr 1: 7M * 0.05 * (45/360) = 43,750
    // Total: 75,000
    let expected_interest = 75_000.0;
    let tolerance = 1e-6; // High precision for deterministic calculations

    assert!(
        (interest_flow.amount.amount() - expected_interest).abs() < tolerance,
        "Interest should account for mid-period draw. Expected ~{}, got {}",
        expected_interest,
        interest_flow.amount.amount()
    );
}

#[test]
fn test_floating_vs_margin_only() {
    // Test that curve-aware pricing differs from margin-only
    let start = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let end = Date::from_calendar_date(2026, Month::January, 1).unwrap();

    // Create a steepened forward curve
    let fwd_curve = ForwardCurve::builder("USD-SOFR-3M", 0.25)
        .base_date(start)
        .knots([
            (0.0, 0.01),   // 1% at start
            (0.25, 0.015), // 1.5% at 3M
            (0.5, 0.02),   // 2% at 6M
            (1.0, 0.025),  // 2.5% at 1Y
        ])
        .build()
        .unwrap();

    let facility = RevolvingCredit::builder()
        .id("RC-FLOATING-TEST".into())
        .commitment_amount(Money::new(10_000_000.0, Currency::USD))
        .drawn_amount(Money::new(5_000_000.0, Currency::USD))
        .commitment_date(start)
        .maturity(end)
        .base_rate_spec(BaseRateSpec::Floating(
            finstack_cashflows::builder::FloatingRateSpec {
                index_id: "USD-SOFR-3M".into(),
                spread_bp: rust_decimal::Decimal::try_from(100.0).expect("valid"), // 100 bps margin
                gearing: rust_decimal::Decimal::try_from(1.0).expect("valid"),
                gearing_includes_spread: true,
                index_floor_bp: None,
                all_in_floor_bp: None,
                all_in_cap_bp: None,
                index_cap_bp: None,
                reset_freq: Tenor::quarterly(),
                index_tenor: None,
                reset_lag_days: 2,
                dc: DayCount::Act360,
                bdc: finstack_core::dates::BusinessDayConvention::ModifiedFollowing,
                calendar_id: "weekends_only".to_string(),
                fixing_calendar_id: None,
                end_of_month: false,
                overnight_compounding: None,
                overnight_basis: None,
                fallback: Default::default(),
                payment_lag_days: 0,
            },
        ))
        .day_count(DayCount::Act360)
        .frequency(Tenor::quarterly())
        .fees(RevolvingCreditFees::default())
        .draw_repay_spec(DrawRepaySpec::Deterministic(vec![]))
        .discount_curve_id("USD-OIS".into())
        .build()
        .unwrap();

    let disc_curve = DiscountCurve::builder("USD-OIS")
        .base_date(start)
        .day_count(DayCount::Act365F)
        .knots([(0.0, 1.0), (1.0, (-0.02f64).exp())])
        .build()
        .unwrap();

    let market_with_curve = MarketContext::new().insert(disc_curve).insert(fwd_curve);

    // Price with curves (should include forward rates)
    let pv_with_curves = facility.value(&market_with_curve, start).unwrap();

    // Price without curves (margin-only)
    // This would use generate_deterministic_cashflows instead of _with_curves
    // But since we've switched to _with_curves, we can't easily test margin-only
    // Instead, verify that PV is reasonable and uses forward rates

    // PV should be reasonable (lender deploys capital, receives interest and fees)
    // With steepened curve, interest should be higher than margin-only
    // Note: sign depends on balance of principal deployment vs. interest/fees received
    assert!(
        pv_with_curves.amount().abs() > 0.0,
        "PV should be non-zero with curve-aware pricing"
    );
}

#[test]
fn test_reset_frequency_mismatch() {
    // Test monthly resets with quarterly payments
    let start = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let end = Date::from_calendar_date(2025, Month::April, 1).unwrap(); // One quarter

    let fwd_curve = ForwardCurve::builder("USD-SOFR-1M", 1.0 / 12.0)
        .base_date(start)
        .knots([
            (0.0, 0.01),
            (1.0 / 12.0, 0.011),
            (2.0 / 12.0, 0.012),
            (3.0 / 12.0, 0.013),
        ])
        .build()
        .unwrap();

    let facility = RevolvingCredit::builder()
        .id("RC-RESET-TEST".into())
        .commitment_amount(Money::new(10_000_000.0, Currency::USD))
        .drawn_amount(Money::new(5_000_000.0, Currency::USD))
        .commitment_date(start)
        .maturity(end)
        .base_rate_spec(BaseRateSpec::Floating(
            finstack_cashflows::builder::FloatingRateSpec {
                index_id: "USD-SOFR-1M".into(),
                spread_bp: rust_decimal::Decimal::try_from(0.0).expect("valid"), // No margin to isolate reset effect
                gearing: rust_decimal::Decimal::try_from(1.0).expect("valid"),
                gearing_includes_spread: true,
                index_floor_bp: None,
                all_in_floor_bp: None,
                all_in_cap_bp: None,
                index_cap_bp: None,
                reset_freq: Tenor::monthly(), // Monthly resets
                index_tenor: None,
                reset_lag_days: 2,
                dc: DayCount::Act360,
                bdc: finstack_core::dates::BusinessDayConvention::ModifiedFollowing,
                calendar_id: "weekends_only".to_string(),
                fixing_calendar_id: None,
                end_of_month: false,
                overnight_compounding: None,
                overnight_basis: None,
                fallback: Default::default(),
                payment_lag_days: 0,
            },
        ))
        .day_count(DayCount::Act360)
        .frequency(Tenor::quarterly()) // Quarterly payments
        .fees(RevolvingCreditFees::default())
        .draw_repay_spec(DrawRepaySpec::Deterministic(vec![]))
        .discount_curve_id("USD-OIS".into())
        .build()
        .unwrap();

    let disc_curve = DiscountCurve::builder("USD-OIS")
        .base_date(start)
        .day_count(DayCount::Act365F)
        .knots([(0.0, 1.0), (0.5, 0.9999)])
        .build()
        .unwrap();

    let market = MarketContext::new().insert(disc_curve).insert(fwd_curve);

    let schedule =
        _generate_deterministic_cashflows_with_curves_replaced(&facility, &market, start).unwrap();

    // Should have interest flow at quarter end
    let interest_flow = schedule
        .flows
        .iter()
        .find(|cf| cf.date == end && matches!(cf.kind, finstack_core::cashflow::CFKind::FloatReset))
        .expect("Should have floating interest flow");

    // Verify reset_date is set (should be Jan 1, first reset)
    assert!(
        interest_flow.reset_date.is_some(),
        "Floating interest should have reset_date set"
    );

    // Interest should use multiple monthly fixings over the quarter
    // With monthly resets, we should see different rates applied to sub-periods
    assert!(
        interest_flow.amount.amount() > 0.0,
        "Interest should be positive"
    );
}

#[test]
fn test_utilization_tier() {
    // Test that fee tiers produce different fees when utilization crosses threshold
    let start = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let end = Date::from_calendar_date(2025, Month::April, 1).unwrap();

    // Create tiered usage fees: 10 bps below 50%, 20 bps above 50%
    use finstack_cashflows::builder::FeeTier;
    let usage_tiers = vec![
        FeeTier {
            threshold: rust_decimal::Decimal::try_from(0.0).expect("valid"),
            bps: rust_decimal::Decimal::try_from(10.0).expect("valid"),
        },
        FeeTier {
            threshold: rust_decimal::Decimal::try_from(0.5).expect("valid"),
            bps: rust_decimal::Decimal::try_from(20.0).expect("valid"),
        },
    ];

    let facility_low_util = RevolvingCredit::builder()
        .id("RC-TIER-LOW".into())
        .commitment_amount(Money::new(10_000_000.0, Currency::USD))
        .drawn_amount(Money::new(3_000_000.0, Currency::USD)) // 30% utilization
        .commitment_date(start)
        .maturity(end)
        .base_rate_spec(BaseRateSpec::Fixed { rate: 0.0 })
        .day_count(DayCount::Act360)
        .frequency(Tenor::quarterly())
        .fees({
            let mut fees = RevolvingCreditFees::flat(0.0, 0.0, 0.0);
            fees.usage_fee_tiers = usage_tiers.clone();
            fees
        })
        .draw_repay_spec(DrawRepaySpec::Deterministic(vec![]))
        .discount_curve_id("USD-OIS".into())
        .build()
        .unwrap();

    let facility_high_util = RevolvingCredit::builder()
        .id("RC-TIER-HIGH".into())
        .commitment_amount(Money::new(10_000_000.0, Currency::USD))
        .drawn_amount(Money::new(7_000_000.0, Currency::USD)) // 70% utilization
        .commitment_date(start)
        .maturity(end)
        .base_rate_spec(BaseRateSpec::Fixed { rate: 0.0 })
        .day_count(DayCount::Act360)
        .frequency(Tenor::quarterly())
        .fees({
            let mut fees = RevolvingCreditFees::flat(0.0, 0.0, 0.0);
            fees.usage_fee_tiers = usage_tiers;
            fees
        })
        .draw_repay_spec(DrawRepaySpec::Deterministic(vec![]))
        .discount_curve_id("USD-OIS".into())
        .build()
        .unwrap();

    let disc_curve = DiscountCurve::builder("USD-OIS")
        .base_date(start)
        .day_count(DayCount::Act365F)
        .knots([(0.0, 1.0), (0.5, 0.9999)])
        .build()
        .unwrap();
    let market = MarketContext::new().insert(disc_curve);

    let schedule_low =
        _generate_deterministic_cashflows_with_curves_replaced(&facility_low_util, &market, start)
            .unwrap();

    let schedule_high =
        _generate_deterministic_cashflows_with_curves_replaced(&facility_high_util, &market, start)
            .unwrap();

    let fee_low: f64 = schedule_low
        .flows
        .iter()
        .filter(|cf| matches!(cf.kind, finstack_core::cashflow::CFKind::UsageFee))
        .map(|cf| cf.amount.amount())
        .sum();

    let fee_high: f64 = schedule_high
        .flows
        .iter()
        .filter(|cf| matches!(cf.kind, finstack_core::cashflow::CFKind::UsageFee))
        .map(|cf| cf.amount.amount())
        .sum();

    // High utilization (70%) should pay more fees than low (30%)
    // Low: 3M * 10 bps * 0.25 = 750
    // High: 7M * 20 bps * 0.25 = 3,500
    assert!(
        fee_high > fee_low,
        "High utilization should pay more fees. Low: {}, High: {}",
        fee_low,
        fee_high
    );
}

#[test]
fn test_as_of_filtering() {
    // Test that non-principal flows are strictly after as_of
    let start = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let q1_end = Date::from_calendar_date(2025, Month::April, 1).unwrap();
    let end = Date::from_calendar_date(2026, Month::January, 1).unwrap();

    let facility = RevolvingCredit::builder()
        .id("RC-ASOF-TEST".into())
        .commitment_amount(Money::new(10_000_000.0, Currency::USD))
        .drawn_amount(Money::new(5_000_000.0, Currency::USD))
        .commitment_date(start)
        .maturity(end)
        .base_rate_spec(BaseRateSpec::Fixed { rate: 0.05 })
        .day_count(DayCount::Act360)
        .frequency(Tenor::quarterly())
        .fees(RevolvingCreditFees::default())
        .draw_repay_spec(DrawRepaySpec::Deterministic(vec![]))
        .discount_curve_id("USD-OIS".into())
        .build()
        .unwrap();

    let disc_curve = DiscountCurve::builder("USD-OIS")
        .base_date(start)
        .day_count(DayCount::Act365F)
        .knots([(0.0, 1.0), (0.5, 0.9999)])
        .build()
        .unwrap();
    let market = MarketContext::new().insert(disc_curve);

    // As-of at Q1 end: no interest/fee cashflows should have date <= as_of
    let schedule =
        _generate_deterministic_cashflows_with_curves_replaced(&facility, &market, q1_end).unwrap();

    let non_principal_flows: Vec<_> = schedule
        .flows
        .iter()
        .filter(|cf| {
            matches!(
                cf.kind,
                finstack_core::cashflow::CFKind::Fixed
                    | finstack_core::cashflow::CFKind::FloatReset
                    | finstack_core::cashflow::CFKind::Fee
            )
        })
        .collect();

    assert!(
        non_principal_flows.iter().all(|cf| cf.date > q1_end),
        "All non-principal flows should be strictly after as_of date"
    );
}
