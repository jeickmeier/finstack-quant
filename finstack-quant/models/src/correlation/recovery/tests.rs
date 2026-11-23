use super::*;
use crate::correlation::error::Error;

#[test]
fn invalid_recovery_inputs_fail_every_construction_path() {
    for (mean, vol, corr) in [
        (-0.1, 0.2, 0.4),
        (1.1, 0.2, 0.4),
        (0.4, -0.1, 0.4),
        (0.4, 0.6, 0.4),
        (0.4, 0.2, -1.1),
        (0.4, 0.2, 1.1),
    ] {
        assert!(CorrelatedRecovery::new(mean, vol, corr).is_err());
        assert!(RecoverySpec::market_correlated(mean, vol, corr).is_err());
        let spec = RecoverySpec::MarketCorrelated {
            mean_recovery: mean,
            recovery_volatility: vol,
            factor_correlation: corr,
        };
        assert!(spec.build().is_err());
        let wire = serde_json::to_value(spec).expect("wire");
        assert!(serde_json::from_value::<RecoverySpec>(wire).is_err());
    }
    for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(CorrelatedRecovery::new(bad, 0.2, 0.4).is_err());
        assert!(CorrelatedRecovery::new(0.4, bad, 0.4).is_err());
        assert!(CorrelatedRecovery::new(0.4, 0.2, bad).is_err());
    }
    for corr in [-1.0, 1.0] {
        let model = CorrelatedRecovery::new(0.4, 0.5, corr).expect("boundary inputs");
        assert!(model.expected_recovery().is_finite());
    }
}

#[test]
fn building_preserves_every_accepted_recovery_location() {
    for mean in [0.0, 0.01, 0.03, 0.40, 0.98, 1.0] {
        let spec = RecoverySpec::market_correlated(mean, 0.0, 0.4).expect("valid recovery");
        let model = spec.build().expect("valid recovery model");
        assert!((model.conditional_recovery(0.0).expect("conditional mean") - mean).abs() < 1e-12);
        assert!((model.expected_recovery() - mean).abs() < 1e-12);
    }
}

#[test]
fn test_recovery_spec_default() {
    let spec = RecoverySpec::default();
    assert!(matches!(spec, RecoverySpec::Constant { rate } if (rate - 0.40).abs() < 1e-10));
}

#[test]
fn test_recovery_spec_builders() {
    let constant = RecoverySpec::constant(0.35).expect("valid recovery rate");
    assert!((constant.build().expect("model").expected_recovery() - 0.35).abs() < 1e-10);

    let correlated =
        RecoverySpec::market_correlated(0.40, 0.25, 0.40).expect("valid recovery inputs");
    assert!((correlated.build().expect("model").expected_recovery() - 0.40).abs() < 0.05);

    let market_std = RecoverySpec::market_standard_stochastic().expect("valid model inputs");
    assert!((market_std.build().expect("model").expected_recovery() - 0.40).abs() < 0.05);
}

#[test]
fn test_recovery_spec_clamping() {
    // Recovery rates are caller input, so out-of-range values are rejected
    // rather than silently clamped.
    let high = RecoverySpec::constant(1.5);
    assert!(matches!(high, Err(Error::InvalidRecoveryInput { .. })));

    let low = RecoverySpec::constant(-0.1);
    assert!(matches!(low, Err(Error::InvalidRecoveryInput { .. })));
}

#[test]
fn test_recovery_spec_build() {
    let specs = vec![
        RecoverySpec::constant(0.40).expect("valid constant recovery"),
        RecoverySpec::market_correlated(0.40, 0.25, 0.40)
            .expect("valid market-correlated recovery"),
    ];

    for spec in specs {
        let model = spec.build().expect("valid recovery model");
        assert!(model.expected_recovery() >= 0.0);
        assert!(model.expected_recovery() <= 1.0);
        assert!(!model.model_name().is_empty());
    }
}

#[test]
fn test_recovery_spec_rejects_beta_variant() {
    // schema-rejection-test
    let err = serde_json::from_str::<RecoverySpec>(r#"{"type":"Beta","mean":0.4,"std_dev":0.15}"#)
        .expect_err("Beta recovery should not deserialize");

    assert!(err.to_string().contains("unknown variant"));
}

#[test]
fn test_recovery_spec_rejects_frye_variant() {
    // schema-rejection-test
    let err = serde_json::from_str::<RecoverySpec>(
        r#"{"type":"Frye","base_lgd":0.6,"lgd_sensitivity":1.5}"#,
    )
    .expect_err("Frye recovery should not deserialize");

    assert!(err.to_string().contains("unknown variant"));
}

#[test]
fn test_lgd_calculation() {
    let model = ConstantRecovery::new(0.40).expect("valid model inputs");
    assert!((model.lgd() - 0.60).abs() < 1e-10);
}
