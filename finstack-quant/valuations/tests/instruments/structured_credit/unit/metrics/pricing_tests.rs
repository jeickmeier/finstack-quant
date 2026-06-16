//! Unit tests for pricing metrics (Accrued, Dirty/Clean Price, WAL).
//!
//! Tests cover:
//! - Accrued interest calculations
//! - Dirty price calculations
//! - Clean price calculations
//! - WAL calculations from cashflows
//! - Mathematical precision and edge cases

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::fixed_income::structured_credit::TrancheCashflows;
use time::Month;

// ============================================================================
// WAL Calculation Tests
// ============================================================================

#[test]
fn test_wal_calculation_equal_payments() {
    // Arrange: Equal principal payments over 4 years
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let cashflows = TrancheCashflows {
        tranche_id: "TEST".to_string(),
        cashflows: vec![],
        detailed_flows: vec![],
        interest_flows: vec![],
        principal_flows: vec![
            (
                Date::from_calendar_date(2026, Month::January, 1).unwrap(),
                Money::new(25_000.0, Currency::USD),
            ),
            (
                Date::from_calendar_date(2027, Month::January, 1).unwrap(),
                Money::new(25_000.0, Currency::USD),
            ),
            (
                Date::from_calendar_date(2028, Month::January, 1).unwrap(),
                Money::new(25_000.0, Currency::USD),
            ),
            (
                Date::from_calendar_date(2029, Month::January, 1).unwrap(),
                Money::new(25_000.0, Currency::USD),
            ),
        ],
        pik_flows: vec![],
        final_balance: Money::new(0.0, Currency::USD),
        total_interest: Money::new(0.0, Currency::USD),
        total_principal: Money::new(100_000.0, Currency::USD),
        total_pik: Money::new(0.0, Currency::USD),
        writedown_flows: vec![],
        total_writedown: Money::new(0.0, Currency::USD),
    };

    // Act
    let wal =
        finstack_quant_valuations::instruments::fixed_income::structured_credit::calculate_tranche_wal(
            &cashflows, as_of,
        )
        .unwrap();

    // Assert: (25k×1 + 25k×2 + 25k×3 + 25k×4) / 100k = 2.5 years
    assert!((wal - 2.5).abs() < 0.01);
}

#[test]
fn test_wal_calculation_front_loaded_payments() {
    // Arrange: Most principal paid early
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let cashflows = TrancheCashflows {
        tranche_id: "TEST".to_string(),
        cashflows: vec![],
        detailed_flows: vec![],
        interest_flows: vec![],
        principal_flows: vec![
            (
                Date::from_calendar_date(2026, Month::January, 1).unwrap(),
                Money::new(70_000.0, Currency::USD),
            ),
            (
                Date::from_calendar_date(2027, Month::January, 1).unwrap(),
                Money::new(20_000.0, Currency::USD),
            ),
            (
                Date::from_calendar_date(2028, Month::January, 1).unwrap(),
                Money::new(10_000.0, Currency::USD),
            ),
        ],
        pik_flows: vec![],
        final_balance: Money::new(0.0, Currency::USD),
        total_interest: Money::new(0.0, Currency::USD),
        total_principal: Money::new(100_000.0, Currency::USD),
        total_pik: Money::new(0.0, Currency::USD),
        writedown_flows: vec![],
        total_writedown: Money::new(0.0, Currency::USD),
    };

    // Act
    let wal =
        finstack_quant_valuations::instruments::fixed_income::structured_credit::calculate_tranche_wal(
            &cashflows, as_of,
        )
        .unwrap();

    // Assert: (70k×1 + 20k×2 + 10k×3) / 100k = 1.4 years
    assert!((wal - 1.4).abs() < 0.01);
}

#[test]
fn test_wal_calculation_back_loaded_payments() {
    // Arrange: Most principal paid late
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let cashflows = TrancheCashflows {
        tranche_id: "TEST".to_string(),
        cashflows: vec![],
        detailed_flows: vec![],
        interest_flows: vec![],
        principal_flows: vec![
            (
                Date::from_calendar_date(2026, Month::January, 1).unwrap(),
                Money::new(10_000.0, Currency::USD),
            ),
            (
                Date::from_calendar_date(2027, Month::January, 1).unwrap(),
                Money::new(20_000.0, Currency::USD),
            ),
            (
                Date::from_calendar_date(2028, Month::January, 1).unwrap(),
                Money::new(70_000.0, Currency::USD),
            ),
        ],
        pik_flows: vec![],
        final_balance: Money::new(0.0, Currency::USD),
        total_interest: Money::new(0.0, Currency::USD),
        total_principal: Money::new(100_000.0, Currency::USD),
        total_pik: Money::new(0.0, Currency::USD),
        writedown_flows: vec![],
        total_writedown: Money::new(0.0, Currency::USD),
    };

    // Act
    let wal =
        finstack_quant_valuations::instruments::fixed_income::structured_credit::calculate_tranche_wal(
            &cashflows, as_of,
        )
        .unwrap();

    // Assert: (10k×1 + 20k×2 + 70k×3) / 100k = 2.6 years
    assert!((wal - 2.6).abs() < 0.01);
}

#[test]
fn test_wal_calculation_single_payment() {
    // Arrange: Bullet payment
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let cashflows = TrancheCashflows {
        tranche_id: "TEST".to_string(),
        cashflows: vec![],
        detailed_flows: vec![],
        interest_flows: vec![],
        principal_flows: vec![(
            Date::from_calendar_date(2030, Month::January, 1).unwrap(),
            Money::new(100_000.0, Currency::USD),
        )],
        pik_flows: vec![],
        final_balance: Money::new(0.0, Currency::USD),
        total_interest: Money::new(0.0, Currency::USD),
        total_principal: Money::new(100_000.0, Currency::USD),
        total_pik: Money::new(0.0, Currency::USD),
        writedown_flows: vec![],
        total_writedown: Money::new(0.0, Currency::USD),
    };

    // Act
    let wal =
        finstack_quant_valuations::instruments::fixed_income::structured_credit::calculate_tranche_wal(
            &cashflows, as_of,
        )
        .unwrap();

    // Assert: Should be exactly 5 years
    assert!((wal - 5.0).abs() < 0.01);
}

#[test]
fn test_wal_calculation_empty_cashflows() {
    // Arrange: No principal payments
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let cashflows = TrancheCashflows {
        tranche_id: "TEST".to_string(),
        cashflows: vec![],
        detailed_flows: vec![],
        interest_flows: vec![],
        principal_flows: vec![],
        pik_flows: vec![],
        final_balance: Money::new(0.0, Currency::USD),
        total_interest: Money::new(0.0, Currency::USD),
        total_principal: Money::new(0.0, Currency::USD),
        total_pik: Money::new(0.0, Currency::USD),
        writedown_flows: vec![],
        total_writedown: Money::new(0.0, Currency::USD),
    };

    // Act
    let wal =
        finstack_quant_valuations::instruments::fixed_income::structured_credit::calculate_tranche_wal(
            &cashflows, as_of,
        )
        .unwrap();

    // Assert
    assert_eq!(wal, 0.0);
}

#[test]
fn test_wal_ignores_past_cashflows() {
    // Arrange: Some cashflows in the past
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let cashflows = TrancheCashflows {
        tranche_id: "TEST".to_string(),
        cashflows: vec![],
        detailed_flows: vec![],
        interest_flows: vec![],
        principal_flows: vec![
            (
                Date::from_calendar_date(2024, Month::January, 1).unwrap(),
                Money::new(50_000.0, Currency::USD), // In the past - should ignore
            ),
            (
                Date::from_calendar_date(2026, Month::January, 1).unwrap(),
                Money::new(50_000.0, Currency::USD),
            ),
        ],
        pik_flows: vec![],
        final_balance: Money::new(0.0, Currency::USD),
        total_interest: Money::new(0.0, Currency::USD),
        total_principal: Money::new(100_000.0, Currency::USD),
        total_pik: Money::new(0.0, Currency::USD),
        writedown_flows: vec![],
        total_writedown: Money::new(0.0, Currency::USD),
    };

    // Act
    let wal =
        finstack_quant_valuations::instruments::fixed_income::structured_credit::calculate_tranche_wal(
            &cashflows, as_of,
        )
        .unwrap();

    // Assert: Only future cashflow counts → 1 year
    assert!((wal - 1.0).abs() < 0.01);
}

// ============================================================================
// Market Standard WAL Tests
// ============================================================================

#[test]
fn test_wal_rmbs_with_psa() {
    // Arrange: Typical RMBS principal schedule (front-loaded due to prepayments)
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let cashflows = TrancheCashflows {
        tranche_id: "RMBS_AAA".to_string(),
        cashflows: vec![],
        detailed_flows: vec![],
        interest_flows: vec![],
        principal_flows: vec![
            (
                Date::from_calendar_date(2027, Month::January, 1).unwrap(),
                Money::new(40_000.0, Currency::USD),
            ),
            (
                Date::from_calendar_date(2029, Month::January, 1).unwrap(),
                Money::new(35_000.0, Currency::USD),
            ),
            (
                Date::from_calendar_date(2031, Month::January, 1).unwrap(),
                Money::new(25_000.0, Currency::USD),
            ),
        ],
        pik_flows: vec![],
        final_balance: Money::new(0.0, Currency::USD),
        total_interest: Money::new(0.0, Currency::USD),
        total_principal: Money::new(100_000.0, Currency::USD),
        total_pik: Money::new(0.0, Currency::USD),
        writedown_flows: vec![],
        total_writedown: Money::new(0.0, Currency::USD),
    };

    // Act
    let wal =
        finstack_quant_valuations::instruments::fixed_income::structured_credit::calculate_tranche_wal(
            &cashflows, as_of,
        )
        .unwrap();

    // Assert: (40k×2 + 35k×4 + 25k×6) / 100k = 3.7 years (typical for RMBS)
    assert!((wal - 3.7).abs() < 0.01);
}

#[test]
fn test_wal_clo_short_duration() {
    // Arrange: Typical CLO with faster amortization
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let cashflows = TrancheCashflows {
        tranche_id: "CLO_AAA".to_string(),
        cashflows: vec![],
        detailed_flows: vec![],
        interest_flows: vec![],
        principal_flows: vec![
            (
                Date::from_calendar_date(2027, Month::January, 1).unwrap(),
                Money::new(60_000.0, Currency::USD),
            ),
            (
                Date::from_calendar_date(2028, Month::January, 1).unwrap(),
                Money::new(40_000.0, Currency::USD),
            ),
        ],
        pik_flows: vec![],
        final_balance: Money::new(0.0, Currency::USD),
        total_interest: Money::new(0.0, Currency::USD),
        total_principal: Money::new(100_000.0, Currency::USD),
        total_pik: Money::new(0.0, Currency::USD),
        writedown_flows: vec![],
        total_writedown: Money::new(0.0, Currency::USD),
    };

    // Act
    let wal =
        finstack_quant_valuations::instruments::fixed_income::structured_credit::calculate_tranche_wal(
            &cashflows, as_of,
        )
        .unwrap();

    // Assert: (60k×2 + 40k×3) / 100k = 2.4 years (typical for CLO AAA)
    assert!((wal - 2.4).abs() < 0.01);
}
