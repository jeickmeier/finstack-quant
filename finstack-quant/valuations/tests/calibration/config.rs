//! Tests for calibration configuration helpers and validation rules.

use finstack_quant_core::currency::Currency;
use finstack_quant_valuations::calibration::{
    CalibrationConfig, RateBounds, RateBoundsPolicy, SolverConfig, ValidationConfig,
};

#[test]
fn calibration_config_effective_rate_bounds_respects_policy() {
    let auto = CalibrationConfig::default();
    let eur_bounds = auto.effective_rate_bounds(Currency::EUR);
    assert_eq!(
        eur_bounds,
        RateBounds::for_currency(Currency::EUR),
        "auto policy should use currency-specific bounds"
    );

    let explicit_bounds = RateBounds {
        min_rate: -0.01,
        max_rate: 0.10,
    };
    let explicit = CalibrationConfig::default().with_rate_bounds(explicit_bounds.clone());
    assert_eq!(explicit.rate_bounds_policy, RateBoundsPolicy::Explicit);
    assert_eq!(
        explicit.effective_rate_bounds(Currency::USD),
        explicit_bounds
    );
}

#[test]
fn rate_bounds_rejects_invalid_range() {
    let err = RateBounds::new(0.05, -0.01).expect_err("min > max should fail");
    assert!(err.to_string().contains("min_rate"));
}

#[test]
fn validation_config_rejects_invalid_forward_limits() {
    let cfg = ValidationConfig {
        min_forward_rate: 0.01,
        ..ValidationConfig::default()
    };
    let err = cfg
        .validate()
        .expect_err("min_forward_rate > 0 should fail");
    assert!(err.to_string().contains("min_forward_rate"));
}

#[test]
fn validation_config_carries_recovery_defaults() {
    let cfg = ValidationConfig::default();
    assert_eq!(cfg.recovery_rate_abs_tolerance, 1e-12);
    assert_eq!(cfg.minimum_lgd_for_hazard_guess, 1e-6);
    cfg.validate().expect("default validation config is valid");
}

#[test]
fn calibration_config_rejects_solver_tolerance_looser_than_fit_tolerance() {
    let cfg = CalibrationConfig {
        solver: SolverConfig::brent_default().with_tolerance(1e-4),
        ..Default::default()
    };

    let err = cfg
        .validate()
        .expect_err("solver tolerance looser than validation tolerance should fail");
    assert!(err.to_string().contains("solver tolerance"));
}
