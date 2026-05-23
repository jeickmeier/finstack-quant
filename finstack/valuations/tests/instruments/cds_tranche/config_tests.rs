//! Configuration and parameter validation tests for CDS Tranche.
//!
//! Tests cover:
//! - Default configuration values
//! - Custom configuration parameters
//! - CS01 bump units
//! - Heterogeneous calculation methods
//! - Configuration parameter bounds
//! - Accumulated loss validation

use finstack_core::currency::Currency;
use finstack_core::money::Money;
use finstack_core::types::Percentage;
use finstack_valuations::instruments::credit_derivatives::cds_tranche::{
    CDSTrancheParams, CDSTranchePricer, CDSTranchePricerConfig, CopulaSpec, HeteroMethod,
};
use time::macros::date;

// ==================== Default Configuration Tests ====================

#[test]
fn test_default_config_quadrature_order() {
    // Arrange & Act
    let config = CDSTranchePricerConfig::default();

    // Assert
    assert_eq!(
        config.quadrature_order, 20,
        "Default quadrature order should be 20 (industry standard)"
    );
}

#[test]
fn test_default_config_uses_issuer_curves() {
    // Arrange & Act
    let config = CDSTranchePricerConfig::default();

    // Assert
    assert!(
        config.use_issuer_curves,
        "Default should use issuer curves when available"
    );
}

#[test]
fn test_default_config_correlation_bounds() {
    // Arrange & Act
    let config = CDSTranchePricerConfig::default();

    // Assert
    assert_eq!(
        config.min_correlation, 0.01,
        "Default min correlation should be 0.01"
    );
    assert_eq!(
        config.max_correlation, 0.99,
        "Default max correlation should be 0.99"
    );
    assert!(
        config.min_correlation < config.max_correlation,
        "Min correlation must be less than max correlation"
    );
}

#[test]
fn test_default_config_cs01_parameters() {
    // Arrange & Act
    let config = CDSTranchePricerConfig::default();

    // Assert
    assert_eq!(
        config.cs01_bump_size, 1.0,
        "Default CS01 bump size should be 1bp"
    );
}

#[test]
fn test_default_config_correlation_bump() {
    // Arrange & Act
    let config = CDSTranchePricerConfig::default();

    // Assert
    assert_eq!(
        config.corr_bump_abs, 0.01,
        "Default correlation bump should be 1%"
    );
}

#[test]
fn test_default_config_accrual_on_default() {
    // Arrange & Act
    let config = CDSTranchePricerConfig::default();

    // Assert
    assert!(
        config.accrual_on_default_enabled,
        "Accrual-on-default should be enabled by default"
    );
    // `aod_allocation_fraction` is deprecated and no longer read by the
    // pricer: accrual-on-default timing is now survival-weighted (integrated
    // against the index credit curve), not a fixed mid-period fraction. The
    // legacy field is intentionally not asserted here.
}

#[test]
fn test_default_config_hetero_method() {
    // Arrange & Act
    let config = CDSTranchePricerConfig::default();

    // Assert
    assert!(
        matches!(config.hetero_method, HeteroMethod::Spa),
        "Default heterogeneous method should be SPA"
    );
}

// ==================== Heterogeneous Method Tests ====================

#[test]
fn test_hetero_method_spa() {
    // Arrange & Act
    let method = HeteroMethod::Spa;

    // Assert
    assert!(matches!(method, HeteroMethod::Spa));
}

#[test]
fn test_hetero_method_exact_convolution() {
    // Arrange & Act
    let method = HeteroMethod::ExactConvolution;

    // Assert
    assert!(matches!(method, HeteroMethod::ExactConvolution));
}

#[test]
fn test_hetero_method_equality() {
    // Arrange
    let spa1 = HeteroMethod::Spa;
    let spa2 = HeteroMethod::Spa;
    let exact = HeteroMethod::ExactConvolution;

    // Assert
    assert_eq!(spa1, spa2, "Same enum variants should be equal");
    assert_ne!(spa1, exact, "Different enum variants should not be equal");
}

// ==================== Custom Configuration Tests ====================

#[test]
fn test_custom_config_quadrature_orders() {
    // Test different valid quadrature orders
    for order in [5u8, 7, 10] {
        // Arrange
        let config = CDSTranchePricerConfig {
            quadrature_order: order,
            ..Default::default()
        };

        // Assert
        assert_eq!(config.quadrature_order, order);
    }
}

#[test]
fn test_pricer_config_builder_methods_wire_copula_and_numerical_settings() {
    let student_t = CDSTranchePricerConfig::default()
        .with_student_t_copula(6.0)
        .expect("valid Student-t df");
    assert!(matches!(
        student_t.copula_spec,
        CopulaSpec::StudentT {
            degrees_of_freedom
        } if (degrees_of_freedom - 6.0).abs() < 1e-12
    ));

    let rfl = CDSTranchePricerConfig::default().with_rfl_copula(0.15);
    assert!(matches!(
        rfl.copula_spec,
        CopulaSpec::RandomFactorLoading {
            loading_volatility
        } if (loading_volatility - 0.15).abs() < 1e-12
    ));

    let rfl_pct = CDSTranchePricerConfig::default().with_rfl_copula_pct(Percentage::new(12.5));
    assert!(matches!(
        rfl_pct.copula_spec,
        CopulaSpec::RandomFactorLoading {
            loading_volatility
        } if (loading_volatility - 0.125).abs() < 1e-12
    ));

    let multi_factor = CDSTranchePricerConfig::default().with_multi_factor_copula(3);
    assert!(matches!(
        multi_factor.copula_spec,
        CopulaSpec::MultiFactor { num_factors } if num_factors == 3
    ));

    let config = CDSTranchePricerConfig::default()
        .with_arbitrage_validation(false)
        .with_quadrature_order(7);
    let pricer = CDSTranchePricer::with_params(config.clone());
    assert!(!config.validate_arbitrage_free);
    assert_eq!(config.quadrature_order, 7);
    assert_eq!(pricer.config().quadrature_order, 7);
    assert!(!pricer.config().validate_arbitrage_free);
}

#[test]
fn test_pricer_config_recovery_builders_populate_recovery_spec() {
    let stochastic = CDSTranchePricerConfig::default().with_stochastic_recovery();
    assert!(stochastic.recovery_spec.is_some());

    let custom =
        CDSTranchePricerConfig::default().with_custom_stochastic_recovery(0.35, 0.20, -0.4);
    let custom_debug = format!("{:?}", custom.recovery_spec);
    assert!(custom.recovery_spec.is_some());
    assert!(custom_debug.contains("0.35"));
    assert!(custom_debug.contains("0.2"));
    assert!(custom_debug.contains("-0.4"));

    let custom_pct = CDSTranchePricerConfig::default().with_custom_stochastic_recovery_pct(
        Percentage::new(45.0),
        Percentage::new(25.0),
        -0.3,
    );
    let custom_pct_debug = format!("{:?}", custom_pct.recovery_spec);
    assert!(custom_pct.recovery_spec.is_some());
    assert!(custom_pct_debug.contains("0.45"));
    assert!(custom_pct_debug.contains("0.25"));

    let constant = CDSTranchePricerConfig::default().with_constant_recovery(0.42);
    let constant_debug = format!("{:?}", constant.recovery_spec);
    assert!(constant.recovery_spec.is_some());
    assert!(constant_debug.contains("0.42"));

    let constant_pct =
        CDSTranchePricerConfig::default().with_constant_recovery_pct(Percentage::new(38.0));
    let constant_pct_debug = format!("{:?}", constant_pct.recovery_spec);
    assert!(constant_pct.recovery_spec.is_some());
    assert!(constant_pct_debug.contains("0.38"));
}

#[test]
fn test_custom_config_correlation_bounds_modification() {
    // Arrange & Act
    let config = CDSTranchePricerConfig {
        min_correlation: 0.05,
        max_correlation: 0.95,
        ..Default::default()
    };

    // Assert
    assert_eq!(config.min_correlation, 0.05);
    assert_eq!(config.max_correlation, 0.95);
}

#[test]
fn test_custom_config_cs01_bump_size() {
    // Arrange & Act
    let config = CDSTranchePricerConfig {
        cs01_bump_size: 0.5,
        ..Default::default()
    };

    // Assert
    assert_eq!(config.cs01_bump_size, 0.5);
}

#[test]
fn test_custom_config_disable_accrual_on_default() {
    // Arrange & Act
    let config = CDSTranchePricerConfig {
        accrual_on_default_enabled: false,
        ..Default::default()
    };

    // Assert
    assert!(!config.accrual_on_default_enabled);
}

#[test]
fn test_custom_config_hetero_method_exact() {
    // Arrange & Act
    let config = CDSTranchePricerConfig {
        hetero_method: HeteroMethod::ExactConvolution,
        ..Default::default()
    };

    // Assert
    assert!(matches!(
        config.hetero_method,
        HeteroMethod::ExactConvolution
    ));
}

#[test]
fn test_custom_config_grid_step() {
    // Arrange & Act
    let config = CDSTranchePricerConfig {
        grid_step: 0.0005,
        ..Default::default()
    };

    // Assert
    assert_eq!(config.grid_step, 0.0005);
}

// ==================== Configuration Clone Tests ====================

#[test]
fn test_config_cloneable() {
    // Arrange
    let config1 = CDSTranchePricerConfig::default();

    // Act
    let config2 = config1.clone();

    // Assert
    assert_eq!(config2.quadrature_order, config1.quadrature_order);
    assert_eq!(config2.min_correlation, config1.min_correlation);
    assert_eq!(config2.cs01_bump_size, config1.cs01_bump_size);
}

#[test]
fn test_config_independent_after_clone() {
    // Arrange
    let mut config1 = CDSTranchePricerConfig::default();
    let config2 = config1.clone();

    // Act
    config1.quadrature_order = 10;

    // Assert
    assert_eq!(config1.quadrature_order, 10);
    assert_eq!(
        config2.quadrature_order, 20,
        "Cloned config should not be affected"
    );
}

// ==================== Accumulated Loss Validation Tests ====================

#[test]
fn test_accumulated_loss_valid_zero() {
    // Arrange
    let params = CDSTrancheParams::equity_tranche(
        "CDX.NA.IG",
        42,
        Money::new(1_000_000.0, Currency::USD),
        date!(2029 - 12 - 20),
        500.0,
    );

    // Act
    let result = params.with_accumulated_loss(0.0);

    // Assert
    assert!(result.is_ok());
    assert_eq!(result.unwrap().accumulated_loss, 0.0);
}

#[test]
fn test_accumulated_loss_valid_mid_range() {
    // Arrange
    let params = CDSTrancheParams::equity_tranche(
        "CDX.NA.IG",
        42,
        Money::new(1_000_000.0, Currency::USD),
        date!(2029 - 12 - 20),
        500.0,
    );

    // Act
    let result = params.with_accumulated_loss(0.5);

    // Assert
    assert!(result.is_ok());
    assert_eq!(result.unwrap().accumulated_loss, 0.5);
}

#[test]
fn test_accumulated_loss_valid_one() {
    // Arrange
    let params = CDSTrancheParams::equity_tranche(
        "CDX.NA.IG",
        42,
        Money::new(1_000_000.0, Currency::USD),
        date!(2029 - 12 - 20),
        500.0,
    );

    // Act
    let result = params.with_accumulated_loss(1.0);

    // Assert
    assert!(result.is_ok());
    assert_eq!(result.unwrap().accumulated_loss, 1.0);
}

#[test]
fn test_accumulated_loss_invalid_negative() {
    // Arrange
    let params = CDSTrancheParams::equity_tranche(
        "CDX.NA.IG",
        42,
        Money::new(1_000_000.0, Currency::USD),
        date!(2029 - 12 - 20),
        500.0,
    );

    // Act
    let result = params.with_accumulated_loss(-0.01);

    // Assert
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        err.to_string().contains("accumulated_loss"),
        "Error should mention accumulated_loss: {}",
        err
    );
}

#[test]
fn test_accumulated_loss_invalid_greater_than_one() {
    // Arrange
    let params = CDSTrancheParams::equity_tranche(
        "CDX.NA.IG",
        42,
        Money::new(1_000_000.0, Currency::USD),
        date!(2029 - 12 - 20),
        500.0,
    );

    // Act
    let result = params.with_accumulated_loss(1.01);

    // Assert
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        err.to_string().contains("accumulated_loss"),
        "Error should mention accumulated_loss: {}",
        err
    );
}

#[test]
fn test_accumulated_loss_invalid_large_value() {
    // Arrange
    let params = CDSTrancheParams::equity_tranche(
        "CDX.NA.IG",
        42,
        Money::new(1_000_000.0, Currency::USD),
        date!(2029 - 12 - 20),
        500.0,
    );

    // Act
    let result = params.with_accumulated_loss(2.5);

    // Assert
    assert!(result.is_err());
}
