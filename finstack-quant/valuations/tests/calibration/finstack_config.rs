//! Shared runtime types and solver contracts for market calibration.
//!
use finstack_quant_core::config::FinstackConfig;
use finstack_quant_valuations::calibration::{
    CalibrationConfig, CalibrationMethod, RateBounds, RateBoundsPolicy, CALIBRATION_CONFIG_KEY,
};
use serde_json::json;

#[test]
fn calibration_config_applies_extension_overrides() {
    let mut cfg = FinstackConfig::default();
    cfg.extensions
        .insert(
            CALIBRATION_CONFIG_KEY,
            json!({
                "solver": {
                    "method": "brent",
                    "tolerance": 1e-8,
                    "max_iterations": 250
                },
                "use_parallel": true,
                "rate_bounds_policy": "explicit",
                "rate_bounds": { "min_rate": -0.01, "max_rate": 0.10 },
                "calibration_method": { "GlobalSolve": { "use_analytical_jacobian": true } }
            }),
        )
        .expect("valid extension key");

    let cfg_out =
        CalibrationConfig::from_finstack_config_or_default(&cfg).expect("apply overrides");
    assert_eq!(cfg_out.solver.tolerance(), 1e-8);
    assert_eq!(cfg_out.solver.max_iterations(), 250);
    assert!(cfg_out.use_parallel);
    assert_eq!(cfg_out.rate_bounds_policy, RateBoundsPolicy::Explicit);
    assert_eq!(
        cfg_out.rate_bounds,
        RateBounds {
            min_rate: -0.01,
            max_rate: 0.10
        }
    );
    assert!(
        matches!(
            cfg_out.calibration_method,
            CalibrationMethod::GlobalSolve {
                use_analytical_jacobian: true
            }
        ),
        "expected calibration method override to propagate"
    );
}

#[test]
fn calibration_config_defaults_without_extension() {
    let cfg = FinstackConfig::default();
    let cfg_out = CalibrationConfig::from_finstack_config_or_default(&cfg).expect("defaults");
    let defaults = CalibrationConfig::default();

    assert_eq!(cfg_out.solver.tolerance(), defaults.solver.tolerance());
    assert_eq!(
        cfg_out.solver.max_iterations(),
        defaults.solver.max_iterations()
    );
    assert_eq!(cfg_out.use_parallel, defaults.use_parallel);
    assert_eq!(cfg_out.solver, defaults.solver);
    assert_eq!(cfg_out.rate_bounds_policy, defaults.rate_bounds_policy);
    assert_eq!(cfg_out.rate_bounds, defaults.rate_bounds);
}

#[test]
fn calibration_config_rejects_unknown_fields_in_extension() {
    let mut cfg = FinstackConfig::default();
    cfg.extensions
        .insert(
            CALIBRATION_CONFIG_KEY,
            json!({
                "unknown_field": true
            }),
        )
        .expect("valid extension key");

    let err = CalibrationConfig::from_finstack_config_or_default(&cfg)
        .expect_err("unknown fields should error");
    let msg = err.to_string();
    assert!(msg.contains("Failed to parse extension"));
    assert!(msg.contains(CALIBRATION_CONFIG_KEY));
}
