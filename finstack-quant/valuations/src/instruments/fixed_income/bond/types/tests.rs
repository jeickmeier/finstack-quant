//! Bond type tests (construction, cashflows, floating, amortization).

use super::*;
use crate::cashflow::builder::{CashFlowSchedule, CouponType, FixedCouponSpec, ScheduleParams};
use crate::cashflow::traits::CashflowProvider;
use crate::instruments::common_impl::traits::{Attributes, Instrument};
use crate::instruments::PricingOverrides;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{BusinessDayConvention, Date, DayCount, StubKind, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::market_data::term_structures::ForwardCurve;
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CurveId, InstrumentId};
use rust_decimal::Decimal;
use time::Month;

#[test]
fn test_bond_with_custom_cashflows() {
    // Setup dates
    let issue = Date::from_calendar_date(2025, Month::January, 15).expect("Valid test date");
    let maturity = Date::from_calendar_date(2027, Month::January, 15).expect("Valid test date");

    // Build a custom cashflow schedule with step-up coupons
    let schedule_params = ScheduleParams {
        freq: Tenor::semi_annual(),
        dc: DayCount::Act365F,
        bdc: BusinessDayConvention::Following,
        calendar_id: "weekends_only".to_string(),
        stub: StubKind::None,
        end_of_month: false,
        payment_lag_days: 0,
        adjust_accrual_dates: false,
    };

    let step1_date = Date::from_calendar_date(2026, Month::January, 15).expect("Valid test date");

    let custom_schedule = CashFlowSchedule::builder()
        .principal(Money::new(1_000_000.0, Currency::USD), issue, maturity)
        .fixed_stepup_decimal(
            &[
                (step1_date, Decimal::new(3, 2)),
                (maturity, Decimal::new(5, 2)),
            ],
            schedule_params,
            CouponType::Cash,
        )
        .build_with_curves(None)
        .expect("CashFlowSchedule builder should succeed with valid test data");

    // Create bond from custom cashflows
    let bond = Bond::from_cashflows(
        "CUSTOM_STEPUP_BOND",
        custom_schedule.clone(),
        "USD-OIS",
        Some(98.5),
    )
    .expect("Bond::from_cashflows should succeed with valid test data");

    // Verify bond properties
    assert_eq!(bond.id.as_str(), "CUSTOM_STEPUP_BOND");
    assert_eq!(bond.discount_curve_id.as_str(), "USD-OIS");
    assert_eq!(
        bond.pricing_overrides.market_quotes.quoted_clean_price,
        Some(98.5)
    );
    assert_eq!(bond.issue_date, issue);
    assert_eq!(bond.maturity, maturity);
    assert!(bond.custom_cashflows.is_some());

    // Create curves for pricing
    let disc_curve = DiscountCurve::builder("USD-OIS")
        .base_date(issue)
        .knots([(0.0, 1.0), (3.0, 0.95)])
        .interp(finstack_quant_core::math::interp::InterpStyle::Linear)
        .build()
        .expect("CashFlowSchedule builder should succeed with valid test data");
    let curves = MarketContext::new().insert(disc_curve);

    // Build schedule and verify it uses custom cashflows
    let flows = bond
        .dated_cashflows(&curves, issue)
        .expect("Schedule building should succeed in test");
    assert!(!flows.is_empty());

    // The signed schedule preserves all flows except pure PIK
    let expected_flow_count = custom_schedule
        .flows
        .iter()
        .filter(|cf| cf.kind != crate::cashflow::primitives::CFKind::PIK)
        .count();
    assert_eq!(flows.len(), expected_flow_count);
}

#[test]
fn test_bond_builder_with_custom_cashflows() {
    let issue = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");
    let maturity = Date::from_calendar_date(2026, Month::January, 1).expect("Valid test date");

    // Build custom cashflow with PIK toggle
    let custom_schedule = CashFlowSchedule::builder()
        .principal(Money::new(1_000_000.0, Currency::USD), issue, maturity)
        .fixed_cf(FixedCouponSpec {
            coupon_type: CouponType::Split {
                cash_pct: Decimal::try_from(0.5).expect("valid"),
                pik_pct: Decimal::try_from(0.5).expect("valid"),
            },
            rate: Decimal::try_from(0.06).expect("valid"),
            freq: Tenor::quarterly(),
            dc: DayCount::Thirty360,
            bdc: BusinessDayConvention::Following,
            calendar_id: "weekends_only".to_string(),
            stub: StubKind::None,
            end_of_month: false,
            payment_lag_days: 0,
        })
        .build_with_curves(None)
        .expect("CashFlowSchedule builder should succeed with valid test data");

    // Use builder pattern (default cashflow_spec since custom_cashflows overrides)
    let bond = Bond::builder()
        .id("PIK_TOGGLE_BOND".into())
        .notional(Money::new(1_000_000.0, Currency::USD))
        .issue_date(issue)
        .maturity(maturity)
        .cashflow_spec(CashflowSpec::default())
        .custom_cashflows_opt(Some(custom_schedule))
        .discount_curve_id(CurveId::new("USD-OIS"))
        .pricing_overrides(PricingOverrides::default().with_quoted_clean_price(99.0))
        .attributes(Attributes::new())
        .build()
        .expect("CashFlowSchedule builder should succeed with valid test data");

    assert_eq!(bond.id.as_str(), "PIK_TOGGLE_BOND");
    assert_eq!(bond.discount_curve_id.as_str(), "USD-OIS");
    assert_eq!(
        bond.pricing_overrides.market_quotes.quoted_clean_price,
        Some(99.0)
    );
    assert!(bond.custom_cashflows.is_some());
    assert_eq!(bond.notional.currency(), Currency::USD);
}

#[test]
fn test_bond_with_cashflows_method() {
    let issue = Date::from_calendar_date(2025, Month::March, 1).expect("Valid test date");
    let maturity = Date::from_calendar_date(2030, Month::March, 1).expect("Valid test date");

    // Create a traditional bond first (builder)
    let mut bond = Bond::builder()
        .id(InstrumentId::new("REGULAR_BOND"))
        .notional(Money::new(1_000_000.0, Currency::USD))
        .issue_date(issue)
        .maturity(maturity)
        .cashflow_spec(
            CashflowSpec::fixed(0.04, Tenor::semi_annual(), DayCount::Act365F)
                .expect("finite test coupon"),
        )
        .discount_curve_id(CurveId::new("USD-OIS"))
        .credit_curve_id_opt(None)
        .pricing_overrides(PricingOverrides::default())
        .call_put_opt(None)
        .custom_cashflows_opt(None)
        .attributes(Attributes::new())
        .settlement_convention_opt(None)
        .build()
        .expect("CashFlowSchedule builder should succeed with valid test data");

    // Build a custom schedule separately
    let custom_schedule = CashFlowSchedule::builder()
        .principal(Money::new(1_000_000.0, Currency::USD), issue, maturity)
        .fixed_cf(FixedCouponSpec {
            coupon_type: CouponType::Cash,
            rate: Decimal::try_from(0.055).expect("valid"), // Different from default spec
            freq: Tenor::quarterly(),
            dc: DayCount::Act365F,
            bdc: BusinessDayConvention::Following,
            calendar_id: "weekends_only".to_string(),
            stub: StubKind::None,
            end_of_month: false,
            payment_lag_days: 0,
        })
        .build_with_curves(None)
        .expect("CashFlowSchedule builder should succeed with valid test data");

    // Apply custom cashflows
    bond = bond.with_cashflows(custom_schedule);

    assert!(bond.custom_cashflows.is_some());
    // The original cashflow_spec is preserved but custom_cashflows takes precedence
}

#[test]
fn test_custom_cashflows_override_regular_generation() {
    let issue = Date::from_calendar_date(2025, Month::June, 1).expect("Valid test date");
    let maturity = Date::from_calendar_date(2026, Month::June, 1).expect("Valid test date");

    // Create bond with regular specs (builder)
    let regular_bond = Bond::builder()
        .id(InstrumentId::new("TEST"))
        .notional(Money::new(1_000_000.0, Currency::USD))
        .issue_date(issue)
        .maturity(maturity)
        .cashflow_spec(
            CashflowSpec::fixed(0.03, Tenor::annual(), DayCount::Act365F)
                .expect("finite test coupon"),
        )
        .discount_curve_id(CurveId::new("USD-OIS"))
        .credit_curve_id_opt(None)
        .pricing_overrides(PricingOverrides::default())
        .call_put_opt(None)
        .custom_cashflows_opt(None)
        .attributes(Attributes::new())
        .settlement_convention_opt(None)
        .build()
        .expect("CashFlowSchedule builder should succeed with valid test data");

    // Same bond with custom cashflows
    let custom_schedule = CashFlowSchedule::builder()
        .principal(Money::new(1_000_000.0, Currency::USD), issue, maturity)
        .fixed_cf(FixedCouponSpec {
            coupon_type: CouponType::Cash,
            rate: Decimal::try_from(0.05).expect("valid"), // Different rate
            freq: Tenor::semi_annual(),                    // Different frequency
            dc: DayCount::Act365F,
            bdc: BusinessDayConvention::Following,
            calendar_id: "weekends_only".to_string(),
            stub: StubKind::None,
            end_of_month: false,
            payment_lag_days: 0,
        })
        .build_with_curves(None)
        .expect("CashFlowSchedule builder should succeed with valid test data");

    let custom_bond = regular_bond.clone().with_cashflows(custom_schedule);

    // Create curves
    let disc_curve = DiscountCurve::builder("USD-OIS")
        .base_date(issue)
        .knots([(0.0, 1.0), (2.0, 0.98)])
        .interp(finstack_quant_core::math::interp::InterpStyle::Linear)
        .build()
        .expect("CashFlowSchedule builder should succeed with valid test data");
    let curves = MarketContext::new().insert(disc_curve);

    // Build schedules
    let regular_flows = regular_bond
        .dated_cashflows(&curves, issue)
        .expect("Schedule building should succeed in test");
    let custom_flows = custom_bond
        .dated_cashflows(&curves, issue)
        .expect("Schedule building should succeed in test");

    // Should have different number of flows due to different frequency
    assert_ne!(regular_flows.len(), custom_flows.len());

    // Custom bond should have semi-annual flows (more flows)
    assert!(custom_flows.len() > regular_flows.len());
}

#[test]
fn test_bond_floating_value() {
    let issue = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");
    let maturity = Date::from_calendar_date(2027, Month::January, 1).expect("Valid test date");
    let notional = Money::new(1_000_000.0, Currency::USD);

    // Curves
    let disc = DiscountCurve::builder("USD-OIS")
        .base_date(issue)
        .knots([(0.0, 1.0), (2.0, 0.95)])
        .interp(InterpStyle::Linear)
        .build()
        .expect("CashFlowSchedule builder should succeed with valid test data");
    let fwd = ForwardCurve::builder("USD-SOFR-3M", 0.25)
        .base_date(issue)
        .knots([(0.0, 0.05), (2.0, 0.055)])
        .interp(InterpStyle::Linear)
        .build()
        .expect("CashFlowSchedule builder should succeed with valid test data");
    let ctx = MarketContext::new().insert(disc).insert(fwd);

    let bond = Bond::floating(
        "FRN-TEST",
        notional,
        "USD-SOFR-3M",
        150,
        issue,
        maturity,
        Tenor::quarterly(),
        DayCount::Act360,
        "USD-OIS",
    )
    .unwrap();

    // Price should be finite and positive under positive forwards
    let pv = bond
        .value(&ctx, issue)
        .expect("Bond valuation should succeed in test");
    assert!(pv.amount().is_finite());
}

#[test]
fn test_bond_frn_ex_coupon_accrual_negative_in_window() {
    use crate::cashflow::primitives::CFKind;
    use time::Duration;

    let issue = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");
    let maturity = Date::from_calendar_date(2027, Month::January, 1).expect("Valid test date");
    let notional = Money::new(1_000_000.0, Currency::USD);

    // Curves
    let disc = DiscountCurve::builder("USD-OIS")
        .base_date(issue)
        .knots([(0.0, 1.0), (2.0, 0.95)])
        .interp(InterpStyle::Linear)
        .build()
        .expect("DiscountCurve builder should succeed in test");
    let fwd = ForwardCurve::builder("USD-SOFR-3M", 0.25)
        .base_date(issue)
        .day_count(DayCount::Act360)
        .knots([(0.0, 0.05), (2.0, 0.055)])
        .interp(InterpStyle::Linear)
        .build()
        .expect("ForwardCurve builder should succeed in test");
    let ctx = MarketContext::new().insert(disc).insert(fwd);

    let mut bond = Bond::floating(
        "FRN-EX-COUPON",
        notional,
        "USD-SOFR-3M",
        150,
        issue,
        maturity,
        Tenor::quarterly(),
        DayCount::Act360,
        "USD-OIS",
    )
    .unwrap();
    bond.settlement_convention = Some(BondSettlementConvention {
        ex_coupon_days: 5,
        ..Default::default()
    });

    // Use the full schedule to locate the first coupon end date
    let full_schedule = bond
        .full_cashflow_schedule(&ctx)
        .expect("Full schedule retrieval should succeed in test");
    let first_coupon_date = full_schedule
        .flows
        .iter()
        .filter(|cf| matches!(cf.kind, CFKind::Fixed | CFKind::FloatReset | CFKind::Stub))
        .map(|cf| cf.date)
        .filter(|d| *d > issue)
        .min()
        .expect("FRN should have at least one coupon date in test");

    let ex_date = first_coupon_date - Duration::days(5);
    let day_before_ex = ex_date - Duration::days(1);

    // Before ex-date, accrued interest should be positive
    let schedule_before = bond
        .full_cashflow_schedule(&ctx)
        .expect("Full schedule should build");
    let ai_before = crate::cashflow::accrual::accrued_interest_amount(
        &schedule_before,
        day_before_ex,
        &bond.accrual_config(),
    )
    .expect("Accrued interest calculation should succeed before ex-date");
    assert!(
        ai_before > 0.0,
        "Accrued interest should be positive before ex-date"
    );

    // On or inside the ex-coupon window, accrued interest is negative: the
    // buyer forgoes the upcoming coupon but compensates the seller for the
    // stub from settlement to period end.
    let schedule_ex = bond
        .full_cashflow_schedule(&ctx)
        .expect("Full schedule should build");
    let ai_ex = crate::cashflow::accrual::accrued_interest_amount(
        &schedule_ex,
        ex_date,
        &bond.accrual_config(),
    )
    .expect("Accrued interest calculation should succeed on ex-date");
    assert!(
        ai_ex < 0.0,
        "Accrued interest in ex-coupon window should be negative, got {}",
        ai_ex
    );
}

#[test]
fn test_amortizing_bond_ex_coupon_accrual_negative_in_window() {
    use crate::cashflow::builder::AmortizationSpec;
    use crate::cashflow::primitives::CFKind;
    use time::Duration;

    let issue = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");
    let maturity = Date::from_calendar_date(2028, Month::January, 1).expect("Valid test date");
    let notional = Money::new(1_000_000.0, Currency::USD);

    // Amortizing bond with annual 5% coupon, 1/3 principal returned each year.
    // StepRemaining schedule specifies remaining balance AFTER each date.
    // After step1: 2/3 remaining (paid 1/3), after step2: 1/3 remaining (paid 2/3),
    // after maturity: 0 remaining (all paid).
    let step1 = Date::from_calendar_date(2026, Month::January, 1).expect("Valid test date");
    let step2 = Date::from_calendar_date(2027, Month::January, 1).expect("Valid test date");
    let amort_spec = AmortizationSpec::StepRemaining {
        schedule: vec![
            (step1, Money::new(2.0 * 1_000_000.0 / 3.0, Currency::USD)), // 2/3 remaining
            (step2, Money::new(1_000_000.0 / 3.0, Currency::USD)),       // 1/3 remaining
            (maturity, Money::new(0.0, Currency::USD)),                  // 0 remaining
        ],
    };
    let base_spec =
        CashflowSpec::fixed(0.05, Tenor::annual(), DayCount::Act365F).expect("finite test coupon");
    let cashflow_spec = CashflowSpec::amortizing(base_spec, amort_spec);

    let mut bond = Bond::builder()
        .id("AMORT-EX-COUPON".into())
        .notional(notional)
        .issue_date(issue)
        .maturity(maturity)
        .cashflow_spec(cashflow_spec)
        .discount_curve_id(CurveId::new("USD-OIS"))
        .pricing_overrides(PricingOverrides::default())
        .attributes(Attributes::new())
        .build()
        .expect("Amortizing bond construction should succeed in test");

    bond.settlement_convention = Some(BondSettlementConvention {
        ex_coupon_days: 7,
        ..Default::default()
    });

    // Curves for pricing (levels are not important for accrual)
    let disc_curve = DiscountCurve::builder("USD-OIS")
        .base_date(issue)
        .knots([(0.0, 1.0), (3.0, 0.9)])
        .interp(InterpStyle::Linear)
        .build()
        .expect("DiscountCurve builder should succeed in test");
    let ctx = MarketContext::new().insert(disc_curve);

    // Use the full schedule to locate the first coupon end date
    let full_schedule = bond
        .full_cashflow_schedule(&ctx)
        .expect("Full schedule retrieval should succeed in test");
    let first_coupon_date = full_schedule
        .flows
        .iter()
        .filter(|cf| matches!(cf.kind, CFKind::Fixed | CFKind::Stub))
        .map(|cf| cf.date)
        .filter(|d| *d > issue)
        .min()
        .expect("Amortizing bond should have at least one coupon date in test");

    let ex_date = first_coupon_date - Duration::days(7);
    let day_before_ex = ex_date - Duration::days(1);

    // Before ex-date, accrued interest should be positive
    let schedule_before = bond
        .full_cashflow_schedule(&ctx)
        .expect("Full schedule should build");
    let ai_before = crate::cashflow::accrual::accrued_interest_amount(
        &schedule_before,
        day_before_ex,
        &bond.accrual_config(),
    )
    .expect("Accrued interest calculation should succeed before ex-date");
    assert!(
        ai_before > 0.0,
        "Accrued interest should be positive before ex-date for amortizing bond"
    );

    // On or inside the ex-coupon window, accrued interest is negative: the
    // buyer forgoes the upcoming coupon but compensates the seller for the
    // stub from settlement to period end.
    let schedule_ex = bond
        .full_cashflow_schedule(&ctx)
        .expect("Full schedule should build");
    let ai_ex = crate::cashflow::accrual::accrued_interest_amount(
        &schedule_ex,
        ex_date,
        &bond.accrual_config(),
    )
    .expect("Accrued interest calculation should succeed on ex-date");
    assert!(
        ai_ex < 0.0,
        "Accrued interest in ex-coupon window should be negative for amortizing bond, got {}",
        ai_ex
    );
}

#[test]
fn test_bond_frn_dated_cashflows_uses_builder() {
    use crate::cashflow::primitives::CFKind;
    use crate::cashflow::traits::CashflowProvider;

    let issue = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");
    let maturity = Date::from_calendar_date(2026, Month::January, 1).expect("Valid test date");

    // Create FRN
    let frn = Bond::floating(
        "FRN-BUILDER-TEST",
        Money::new(1_000_000.0, Currency::USD),
        "USD-SOFR",
        100,
        issue,
        maturity,
        Tenor::quarterly(),
        DayCount::Act360,
        "USD-OIS",
    )
    .unwrap();

    // Create market with forward curve
    let disc_curve = DiscountCurve::builder("USD-OIS")
        .base_date(issue)
        .knots([(0.0, 1.0), (1.0, 0.95)])
        .interp(InterpStyle::Linear)
        .build()
        .expect("CashFlowSchedule builder should succeed with valid test data");

    let fwd_curve = ForwardCurve::builder("USD-SOFR", 0.25)
        .base_date(issue)
        .day_count(DayCount::Act360)
        .knots([(0.0, 0.03), (1.0, 0.035)])
        .build()
        .expect("CashFlowSchedule builder should succeed with valid test data");

    let market = MarketContext::new().insert(disc_curve).insert(fwd_curve);

    // Get full schedule to verify it includes FloatReset CFKind
    let full_schedule = frn
        .full_cashflow_schedule(&market)
        .expect("Full schedule retrieval should succeed in test");
    let has_floating = full_schedule
        .flows
        .iter()
        .any(|cf| matches!(cf.kind, CFKind::FloatReset));
    assert!(
        has_floating,
        "Full schedule should include CFKind::FloatReset for FRN"
    );

    // The holder-view dated cashflow projection should flatten the canonical schedule.
    let flows = frn
        .dated_cashflows(&market, issue)
        .expect("Schedule building should succeed in test");
    assert!(!flows.is_empty(), "FRN should have cashflows");

    // Verify flows include floating coupons (should be > just redemption)
    assert!(
        flows.len() > 1,
        "FRN should have coupon flows + redemption, got {} flows",
        flows.len()
    );
}

#[test]
fn test_bond_amortization_signed_schedule_preserves_all_flows() {
    use crate::cashflow::builder::AmortizationSpec;
    use crate::cashflow::primitives::CFKind;
    use crate::cashflow::traits::CashflowProvider;

    let issue = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");
    let maturity = Date::from_calendar_date(2027, Month::January, 1).expect("Valid test date");

    // Create amortizing bond using CashflowSpec::Amortizing
    let step1 = Date::from_calendar_date(2026, Month::January, 1).expect("Valid test date");
    let amort_spec = AmortizationSpec::StepRemaining {
        schedule: vec![
            (step1, Money::new(500_000.0, Currency::USD)),
            (maturity, Money::new(0.0, Currency::USD)),
        ],
    };
    let base_spec = CashflowSpec::fixed(0.05, Tenor::semi_annual(), DayCount::Thirty360)
        .expect("finite test coupon");
    let cashflow_spec = CashflowSpec::amortizing(base_spec, amort_spec);

    let bond = Bond::builder()
        .id("AMORT-TEST".into())
        .notional(Money::new(1_000_000.0, Currency::USD))
        .issue_date(issue)
        .maturity(maturity)
        .cashflow_spec(cashflow_spec)
        .discount_curve_id(CurveId::new("USD-OIS"))
        .pricing_overrides(PricingOverrides::default())
        .attributes(Attributes::new())
        .build()
        .expect("CashFlowSchedule builder should succeed with valid test data");

    let disc_curve = DiscountCurve::builder("USD-OIS")
        .base_date(issue)
        .knots([(0.0, 1.0), (2.0, 0.95)])
        .interp(InterpStyle::Linear)
        .build()
        .expect("CashFlowSchedule builder should succeed with valid test data");
    let market = MarketContext::new().insert(disc_curve);

    // Get full schedule to check internal representation
    let full_schedule = bond
        .full_cashflow_schedule(&market)
        .expect("Full schedule retrieval should succeed in test");

    // Find initial notional (should be negative - issuer receives)
    let initial_notional = full_schedule
        .flows
        .iter()
        .find(|cf| cf.date == issue && matches!(cf.kind, CFKind::Notional));
    assert!(
        initial_notional.is_some(),
        "Full schedule should have initial notional"
    );
    assert!(
        initial_notional
            .expect("Initial notional should exist")
            .amount
            .amount()
            < 0.0,
        "Initial notional should be negative (issuer receives)"
    );

    // Find amortization flows (should be positive in full schedule)
    let amort_flows: Vec<_> = full_schedule
        .flows
        .iter()
        .filter(|cf| matches!(cf.kind, CFKind::Amortization))
        .collect();
    assert!(!amort_flows.is_empty(), "Should have amortization flows");
    for cf in &amort_flows {
        assert!(
            cf.amount.amount() > 0.0,
            "Amortization in full schedule should be positive"
        );
    }

    // The signed schedule preserves all flows including negative notionals.
    let flows = bond
        .dated_cashflows(&market, issue)
        .expect("Schedule building should succeed in test");

    // Initial draw should be present (negative notional)
    let has_negative_initial = flows.iter().any(|(d, m)| *d == issue && m.amount() < 0.0);
    assert!(
        has_negative_initial,
        "Signed schedule preserves the initial negative notional draw"
    );

    // Amortization should still appear as positive principal repayments
    let amort_in_schedule: Vec<_> = flows
        .iter()
        .filter(|(d, _)| *d == step1 || *d == maturity)
        .collect();
    let has_positive_amort = amort_in_schedule.iter().any(|(_, m)| m.amount() > 0.0);
    assert!(
        has_positive_amort,
        "Amortization in signed schedule should be positive (principal repayment)"
    );

    // Final redemption at maturity: the maturity date can include coupon,
    // amortization, and/or redemption flows with both positive and negative signs.
}

#[test]
fn test_amortizing_bond_pv_greater_than_bullet_for_same_yield() {
    use crate::instruments::common_impl::traits::Instrument;

    let issue = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");
    let maturity = Date::from_calendar_date(2028, Month::January, 1).expect("Valid test date");

    let notional = Money::new(1_000_000.0, Currency::USD);

    // Common discount curve: flat-ish, just needs to be decreasing
    let disc_curve = DiscountCurve::builder("USD-OIS")
        .base_date(issue)
        .knots([(0.0, 1.0), (1.0, 0.97), (2.0, 0.94), (3.0, 0.91)])
        .interp(InterpStyle::Linear)
        .build()
        .expect("DiscountCurve builder should succeed in test");
    let market = MarketContext::new().insert(disc_curve);

    // Bullet bond: 3-year annual, 1% coupon, full principal at maturity
    let bullet_cashflow_spec =
        CashflowSpec::fixed(0.01, Tenor::annual(), DayCount::Act365F).expect("finite test coupon");
    let bullet_bond = Bond::builder()
        .id("BULLET-TEST".into())
        .notional(notional)
        .issue_date(issue)
        .maturity(maturity)
        .cashflow_spec(bullet_cashflow_spec)
        .discount_curve_id(CurveId::new("USD-OIS"))
        .pricing_overrides(PricingOverrides::default())
        .attributes(Attributes::new())
        .build()
        .expect("Bullet bond construction should succeed in test");

    // Amortizing bond with same coupon but 1/3 principal returned each year.
    // StepRemaining schedule specifies remaining balance AFTER each date.
    // After step1: 2/3 remaining (paid 1/3), after step2: 1/3 remaining (paid 2/3).
    let amort_step1 = Date::from_calendar_date(2026, Month::January, 1).expect("Valid test date");
    let amort_step2 = Date::from_calendar_date(2027, Month::January, 1).expect("Valid test date");
    let amort_schedule = AmortizationSpec::StepRemaining {
        schedule: vec![
            (
                amort_step1,
                Money::new(2.0 * 1_000_000.0 / 3.0, Currency::USD), // 2/3 remaining
            ),
            (amort_step2, Money::new(1_000_000.0 / 3.0, Currency::USD)), // 1/3 remaining
            (maturity, Money::new(0.0, Currency::USD)),                  // 0 remaining
        ],
    };
    let amort_base_spec =
        CashflowSpec::fixed(0.01, Tenor::annual(), DayCount::Act365F).expect("finite test coupon");
    let amort_spec = CashflowSpec::amortizing(amort_base_spec, amort_schedule);
    let amort_bond = Bond::builder()
        .id("AMORT-TEST-PV".into())
        .notional(notional)
        .issue_date(issue)
        .maturity(maturity)
        .cashflow_spec(amort_spec)
        .discount_curve_id(CurveId::new("USD-OIS"))
        .pricing_overrides(PricingOverrides::default())
        .attributes(Attributes::new())
        .build()
        .expect("Amortizing bond construction should succeed in test");

    let pv_bullet = bullet_bond
        .value(&market, issue)
        .expect("Bullet bond valuation should succeed in test")
        .amount();
    let pv_amort = amort_bond
        .value(&market, issue)
        .expect("Amortizing bond valuation should succeed in test")
        .amount();

    // With earlier principal repayments and a coupon below the curve's
    // effective yield, the amortizing bond should have a higher PV than
    // the bullet (principal is returned sooner and reinvested at higher
    // rates).
    assert!(
        pv_amort > pv_bullet,
        "Amortizing bond PV ({}) should be greater than bullet PV ({}) for the same yield curve",
        pv_amort,
        pv_bullet
    );
}
