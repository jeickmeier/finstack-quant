//! Integration tests for deal-type specific behavior.
//!
//! Tests that CLO, ABS, RMBS, and CMBS have correct defaults and behavior.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, Tenor};
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::fixed_income::structured_credit::{
    AssetPool, DealType, PoolAsset, PrepaymentCurve, PrepaymentModelSpec, StructuredCredit,
    Tranche, TrancheCoupon, TrancheSeniority, TrancheStructure,
};
use time::Month;

fn maturity_date() -> Date {
    Date::from_calendar_date(2030, Month::December, 31).unwrap()
}

fn create_minimal_pool(deal_type: DealType) -> AssetPool {
    let mut pool = AssetPool::new("POOL", deal_type, Currency::USD);
    pool.assets.push(PoolAsset::fixed_rate_bond(
        "A1",
        Money::new(10_000_000.0, Currency::USD).expect("valid money fixture"),
        0.06,
        maturity_date(),
        finstack_quant_core::dates::DayCount::Thirty360,
    ));
    pool
}

fn create_minimal_tranches() -> TrancheStructure {
    let tranche = Tranche::new(
        "SENIOR",
        0.0,
        100.0,
        TrancheSeniority::Senior,
        Money::new(10_000_000.0, Currency::USD).expect("valid money fixture"),
        TrancheCoupon::Fixed { rate: 0.04 },
        maturity_date(),
    )
    .unwrap();
    TrancheStructure::new(vec![tranche]).unwrap()
}

// CLO-specific Tests

#[test]
fn test_clo_default_payment_frequency() {
    // Arrange & Act
    let clo = StructuredCredit::new_clo(
        "TEST_CLO",
        create_minimal_pool(DealType::Clo),
        create_minimal_tranches(),
        Date::from_calendar_date(2024, Month::January, 1).unwrap(),
        maturity_date(),
        "USD-OIS",
    );

    // Assert: CLO should default to quarterly payments
    assert_eq!(clo.frequency, Tenor::quarterly());
}

#[test]
fn test_clo_default_prepayment_model() {
    // Arrange & Act
    let clo = StructuredCredit::new_clo(
        "TEST_CLO",
        create_minimal_pool(DealType::Clo),
        create_minimal_tranches(),
        Date::from_calendar_date(2024, Month::January, 1).unwrap(),
        maturity_date(),
        "USD-OIS",
    );

    // Assert: CLO should use constant CPR
    assert_eq!(clo.credit_model.prepayment_spec.cpr, 0.15); // 15% CPR standard
    assert!(
        clo.credit_model.prepayment_spec.curve.is_none()
            || matches!(
                clo.credit_model.prepayment_spec.curve,
                Some(PrepaymentCurve::Constant)
            )
    );
}

#[test]
fn test_clo_default_assumptions() {
    // Arrange & Act
    let clo = StructuredCredit::new_clo(
        "TEST_CLO",
        create_minimal_pool(DealType::Clo),
        create_minimal_tranches(),
        Date::from_calendar_date(2024, Month::January, 1).unwrap(),
        maturity_date(),
        "USD-OIS",
    );

    // Assert: CLO standard assumptions
    assert_eq!(clo.credit_model.default_spec.cdr, 0.02); // 2% CDR
    assert_eq!(clo.credit_model.recovery_spec.rate, 0.40); // 40% recovery
    assert_eq!(clo.credit_model.prepayment_spec.cpr, 0.15); // 15% CPR
}

// ABS-specific Tests

#[test]
fn test_abs_default_payment_frequency() {
    // Arrange & Act
    let abs = StructuredCredit::new_abs(
        "TEST_ABS",
        create_minimal_pool(DealType::Abs),
        create_minimal_tranches(),
        Date::from_calendar_date(2024, Month::January, 1).unwrap(),
        maturity_date(),
        "USD-OIS",
    );

    // Assert: ABS should default to monthly payments
    assert_eq!(abs.frequency, Tenor::monthly());
}

#[test]
fn test_abs_default_assumptions() {
    // Arrange & Act
    let abs = StructuredCredit::new_abs(
        "TEST_ABS",
        create_minimal_pool(DealType::Abs),
        create_minimal_tranches(),
        Date::from_calendar_date(2024, Month::January, 1).unwrap(),
        maturity_date(),
        "USD-OIS",
    );

    // Assert: Auto ABS standard assumptions
    assert_eq!(abs.credit_model.default_spec.cdr, 0.02); // 2% CDR
    assert_eq!(abs.credit_model.recovery_spec.rate, 0.45); // 45% recovery (updated)
    assert!(
        (abs.credit_model
            .prepayment_spec
            .smm(0)
            .expect("monthly ABS speed")
            - 0.015)
            .abs()
            < 1e-14
    ); // 1.5% ABS
}

// RMBS-specific Tests

#[test]
fn test_rmbs_default_payment_frequency() {
    // Arrange & Act
    let rmbs = StructuredCredit::new_rmbs(
        "TEST_RMBS",
        create_minimal_pool(DealType::Rmbs),
        create_minimal_tranches(),
        Date::from_calendar_date(2024, Month::January, 1).unwrap(),
        maturity_date(),
        "USD-OIS",
    );

    // Assert: RMBS should default to monthly payments
    assert_eq!(rmbs.frequency, Tenor::monthly());
}

#[test]
fn test_rmbs_default_prepayment_model() {
    // Arrange & Act
    let rmbs = StructuredCredit::new_rmbs(
        "TEST_RMBS",
        create_minimal_pool(DealType::Rmbs),
        create_minimal_tranches(),
        Date::from_calendar_date(2024, Month::January, 1).unwrap(),
        maturity_date(),
        "USD-OIS",
    );

    // Assert: RMBS should use PSA model
    assert_eq!(rmbs.credit_model.prepayment_spec.cpr, 0.06); // 100% PSA terminal = 6% CPR
    match rmbs.credit_model.prepayment_spec.curve {
        Some(PrepaymentCurve::Psa { speed_multiplier }) => {
            assert_eq!(speed_multiplier, 1.0); // 100% PSA
        }
        _ => panic!("Expected PSA curve for RMBS"),
    }
}

#[test]
fn test_rmbs_default_assumptions() {
    // Arrange & Act
    let rmbs = StructuredCredit::new_rmbs(
        "TEST_RMBS",
        create_minimal_pool(DealType::Rmbs),
        create_minimal_tranches(),
        Date::from_calendar_date(2024, Month::January, 1).unwrap(),
        maturity_date(),
        "USD-OIS",
    );

    // Assert: RMBS standard assumptions
    assert_eq!(rmbs.credit_model.default_spec.cdr, 0.006); // 0.6% CDR
    assert_eq!(rmbs.credit_model.recovery_spec.rate, 0.60); // 60% recovery
    assert_eq!(
        rmbs.credit_model.prepayment_spec,
        PrepaymentModelSpec::psa(1.0)
    ); // 100% PSA
}

// CMBS-specific Tests

#[test]
fn test_cmbs_default_payment_frequency() {
    // Arrange & Act
    let cmbs = StructuredCredit::new_cmbs(
        "TEST_CMBS",
        create_minimal_pool(DealType::Cmbs),
        create_minimal_tranches(),
        Date::from_calendar_date(2024, Month::January, 1).unwrap(),
        maturity_date(),
        "USD-OIS",
    );

    // Assert: CMBS should default to monthly payments
    assert_eq!(cmbs.frequency, Tenor::monthly());
}

#[test]
fn test_cmbs_default_assumptions() {
    // Arrange & Act
    let cmbs = StructuredCredit::new_cmbs(
        "TEST_CMBS",
        create_minimal_pool(DealType::Cmbs),
        create_minimal_tranches(),
        Date::from_calendar_date(2024, Month::January, 1).unwrap(),
        maturity_date(),
        "USD-OIS",
    );

    // Assert: CMBS standard assumptions
    assert_eq!(cmbs.credit_model.default_spec.cdr, 0.005); // 0.5% CDR
    assert_eq!(cmbs.credit_model.recovery_spec.rate, 0.65); // 65% recovery
    assert_eq!(cmbs.credit_model.prepayment_spec.cpr, 0.10); // 10% CPR
}

// Cross-Instrument Consistency Tests

#[test]
fn test_all_deal_types_have_correct_classification() {
    // Arrange & Act
    let clo = StructuredCredit::new_clo(
        "CLO",
        create_minimal_pool(DealType::Clo),
        create_minimal_tranches(),
        Date::from_calendar_date(2024, Month::January, 1).unwrap(),
        maturity_date(),
        "USD-OIS",
    );

    let abs = StructuredCredit::new_abs(
        "ABS",
        create_minimal_pool(DealType::Abs),
        create_minimal_tranches(),
        Date::from_calendar_date(2024, Month::January, 1).unwrap(),
        maturity_date(),
        "USD-OIS",
    );

    let rmbs = StructuredCredit::new_rmbs(
        "RMBS",
        create_minimal_pool(DealType::Rmbs),
        create_minimal_tranches(),
        Date::from_calendar_date(2024, Month::January, 1).unwrap(),
        maturity_date(),
        "USD-OIS",
    );

    let cmbs = StructuredCredit::new_cmbs(
        "CMBS",
        create_minimal_pool(DealType::Cmbs),
        create_minimal_tranches(),
        Date::from_calendar_date(2024, Month::January, 1).unwrap(),
        maturity_date(),
        "USD-OIS",
    );

    // Assert
    assert_eq!(clo.deal_type, DealType::Clo);
    assert_eq!(abs.deal_type, DealType::Abs);
    assert_eq!(rmbs.deal_type, DealType::Rmbs);
    assert_eq!(cmbs.deal_type, DealType::Cmbs);
}
