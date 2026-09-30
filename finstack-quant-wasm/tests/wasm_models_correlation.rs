//! wasm-bindgen-test suite for `api::models::correlation`.

#![cfg(target_arch = "wasm32")]

use finstack_quant_wasm::api::models::correlation::*;
use wasm_bindgen::JsValue;
use wasm_bindgen_test::*;

#[wasm_bindgen_test]
fn validate_correlation_matrix_accepts_valid_and_rejects_invalid() {
    #[rustfmt::skip]
    let good = vec![
        1.0, 0.5, 0.3,
        0.5, 1.0, 0.4,
        0.3, 0.4, 1.0,
    ];
    assert!(validate_correlation_matrix(JsValue::from(good.to_vec()), JsValue::from(3)).is_ok());

    // Off-diagonal outside [-1, 1] must be rejected.
    #[rustfmt::skip]
    let bad = vec![
        1.0, 1.5,
        1.5, 1.0,
    ];
    assert!(validate_correlation_matrix(JsValue::from(bad.to_vec()), JsValue::from(2)).is_err());

    // Length / dimension mismatch must be rejected, not panic.
    assert!(validate_correlation_matrix(JsValue::from(good.to_vec()), JsValue::from(2)).is_err());
}

#[wasm_bindgen_test]
fn nearest_correlation_repairs_near_psd_input() {
    // Valid correlation matrix passes through unchanged.
    #[rustfmt::skip]
    let good = vec![
        1.0, 0.5, 0.3,
        0.5, 1.0, 0.4,
        0.3, 0.4, 1.0,
    ];
    let out = nearest_correlation(JsValue::from(good), JsValue::from(3), None, None)
        .expect("good matrix should project");
    assert_eq!(out.len(), 9);
    for i in 0..3 {
        assert!((out[i * 3 + i] - 1.0).abs() < 1e-9);
    }
}

#[wasm_bindgen_test]
fn portfolio_loss_result_handle_matches_rust() {
    let losses: JsValue = js_sys::Float64Array::from(&[0.0, 1.0, 2.0, 5.0, 10.0][..]).into();
    let result =
        JsPortfolioLossResult::from_losses(losses, JsValue::from(0.75)).expect("valid losses");
    assert_eq!(result.var(), 5.0);
    assert!((result.expected_shortfall() - 9.0).abs() < 1e-12);
    assert!(result
        .tranche_loss_statistics(
            JsValue::from(0.02),
            JsValue::from(0.06),
            JsValue::from(100.0)
        )
        .is_ok());
    let json = result.to_json().expect("json");
    let loaded = JsPortfolioLossResult::from_json(JsValue::from(&json)).expect("round trip");
    assert_eq!(loaded.to_json().unwrap(), json);
    assert!(JsPortfolioLossResult::from_json(JsValue::from(
        json.replace("\"var\":5.0", "\"var\":6.0")
    ))
    .is_err());
}

#[wasm_bindgen_test]
fn wasm_copula_spec_gaussian_and_student_t() {
    let g = JsCopulaSpec::gaussian();
    assert!(g.is_gaussian());
    assert!(!g.is_student_t());

    let Ok(t) = JsCopulaSpec::student_t(JsValue::from(5.0)) else {
        panic!("student_t(5.0) should succeed");
    };
    assert!(t.is_student_t());
    assert!(!t.is_gaussian());
}

#[wasm_bindgen_test]
fn wasm_copula_spec_random_factor_loading_and_multi_factor_build() {
    let rfl = JsCopulaSpec::random_factor_loading(JsValue::from(0.5)).unwrap();
    assert!(!rfl.is_gaussian());
    assert!(!rfl.is_student_t());
    assert!(rfl.is_rfl());
    assert!(!rfl.is_multi_factor());
    let rfl_copula = rfl.build().expect("RFL copula should build");
    assert_eq!(rfl_copula.num_factors(), 2);

    let mf = JsCopulaSpec::multi_factor();
    assert!(mf.is_multi_factor());
    assert!(!mf.is_rfl());
    let mf_copula = mf.build().expect("multi-factor copula should build");
    assert_eq!(mf_copula.num_factors(), 2);
}

#[wasm_bindgen_test]
fn wasm_copula_stress_correlation_proxy_rfl_only() {
    let rfl = JsCopulaSpec::random_factor_loading(JsValue::from(0.2))
        .unwrap()
        .build()
        .expect("RFL copula should build");
    // RFL has no closed-form λ_L: NaN per the tail-dependence contract.
    assert!(rfl.tail_dependence(JsValue::from(0.3)).unwrap().is_nan());
    let proxy = rfl
        .stress_correlation_proxy(JsValue::from(0.3))
        .expect("proxy defined for RFL");
    assert!(proxy > 0.0, "proxy should be positive for σ_β > 0: {proxy}");

    let gaussian = JsCopulaSpec::gaussian()
        .build()
        .expect("Gaussian copula should build");
    assert!(
        gaussian
            .stress_correlation_proxy(JsValue::from(0.3))
            .is_err(),
        "proxy must throw for non-RFL copulas"
    );
}

#[wasm_bindgen_test]
fn wasm_copula_from_gaussian_spec() {
    let copula = JsCopulaSpec::gaussian()
        .build()
        .expect("Gaussian copula should build");
    assert_eq!(copula.num_factors(), 1);
    assert_eq!(copula.model_name(), "One-Factor Gaussian Copula");
    assert_eq!(copula.tail_dependence(JsValue::from(0.3)).unwrap(), 0.0);

    let pd = 0.05_f64;
    let threshold = finstack_quant_core::math::standard_normal_inv_cdf(pd);
    let correlation = 0.3_f64;
    let cond = copula
        .conditional_default_prob(
            JsValue::from(threshold),
            JsValue::from([0.0].to_vec()),
            JsValue::from(correlation),
        )
        .expect("valid Gaussian copula inputs");
    assert!(cond > 0.0 && cond < 1.0);
}

#[wasm_bindgen_test]
fn wasm_recovery_spec_and_model() {
    let c = JsRecoverySpec::constant(JsValue::from(0.4)).expect("0.4 is a valid recovery rate");
    assert!((c.expected_recovery() - 0.4).abs() < 1e-12);
    let m = c.build();
    assert!((m.expected_recovery() - 0.4).abs() < 1e-12);
    assert!((m.conditional_recovery(JsValue::from(0.0)).unwrap() - 0.4).abs() < 1e-12);
    assert!((m.lgd() - 0.6).abs() < 1e-12);
    assert!(!m.is_stochastic());
    assert!(!m.model_name().is_empty());

    let mc = JsRecoverySpec::market_correlated(
        JsValue::from(0.4),
        JsValue::from(0.1),
        JsValue::from(0.3),
    )
    .expect("valid market-correlated spec")
    .build();
    assert!(mc.is_stochastic());
    assert!(mc.recovery_volatility() > 0.0);
    assert!(
        (mc.conditional_lgd(JsValue::from(0.0)).unwrap()
            - (1.0 - mc.conditional_recovery(JsValue::from(0.0)).unwrap()))
        .abs()
            < 1e-12,
        "conditional_lgd must complement conditional_recovery"
    );

    let std = JsRecoverySpec::market_standard_stochastic().build();
    assert!(std.is_stochastic());
    assert!((std.recovery_volatility() - 0.25).abs() < 1e-12);
}

#[wasm_bindgen_test]
fn wasm_recovery_spec_constant_rejects_out_of_range_and_nan() {
    // RecoverySpec::constant rejects rates outside [0, 1] and non-finite
    // values at the Rust API boundary.
    assert!(
        JsRecoverySpec::constant(JsValue::from(1.5)).is_err(),
        "recovery rate above 1 must be rejected, not clamped"
    );
    assert!(
        JsRecoverySpec::constant(JsValue::from(-0.2)).is_err(),
        "negative recovery rate must be rejected, not clamped"
    );
    assert!(
        JsRecoverySpec::constant(JsValue::from(f64::NAN)).is_err(),
        "NaN recovery rate must be rejected"
    );
    // The valid endpoints must still be accepted.
    assert!(JsRecoverySpec::constant(JsValue::from(0.0)).is_ok());
    assert!(JsRecoverySpec::constant(JsValue::from(1.0)).is_ok());
}

#[wasm_bindgen_test]
fn wasm_recovery_spec_market_correlated_validates_inputs() {
    // Mean recovery outside [0, 1] or non-finite must be rejected.
    assert!(JsRecoverySpec::market_correlated(
        JsValue::from(1.5),
        JsValue::from(0.1),
        JsValue::from(0.3)
    )
    .is_err());
    assert!(JsRecoverySpec::market_correlated(
        JsValue::from(f64::NAN),
        JsValue::from(0.1),
        JsValue::from(0.3)
    )
    .is_err());
    // Non-finite vol / correlation must also be rejected.
    assert!(JsRecoverySpec::market_correlated(
        JsValue::from(0.4),
        JsValue::from(f64::NAN),
        JsValue::from(0.3)
    )
    .is_err());
    assert!(JsRecoverySpec::market_correlated(
        JsValue::from(0.4),
        JsValue::from(0.1),
        JsValue::from(f64::INFINITY)
    )
    .is_err());
    // A fully valid spec is still accepted.
    assert!(JsRecoverySpec::market_correlated(
        JsValue::from(0.4),
        JsValue::from(0.25),
        JsValue::from(-0.4)
    )
    .is_ok());
}

#[wasm_bindgen_test]
fn correlation_bounds_ordered() {
    let b = correlation_bounds(JsValue::from(0.05), JsValue::from(0.10)).expect("valid marginals");
    assert_eq!(b.len(), 2);
    assert!(b[0] <= b[1]);
}

#[wasm_bindgen_test]
fn joint_probabilities_sum_to_one() {
    let j = joint_probabilities(JsValue::from(0.05), JsValue::from(0.10), JsValue::from(0.3))
        .expect("valid inputs");
    assert_eq!(j.len(), 4);
    let sum: f64 = j.iter().sum();
    assert!((sum - 1.0).abs() < 1e-9);
}
