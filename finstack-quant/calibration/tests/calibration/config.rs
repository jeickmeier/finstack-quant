//! Tests for calibration configuration helpers and validation rules.

use finstack_quant_calibration::{
    CalibrationConfig, RateBounds, RateBoundsPolicy, SolverConfig, ValidationConfig,
};
use finstack_quant_core::currency::Currency;

#[test]
fn calibration_engine_rejects_invalid_solver_controls_before_execution() {
    use finstack_quant_calibration::api::engine::{execute_json, ExecutionStage};

    for solver in [
        serde_json::json!({"tolerance": 0.0}),
        serde_json::json!({"tolerance": -1e-8}),
        serde_json::json!({"max_iterations": 0}),
    ] {
        let envelope = serde_json::json!({
            "schema": "finstack_quant.calibration/1",
            "plan": {
                "id": "invalid_solver",
                "quote_sets": {},
                "settings": {"solver": solver},
                "steps": []
            },
            "market_data": [],
            "prior_market": []
        });
        let error = execute_json(&envelope.to_string()).expect_err("invalid solver controls");
        assert_eq!(error.details().stage, ExecutionStage::Configuration);
    }
}

#[test]
fn calibration_engine_rejects_invalid_discount_controls_before_execution() {
    use finstack_quant_calibration::api::engine::{execute_json, ExecutionStage};

    for discount_curve in [
        serde_json::json!({"df_hard_min": 2.0, "df_hard_max": 1.0}),
        serde_json::json!({"df_hard_min": 1.0, "df_hard_max": 1.0}),
        serde_json::json!({"df_hard_min": 0.0}),
        serde_json::json!({"jacobian_step_size": 0.0}),
        serde_json::json!({"jacobian_step_size": -1e-6}),
        serde_json::json!({"scan_grid_step": 0.0}),
        serde_json::json!({"scan_grid_points": 0}),
        serde_json::json!({"min_scan_grid_points": 0}),
    ] {
        let envelope = serde_json::json!({
            "schema": "finstack_quant.calibration/1",
            "plan": {
                "id": "invalid_discount_controls",
                "quote_sets": {},
                "settings": {"discount_curve": discount_curve},
                "steps": []
            },
            "market_data": [],
            "prior_market": []
        });
        let error = execute_json(&envelope.to_string()).expect_err("invalid discount controls");
        assert_eq!(error.details().stage, ExecutionStage::Configuration);
    }
}

#[test]
fn calibration_config_rejects_non_finite_discount_bounds_and_steps() {
    type SetControl = fn(&mut finstack_quant_calibration::DiscountCurveSolveConfig, f64);
    let controls: [(&str, SetControl); 5] = [
        ("df_hard_min", |config, value| config.df_hard_min = value),
        ("df_hard_max", |config, value| config.df_hard_max = value),
        ("jacobian_step_size", |config, value| {
            config.jacobian_step_size = value
        }),
        ("scan_grid_step", |config, value| {
            config.scan_grid_step = value
        }),
        ("min_t_spot", |config, value| config.min_t_spot = value),
    ];
    for (label, set) in controls {
        for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, 0.0, -1.0] {
            let mut config = CalibrationConfig::default();
            set(&mut config.discount_curve, value);
            let error = config.validate().expect_err("invalid numerical control");
            assert!(
                error.to_string().contains(label),
                "{label}={value}: {error}"
            );
        }
    }

    let mut config = CalibrationConfig::default();
    config.discount_curve.scan_grid_points = usize::MAX;
    assert!(config.validate().is_err());
    config.discount_curve = Default::default();
    config.discount_curve.min_scan_grid_points = usize::MAX;
    assert!(config.validate().is_err());
}

#[test]
fn partial_discount_config_retains_a_positive_default_jacobian_step() {
    let config: CalibrationConfig = serde_json::from_value(serde_json::json!({
        "discount_curve": {"validation_tolerance": 1e-7}
    }))
    .expect("partial discount settings");
    assert_eq!(
        config.discount_curve.jacobian_step_size,
        CalibrationConfig::default()
            .discount_curve
            .jacobian_step_size,
    );
    config
        .validate()
        .expect("default finite-difference control");
}

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
fn rate_bounds_reject_non_finite_endpoints() {
    for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(RateBounds::new(value, 0.5).is_err());
        assert!(RateBounds::new(-0.05, value).is_err());
    }
}

#[test]
fn validation_config_rejects_non_finite_thresholds() {
    type SetThreshold = fn(&mut ValidationConfig, f64);
    let thresholds: [(&str, SetThreshold); 11] = [
        ("min_forward_rate", |config, value| {
            config.min_forward_rate = value
        }),
        ("max_forward_rate", |config, value| {
            config.max_forward_rate = value
        }),
        ("tolerance", |config, value| config.tolerance = value),
        ("max_hazard_rate", |config, value| {
            config.max_hazard_rate = value
        }),
        ("min_cpi_growth", |config, value| {
            config.min_cpi_growth = value
        }),
        ("max_cpi_growth", |config, value| {
            config.max_cpi_growth = value
        }),
        ("min_fwd_inflation", |config, value| {
            config.min_fwd_inflation = value
        }),
        ("max_fwd_inflation", |config, value| {
            config.max_fwd_inflation = value
        }),
        ("max_volatility", |config, value| {
            config.max_volatility = value
        }),
        ("butterfly_upper_ratio", |config, value| {
            config.butterfly_upper_ratio = value
        }),
        ("butterfly_lower_ratio", |config, value| {
            config.butterfly_lower_ratio = value
        }),
    ];
    for (label, set) in thresholds {
        for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let mut config = ValidationConfig::default();
            set(&mut config, value);
            let error = config.validate().expect_err("non-finite threshold");
            assert!(
                error.to_string().contains(label),
                "{label}={value}: {error}"
            );
        }
    }
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
        solver: SolverConfig::default().with_tolerance(1e-4),
        ..Default::default()
    };

    let err = cfg
        .validate()
        .expect_err("solver tolerance looser than validation tolerance should fail");
    assert!(err.to_string().contains("solver tolerance"));
}
