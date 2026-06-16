//! Unit tests for behavioral model specifications.
//!
//! Tests cover:
//! - Prepayment model spec calculations
//! - Default model spec calculations
//! - Recovery model spec calculations
//! - JSON serialization/deserialization
//!
//! Note: Behavioral model specs are now unified in the cashflow builder
//! and re-exported from structured_credit. Core functionality is tested
//! in the builder module tests.

use finstack_quant_valuations::instruments::fixed_income::structured_credit::{
    DefaultModelSpec, PrepaymentModelSpec, RecoveryModelSpec,
};

// ============================================================================
// Prepayment Model Spec Tests
// ============================================================================

#[test]
fn test_prepayment_spec_psa() {
    let spec = PrepaymentModelSpec::psa(1.5);

    // Month 30: 150% PSA = 9% CPR
    let smm = spec.smm(30).expect("valid CPR");
    let expected_smm = 1.0 - (1.0 - 0.09_f64).powf(1.0 / 12.0);
    assert!((smm - expected_smm).abs() < 0.001);
}

#[test]
fn test_prepayment_spec_constant_cpr() {
    let spec = PrepaymentModelSpec::constant_cpr(0.12);

    let smm = spec.smm(12).expect("valid CPR");
    let expected_smm = 1.0 - (1.0 - 0.12_f64).powf(1.0 / 12.0);
    assert!((smm - expected_smm).abs() < 0.0001);
}

// ============================================================================
// Default Model Spec Tests
// ============================================================================

#[test]
fn test_default_spec_sda() {
    let spec = DefaultModelSpec::sda(2.0); // 200% SDA

    // Should have ramp up and decline pattern
    let month_10 = spec.mdr(10).expect("valid CDR");
    let month_30 = spec.mdr(30).expect("valid CDR"); // Peak
    let month_70 = spec.mdr(70).expect("valid CDR"); // Terminal

    assert!(month_30 > month_10);
    assert!(month_30 > month_70);
}

#[test]
fn test_default_spec_constant_cdr() {
    let spec = DefaultModelSpec::constant_cdr(0.02);

    let mdr = spec.mdr(12).expect("valid CDR");
    let expected_mdr = 1.0 - (1.0 - 0.02_f64).powf(1.0 / 12.0);
    assert!((mdr - expected_mdr).abs() < 0.0001);
}

// ============================================================================
// Recovery Model Spec Tests
// ============================================================================

#[test]
fn test_recovery_spec() {
    let spec = RecoveryModelSpec::with_lag(0.40, 12);

    assert_eq!(spec.rate, 0.40);
    assert_eq!(spec.recovery_lag, 12);
}
