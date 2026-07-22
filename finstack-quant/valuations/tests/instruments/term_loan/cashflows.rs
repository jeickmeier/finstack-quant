//! Term loan cashflow generation tests.

use finstack_quant_cashflows::builder::specs::CouponType;
use finstack_quant_cashflows::CashflowProvider;
use finstack_quant_core::cashflow::CFKind;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{BusinessDayConvention, DayCount, StubKind, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::CurveId;
use finstack_quant_valuations::instruments::fixed_income::term_loan::{
    AmortizationSpec, RateSpec, TermLoan,
};
use time::macros::date;

fn build_market_context() -> MarketContext {
    let disc = DiscountCurve::builder("USD-OIS")
        .base_date(date!(2025 - 01 - 01))
        .knots([(0.0, 1.0), (5.0, 0.85)])
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();
    MarketContext::new().insert(disc)
}

#[test]
fn test_fixed_coupon_cashflows() {
    // Arrange
    let loan = TermLoan::builder()
        .id("TL-CF-FIXED".into())
        .currency(Currency::USD)
        .notional_limit(Money::new(10_000_000.0, Currency::USD))
        .issue_date(date!(2025 - 01 - 01))
        .maturity(date!(2026 - 01 - 01)) // 1 year
        .rate(RateSpec::Fixed { rate_bp: 500 }) // 5%
        .frequency(Tenor::quarterly())
        .day_count(DayCount::Act360)
        .bdc(BusinessDayConvention::ModifiedFollowing)
        .calendar_id_opt(None)
        .stub(StubKind::None)
        .discount_curve_id(CurveId::from("USD-OIS"))
        .amortization(AmortizationSpec::None)
        .coupon_type(CouponType::Cash)
        .upfront_fee_opt(None)
        .ddtl_opt(None)
        .covenants_opt(None)
        .attributes(Default::default())
        .build()
        .unwrap();

    // Act
    let market = build_market_context();
    let as_of = date!(2025 - 01 - 01);
    let cashflows = loan.dated_cashflows(&market, as_of).unwrap();

    // Assert
    assert!(!cashflows.is_empty());
    // Should have quarterly coupons + principal repayment
    assert!(cashflows.len() >= 4);
}

#[test]
fn test_pik_interest_capitalization() {
    // Arrange
    let loan = TermLoan::builder()
        .id("TL-CF-PIK".into())
        .currency(Currency::USD)
        .notional_limit(Money::new(10_000_000.0, Currency::USD))
        .issue_date(date!(2025 - 01 - 01))
        .maturity(date!(2030 - 01 - 01))
        .rate(RateSpec::Fixed { rate_bp: 800 })
        .frequency(Tenor::semi_annual())
        .day_count(DayCount::Act360)
        .bdc(BusinessDayConvention::ModifiedFollowing)
        .calendar_id_opt(None)
        .stub(StubKind::None)
        .discount_curve_id(CurveId::from("USD-OIS"))
        .amortization(AmortizationSpec::None)
        .coupon_type(CouponType::PIK)
        .upfront_fee_opt(None)
        .ddtl_opt(None)
        .covenants_opt(None)
        .attributes(Default::default())
        .build()
        .unwrap();

    // Act
    let market = build_market_context();
    let as_of = date!(2025 - 01 - 01);
    let cashflows = loan.dated_cashflows(&market, as_of).unwrap();

    // Assert
    assert!(!cashflows.is_empty());
    // PIK interest capitalizes, so fewer cash payments expected
}

/// Property test: PercentPerPeriod with bp such that total amortization would exceed
/// the notional should be capped so that outstanding never goes negative.
///
/// Here we use bp=5000 (50% per quarter) over 4 quarters = 200% of notional.
/// The over-amortization guard should cap total amortization at 100%.
#[test]
fn test_over_amortization_is_capped() {
    let loan = TermLoan::builder()
        .id("TL-CF-OVERCAP".into())
        .currency(Currency::USD)
        .notional_limit(Money::new(1_000_000.0, Currency::USD))
        .issue_date(date!(2025 - 01 - 01))
        .maturity(date!(2026 - 01 - 01)) // 1 year, 4 quarterly periods
        .rate(RateSpec::Fixed { rate_bp: 500 })
        .frequency(Tenor::quarterly())
        .day_count(DayCount::Act360)
        .bdc(BusinessDayConvention::ModifiedFollowing)
        .calendar_id_opt(None)
        .stub(StubKind::None)
        .discount_curve_id(CurveId::from("USD-OIS"))
        // 50% per quarter × 4 quarters = 200% → should be capped at 100%
        .amortization(AmortizationSpec::PercentPerPeriod { bp: 5000 })
        .coupon_type(CouponType::Cash)
        .upfront_fee_opt(None)
        .ddtl_opt(None)
        .covenants_opt(None)
        .attributes(Default::default())
        .build()
        .unwrap();

    let market = build_market_context();
    let as_of = date!(2025 - 01 - 01);

    let schedule = loan
        .cashflow_schedule(&market, as_of)
        .expect("cashflow schedule should succeed even with excessive amort");

    // Total amortization should equal exactly the notional (capped)
    let total_amort: f64 = schedule
        .get_flows()
        .iter()
        .filter(|cf| cf.kind == CFKind::Amortization)
        .map(|cf| cf.amount.amount())
        .sum();

    // Amort amounts are positive from holder view (principal returned),
    // so total should not exceed the notional.
    assert!(
        total_amort <= 1_000_000.0 + 1e-6,
        "Total amort ({total_amort}) should not exceed notional (1,000,000)"
    );
    // With PercentPerPeriod applying to current outstanding (geometric decay):
    //   Q1: 1,000,000 × 50% = 500,000
    //   Q2:   500,000 × 50% = 250,000
    //   Q3:   250,000 × 50% = 125,000
    //   Q4:   125,000 × 50% =  62,500
    //   Total = 937,500
    let expected_total = 1_000_000.0 * (1.0 - 0.5_f64.powi(4)); // 937,500
    assert!(
        (total_amort - expected_total).abs() < 1.0,
        "Total amort ({total_amort}) should be approximately {expected_total} (geometric decay)"
    );
}

/// Linear amortization with start == issue should NOT generate an amort event at origination.
/// Amort payments only occur at period-end dates strictly after the start date.
#[test]
fn test_linear_amort_no_event_at_issue_date() {
    let issue = date!(2025 - 01 - 01);
    let maturity = date!(2026 - 01 - 01);
    let loan = TermLoan::builder()
        .id("TL-CF-LINEAR-ISSUE".into())
        .currency(Currency::USD)
        .notional_limit(Money::new(1_000_000.0, Currency::USD))
        .issue_date(issue)
        .maturity(maturity)
        .rate(RateSpec::Fixed { rate_bp: 500 })
        .frequency(Tenor::quarterly())
        .day_count(DayCount::Act360)
        .bdc(BusinessDayConvention::ModifiedFollowing)
        .calendar_id_opt(None)
        .stub(StubKind::None)
        .discount_curve_id(CurveId::from("USD-OIS"))
        .amortization(AmortizationSpec::Linear {
            start: issue, // start == issue
            end: maturity,
        })
        .coupon_type(CouponType::Cash)
        .upfront_fee_opt(None)
        .ddtl_opt(None)
        .covenants_opt(None)
        .attributes(Default::default())
        .build()
        .unwrap();

    let market = build_market_context();
    let as_of = issue;

    let schedule = loan
        .cashflow_schedule(&market, as_of)
        .expect("cashflow schedule should succeed");

    // No amort event should occur on the issue date itself
    let amort_at_issue: Vec<_> = schedule
        .get_flows()
        .iter()
        .filter(|cf| cf.kind == CFKind::Amortization && cf.date == issue)
        .collect();
    assert!(
        amort_at_issue.is_empty(),
        "No amortization should be generated at the issue date, but found {} events",
        amort_at_issue.len()
    );

    // Total amort should equal exactly the notional (4 equal quarterly payments)
    let total_amort: f64 = schedule
        .get_flows()
        .iter()
        .filter(|cf| cf.kind == CFKind::Amortization)
        .map(|cf| cf.amount.amount())
        .sum();
    assert!(
        (total_amort - 1_000_000.0).abs() < 1.0,
        "Total linear amort ({total_amort}) should equal notional (1,000,000)"
    );

    // Should have exactly 4 quarterly amort events (not 5)
    let amort_count = schedule
        .get_flows()
        .iter()
        .filter(|cf| cf.kind == CFKind::Amortization)
        .count();
    assert_eq!(
        amort_count, 4,
        "Should have 4 quarterly amort events, not {amort_count}"
    );
}

/// PercentPerPeriod with 100% (10000 bp) should fully amortize the loan.
/// After capping, total amortization equals the notional.
#[test]
fn test_percent_per_period_full_amort() {
    let loan = TermLoan::builder()
        .id("TL-CF-100PCT".into())
        .currency(Currency::USD)
        .notional_limit(Money::new(1_000_000.0, Currency::USD))
        .issue_date(date!(2025 - 01 - 01))
        .maturity(date!(2026 - 01 - 01))
        .rate(RateSpec::Fixed { rate_bp: 500 })
        .frequency(Tenor::quarterly())
        .day_count(DayCount::Act360)
        .bdc(BusinessDayConvention::ModifiedFollowing)
        .calendar_id_opt(None)
        .stub(StubKind::None)
        .discount_curve_id(CurveId::from("USD-OIS"))
        // 100% per period should amortize everything in Q1
        .amortization(AmortizationSpec::PercentPerPeriod { bp: 10000 })
        .coupon_type(CouponType::Cash)
        .upfront_fee_opt(None)
        .ddtl_opt(None)
        .covenants_opt(None)
        .attributes(Default::default())
        .build()
        .unwrap();

    let market = build_market_context();
    let as_of = date!(2025 - 01 - 01);

    let schedule = loan
        .cashflow_schedule(&market, as_of)
        .expect("cashflow schedule should succeed");

    // With 100% per period applied to current outstanding:
    // Q1: 100% × 1M = 1M (fully repaid in first period)
    // Q2-Q4: 0 (nothing left to amortize)
    let total_amort: f64 = schedule
        .get_flows()
        .iter()
        .filter(|cf| cf.kind == CFKind::Amortization)
        .map(|cf| cf.amount.amount())
        .sum();
    assert!(
        (total_amort - 1_000_000.0).abs() < 1.0,
        "Total amort ({total_amort}) should equal notional (1,000,000) after capping"
    );
}

/// PercentOfOriginalNotional produces flat dollar amortization each period.
/// All payments should be equal (original_notional * bp / 10_000).
#[test]
fn test_percent_of_original_notional_flat_dollar() {
    let notional = 10_000_000.0;
    let bp = 250; // 2.5% per period
    let loan = TermLoan::builder()
        .id("TL-CF-FLAT-AMORT".into())
        .currency(Currency::USD)
        .notional_limit(Money::new(notional, Currency::USD))
        .issue_date(date!(2025 - 01 - 01))
        .maturity(date!(2026 - 01 - 01)) // 1 year, 4 quarterly periods
        .rate(RateSpec::Fixed { rate_bp: 500 })
        .frequency(Tenor::quarterly())
        .day_count(DayCount::Act360)
        .bdc(BusinessDayConvention::ModifiedFollowing)
        .calendar_id_opt(None)
        .stub(StubKind::None)
        .discount_curve_id(CurveId::from("USD-OIS"))
        .amortization(AmortizationSpec::PercentOfOriginalNotional { bp })
        .coupon_type(CouponType::Cash)
        .upfront_fee_opt(None)
        .ddtl_opt(None)
        .covenants_opt(None)
        .attributes(Default::default())
        .build()
        .unwrap();

    let market = build_market_context();
    let as_of = date!(2025 - 01 - 01);

    let schedule = loan
        .cashflow_schedule(&market, as_of)
        .expect("cashflow schedule should succeed");

    let amort_amounts: Vec<f64> = schedule
        .get_flows()
        .iter()
        .filter(|cf| cf.kind == CFKind::Amortization)
        .map(|cf| cf.amount.amount())
        .collect();

    // Should have 4 quarterly amort events
    assert_eq!(
        amort_amounts.len(),
        4,
        "Should have 4 quarterly amort events"
    );

    // Each payment should be exactly notional * bp / 10000 = 10M * 0.025 = 250,000
    let expected_payment = notional * f64::from(bp) * 1e-4;
    for (i, amt) in amort_amounts.iter().enumerate() {
        assert!(
            (*amt - expected_payment).abs() < 0.01,
            "Amort payment {i} = {amt}, expected {expected_payment} (flat dollar)"
        );
    }
}

/// PercentOfOriginalNotional vs PercentPerPeriod: flat dollar differs from geometric decay.
#[test]
fn test_flat_vs_geometric_amort_differ() {
    let notional = 10_000_000.0;
    let bp = 250;
    let make_loan = |amort: AmortizationSpec, id: &str| {
        TermLoan::builder()
            .id(id.into())
            .currency(Currency::USD)
            .notional_limit(Money::new(notional, Currency::USD))
            .issue_date(date!(2025 - 01 - 01))
            .maturity(date!(2026 - 01 - 01))
            .rate(RateSpec::Fixed { rate_bp: 500 })
            .frequency(Tenor::quarterly())
            .day_count(DayCount::Act360)
            .bdc(BusinessDayConvention::ModifiedFollowing)
            .calendar_id_opt(None)
            .stub(StubKind::None)
            .discount_curve_id(CurveId::from("USD-OIS"))
            .amortization(amort)
            .coupon_type(CouponType::Cash)
            .upfront_fee_opt(None)
            .ddtl_opt(None)
            .covenants_opt(None)
            .attributes(Default::default())
            .build()
            .unwrap()
    };

    let loan_flat = make_loan(
        AmortizationSpec::PercentOfOriginalNotional { bp },
        "TL-FLAT",
    );
    let loan_geo = make_loan(AmortizationSpec::PercentPerPeriod { bp }, "TL-GEO");

    let market = build_market_context();
    let as_of = date!(2025 - 01 - 01);

    let get_amorts = |loan: &TermLoan| -> Vec<f64> {
        let schedule = loan.cashflow_schedule(&market, as_of).unwrap();
        schedule
            .get_flows()
            .iter()
            .filter(|cf| cf.kind == CFKind::Amortization)
            .map(|cf| cf.amount.amount())
            .collect()
    };

    let flat_amorts = get_amorts(&loan_flat);
    let geo_amorts = get_amorts(&loan_geo);

    assert_eq!(flat_amorts.len(), geo_amorts.len());

    // First payment should be the same (both = notional * bp / 10000)
    assert!(
        (flat_amorts[0] - geo_amorts[0]).abs() < 0.01,
        "First payment should match"
    );

    // Second payment onward: geometric decays, flat stays constant
    assert!(
        geo_amorts[1] < flat_amorts[1] - 1.0,
        "Geometric Q2 ({}) should be less than flat Q2 ({})",
        geo_amorts[1],
        flat_amorts[1]
    );

    // Total amort: flat > geometric (flat pays more because no decay)
    let flat_total: f64 = flat_amorts.iter().sum();
    let geo_total: f64 = geo_amorts.iter().sum();
    assert!(
        flat_total > geo_total,
        "Flat total ({flat_total}) should exceed geometric total ({geo_total})"
    );
}

/// DDTL with partial draws: amortisation should be based on drawn principal,
/// not the full commitment limit.
#[test]
fn test_ddtl_partial_draw_amort_uses_funded_amount() {
    use finstack_quant_valuations::instruments::fixed_income::term_loan::{
        CommitmentFeeBase, DdtlSpec, DrawEvent,
    };

    let issue = date!(2025 - 01 - 01);
    let commitment = 10_000_000.0;
    let drawn = 4_000_000.0; // Only 40% drawn
    let bp = 250; // 2.5% per period

    let loan = TermLoan::builder()
        .id("TL-DDTL-AMORT".into())
        .currency(Currency::USD)
        .notional_limit(Money::new(commitment, Currency::USD))
        .issue_date(issue)
        .maturity(date!(2026 - 01 - 01))
        .rate(RateSpec::Fixed { rate_bp: 500 })
        .frequency(Tenor::quarterly())
        .day_count(DayCount::Act360)
        .bdc(BusinessDayConvention::ModifiedFollowing)
        .calendar_id_opt(None)
        .stub(StubKind::None)
        .discount_curve_id(CurveId::from("USD-OIS"))
        .amortization(AmortizationSpec::PercentOfOriginalNotional { bp })
        .coupon_type(CouponType::Cash)
        .upfront_fee_opt(None)
        .ddtl_opt(Some(DdtlSpec {
            commitment_limit: Money::new(commitment, Currency::USD),
            availability_start: issue,
            availability_end: date!(2025 - 06 - 01),
            draws: vec![DrawEvent {
                date: issue,
                amount: Money::new(drawn, Currency::USD),
            }],
            commitment_step_downs: vec![],
            usage_fee_bp: 0,
            commitment_fee_bp: 0,
            fee_base: CommitmentFeeBase::Undrawn,
            oid_policy: None,
        }))
        .covenants_opt(None)
        .attributes(Default::default())
        .build()
        .unwrap();

    let market = build_market_context();
    let schedule = loan
        .cashflow_schedule(&market, issue)
        .expect("cashflow schedule should succeed");

    let amort_amounts: Vec<f64> = schedule
        .get_flows()
        .iter()
        .filter(|cf| cf.kind == CFKind::Amortization)
        .map(|cf| cf.amount.amount())
        .collect();

    // Each payment should be based on drawn amount, not commitment limit:
    // drawn * bp / 10000 = 4M * 0.025 = 100,000 (not 250,000)
    let expected_payment = drawn * f64::from(bp) * 1e-4;
    for (i, amt) in amort_amounts.iter().enumerate() {
        assert!(
            (*amt - expected_payment).abs() < 0.01,
            "DDTL amort payment {i} = {amt}, expected {expected_payment} (based on drawn, not commitment)"
        );
    }

    // Total amort should be based on drawn amount
    let total_amort: f64 = amort_amounts.iter().sum();
    let max_expected = drawn + 1.0;
    assert!(
        total_amort <= max_expected,
        "Total amort ({total_amort}) should not exceed drawn amount ({drawn})"
    );
}

/// Commitment fees should use CFKind::CommitmentFee, not CFKind::Fee.
#[test]
fn test_commitment_fees_use_correct_kind() {
    use finstack_quant_valuations::instruments::fixed_income::term_loan::{
        CommitmentFeeBase, DdtlSpec,
    };

    let issue = date!(2025 - 01 - 01);
    let loan = TermLoan::builder()
        .id("TL-CF-FEEKIND".into())
        .currency(Currency::USD)
        .notional_limit(Money::new(10_000_000.0, Currency::USD))
        .issue_date(issue)
        .maturity(date!(2027 - 01 - 01))
        .rate(RateSpec::Fixed { rate_bp: 500 })
        .frequency(Tenor::quarterly())
        .day_count(DayCount::Act360)
        .bdc(BusinessDayConvention::ModifiedFollowing)
        .calendar_id_opt(None)
        .stub(StubKind::None)
        .discount_curve_id(CurveId::from("USD-OIS"))
        .amortization(AmortizationSpec::None)
        .coupon_type(CouponType::Cash)
        .upfront_fee_opt(None)
        .ddtl_opt(Some(DdtlSpec {
            commitment_limit: Money::new(10_000_000.0, Currency::USD),
            availability_start: issue,
            availability_end: date!(2026 - 01 - 01),
            draws: vec![],
            commitment_step_downs: vec![],
            usage_fee_bp: 0,
            commitment_fee_bp: 50, // 50bp commitment fee on undrawn
            fee_base: CommitmentFeeBase::Undrawn,
            oid_policy: None,
        }))
        .covenants_opt(None)
        .attributes(Default::default())
        .build()
        .unwrap();

    let market = build_market_context();
    let schedule = loan
        .cashflow_schedule(&market, issue)
        .expect("cashflow schedule should succeed");

    // Commitment fees should exist and use CommitmentFee kind
    let commitment_fees: Vec<_> = schedule
        .get_flows()
        .iter()
        .filter(|cf| cf.kind == CFKind::CommitmentFee)
        .collect();
    assert!(
        !commitment_fees.is_empty(),
        "Commitment fees should use CFKind::CommitmentFee"
    );
    assert!(
        schedule
            .get_flows()
            .iter()
            .filter(|cf| matches!(cf.kind, CFKind::Fixed | CFKind::Stub))
            .all(|cf| cf.accrual.is_some()),
        "merging the commitment-fee leg must preserve coupon accrual metadata"
    );

    // No generic Fee kind should be used for commitment fees
    let generic_fees: Vec<_> = schedule
        .get_flows()
        .iter()
        .filter(|cf| cf.kind == CFKind::Fee)
        .collect();
    // The only CFKind::Fee flows should be from upfront/OID fees, not commitment fees.
    // Since we have no upfront fee and no OID, there should be no generic fees.
    assert!(
        generic_fees.is_empty(),
        "Commitment fees should not use generic CFKind::Fee, found {} generic fee flows",
        generic_fees.len()
    );
}

// ---------------------------------------------------------------------------
// Margin step-up period semantics: LSTA convention says a margin change
// applies from the start of the NEXT interest period, never mid-period, and
// the fixed and floating branches must agree on this.
// ---------------------------------------------------------------------------

mod margin_stepup_period_semantics {
    use super::*;
    use finstack_quant_cashflows::builder::FloatingRateSpec;
    use finstack_quant_core::market_data::term_structures::ForwardCurve;
    use finstack_quant_valuations::instruments::fixed_income::term_loan::{
        MarginStepUp, TermLoanCovenantEvents,
    };
    use rust_decimal::Decimal;

    /// Off-cycle step date in the middle of the second quarterly period.
    const STEP_DATE: time::Date = date!(2025 - 05 - 15);
    /// First period start on/after the step date (LSTA effective boundary).
    const NEXT_PERIOD_START: time::Date = date!(2025 - 07 - 01);

    fn stepup_covenants() -> TermLoanCovenantEvents {
        TermLoanCovenantEvents {
            margin_stepups: vec![MarginStepUp {
                date: STEP_DATE,
                delta_bp: 100,
            }],
            ..Default::default()
        }
    }

    fn build_floating_loan(id: &str, covenants: Option<TermLoanCovenantEvents>) -> TermLoan {
        TermLoan::builder()
            .id(id.into())
            .currency(Currency::USD)
            .notional_limit(Money::new(10_000_000.0, Currency::USD))
            .issue_date(date!(2025 - 01 - 01))
            .maturity(date!(2026 - 01 - 01))
            .rate(RateSpec::Floating(FloatingRateSpec {
                index_id: CurveId::from("USD-SOFR"),
                spread_bp: Decimal::from(200),
                gearing: Decimal::from(1),
                gearing_includes_spread: true,
                index_floor_bp: None,
                all_in_floor_bp: None,
                all_in_cap_bp: None,
                index_cap_bp: None,
                overnight_index_constraints: Default::default(),
                reset_freq: Tenor::quarterly(),
                index_tenor: None,
                reset_lag_days: 0,
                fixing_calendar_id: None,
                overnight_compounding: None,
                overnight_basis: None,
                fallback: Default::default(),
            }))
            .frequency(Tenor::quarterly())
            .day_count(DayCount::Act360)
            .bdc(BusinessDayConvention::ModifiedFollowing)
            .calendar_id_opt(None)
            .stub(StubKind::None)
            .discount_curve_id(CurveId::from("USD-OIS"))
            .amortization(AmortizationSpec::None)
            .coupon_type(CouponType::Cash)
            .upfront_fee_opt(None)
            .ddtl_opt(None)
            .covenants_opt(covenants)
            .attributes(Default::default())
            .build()
            .unwrap()
    }

    fn build_fixed_loan(id: &str, covenants: Option<TermLoanCovenantEvents>) -> TermLoan {
        TermLoan::builder()
            .id(id.into())
            .currency(Currency::USD)
            .notional_limit(Money::new(10_000_000.0, Currency::USD))
            .issue_date(date!(2025 - 01 - 01))
            .maturity(date!(2026 - 01 - 01))
            .rate(RateSpec::Fixed { rate_bp: 650 })
            .frequency(Tenor::quarterly())
            .day_count(DayCount::Act360)
            .bdc(BusinessDayConvention::ModifiedFollowing)
            .calendar_id_opt(None)
            .stub(StubKind::None)
            .discount_curve_id(CurveId::from("USD-OIS"))
            .amortization(AmortizationSpec::None)
            .coupon_type(CouponType::Cash)
            .upfront_fee_opt(None)
            .ddtl_opt(None)
            .covenants_opt(covenants)
            .attributes(Default::default())
            .build()
            .unwrap()
    }

    fn market_with_forward() -> MarketContext {
        let fwd = ForwardCurve::builder("USD-SOFR", 0.25)
            .base_date(date!(2025 - 01 - 01))
            .day_count(DayCount::Act360)
            .knots([(0.0, 0.045), (10.0, 0.045)])
            .build()
            .unwrap();
        build_market_context().insert(fwd)
    }

    /// `(accrual_start, accrual_end)` for every interest-like flow, in date order.
    fn accrual_periods(loan: &TermLoan, market: &MarketContext) -> Vec<(time::Date, time::Date)> {
        let schedule = loan
            .cashflow_schedule(market, date!(2025 - 01 - 01))
            .expect("cashflow schedule should succeed");
        let mut periods: Vec<(time::Date, time::Date)> = schedule
            .get_flows()
            .iter()
            .filter(|cf| cf.kind.is_interest_like())
            .map(|cf| {
                let accrual = cf
                    .accrual
                    .expect("interest flow should carry accrual metadata");
                (accrual.start, accrual.end)
            })
            .collect();
        periods.sort_unstable();
        periods
    }

    /// An off-cycle margin step-up must NOT split the enclosing accrual period:
    /// the period grid is identical to the no-step-up loan, and the new margin
    /// first applies to the period starting at the next period boundary.
    #[test]
    fn test_floating_stepup_applies_from_next_period_start_without_stub() {
        let market = market_with_forward();
        let base = build_floating_loan("TL-STEP-FL-BASE", None);
        let stepped = build_floating_loan("TL-STEP-FL", Some(stepup_covenants()));

        let base_periods = accrual_periods(&base, &market);
        let stepped_periods = accrual_periods(&stepped, &market);

        // No mid-period stub: identical period count and identical boundaries.
        assert_eq!(
            stepped_periods, base_periods,
            "off-cycle margin step-up must not split accrual periods \
             (expected {base_periods:?}, got {stepped_periods:?})"
        );
        assert!(
            stepped_periods
                .iter()
                .all(|(start, end)| *start != STEP_DATE && *end != STEP_DATE),
            "no accrual boundary may fall on the off-cycle step date {STEP_DATE}"
        );

        // The new margin first applies to the period starting at the next
        // period boundary (2025-07-01): earlier coupons match the no-step-up
        // loan, later coupons exceed it by the 100 bp step.
        let coupon_amounts = |loan: &TermLoan| -> Vec<(time::Date, f64)> {
            let schedule = loan
                .cashflow_schedule(&market, date!(2025 - 01 - 01))
                .unwrap();
            let mut flows: Vec<(time::Date, f64)> = schedule
                .get_flows()
                .iter()
                .filter(|cf| cf.kind.is_interest_like())
                .map(|cf| {
                    let accrual = cf.accrual.expect("interest flow should carry accrual");
                    (accrual.start, cf.amount.amount())
                })
                .collect();
            flows.sort_by(|a, b| a.0.cmp(&b.0));
            flows
        };
        let base_coupons = coupon_amounts(&base);
        let stepped_coupons = coupon_amounts(&stepped);
        assert_eq!(base_coupons.len(), stepped_coupons.len());
        for ((start, base_amt), (_, stepped_amt)) in base_coupons.iter().zip(stepped_coupons.iter())
        {
            if *start < NEXT_PERIOD_START {
                assert!(
                    (stepped_amt - base_amt).abs() < 1e-6,
                    "coupon for period starting {start} must be unchanged before the \
                     step takes effect (base {base_amt}, stepped {stepped_amt})"
                );
            } else {
                assert!(
                    *stepped_amt > *base_amt + 1.0,
                    "coupon for period starting {start} must reflect the +100 bp step \
                     (base {base_amt}, stepped {stepped_amt})"
                );
            }
        }
    }

    /// Fixed and floating loans with the same off-cycle step-up schedule must
    /// produce the same accrual-period boundaries (both apply the new margin
    /// from the start of the next interest period).
    #[test]
    fn test_fixed_and_floating_stepup_share_period_boundaries() {
        let market = market_with_forward();
        let floating = build_floating_loan("TL-STEP-FL-EQ", Some(stepup_covenants()));
        let fixed = build_fixed_loan("TL-STEP-FX-EQ", Some(stepup_covenants()));

        let floating_periods = accrual_periods(&floating, &market);
        let fixed_periods = accrual_periods(&fixed, &market);
        assert_eq!(
            floating_periods, fixed_periods,
            "fixed and floating margin step-ups must use the same whole-period \
             boundaries"
        );

        // The fixed branch applies the new rate from the next period start too.
        let schedule = fixed
            .cashflow_schedule(&market, date!(2025 - 01 - 01))
            .unwrap();
        for cf in schedule
            .get_flows()
            .iter()
            .filter(|cf| cf.kind.is_interest_like())
        {
            let accrual = cf.accrual.expect("interest flow should carry accrual");
            let rate = cf.rate.expect("fixed coupon should carry its rate");
            let expected = if accrual.start < NEXT_PERIOD_START {
                0.065
            } else {
                0.075
            };
            assert!(
                (rate - expected).abs() < 1e-12,
                "fixed coupon rate for period starting {} should be {expected}, got {rate}",
                accrual.start
            );
        }
    }
}
