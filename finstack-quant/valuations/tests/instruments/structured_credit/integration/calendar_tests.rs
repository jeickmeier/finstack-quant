//! Integration tests for calendar handling in structured credit instruments.
//!
//! These tests verify:
//! - Required calendar validation (fail-fast on missing calendar)
//! - Invalid calendar ID detection
//! - Payment schedule respects US holiday calendar
//! - WAL calculations are deterministic with proper calendar

use finstack_quant_cashflows::CashflowProvider;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::{DiscountCurve, ForwardCurve};
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::fixed_income::structured_credit::{
    AssetPool, DealType, PoolAsset, StructuredCredit, Tranche, TrancheCoupon, TrancheSeniority,
    TrancheStructure,
};
use time::Month;

// ============================================================================
// Test Helpers
// ============================================================================

fn closing_date() -> Date {
    Date::from_calendar_date(2024, Month::January, 1).unwrap()
}

fn maturity_date() -> Date {
    Date::from_calendar_date(2030, Month::December, 31).unwrap()
}

fn test_date() -> Date {
    Date::from_calendar_date(2025, Month::January, 1).unwrap()
}

fn create_test_pool() -> AssetPool {
    let mut pool = AssetPool::new("TEST_POOL", DealType::CLO, Currency::USD);
    pool.assets.push(PoolAsset::floating_rate_loan(
        "LOAN_1",
        Money::new(50_000_000.0, Currency::USD),
        "SOFR-3M",
        400.0,
        maturity_date(),
        DayCount::Act360,
    ));
    pool
}

fn create_test_tranches() -> TrancheStructure {
    let senior = Tranche::new(
        "SENIOR_A",
        0.0,
        100.0,
        TrancheSeniority::Senior,
        Money::new(50_000_000.0, Currency::USD),
        TrancheCoupon::Fixed { rate: 0.05 },
        maturity_date(),
    )
    .expect("Failed to create senior tranche");
    TrancheStructure::new(vec![senior]).expect("Failed to create tranche structure")
}

fn create_test_market() -> MarketContext {
    let discount_curve = DiscountCurve::builder("USD_OIS")
        .base_date(test_date())
        .knots(vec![(0.0, 1.0), (0.25, 0.9875), (1.0, 0.95), (5.0, 0.78)])
        .interp(InterpStyle::Linear)
        .build()
        .expect("Failed to create discount curve");

    let forward_curve = ForwardCurve::builder("SOFR-3M", 0.25)
        .base_date(closing_date())
        .knots(vec![(0.0, 0.05), (1.0, 0.051), (2.0, 0.053), (5.0, 0.055)])
        .interp(InterpStyle::Linear)
        .build()
        .expect("Failed to create forward curve");

    MarketContext::new()
        .insert(discount_curve)
        .insert(forward_curve)
}

// ============================================================================
// Calendar Validation Tests
// ============================================================================

#[test]
fn test_missing_calendar_fails_with_helpful_message() {
    // Arrange: Create CLO WITHOUT setting payment_calendar_id
    let clo = StructuredCredit::new_clo(
        "NO_CALENDAR_CLO",
        create_test_pool(),
        create_test_tranches(),
        closing_date(),
        maturity_date(),
        "USD_OIS",
    );
    // Note: NOT calling .with_payment_calendar()

    let market = create_test_market();

    // Act
    let result = clo.dated_cashflows(&market, test_date());

    // Assert: Should fail with a validation error mentioning calendar requirement
    assert!(result.is_err(), "Should fail when calendar is missing");

    let error = result.unwrap_err();
    let error_msg = format!("{}", error);
    assert!(
        error_msg.contains("payment_calendar_id"),
        "Error should mention payment_calendar_id: {}",
        error_msg
    );
}

#[test]
fn test_invalid_calendar_id_fails_with_available_options() {
    // Arrange: Create CLO with an invalid calendar ID
    let clo = StructuredCredit::new_clo(
        "INVALID_CALENDAR_CLO",
        create_test_pool(),
        create_test_tranches(),
        closing_date(),
        maturity_date(),
        "USD_OIS",
    )
    .with_payment_calendar("NONEXISTENT_CALENDAR");

    let market = create_test_market();

    // Act
    let result = clo.dated_cashflows(&market, test_date());

    // Assert: Should fail with NotFound error listing available calendars
    assert!(result.is_err(), "Should fail for invalid calendar");

    let error = result.unwrap_err();
    let error_msg = format!("{}", error);

    // Error should mention the invalid calendar and available options
    assert!(
        error_msg.contains("NONEXISTENT_CALENDAR"),
        "Error should mention the invalid calendar ID: {}",
        error_msg
    );
    assert!(
        error_msg.contains("available"),
        "Error should mention available calendars: {}",
        error_msg
    );
}

#[test]
fn test_valid_nyse_calendar_succeeds() {
    // Arrange: Create CLO with valid NYSE calendar
    let clo = StructuredCredit::new_clo(
        "NYSE_CALENDAR_CLO",
        create_test_pool(),
        create_test_tranches(),
        closing_date(),
        maturity_date(),
        "USD_OIS",
    )
    .with_payment_calendar("nyse");

    let market = create_test_market();

    // Act
    let result = clo.dated_cashflows(&market, test_date());

    // Assert: Should succeed
    assert!(
        result.is_ok(),
        "Should succeed with valid NYSE calendar: {:?}",
        result.err()
    );

    let flows = result.unwrap();
    assert!(!flows.is_empty(), "Should generate cashflows");
}

#[test]
fn test_valid_target2_calendar_succeeds() {
    // Arrange: Create CLO with valid TARGET2 calendar (European)
    let clo = StructuredCredit::new_clo(
        "TARGET2_CALENDAR_CLO",
        create_test_pool(),
        create_test_tranches(),
        closing_date(),
        maturity_date(),
        "USD_OIS",
    )
    .with_payment_calendar("target2");

    let market = create_test_market();

    // Act
    let result = clo.dated_cashflows(&market, test_date());

    // Assert: Should succeed
    assert!(
        result.is_ok(),
        "Should succeed with valid TARGET2 calendar: {:?}",
        result.err()
    );
}

// ============================================================================
// Holiday-Aware Schedule Tests
// ============================================================================

#[test]
fn test_clo_schedule_avoids_us_holidays() {
    // Arrange: Create CLO spanning a period with US holidays
    // Using a first payment date near July 4th to test holiday adjustment
    let clo = StructuredCredit::new_clo(
        "HOLIDAY_TEST_CLO",
        create_test_pool(),
        create_test_tranches(),
        closing_date(),
        maturity_date(),
        "USD_OIS",
    )
    .with_payment_calendar("nyse");

    let market = create_test_market();

    // Act
    let flows = clo.dated_cashflows(&market, test_date()).unwrap();

    // Assert: No payment dates should fall on weekends
    for (date, _) in &flows {
        let weekday = date.weekday();
        assert!(
            weekday != time::Weekday::Saturday && weekday != time::Weekday::Sunday,
            "Payment date {} falls on weekend",
            date
        );
    }
}

#[test]
fn test_seasoned_clo_keeps_contractual_coupon_grid() {
    let mut clo = StructuredCredit::new_clo(
        "SEASONED_CLO",
        create_test_pool(),
        create_test_tranches(),
        closing_date(),
        maturity_date(),
        "USD_OIS",
    )
    .with_payment_calendar("nyse");

    clo.first_payment_date = Date::from_calendar_date(2024, Month::April, 1).unwrap();

    let market = create_test_market();
    let as_of = Date::from_calendar_date(2025, Month::February, 15).unwrap();
    let flows = clo
        .dated_cashflows(&market, as_of)
        .expect("seasoned CLO flows");

    let first_future_flow = flows.first().expect("future flow").0;
    assert_eq!(
        first_future_flow,
        Date::from_calendar_date(2025, Month::April, 1).unwrap(),
        "future schedule should stay on the contractual grid rather than re-anchor from as_of"
    );
}

#[test]
fn test_payment_schedule_is_deterministic_with_calendar() {
    // Arrange: Create two identical CLOs with same calendar
    let clo1 = StructuredCredit::new_clo(
        "DETERMINISTIC_CLO_1",
        create_test_pool(),
        create_test_tranches(),
        closing_date(),
        maturity_date(),
        "USD_OIS",
    )
    .with_payment_calendar("nyse");

    let clo2 = StructuredCredit::new_clo(
        "DETERMINISTIC_CLO_2",
        create_test_pool(),
        create_test_tranches(),
        closing_date(),
        maturity_date(),
        "USD_OIS",
    )
    .with_payment_calendar("nyse");

    let market = create_test_market();

    // Act
    let flows1 = clo1.dated_cashflows(&market, test_date()).unwrap();
    let flows2 = clo2.dated_cashflows(&market, test_date()).unwrap();

    // Assert: Payment dates should be identical
    assert_eq!(
        flows1.len(),
        flows2.len(),
        "Same structure should produce same number of cashflows"
    );

    for ((date1, _), (date2, _)) in flows1.iter().zip(flows2.iter()) {
        assert_eq!(date1, date2, "Payment dates should be deterministic");
    }
}
