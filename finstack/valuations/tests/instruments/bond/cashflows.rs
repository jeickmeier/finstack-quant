//! Bond cashflow generation tests.
//!
//! Tests cashflow generation for:
//! - Fixed-rate bonds
//! - Floating-rate bonds (FRNs)
//! - Amortizing bonds
//! - Custom cashflow schedules
//! - PIK and step-up structures

use finstack_cashflows::builder::AmortizationSpec;
use finstack_cashflows::builder::{CashFlowSchedule, CouponType, FixedCouponSpec, ScheduleParams};
use finstack_cashflows::CashflowProvider;
use finstack_core::currency::Currency;
use finstack_core::dates::{BusinessDayConvention, Date, DayCount, StubKind, Tenor};
use finstack_core::market_data::context::MarketContext;
use finstack_core::market_data::term_structures::{DiscountCurve, ForwardCurve};
use finstack_core::money::Money;
use finstack_core::types::CurveId;
use finstack_valuations::instruments::fixed_income::bond::{Bond, CashflowSpec};
use finstack_valuations::instruments::PricingOverrides;
use rust_decimal::Decimal;
use time::macros::date;

fn create_test_curves(base_date: Date) -> MarketContext {
    let disc = DiscountCurve::builder("USD-OIS")
        .base_date(base_date)
        .day_count(DayCount::Act365F)
        .knots([(0.0, 1.0), (1.0, 0.95), (5.0, 0.80)])
        .build()
        .unwrap();

    let fwd = ForwardCurve::builder("USD-SOFR-3M", 0.25)
        .base_date(base_date)
        .knots([(0.0, 0.05), (5.0, 0.055)])
        .build()
        .unwrap();

    MarketContext::new().insert(disc).insert(fwd)
}

#[test]
fn test_fixed_rate_cashflows() {
    let as_of = date!(2025 - 01 - 01);
    let maturity = date!(2027 - 01 - 01);

    let bond = Bond::fixed(
        "FIXED_CF",
        Money::new(1000.0, Currency::USD),
        0.06,
        as_of,
        maturity,
        "USD-OIS",
    )
    .unwrap();

    let curves = create_test_curves(as_of);
    let flows = bond.dated_cashflows(&curves, as_of).unwrap();

    // Should have coupons + principal
    assert!(!flows.is_empty());

    // All flows on or after as_of (signed canonical schedule includes issue-date flows)
    for (date, _amount) in &flows {
        assert!(*date >= as_of);
    }

    // Last flow should include principal
    let last_flow = flows.last().unwrap();
    assert!(last_flow.1.amount() > 30.0); // Coupon + principal
}

#[test]
fn test_cashflow_dates_alignment() {
    let as_of = date!(2025 - 01 - 01);
    let maturity = date!(2030 - 01 - 01);

    let bond = Bond::fixed(
        "CF_DATES",
        Money::new(1000.0, Currency::USD),
        0.05,
        as_of,
        maturity,
        "USD-OIS",
    )
    .unwrap();

    let curves = create_test_curves(as_of);
    let flows = bond.dated_cashflows(&curves, as_of).unwrap();

    // Verify dates are in chronological order (allowing for same-date flows)
    for i in 1..flows.len() {
        assert!(
            flows[i].0 >= flows[i - 1].0,
            "Flows should be in chronological order: flow[{}]={:?} < flow[{}]={:?}",
            i,
            flows[i].0,
            i - 1,
            flows[i - 1].0
        );
    }

    // Last flow should be at or before maturity
    assert!(flows.last().unwrap().0 <= maturity);
}

#[test]
fn test_quarterly_coupon_frequency() {
    let as_of = date!(2025 - 01 - 01);
    let maturity = date!(2026 - 01 - 01);

    let bond = Bond::builder()
        .id("QUARTERLY".into())
        .notional(Money::new(1000.0, Currency::USD))
        .issue_date(as_of)
        .maturity(maturity)
        .cashflow_spec(CashflowSpec::fixed(
            0.04,
            Tenor::quarterly(),
            DayCount::Act365F,
        ).expect("finite test coupon"))
        .discount_curve_id("USD-OIS".into())
        .pricing_overrides(PricingOverrides::default())
        .build()
        .unwrap();

    let curves = create_test_curves(as_of);
    let flows = bond.dated_cashflows(&curves, as_of).unwrap();

    // 1 year = 4 quarters + principal + initial negative notional
    assert_eq!(flows.len(), 6);
}

#[test]
fn test_floating_rate_cashflows() {
    let as_of = date!(2025 - 01 - 01);
    let maturity = date!(2027 - 01 - 01);

    let bond = Bond::builder()
        .id("FRN".into())
        .notional(Money::new(1000.0, Currency::USD))
        .issue_date(as_of)
        .maturity(maturity)
        .cashflow_spec(CashflowSpec::floating(
            CurveId::new("USD-SOFR-3M"),
            150.0,
            Tenor::quarterly(),
            DayCount::Act360,
        ).expect("finite test rate"))
        .discount_curve_id(CurveId::new("USD-OIS"))
        .pricing_overrides(PricingOverrides::default())
        .build()
        .unwrap();

    let curves = create_test_curves(as_of);
    let flows = bond.dated_cashflows(&curves, as_of).unwrap();

    // Should have floating coupons + principal
    assert!(!flows.is_empty());

    // Signed canonical schedule: coupon flows are positive, initial notional is negative
    let positive_count = flows.iter().filter(|(_, a)| a.amount() > 0.0).count();
    assert!(positive_count > 0, "Should have positive coupon flows");
}

#[test]
fn test_amortizing_bond_linear() {
    let as_of = date!(2025 - 01 - 01);
    let maturity = date!(2027 - 01 - 01);

    let bond = Bond::builder()
        .id("AMORT_LINEAR".into())
        .notional(Money::new(1000.0, Currency::USD))
        .issue_date(as_of)
        .maturity(maturity)
        .cashflow_spec(CashflowSpec::amortizing(
            CashflowSpec::fixed(0.05, Tenor::semi_annual(), DayCount::Act365F).expect("finite test coupon"),
            AmortizationSpec::LinearTo {
                final_notional: Money::new(400.0, Currency::USD),
            },
        ))
        .discount_curve_id("USD-OIS".into())
        .pricing_overrides(PricingOverrides::default())
        .build()
        .unwrap();

    let curves = create_test_curves(as_of);
    let flows = bond.dated_cashflows(&curves, as_of).unwrap();

    // Should have cashflows (coupons + amortization + redemption)
    assert!(!flows.is_empty(), "Amortizing bond should have cashflows");

    // Signed canonical schedule includes both positive flows (coupons,
    // amortization, redemption) and a negative initial notional.
    let positive_count = flows.iter().filter(|(_, a)| a.amount() > 0.0).count();
    assert!(
        positive_count > 0,
        "Amortizing bond should have positive coupon/amortization flows"
    );

    let total: f64 = flows.iter().map(|(_, amt)| amt.amount()).sum();
    assert!(total.is_finite(), "Total cashflow should be finite");
}

#[test]
fn test_custom_cashflows_from_schedule() {
    let issue = date!(2025 - 01 - 01);
    let maturity = date!(2027 - 01 - 01);

    // Build custom step-up schedule
    let step1 = date!(2026 - 01 - 01);
    let params = ScheduleParams {
        freq: Tenor::semi_annual(),
        dc: DayCount::Act365F,
        bdc: BusinessDayConvention::Following,
        calendar_id: "weekends_only".to_string(),
        stub: StubKind::None,
        end_of_month: false,
        payment_lag_days: 0,
    };

    let custom_schedule = CashFlowSchedule::builder()
        .principal(Money::new(1000.0, Currency::USD), issue, maturity)
        .fixed_stepup_decimal(
            &[(step1, Decimal::new(4, 2)), (maturity, Decimal::new(6, 2))],
            params,
            CouponType::Cash,
        )
        .build_with_curves(None)
        .unwrap();

    let bond = Bond::from_cashflows("CUSTOM", custom_schedule, "USD-OIS", Some(98.0)).unwrap();

    let curves = create_test_curves(issue);
    let flows = bond.dated_cashflows(&curves, issue).unwrap();

    // Should use custom cashflows
    assert!(!flows.is_empty());
}

#[test]
fn test_pik_cashflows() {
    let issue = date!(2025 - 01 - 01);
    let maturity = date!(2027 - 01 - 01);

    // Build PIK toggle schedule
    let custom_schedule = CashFlowSchedule::builder()
        .principal(Money::new(1000.0, Currency::USD), issue, maturity)
        .fixed_cf(FixedCouponSpec {
            coupon_type: CouponType::PIK,
            rate: rust_decimal::Decimal::try_from(0.08).expect("valid"),
            freq: Tenor::semi_annual(),
            dc: DayCount::Act365F,
            bdc: BusinessDayConvention::Following,
            calendar_id: "weekends_only".to_string(),
            stub: StubKind::None,
            end_of_month: false,
            payment_lag_days: 0,
        })
        .build_with_curves(None)
        .unwrap();

    let bond = Bond::from_cashflows("PIK", custom_schedule, "USD-OIS", None).unwrap();

    let curves = create_test_curves(issue);
    let schedule = bond.cashflow_schedule(&curves, issue).unwrap();

    assert!(
        schedule
            .flows
            .iter()
            .all(|cf| cf.kind != finstack_cashflows::primitives::CFKind::PIK),
        "holder-view cashflow_schedule should exclude PIK accretion"
    );
    assert!(!schedule.flows.is_empty());
}

#[test]
fn test_cashflows_for_matured_bond() {
    let as_of = date!(2025 - 01 - 01);
    let issue = date!(2020 - 01 - 01);
    let maturity = date!(2024 - 01 - 01); // Already matured

    let bond = Bond::fixed(
        "MATURED",
        Money::new(1000.0, Currency::USD),
        0.05,
        issue,
        maturity,
        "USD-OIS",
    )
    .unwrap();

    let curves = create_test_curves(as_of);
    let flows = bond.dated_cashflows(&curves, as_of).unwrap();

    // The cashflow builder generates all flows from issue to maturity.
    // The CashflowProvider trait's dated_cashflows path filters to future holder-view flows.
    // However, the builder itself may still include all historical flows.
    // What matters is that when pricing, only future flows are used.

    // Verify no flows are after the maturity date
    for (date, _) in &flows {
        assert!(*date <= maturity, "No flows should be after maturity");
    }
}

#[test]
fn test_cashflows_with_short_front_stub() {
    let as_of = date!(2025 - 01 - 15); // Mid-month start
    let maturity = date!(2027 - 01 - 01);

    use finstack_cashflows::builder::specs::CouponType;
    let bond = Bond::builder()
        .id("STUB_SHORT".into())
        .notional(Money::new(1000.0, Currency::USD))
        .issue_date(as_of)
        .maturity(maturity)
        .cashflow_spec(CashflowSpec::Fixed(FixedCouponSpec {
            coupon_type: CouponType::Cash,
            rate: rust_decimal::Decimal::try_from(0.05).expect("valid"),
            freq: Tenor::semi_annual(),
            dc: DayCount::Act365F,
            bdc: BusinessDayConvention::Following,
            calendar_id: "weekends_only".to_string(),
            stub: StubKind::ShortFront,
            end_of_month: false,
            payment_lag_days: 0,
        }))
        .discount_curve_id("USD-OIS".into())
        .pricing_overrides(PricingOverrides::default())
        .build()
        .unwrap();

    let curves = create_test_curves(as_of);
    let flows = bond.dated_cashflows(&curves, as_of).unwrap();

    // Should generate cashflows with stub handling
    assert!(!flows.is_empty());

    // All flows on or after issue date (signed canonical schedule)
    for (date, _) in &flows {
        assert!(*date >= as_of, "All flows should be on or after issue");
    }

    // Should have multiple payment dates through to maturity
    assert!(flows.len() > 1, "Should have multiple cashflows");
}

#[test]
fn test_zero_coupon_cashflows() {
    let as_of = date!(2025 - 01 - 01);
    let maturity = date!(2030 - 01 - 01);

    let bond = Bond::fixed(
        "ZERO",
        Money::new(1000.0, Currency::USD),
        0.0,
        as_of,
        maturity,
        "USD-OIS",
    )
    .unwrap();

    let curves = create_test_curves(as_of);
    let flows = bond.dated_cashflows(&curves, as_of).unwrap();

    // Zero coupon bond: only principal at maturity
    // The schedule builder may still generate coupon periods but with 0 amounts
    // Or it may optimize to just the redemption
    assert!(!flows.is_empty());

    // Last flow should be the principal
    let last_flow = flows.last().unwrap();
    assert_eq!(last_flow.0, maturity);
    assert!(last_flow.1.amount() > 0.0);
}

#[test]
fn test_cashflows_notional_scaling() {
    let as_of = date!(2025 - 01 - 01);
    let maturity = date!(2027 - 01 - 01);

    let notionals = vec![100.0, 1_000.0, 1_000_000.0, 10_000_000.0];

    for notional_amt in notionals {
        let bond = Bond::fixed(
            format!("SCALE_{}", notional_amt),
            Money::new(notional_amt, Currency::USD),
            0.05,
            as_of,
            maturity,
            "USD-OIS",
        )
        .unwrap();

        let curves = create_test_curves(as_of);
        let flows = bond.dated_cashflows(&curves, as_of).unwrap();

        assert!(!flows.is_empty());

        // Last flow should scale with notional
        let last_amount = flows.last().unwrap().1.amount();
        assert!(last_amount > notional_amt * 0.5); // At least includes partial principal
    }
}

#[test]
fn test_cashflow_schedule_fixed() {
    let as_of = date!(2025 - 01 - 01);
    let maturity = date!(2027 - 01 - 01);

    let bond = Bond::fixed(
        "FULL_SCHED",
        Money::new(1000.0, Currency::USD),
        0.06,
        as_of,
        maturity,
        "USD-OIS",
    )
    .unwrap();

    let curves = create_test_curves(as_of);
    let full_schedule = bond.cashflow_schedule(&curves, as_of).unwrap();

    // Should have flows with CFKind metadata
    assert!(!full_schedule.flows.is_empty());
    assert_eq!(full_schedule.notional.initial.amount(), 1000.0);
}

#[test]
fn test_cashflow_schedule_floating() {
    let as_of = date!(2025 - 01 - 01);
    let maturity = date!(2027 - 01 - 01);

    let bond = Bond::builder()
        .id("FULL_FRN".into())
        .notional(Money::new(1000.0, Currency::USD))
        .issue_date(as_of)
        .maturity(maturity)
        .cashflow_spec(CashflowSpec::floating(
            CurveId::new("USD-SOFR-3M"),
            100.0,
            Tenor::quarterly(),
            DayCount::Act360,
        ).expect("finite test rate"))
        .discount_curve_id(CurveId::new("USD-OIS"))
        .pricing_overrides(PricingOverrides::default())
        .build()
        .unwrap();

    let curves = create_test_curves(as_of);
    let full_schedule = bond.cashflow_schedule(&curves, as_of).unwrap();

    assert!(!full_schedule.flows.is_empty());
}

#[test]
fn test_cashflows_day_count_conventions() {
    let as_of = date!(2025 - 01 - 01);
    let maturity = date!(2027 - 01 - 01);

    let day_counts = vec![
        DayCount::Thirty360,
        DayCount::Act360,
        DayCount::Act365F,
        DayCount::ActAct,
    ];

    for dc in day_counts {
        let bond = Bond::builder()
            .id(format!("DC_{:?}", dc).into())
            .notional(Money::new(1000.0, Currency::USD))
            .issue_date(as_of)
            .maturity(maturity)
            .cashflow_spec(CashflowSpec::fixed(0.05, Tenor::semi_annual(), dc).expect("finite test coupon"))
            .discount_curve_id("USD-OIS".into())
            .pricing_overrides(PricingOverrides::default())
            .build()
            .unwrap();

        let curves = create_test_curves(as_of);
        let flows = bond.dated_cashflows(&curves, as_of).unwrap();

        // All day count conventions should produce valid cashflows
        assert!(!flows.is_empty());
    }
}

#[test]
fn test_amortizing_full_redemption() {
    let as_of = date!(2025 - 01 - 01);
    let maturity = date!(2027 - 01 - 01);

    let bond = Bond::builder()
        .id("AMORT_FULL".into())
        .notional(Money::new(1000.0, Currency::USD))
        .issue_date(as_of)
        .maturity(maturity)
        .cashflow_spec(CashflowSpec::amortizing(
            CashflowSpec::fixed(0.05, Tenor::semi_annual(), DayCount::Act365F).expect("finite test coupon"),
            AmortizationSpec::LinearTo {
                final_notional: Money::new(0.0, Currency::USD),
            },
        ))
        .discount_curve_id("USD-OIS".into())
        .pricing_overrides(PricingOverrides::default())
        .build()
        .unwrap();

    let curves = create_test_curves(as_of);
    let flows = bond.dated_cashflows(&curves, as_of).unwrap();

    // Should have amortization flows throughout
    assert!(!flows.is_empty());
}

/// Tests Act/Act ISMA day count with semi-annual frequency.
///
/// Market standard: ICMA Rule 251 requires frequency context for Act/Act ISMA.
/// For a full semi-annual coupon period, accrual should equal 0.5 (6 months / 12 months).
#[test]
fn test_actact_isma_daycount_context() {
    let issue = date!(2025 - 01 - 01);
    let maturity = date!(2027 - 01 - 01); // Exactly 2 years

    let bond = Bond::builder()
        .id("ACTACT_ISMA".into())
        .notional(Money::new(1_000_000.0, Currency::USD))
        .issue_date(issue)
        .maturity(maturity)
        .cashflow_spec(CashflowSpec::Fixed(FixedCouponSpec {
            coupon_type: CouponType::Cash,
            rate: rust_decimal::Decimal::try_from(0.06).expect("valid"), // 6% coupon
            freq: Tenor::semi_annual(),
            dc: DayCount::ActActIsma, // ISMA convention requires frequency context
            bdc: BusinessDayConvention::Following,
            calendar_id: "weekends_only".to_string(),
            stub: StubKind::None,
            end_of_month: false,
            payment_lag_days: 0,
        }))
        .discount_curve_id("USD-OIS".into())
        .pricing_overrides(PricingOverrides::default())
        .build()
        .unwrap();

    let curves = create_test_curves(issue);
    let full_schedule = bond.cashflow_schedule(&curves, issue).unwrap();

    // Find coupon flows (exclude principal redemption)
    let coupon_flows: Vec<_> = full_schedule
        .flows
        .iter()
        .filter(|cf| {
            matches!(
                cf.kind,
                finstack_cashflows::primitives::CFKind::Fixed
                    | finstack_cashflows::primitives::CFKind::Stub
            )
        })
        .collect();

    // Should have 4 semi-annual coupons across 2 years
    assert_eq!(
        coupon_flows.len(),
        4,
        "Expected 4 semi-annual coupons, got {}",
        coupon_flows.len()
    );

    // Each full coupon period should have accrual_factor = 0.5 year fraction
    // Act/Act ISMA with semi-annual frequency returns 0.5 for a full 6-month period
    // (This validates that DayCountContext with frequency is being used correctly)
    for cf in &coupon_flows {
        // For Act/Act ISMA with semi-annual frequency, a full coupon period
        // should have accrual_factor = 0.5 (6 months / 12 months = 0.5 year)
        let expected_accrual = 0.5; // Half year for semi-annual
        let tolerance = 1e-10; // Very tight tolerance for determinism

        assert!(
            (cf.accrual_factor - expected_accrual).abs() < tolerance,
            "Act/Act ISMA accrual factor mismatch: got {}, expected {} ± {}",
            cf.accrual_factor,
            expected_accrual,
            tolerance
        );

        // Expected coupon = notional * rate * accrual_factor
        // = 1,000,000 * 0.06 * 0.5 = 30,000 per semi-annual period
        let expected_coupon = 1_000_000.0 * 0.06 * cf.accrual_factor;
        assert!(
            (cf.amount.amount() - expected_coupon).abs() < 0.01,
            "Coupon amount mismatch: got {}, expected {}",
            cf.amount.amount(),
            expected_coupon
        );
    }
}

/// Tests Bus/252 day count with calendar context.
///
/// Market standard: Bus/252 requires calendar-aware business day counting.
/// Accrual = (business days between dates) / 252
#[test]
fn test_bus252_daycount_with_calendar() {
    let issue = date!(2025 - 01 - 01);
    let maturity = date!(2026 - 01 - 01); // 1 year

    let bond = Bond::builder()
        .id("BUS252".into())
        .notional(Money::new(1_000_000.0, Currency::USD))
        .issue_date(issue)
        .maturity(maturity)
        .cashflow_spec(CashflowSpec::Fixed(FixedCouponSpec {
            coupon_type: CouponType::Cash,
            rate: rust_decimal::Decimal::try_from(0.05).expect("valid"), // 5% coupon
            freq: Tenor::quarterly(),
            dc: DayCount::Bus252, // Requires calendar context
            bdc: BusinessDayConvention::Following,
            calendar_id: "USNY".to_string(), // New York calendar
            stub: StubKind::None,
            end_of_month: false,
            payment_lag_days: 0,
        }))
        .discount_curve_id("USD-OIS".into())
        .pricing_overrides(PricingOverrides::default())
        .build()
        .unwrap();

    let curves = create_test_curves(issue);
    let full_schedule = bond.cashflow_schedule(&curves, issue).unwrap();

    // Find coupon flows
    let coupon_flows: Vec<_> = full_schedule
        .flows
        .iter()
        .filter(|cf| {
            matches!(
                cf.kind,
                finstack_cashflows::primitives::CFKind::Fixed
                    | finstack_cashflows::primitives::CFKind::Stub
            )
        })
        .collect();

    // Should have 4 quarterly coupons across 1 year
    assert_eq!(
        coupon_flows.len(),
        4,
        "Expected 4 quarterly coupons, got {}",
        coupon_flows.len()
    );

    // For Bus/252, each accrual factor should be (business_days / 252)
    // We can't predict exact values without simulating the calendar,
    // but we can verify:
    // 1. All accrual factors are positive and reasonable
    // 2. The calendar context is being used (not default which would error)
    for cf in &coupon_flows {
        // Business days in a quarter: roughly 63 days (252/4)
        // So accrual should be around 0.25, but varies by actual business days
        assert!(
            cf.accrual_factor > 0.0 && cf.accrual_factor < 0.35,
            "Bus/252 accrual factor out of reasonable range: {}",
            cf.accrual_factor
        );

        // Coupon should be positive and finite
        assert!(
            cf.amount.amount() > 0.0 && cf.amount.amount().is_finite(),
            "Coupon amount invalid: {}",
            cf.amount.amount()
        );
    }

    // The key validation is that Bus/252 with calendar context doesn't error
    // (which it would if calendar wasn't passed to DayCountContext)
    // The actual total can vary based on calendar holidays and stub handling
    let total_yf: f64 = coupon_flows.iter().map(|cf| cf.accrual_factor).sum();

    // Verify total is positive and reasonable (between 0.5 and 1.5 for a year)
    assert!(
        total_yf > 0.5 && total_yf < 1.5,
        "Total Bus/252 accrual should be reasonable for a year, got {}",
        total_yf
    );
}
