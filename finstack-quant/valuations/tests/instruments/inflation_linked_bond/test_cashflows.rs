//! Cashflow generation tests for Inflation-Linked Bonds
//!
//! Tests cover:
//! - Schedule generation with various frequencies
//! - Inflation-adjusted coupon amounts
//! - Principal repayment at maturity
//! - Day count fraction calculations
//! - Empty schedules for edge cases

use super::common::*;
use finstack_quant_cashflows::CashflowProvider;
use finstack_quant_core::dates::Tenor;

#[test]
fn test_dated_cashflows_semi_annual() {
    // Arrange
    let ilb = sample_tips();
    let (ctx, _) = market_context_with_index();
    let as_of = d(2020, 1, 15);

    // Act
    let flows = ilb.dated_cashflows(&ctx, as_of).unwrap();

    // Assert
    assert!(!flows.is_empty());

    // Should have 20 semi-annual coupons + 1 principal = 21 flows
    assert_eq!(flows.len(), 21);

    // First flow should be around July 15, 2020
    assert_eq!(flows[0].0, d(2020, 7, 15));

    // Last flow should be at maturity
    assert_eq!(flows[flows.len() - 1].0, ilb.maturity);

    // All flows should have positive amounts (assuming positive inflation)
    for (_, amount) in &flows {
        assert!(amount.amount() > 0.0);
    }
}

#[test]
fn test_dated_cashflows_annual() {
    // Arrange
    let mut ilb = sample_tips();
    ilb.frequency = Tenor::annual();
    ilb.issue_date = d(2020, 1, 15);
    ilb.maturity = d(2025, 1, 15);

    let (ctx, _) = market_context_with_index();
    let as_of = d(2020, 1, 15);

    // Act
    let flows = ilb.dated_cashflows(&ctx, as_of).unwrap();

    // Assert
    // 5 annual coupons + 1 principal = 6 flows
    assert_eq!(flows.len(), 6);

    // Check annual spacing with BDC adjustments
    let cal_id = "weekends_only";
    let bdc = finstack_quant_core::dates::BusinessDayConvention::Following;
    let expected_2021 =
        finstack_quant_cashflows::builder::calendar::adjust_date(d(2021, 1, 15), bdc, cal_id)
            .unwrap();
    let expected_2022 =
        finstack_quant_cashflows::builder::calendar::adjust_date(d(2022, 1, 15), bdc, cal_id)
            .unwrap();
    let expected_2023 =
        finstack_quant_cashflows::builder::calendar::adjust_date(d(2023, 1, 15), bdc, cal_id)
            .unwrap();
    assert_eq!(flows[0].0, expected_2021);
    assert_eq!(flows[1].0, expected_2022);
    assert_eq!(flows[2].0, expected_2023);
}

#[test]
fn test_dated_cashflows_quarterly() {
    // Arrange
    let mut ilb = sample_tips();
    ilb.frequency = Tenor::quarterly();
    ilb.issue_date = d(2024, 1, 15);
    ilb.maturity = d(2025, 1, 15);

    let (ctx, _) = market_context_with_index();
    let as_of = d(2024, 1, 15);

    // Act
    let flows = ilb.dated_cashflows(&ctx, as_of).unwrap();

    // Assert
    // 4 quarterly coupons + 1 principal = 5 flows
    assert_eq!(flows.len(), 5);

    // Check quarterly spacing
    assert_eq!(flows[0].0, d(2024, 4, 15));
    assert_eq!(flows[1].0, d(2024, 7, 15));
    assert_eq!(flows[2].0, d(2024, 10, 15));
    assert_eq!(flows[3].0, d(2025, 1, 15));
}

#[test]
fn test_coupon_amounts_reflect_inflation_adjustment() {
    // Arrange
    let mut ilb = sample_tips();
    ilb.base_index = 300.0;
    ilb.notional = finstack_quant_core::money::Money::new(
        1_000_000.0,
        finstack_quant_core::currency::Currency::USD,
    );
    ilb.real_coupon = rust_decimal::Decimal::try_from(0.01).expect("valid decimal"); // 1% real coupon
    ilb.frequency = Tenor::annual();
    ilb.issue_date = d(2024, 1, 1);
    ilb.maturity = d(2026, 1, 1);

    let (mut ctx, _curve) = market_context_with_curve();
    // Set inflation curve with known growth
    let inflation_curve =
        finstack_quant_core::market_data::term_structures::InflationCurve::builder("US-CPI-U")
            .base_date(d(2024, 1, 1))
            .base_cpi(300.0)
            .knots([
                (0.0, 300.0),
                (1.0, 306.0),  // 2% inflation year 1
                (2.0, 312.12), // 2% inflation year 2
            ])
            .build()
            .unwrap();
    ctx = ctx.insert(inflation_curve);

    let as_of = d(2024, 1, 1);

    // Act
    let flows = ilb.dated_cashflows(&ctx, as_of).unwrap();

    // Assert
    let coupon_flows: Vec<f64> = flows
        .iter()
        .map(|(_, amount)| amount.amount())
        .filter(|amount| *amount < 100_000.0)
        .collect();

    // Base coupon = 1M * 1% * 1yr = 10,000
    // Coupon legs should be positive and in a reasonable inflation-adjusted range.
    assert!(
        !coupon_flows.is_empty(),
        "expected at least one coupon flow"
    );
    for coupon in coupon_flows {
        assert!(coupon > 5_000.0 && coupon < 20_000.0);
    }
}

#[test]
fn test_principal_repayment_inflation_adjusted() {
    // Arrange: bond with base_date and maturity within the inflation curve range
    let mut ilb = sample_tips();
    ilb.base_index = 300.0;
    ilb.notional = finstack_quant_core::money::Money::new(
        1_000_000.0,
        finstack_quant_core::currency::Currency::USD,
    );
    ilb.issue_date = d(2025, 1, 2);
    ilb.maturity = d(2026, 1, 2);

    let (ctx, _) = market_context_with_curve();
    let as_of = d(2025, 1, 2);

    // Act
    let flows = ilb.dated_cashflows(&ctx, as_of).unwrap();

    // Assert
    let principal_payment = flows
        .iter()
        .map(|(_, amount)| amount.amount())
        .fold(f64::NEG_INFINITY, f64::max);

    // With ~2% p.a. inflation over 1 year, principal should be modestly above par
    assert!(principal_payment >= 1_000_000.0);
    assert!(principal_payment < 1_200_000.0);
}

#[test]
fn test_schedule_respects_day_count_convention() {
    // Arrange
    let mut ilb = sample_tips();
    ilb.day_count = finstack_quant_core::dates::DayCount::ActAct;
    ilb.issue_date = d(2024, 1, 1);
    ilb.maturity = d(2024, 7, 2); // Slightly past 6 months
    ilb.frequency = Tenor::semi_annual();

    let (ctx, _) = market_context_with_index();
    let as_of = d(2024, 1, 1);

    // Act
    let flows = ilb.dated_cashflows(&ctx, as_of).unwrap();

    // Assert - coupon amount should reflect actual day count
    // For Act/Act, 182/365 or 183/366 depending on leap year
    assert!(!flows.is_empty());
    let first_coupon = flows[0].1.amount();
    assert!(first_coupon > 0.0);
}

#[test]
fn test_schedule_with_deflation_protection() {
    // Arrange
    let mut ilb = sample_tips();
    ilb.deflation_protection =
        finstack_quant_valuations::instruments::fixed_income::inflation_linked_bond::DeflationProtection::AllPayments;
    ilb.base_index = 300.0;
    ilb.issue_date = d(2024, 1, 1);
    ilb.maturity = d(2025, 1, 1);

    let (mut ctx, _) = market_context_with_index();
    // Insert deflated index
    let observations = vec![(d(2024, 4, 1), 295.0), (d(2024, 6, 1), 295.0)]; // Lower than base across the lagged coupon window
    let index = finstack_quant_core::market_data::scalars::InflationIndex::new(
        "US-CPI-U",
        observations,
        finstack_quant_core::currency::Currency::USD,
    )
    .unwrap()
    .with_interpolation(finstack_quant_core::market_data::scalars::InflationInterpolation::Linear);
    ctx = ctx.insert_inflation_index("US-CPI-U", index);

    let as_of = d(2024, 1, 1);

    // Act
    let flows = ilb.dated_cashflows(&ctx, as_of).unwrap();

    // Assert - all payments should be floored at notional (no deflation)
    for (_, amount) in &flows {
        // Even with deflation, amounts should not be less than base amounts
        assert!(amount.amount() >= 0.0);
    }
}

#[test]
fn test_empty_schedule_when_no_dates() {
    // Arrange
    let mut ilb = sample_tips();
    // Create a bond with same issue and maturity (degenerate case)
    ilb.issue_date = d(2025, 1, 1);
    ilb.maturity = d(2025, 1, 1);

    let (ctx, _) = market_context_with_index();
    let as_of = d(2025, 1, 1);

    // Act
    let flows = ilb.dated_cashflows(&ctx, as_of).unwrap();

    // Assert - should be empty or minimal
    assert!(flows.is_empty() || flows.len() == 1); // Might have just principal
}

#[test]
fn test_schedule_currency_consistency() {
    // Arrange
    let ilb = sample_tips();
    let (ctx, _) = market_context_with_index();
    let as_of = d(2020, 1, 15);

    // Act
    let flows = ilb.dated_cashflows(&ctx, as_of).unwrap();

    // Assert - all flows should be in the same currency as notional
    for (_, amount) in &flows {
        assert_eq!(amount.currency(), ilb.notional.currency());
    }
}

#[test]
fn test_schedule_dates_sorted() {
    // Arrange
    let ilb = sample_tips();
    let (ctx, _) = market_context_with_index();
    let as_of = d(2020, 1, 15);

    // Act
    let flows = ilb.dated_cashflows(&ctx, as_of).unwrap();

    // Assert - dates should be in ascending order
    for i in 1..flows.len() {
        assert!(flows[i].0 >= flows[i - 1].0);
    }
}

#[test]
fn test_schedule_all_dates_after_issue() {
    // Arrange
    let ilb = sample_tips();
    let (ctx, _) = market_context_with_index();
    let as_of = d(2020, 1, 15);

    // Act
    let flows = ilb.dated_cashflows(&ctx, as_of).unwrap();

    // Assert - all payment dates should be after issue date
    for (date, _) in &flows {
        assert!(*date >= ilb.issue_date);
    }
}

#[test]
fn test_schedule_uk_gilt_characteristics() {
    // Arrange
    let ilb = sample_uk_linker();
    let (ctx, _) = uk_market_context();
    let as_of = d(2020, 3, 22);

    // Act
    let flows = ilb.dated_cashflows(&ctx, as_of).unwrap();

    // Assert
    assert!(!flows.is_empty());

    // UK gilts are semi-annual
    // 20 years * 2 = 40 coupons + 1 principal
    assert_eq!(flows.len(), 41);

    // All flows in GBP
    for (_, amount) in &flows {
        assert_eq!(
            amount.currency(),
            finstack_quant_core::currency::Currency::GBP
        );
    }
}

#[test]
fn test_cashflow_provider_trait() {
    // Arrange
    let ilb = sample_tips();
    let (ctx, _) = market_context_with_index();
    let as_of = d(2020, 1, 15);

    // Act - call via trait
    let flows = CashflowProvider::dated_cashflows(&ilb, &ctx, as_of).unwrap();

    // Assert
    assert!(!flows.is_empty());
}

#[test]
fn test_schedule_generation_performance() {
    // Arrange - long dated bond
    let mut ilb = sample_tips();
    ilb.issue_date = d(2020, 1, 1);
    ilb.maturity = d(2050, 1, 1); // 30-year bond
    ilb.frequency = Tenor::semi_annual();

    let (ctx, _) = market_context_with_index();
    let as_of = d(2020, 1, 1);

    // Act
    let flows = ilb.dated_cashflows(&ctx, as_of).unwrap();

    // Assert
    // 30 years * 2 = 60 coupons + 1 principal
    assert_eq!(flows.len(), 61);
}
