//! JSON serialization roundtrip tests for Monte Carlo process parameter structs.
//!
//! Ensures that all MC process parameters can be serialized and deserialized
//! consistently when the `serde` feature is enabled.
#![allow(clippy::expect_used)]

use crate::closed_form::heston::HestonPricingParams;
use crate::monte_carlo::process::{
    brownian::BrownianParams, cir::CirParams, gbm::GbmParams, multi_ou::MultiOuParams,
    ou::HullWhite1FParams, schwartz_smith::SchwartzSmithParams,
};

/// Helper function to perform JSON roundtrip serialization test
fn roundtrip_json<T>(value: &T) -> T
where
    T: serde::Serialize + serde::de::DeserializeOwned + std::fmt::Debug,
{
    let json = serde_json::to_string_pretty(value).expect("Failed to serialize to JSON");
    println!("JSON representation:\n{}\n", json);
    serde_json::from_str(&json).expect("Failed to deserialize from JSON")
}

#[test]
fn test_gbm_params_serialization() {
    let params = GbmParams::new(
        0.05, // r = 5% risk-free rate
        0.02, // q = 2% dividend yield
        0.20, // σ = 20% volatility
    )
    .unwrap();

    let restored = roundtrip_json(&params);

    // Compare fields
    assert_eq!(params.r, restored.r);
    assert_eq!(params.q, restored.q);
    assert_eq!(params.sigma, restored.sigma);
}

#[test]
fn test_heston_params_serialization() {
    let params = HestonPricingParams::new(
        0.05, // r = 5% risk-free rate
        0.02, // q = 2% dividend yield
        2.0,  // κ = mean reversion speed
        0.04, // θ = long-term variance (20% long-term vol)
        0.3,  // σᵥ = vol of vol
        -0.7, // ρ = correlation (typically negative for equity)
        0.04, // v₀ = initial variance (20% current vol)
    )
    .expect("valid");

    let restored = roundtrip_json(&params);

    // Compare fields
    assert_eq!(params.r, restored.r);
    assert_eq!(params.q, restored.q);
    assert_eq!(params.kappa, restored.kappa);
    assert_eq!(params.theta, restored.theta);
    assert_eq!(params.sigma_v, restored.sigma_v);
    assert_eq!(params.rho, restored.rho);
    assert_eq!(params.v0, restored.v0);

    // Verify Feller condition is preserved
    assert_eq!(
        params.satisfies_feller(),
        restored.satisfies_feller(),
        "Feller condition should be preserved"
    );
}

#[test]
fn test_cir_params_serialization() {
    let params = CirParams::new(
        0.5,  // κ = mean reversion speed
        0.04, // θ = long-term mean
        0.1,  // σ = volatility
    )
    .unwrap();

    let restored = roundtrip_json(&params);

    // Compare fields
    assert_eq!(params.kappa, restored.kappa);
    assert_eq!(params.theta, restored.theta);
    assert_eq!(params.sigma, restored.sigma);

    // Verify Feller condition is preserved
    assert_eq!(
        params.satisfies_feller(),
        restored.satisfies_feller(),
        "Feller condition should be preserved"
    );
}

#[test]
fn test_hull_white_1f_params_serialization() {
    let params = HullWhite1FParams::new(
        0.1,  // κ = mean reversion speed
        0.01, // σ = volatility
        0.03, // θ = constant mean reversion level
    )
    .expect("valid Hull-White parameters");

    let restored = roundtrip_json(&params);

    // Compare fields
    assert_eq!(params.kappa, restored.kappa);
    assert_eq!(params.model, restored.model);
    assert_eq!(params.theta_values(), restored.theta_values());
    assert_eq!(params.theta_times(), restored.theta_times());

    // Verify theta function behavior is preserved
    assert_eq!(params.theta_at_time(0.0), restored.theta_at_time(0.0));
    assert_eq!(params.theta_at_time(1.0), restored.theta_at_time(1.0));
}

#[test]
fn test_hull_white_1f_params_time_dependent_serialization() {
    let theta_curve = vec![0.02, 0.03, 0.04];
    let theta_times = vec![0.0, 1.0, 2.0];

    let params = HullWhite1FParams::with_time_dependent_theta(
        0.1,  // κ
        0.01, // σ
        theta_curve,
        theta_times,
    )
    .expect("valid Hull-White theta schedule");

    let restored = roundtrip_json(&params);

    // Compare fields
    assert_eq!(params.kappa, restored.kappa);
    assert_eq!(params.model, restored.model);
    assert_eq!(params.theta_values(), restored.theta_values());
    assert_eq!(params.theta_times(), restored.theta_times());

    // Verify theta at various times
    for t in [0.0, 0.5, 1.0, 1.5, 2.0, 10.0] {
        assert_eq!(
            params.theta_at_time(t),
            restored.theta_at_time(t),
            "theta at time {} should match",
            t
        );
    }
}

#[test]
fn test_multi_ou_params_serialization() {
    let params = MultiOuParams::new(
        vec![2.0, 1.0],  // kappas
        vec![1.0, -1.0], // thetas
        vec![0.3, 0.4],  // sigmas
        None,            // no correlation
    );

    let restored = roundtrip_json(&params);

    // Compare fields
    assert_eq!(params.kappas, restored.kappas);
    assert_eq!(params.thetas, restored.thetas);
    assert_eq!(params.sigmas, restored.sigmas);
    assert_eq!(params.correlation, restored.correlation);
}

#[test]
fn test_multi_ou_params_with_correlation_serialization() {
    let corr = vec![1.0, 0.5, 0.5, 1.0];
    let params = MultiOuParams::new(
        vec![2.0, 1.0],  // kappas
        vec![1.0, -1.0], // thetas
        vec![0.3, 0.4],  // sigmas
        Some(corr),
    );

    let restored = roundtrip_json(&params);

    // Compare fields
    assert_eq!(params.kappas, restored.kappas);
    assert_eq!(params.thetas, restored.thetas);
    assert_eq!(params.sigmas, restored.sigmas);
    assert_eq!(params.correlation, restored.correlation);
    assert!(restored.correlation.is_some());
}

#[test]
fn test_brownian_params_serialization() {
    let params = BrownianParams::new(
        0.1, // μ = drift
        0.3, // σ = diffusion
    );

    let restored = roundtrip_json(&params);

    // Compare fields
    assert_eq!(params.mu, restored.mu);
    assert_eq!(params.sigma, restored.sigma);
}

#[test]
fn test_schwartz_smith_params_serialization() {
    let params = SchwartzSmithParams::new(
        2.0,  // κ_X = mean reversion speed
        0.30, // σ_X = short-term volatility
        0.02, // μ_Y = long-term drift
        0.15, // σ_Y = long-term volatility
        -0.5, // ρ = correlation
    )
    .expect("valid params");

    let restored = roundtrip_json(&params);

    // Compare fields
    assert_eq!(params.kappa, restored.kappa);
    assert_eq!(params.sigma_x, restored.sigma_x);
    assert_eq!(params.mu_y, restored.mu_y);
    assert_eq!(params.sigma_y, restored.sigma_y);
    assert_eq!(params.rho_xy, restored.rho_xy);
}

#[test]
fn test_edge_case_zero_volatilities() {
    // Test that zero volatilities serialize correctly (though may not be practical)
    let params = BrownianParams::new(0.0, 0.0);
    let restored = roundtrip_json(&params);
    assert_eq!(params.mu, restored.mu);
    assert_eq!(params.sigma, restored.sigma);
}

#[test]
fn test_edge_case_extreme_correlations() {
    // Test perfect positive correlation
    let params_pos = SchwartzSmithParams::new(2.0, 0.3, 0.02, 0.15, 1.0).expect("valid");
    let restored_pos = roundtrip_json(&params_pos);
    assert_eq!(params_pos.rho_xy, restored_pos.rho_xy);
    assert_eq!(restored_pos.rho_xy, 1.0);

    // Test perfect negative correlation
    let params_neg = SchwartzSmithParams::new(2.0, 0.3, 0.02, 0.15, -1.0).expect("valid");
    let restored_neg = roundtrip_json(&params_neg);
    assert_eq!(params_neg.rho_xy, restored_neg.rho_xy);
    assert_eq!(restored_neg.rho_xy, -1.0);
}

#[test]
// schema-rejection-test: retired `kappa_x` / `rho` keys (now `kappa` / `rho_xy`).
fn test_schwartz_smith_params_reject_retired_keys() {
    let old = r#"{"kappa_x":2.0,"sigma_x":0.3,"mu_y":0.02,"sigma_y":0.15,"rho":-0.5}"#;
    assert!(serde_json::from_str::<SchwartzSmithParams>(old).is_err());
}
