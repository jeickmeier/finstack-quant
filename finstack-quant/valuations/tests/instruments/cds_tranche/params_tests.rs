//! Parameter validation tests for CDS Tranche.
//!
//! Tests cover:
//! - Copula specification validation on pricer construction
//! - Accumulated (realized) loss validation

use finstack_quant_core::currency::Currency;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::credit_derivatives::cds_tranche::{
    CdsTrancheParams, CdsTranchePricer, CopulaSpec,
};
use time::macros::date;

// ==================== Copula Specification Tests ====================

#[test]
fn pricer_accepts_every_copula_family() {
    for spec in [
        CopulaSpec::Gaussian,
        CopulaSpec::student_t(6.0).expect("valid Student-t df"),
        CopulaSpec::random_factor_loading(0.15),
        CopulaSpec::multi_factor(),
    ] {
        let pricer = CdsTranchePricer::with_copula(spec.clone()).expect("valid copula");
        assert_eq!(*pricer.get_copula_spec(), spec);
    }
}

#[test]
fn pricer_rejects_invalid_direct_student_t_spec() {
    let error = CdsTranchePricer::with_copula(CopulaSpec::StudentT {
        degrees_of_freedom: 2.0,
    })
    .err()
    .expect("invalid Student-t df must fail");
    assert!(error.to_string().contains("degrees of freedom"));
}

#[test]
fn pricer_rejects_out_of_range_loading_volatility() {
    let error = CdsTranchePricer::with_copula(CopulaSpec::RandomFactorLoading {
        loading_volatility: 0.75,
    })
    .err()
    .expect("loading volatility above 0.5 must fail");
    assert!(error.to_string().contains("loading_volatility"));
}

// ==================== Accumulated Loss Validation Tests ====================

#[test]
fn test_accumulated_loss_valid_zero() {
    // Arrange
    let params = CdsTrancheParams::equity_tranche(
        "CDX.NA.IG",
        42,
        Money::new(1_000_000.0, Currency::USD).expect("valid money fixture"),
        date!(2029 - 12 - 20),
        500.0,
    );

    // Act
    let result = params.with_realized_loss(0.0);

    // Assert
    assert!(result.is_ok());
    assert_eq!(result.unwrap().realized_loss, 0.0);
}

#[test]
fn test_accumulated_loss_valid_mid_range() {
    // Arrange
    let params = CdsTrancheParams::equity_tranche(
        "CDX.NA.IG",
        42,
        Money::new(1_000_000.0, Currency::USD).expect("valid money fixture"),
        date!(2029 - 12 - 20),
        500.0,
    );

    // Act
    let result = params.with_realized_loss(0.5);

    // Assert
    assert!(result.is_ok());
    assert_eq!(result.unwrap().realized_loss, 0.5);
}

#[test]
fn test_accumulated_loss_valid_one() {
    // Arrange
    let params = CdsTrancheParams::equity_tranche(
        "CDX.NA.IG",
        42,
        Money::new(1_000_000.0, Currency::USD).expect("valid money fixture"),
        date!(2029 - 12 - 20),
        500.0,
    );

    // Act
    let result = params.with_realized_loss(1.0);

    // Assert
    assert!(result.is_ok());
    assert_eq!(result.unwrap().realized_loss, 1.0);
}

#[test]
fn test_accumulated_loss_invalid_negative() {
    // Arrange
    let params = CdsTrancheParams::equity_tranche(
        "CDX.NA.IG",
        42,
        Money::new(1_000_000.0, Currency::USD).expect("valid money fixture"),
        date!(2029 - 12 - 20),
        500.0,
    );

    // Act
    let result = params.with_realized_loss(-0.01);

    // Assert
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        err.to_string().contains("realized_loss"),
        "Error should mention realized_loss: {}",
        err
    );
}

#[test]
fn test_accumulated_loss_invalid_greater_than_one() {
    // Arrange
    let params = CdsTrancheParams::equity_tranche(
        "CDX.NA.IG",
        42,
        Money::new(1_000_000.0, Currency::USD).expect("valid money fixture"),
        date!(2029 - 12 - 20),
        500.0,
    );

    // Act
    let result = params.with_realized_loss(1.01);

    // Assert
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        err.to_string().contains("realized_loss"),
        "Error should mention realized_loss: {}",
        err
    );
}

#[test]
fn test_accumulated_loss_invalid_large_value() {
    // Arrange
    let params = CdsTrancheParams::equity_tranche(
        "CDX.NA.IG",
        42,
        Money::new(1_000_000.0, Currency::USD).expect("valid money fixture"),
        date!(2029 - 12 - 20),
        500.0,
    );

    // Act
    let result = params.with_realized_loss(2.5);

    // Assert
    assert!(result.is_err());
}
