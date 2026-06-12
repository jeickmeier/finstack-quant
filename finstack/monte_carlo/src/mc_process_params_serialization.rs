//! JSON serialization roundtrip tests for Monte Carlo process parameter structs.
//!
//! Ensures that all MC process parameters can be serialized and deserialized
//! consistently when the `serde` feature is enabled.
#![allow(clippy::expect_used)]

use crate::process::{
    bates::BatesParams, brownian::BrownianParams, cir::CirParams, gbm::GbmParams,
    heston::HestonParams, jump_diffusion::MertonJumpParams, multi_ou::MultiOuParams,
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
    let params = HestonParams::new(
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
fn test_bates_params_serialization() {
    let heston = HestonParams::new(0.05, 0.02, 0.5, 0.04, 0.3, -0.7, 0.04).expect("valid");
    let jump = MertonJumpParams::new(0.05, 0.02, 0.0, 1.0, -0.05, 0.1).unwrap();
    let params = BatesParams::new(heston, jump).expect("matching r/q");

    let restored = roundtrip_json(&params);

    // Compare Heston component fields
    assert_eq!(params.heston.r, restored.heston.r);
    assert_eq!(params.heston.q, restored.heston.q);
    assert_eq!(params.heston.kappa, restored.heston.kappa);
    assert_eq!(params.heston.theta, restored.heston.theta);
    assert_eq!(params.heston.sigma_v, restored.heston.sigma_v);
    assert_eq!(params.heston.rho, restored.heston.rho);
    assert_eq!(params.heston.v0, restored.heston.v0);

    // Compare jump component fields
    assert_eq!(params.jump.gbm.r, restored.jump.gbm.r);
    assert_eq!(params.jump.gbm.q, restored.jump.gbm.q);
    assert_eq!(params.jump.gbm.sigma, restored.jump.gbm.sigma);
    assert_eq!(params.jump.lambda, restored.jump.lambda);
    assert_eq!(params.jump.mu_j, restored.jump.mu_j);
    assert_eq!(params.jump.sigma_j, restored.jump.sigma_j);

    // Verify derived quantities are preserved
    assert!((params.compensated_drift() - restored.compensated_drift()).abs() < 1e-10);
}

#[test]
fn test_hull_white_1f_params_serialization() {
    let params = HullWhite1FParams::new(
        0.1,  // κ = mean reversion speed
        0.01, // σ = volatility
        0.03, // θ = constant mean reversion level
    );

    let restored = roundtrip_json(&params);

    // Compare fields
    assert_eq!(params.kappa, restored.kappa);
    assert_eq!(params.sigma, restored.sigma);
    assert_eq!(params.theta_curve, restored.theta_curve);
    assert_eq!(params.theta_times, restored.theta_times);

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
    );

    let restored = roundtrip_json(&params);

    // Compare fields
    assert_eq!(params.kappa, restored.kappa);
    assert_eq!(params.sigma, restored.sigma);
    assert_eq!(params.theta_curve, restored.theta_curve);
    assert_eq!(params.theta_times, restored.theta_times);

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
fn test_merton_jump_params_serialization() {
    let params = MertonJumpParams::new(
        0.05,  // r = risk-free rate
        0.02,  // q = dividend yield
        0.2,   // σ = continuous volatility
        2.0,   // λ = jump intensity (2 jumps/year)
        -0.05, // μ_J = mean of log-jump
        0.1,   // σ_J = std dev of log-jump
    )
    .unwrap();

    let restored = roundtrip_json(&params);

    // Compare GBM component
    assert_eq!(params.gbm.r, restored.gbm.r);
    assert_eq!(params.gbm.q, restored.gbm.q);
    assert_eq!(params.gbm.sigma, restored.gbm.sigma);

    // Compare jump parameters
    assert_eq!(params.lambda, restored.lambda);
    assert_eq!(params.mu_j, restored.mu_j);
    assert_eq!(params.sigma_j, restored.sigma_j);

    // Verify derived quantities are preserved
    assert!((params.jump_compensation() - restored.jump_compensation()).abs() < 1e-10);
    assert!((params.compensated_drift() - restored.compensated_drift()).abs() < 1e-10);
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
    assert_eq!(params.kappa_x, restored.kappa_x);
    assert_eq!(params.sigma_x, restored.sigma_x);
    assert_eq!(params.mu_y, restored.mu_y);
    assert_eq!(params.sigma_y, restored.sigma_y);
    assert_eq!(params.rho, restored.rho);
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
    assert_eq!(params_pos.rho, restored_pos.rho);
    assert_eq!(restored_pos.rho, 1.0);

    // Test perfect negative correlation
    let params_neg = SchwartzSmithParams::new(2.0, 0.3, 0.02, 0.15, -1.0).expect("valid");
    let restored_neg = roundtrip_json(&params_neg);
    assert_eq!(params_neg.rho, restored_neg.rho);
    assert_eq!(restored_neg.rho, -1.0);
}
