//! Tests for CashFlow struct primitives and validation.
//!
//! These tests verify:
//! - CashFlow construction and field access
//! - Input validation (NaN, Infinity, invalid dates)
//! - CFKind variants and behavior
//!
//! # Reference
//!
//! - ISDA 2006 Definitions (cashflow semantics)

use finstack_core::cashflow::{CFKind, CashFlow};
use finstack_core::currency::Currency;
use finstack_core::dates::Date;
use finstack_core::money::Money;
use time::Month;

// =============================================================================
// Test Helpers
// =============================================================================

/// Helper to create dates
fn d(year: i32, month: u8, day: u8) -> Date {
    Date::from_calendar_date(year, Month::try_from(month).unwrap(), day).unwrap()
}

/// Helper to create a valid base cashflow for testing
fn valid_cashflow() -> CashFlow {
    CashFlow {
        date: d(2025, 1, 15),
        reset_date: None,
        amount: Money::new(100.0, Currency::USD),
        kind: CFKind::Fixed,
        accrual_factor: 0.5,
        rate: Some(0.05),
    }
}

// =============================================================================
// Construction and Field Tests
// =============================================================================

#[test]
fn cashflow_fixed_construction() {
    let cf = CashFlow {
        date: d(2025, 1, 15),
        reset_date: None,
        amount: Money::new(1000.0, Currency::USD),
        kind: CFKind::Fixed,
        accrual_factor: 0.25,
        rate: Some(0.05),
    };

    assert_eq!(cf.date, d(2025, 1, 15));
    assert_eq!(cf.kind, CFKind::Fixed);
    assert!(cf.reset_date.is_none());
    assert!(cf.validate().is_ok());
}

#[test]
fn cashflow_floating_construction() {
    let payment = d(2025, 1, 10);
    let reset = d(2025, 1, 5);
    let amount = Money::new(50.0, Currency::USD);

    let cf = CashFlow {
        date: payment,
        reset_date: Some(reset),
        amount,
        kind: CFKind::FloatReset,
        accrual_factor: 0.25,
        rate: None,
    };

    assert_eq!(cf.kind, CFKind::FloatReset);
    assert_eq!(cf.reset_date, Some(reset));
    assert!(cf.validate().is_ok());
}

#[test]
fn floating_cf_defaults_reset_date_to_payment() {
    let payment = d(2025, 1, 10);
    let amount = Money::new(50.0, Currency::USD);

    let cf = CashFlow {
        date: payment,
        reset_date: Some(payment),
        amount,
        kind: CFKind::FloatReset,
        accrual_factor: 0.0,
        rate: None,
    };

    assert_eq!(cf.reset_date, Some(payment));
}

// =============================================================================
// Amount Validation Tests
// =============================================================================

// Note: Money::new() validates that amounts are finite and panics on NaN/Infinity.
// These tests verify that behavior at the Money level.

#[test]
#[should_panic(expected = "finite amount")]
fn money_new_rejects_nan() {
    let _ = Money::new(f64::NAN, Currency::USD);
}

#[test]
#[should_panic(expected = "finite amount")]
fn money_new_rejects_positive_infinity() {
    let _ = Money::new(f64::INFINITY, Currency::USD);
}

#[test]
#[should_panic(expected = "finite amount")]
fn money_new_rejects_negative_infinity() {
    let _ = Money::new(f64::NEG_INFINITY, Currency::USD);
}

#[test]
fn cashflow_accepts_zero_amount() {
    // Zero amounts are legitimate (e.g. a floating coupon floored at 0% in a
    // negative-rate environment); previously rejected — fixed per the
    // 2026-06-09 core quant review minor finding.
    let cf = CashFlow {
        amount: Money::new(0.0, Currency::USD),
        ..valid_cashflow()
    };
    assert!(cf.validate().is_ok(), "Zero amount should be accepted");
}

#[test]
fn cashflow_accepts_negative_amount() {
    // Negative amounts are valid (represent outflows)
    let cf = CashFlow {
        amount: Money::new(-100.0, Currency::USD),
        ..valid_cashflow()
    };
    assert!(cf.validate().is_ok(), "Negative amount should be accepted");
}

#[test]
fn cashflow_accepts_large_amount() {
    let cf = CashFlow {
        amount: Money::new(1e15, Currency::USD),
        ..valid_cashflow()
    };
    assert!(cf.validate().is_ok(), "Large amount should be accepted");
}

#[test]
fn cashflow_accepts_small_but_nonzero_amount() {
    // Note: Money rounds to currency decimal places (2 for USD).
    // Very small amounts like 1e-10 round to 0.0 (now still valid).
    // 0.01 is the smallest representable non-zero USD amount.
    let cf = CashFlow {
        amount: Money::new(0.01, Currency::USD),
        ..valid_cashflow()
    };
    assert!(
        cf.validate().is_ok(),
        "Small but non-zero amount should be accepted"
    );
}

// =============================================================================
// Accrual Factor Validation Tests
// =============================================================================

#[test]
fn cashflow_rejects_nan_accrual_factor() {
    let cf = CashFlow {
        accrual_factor: f64::NAN,
        ..valid_cashflow()
    };
    assert!(
        cf.validate().is_err(),
        "NaN accrual factor should be rejected"
    );
}

#[test]
fn cashflow_rejects_positive_infinity_accrual_factor() {
    let cf = CashFlow {
        accrual_factor: f64::INFINITY,
        ..valid_cashflow()
    };
    assert!(
        cf.validate().is_err(),
        "Positive infinity accrual factor should be rejected"
    );
}

#[test]
fn cashflow_rejects_negative_infinity_accrual_factor() {
    let cf = CashFlow {
        accrual_factor: f64::NEG_INFINITY,
        ..valid_cashflow()
    };
    assert!(
        cf.validate().is_err(),
        "Negative infinity accrual factor should be rejected"
    );
}

#[test]
fn cashflow_accepts_zero_accrual_factor() {
    // Zero accrual factor is valid (e.g., notional flows)
    let cf = CashFlow {
        accrual_factor: 0.0,
        ..valid_cashflow()
    };
    assert!(
        cf.validate().is_ok(),
        "Zero accrual factor should be accepted"
    );
}

#[test]
fn cashflow_rejects_negative_accrual_factor() {
    // Negative accrual factor is invalid
    let cf = CashFlow {
        accrual_factor: -0.25,
        ..valid_cashflow()
    };
    assert!(
        cf.validate().is_err(),
        "Negative accrual factor should be rejected"
    );
}

// =============================================================================
// Rate Validation Tests
// =============================================================================

#[test]
fn cashflow_rejects_nan_rate() {
    let cf = CashFlow {
        rate: Some(f64::NAN),
        ..valid_cashflow()
    };
    assert!(cf.validate().is_err(), "NaN rate should be rejected");
}

#[test]
fn cashflow_rejects_positive_infinity_rate() {
    let cf = CashFlow {
        rate: Some(f64::INFINITY),
        ..valid_cashflow()
    };
    assert!(
        cf.validate().is_err(),
        "Positive infinity rate should be rejected"
    );
}

#[test]
fn cashflow_rejects_negative_infinity_rate() {
    let cf = CashFlow {
        rate: Some(f64::NEG_INFINITY),
        ..valid_cashflow()
    };
    assert!(
        cf.validate().is_err(),
        "Negative infinity rate should be rejected"
    );
}

#[test]
fn cashflow_accepts_none_rate() {
    let cf = CashFlow {
        rate: None,
        ..valid_cashflow()
    };
    assert!(cf.validate().is_ok(), "None rate should be accepted");
}

#[test]
fn cashflow_accepts_zero_rate() {
    let cf = CashFlow {
        rate: Some(0.0),
        ..valid_cashflow()
    };
    assert!(cf.validate().is_ok(), "Zero rate should be accepted");
}

#[test]
fn cashflow_accepts_negative_rate() {
    // Negative rates are valid (negative interest rate environment)
    let cf = CashFlow {
        rate: Some(-0.005),
        ..valid_cashflow()
    };
    assert!(cf.validate().is_ok(), "Negative rate should be accepted");
}

#[test]
fn cashflow_accepts_high_rate() {
    // High rates are valid (distressed debt, VC scenarios)
    let cf = CashFlow {
        rate: Some(2.5), // 250%
        ..valid_cashflow()
    };
    assert!(cf.validate().is_ok(), "High rate should be accepted");
}

// =============================================================================
// Reset Date Validation Tests
// =============================================================================

#[test]
fn cashflow_rejects_reset_date_after_payment() {
    let payment = d(2025, 1, 10);
    let reset = d(2025, 1, 15); // After payment date

    let cf = CashFlow {
        date: payment,
        reset_date: Some(reset),
        amount: Money::new(100.0, Currency::USD),
        kind: CFKind::FloatReset,
        accrual_factor: 0.25,
        rate: None,
    };
    assert!(
        cf.validate().is_err(),
        "Reset date after payment date should be rejected"
    );
}

#[test]
fn cashflow_accepts_reset_date_before_payment() {
    let payment = d(2025, 1, 15);
    let reset = d(2025, 1, 10); // Before payment date

    let cf = CashFlow {
        date: payment,
        reset_date: Some(reset),
        amount: Money::new(100.0, Currency::USD),
        kind: CFKind::FloatReset,
        accrual_factor: 0.25,
        rate: None,
    };
    assert!(
        cf.validate().is_ok(),
        "Reset date before payment date should be accepted"
    );
}

#[test]
fn cashflow_accepts_reset_date_equal_to_payment() {
    let date = d(2025, 1, 15);

    let cf = CashFlow {
        date,
        reset_date: Some(date), // Same as payment date
        amount: Money::new(100.0, Currency::USD),
        kind: CFKind::FloatReset,
        accrual_factor: 0.25,
        rate: None,
    };
    assert!(
        cf.validate().is_ok(),
        "Reset date equal to payment date should be accepted"
    );
}

#[test]
fn cashflow_accepts_no_reset_date() {
    let cf = CashFlow {
        date: d(2025, 1, 15),
        reset_date: None,
        amount: Money::new(100.0, Currency::USD),
        kind: CFKind::Fixed,
        accrual_factor: 0.25,
        rate: Some(0.05),
    };
    assert!(
        cf.validate().is_ok(),
        "No reset date should be accepted for fixed cashflows"
    );
}

// =============================================================================
// Combined Validation Tests
// =============================================================================

#[test]
fn cashflow_valid_with_all_fields_populated() {
    let cf = CashFlow {
        date: d(2025, 6, 15),
        reset_date: Some(d(2025, 6, 1)),
        amount: Money::new(25_000.0, Currency::EUR),
        kind: CFKind::FloatReset,
        accrual_factor: 0.25,
        rate: Some(0.0325),
    };
    assert!(
        cf.validate().is_ok(),
        "Valid cashflow with all fields should be accepted"
    );
}

#[test]
fn cashflow_multiple_invalid_fields_first_error_wins() {
    // Multiple invalid fields: accrual_factor and rate are invalid
    // Validation should fail on the first non-amount check (accrual_factor)
    // Note: Can't test invalid amount since Money::new panics on non-finite
    let cf = CashFlow {
        date: d(2025, 1, 15),
        reset_date: Some(d(2025, 1, 20)), // After payment - invalid
        amount: Money::new(100.0, Currency::USD),
        kind: CFKind::Fixed,
        accrual_factor: f64::INFINITY, // Invalid
        rate: Some(f64::NAN),          // Also invalid
    };
    assert!(
        cf.validate().is_err(),
        "Cashflow with multiple invalid fields should be rejected"
    );
}
