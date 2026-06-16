//! Tests for `RevolvingCredit::validate()` method.
//!
//! Verifies that structural invariants are enforced:
//! - drawn_amount <= commitment_amount
//! - recovery_rate bounds
//! - fee tier ordering
//! - date ordering
//! - currency consistency

use finstack_quant_cashflows::builder::FeeTier;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{DayCount, Tenor};
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::fixed_income::revolving_credit::{
    BaseRateSpec, DrawRepaySpec, RevolvingCredit, RevolvingCreditFees,
};
use rust_decimal::Decimal;
use time::macros::date;

/// Helper to build a valid facility for mutation-based testing.
fn valid_facility() -> RevolvingCredit {
    RevolvingCredit::builder()
        .id("RC-VALIDATE".into())
        .commitment_amount(Money::new(10_000_000.0, Currency::USD))
        .drawn_amount(Money::new(5_000_000.0, Currency::USD))
        .commitment_date(date!(2025 - 01 - 01))
        .maturity(date!(2026 - 01 - 01))
        .base_rate_spec(BaseRateSpec::Fixed { rate: 0.05 })
        .day_count(DayCount::Act360)
        .frequency(Tenor::quarterly())
        .fees(RevolvingCreditFees::flat(25.0, 10.0, 5.0))
        .draw_repay_spec(DrawRepaySpec::Deterministic(vec![]))
        .discount_curve_id("USD-OIS".into())
        .recovery_rate(0.0)
        .build()
        .unwrap()
}

#[test]
fn test_validate_valid_facility_passes() {
    let facility = valid_facility();
    assert!(facility.validate().is_ok());
}

#[test]
fn test_validate_drawn_exceeds_commitment() {
    // Build directly to bypass builder (which doesn't validate this)
    let mut facility = valid_facility();
    facility.drawn_amount = Money::new(15_000_000.0, Currency::USD);

    let result = facility.validate();
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("must not exceed commitment_amount"),
        "Expected 'must not exceed commitment_amount' error, got: {}",
        err_msg
    );
}

#[test]
fn test_validate_drawn_equals_commitment_passes() {
    let mut facility = valid_facility();
    facility.drawn_amount = facility.commitment_amount;

    assert!(facility.validate().is_ok());
}

#[test]
fn test_validate_recovery_rate_negative() {
    let mut facility = valid_facility();
    facility.recovery_rate = -0.1;

    let result = facility.validate();
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("recovery_rate"),
        "Expected recovery_rate error, got: {}",
        err_msg
    );
}

#[test]
fn test_validate_recovery_rate_one() {
    let mut facility = valid_facility();
    facility.recovery_rate = 1.0;

    let result = facility.validate();
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("recovery_rate"),
        "Expected recovery_rate error, got: {}",
        err_msg
    );
}

#[test]
fn test_validate_recovery_rate_valid_boundary() {
    let mut facility = valid_facility();
    // Just below the max
    facility.recovery_rate = 0.999;
    assert!(facility.validate().is_ok());

    // Zero is valid
    facility.recovery_rate = 0.0;
    assert!(facility.validate().is_ok());

    // Typical value
    facility.recovery_rate = 0.4;
    assert!(facility.validate().is_ok());
}

#[test]
fn test_validate_fee_tiers_unsorted_commitment() {
    let mut facility = valid_facility();
    // Set unsorted commitment fee tiers (descending instead of ascending)
    facility.fees.commitment_fee_tiers = vec![
        FeeTier {
            threshold: Decimal::try_from(0.5).unwrap(),
            bps: Decimal::try_from(30.0).unwrap(),
        },
        FeeTier {
            threshold: Decimal::try_from(0.0).unwrap(), // Lower threshold after higher
            bps: Decimal::try_from(25.0).unwrap(),
        },
    ];

    let result = facility.validate();
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("commitment_fee_tiers") && err_msg.contains("sorted"),
        "Expected fee tier ordering error, got: {}",
        err_msg
    );
}

#[test]
fn test_validate_fee_tiers_unsorted_usage() {
    let mut facility = valid_facility();
    // Set unsorted usage fee tiers
    facility.fees.usage_fee_tiers = vec![
        FeeTier {
            threshold: Decimal::try_from(0.75).unwrap(),
            bps: Decimal::try_from(15.0).unwrap(),
        },
        FeeTier {
            threshold: Decimal::try_from(0.25).unwrap(),
            bps: Decimal::try_from(10.0).unwrap(),
        },
    ];

    let result = facility.validate();
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("usage_fee_tiers") && err_msg.contains("sorted"),
        "Expected fee tier ordering error, got: {}",
        err_msg
    );
}

#[test]
fn test_validate_fee_tiers_sorted_passes() {
    let mut facility = valid_facility();
    // Properly sorted tiers
    facility.fees.commitment_fee_tiers = vec![
        FeeTier {
            threshold: Decimal::try_from(0.0).unwrap(),
            bps: Decimal::try_from(25.0).unwrap(),
        },
        FeeTier {
            threshold: Decimal::try_from(0.5).unwrap(),
            bps: Decimal::try_from(30.0).unwrap(),
        },
        FeeTier {
            threshold: Decimal::try_from(0.75).unwrap(),
            bps: Decimal::try_from(35.0).unwrap(),
        },
    ];

    assert!(facility.validate().is_ok());
}

#[test]
fn test_validate_negative_facility_fee() {
    let mut facility = valid_facility();
    facility.fees.facility_fee_bp = -5.0;

    let result = facility.validate();
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("facility_fee_bp"),
        "Expected facility_fee_bp error, got: {}",
        err_msg
    );
}

#[test]
fn test_validate_non_finite_fixed_rate() {
    let mut facility = valid_facility();
    facility.base_rate_spec = BaseRateSpec::Fixed {
        rate: f64::INFINITY,
    };

    let result = facility.validate();
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("finite"),
        "Expected finite rate error, got: {}",
        err_msg
    );
}

#[test]
fn test_validate_nan_fixed_rate() {
    let mut facility = valid_facility();
    facility.base_rate_spec = BaseRateSpec::Fixed { rate: f64::NAN };

    let result = facility.validate();
    assert!(result.is_err());
}

#[test]
fn test_validate_currency_mismatch() {
    let mut facility = valid_facility();
    facility.drawn_amount = Money::new(5_000_000.0, Currency::EUR);

    let result = facility.validate();
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("currency"),
        "Expected currency mismatch error, got: {}",
        err_msg
    );
}

#[test]
fn test_validate_maturity_before_commitment() {
    // This is also caught by the builder macro, but validate should also catch it
    let mut facility = valid_facility();
    facility.maturity = date!(2024 - 01 - 01); // Before commitment date

    let result = facility.validate();
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("commitment_date") && err_msg.contains("maturity"),
        "Expected date ordering error, got: {}",
        err_msg
    );
}

#[test]
fn test_validate_zero_commitment() {
    let mut facility = valid_facility();
    facility.commitment_amount = Money::new(0.0, Currency::USD);
    facility.drawn_amount = Money::new(0.0, Currency::USD);

    let result = facility.validate();
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("commitment_amount"),
        "Expected positive commitment error, got: {}",
        err_msg
    );
}

#[test]
fn test_validate_fee_tiers_duplicate_threshold() {
    let mut facility = valid_facility();
    // Two tiers with the same threshold — the first would be unreachable
    facility.fees.commitment_fee_tiers = vec![
        FeeTier {
            threshold: Decimal::try_from(0.5).unwrap(),
            bps: Decimal::try_from(25.0).unwrap(),
        },
        FeeTier {
            threshold: Decimal::try_from(0.5).unwrap(), // Duplicate threshold
            bps: Decimal::try_from(30.0).unwrap(),
        },
    ];

    let result = facility.validate();
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("commitment_fee_tiers") && err_msg.contains("strictly ascending"),
        "Expected strictly ascending error for duplicate thresholds, got: {}",
        err_msg
    );
}

#[test]
fn test_validate_negative_drawn_amount() {
    let mut facility = valid_facility();
    facility.drawn_amount = Money::new(-1_000_000.0, Currency::USD);

    let result = facility.validate();
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    // Should report non-negative error (not "must not exceed commitment" which would
    // also be vacuously true for negative amounts if checked first)
    assert!(
        err_msg.contains("non-negative"),
        "Expected non-negative error, got: {}",
        err_msg
    );
}
