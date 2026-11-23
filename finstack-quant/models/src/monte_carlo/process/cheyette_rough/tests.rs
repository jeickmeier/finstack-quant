use super::*;
use crate::monte_carlo::traits::{PathState, ProcessMetadata, StochasticProcess, state_keys};
use finstack_quant_core::math::fractional::HurstExponent;

fn make_sigma_base() -> BaseVolatility {
    BaseVolatility::flat(0.005).expect("valid flat curve")
}

fn make_hurst(h: f64) -> HurstExponent {
    HurstExponent::new(h).expect("valid hurst")
}

fn make_phi_points() -> Vec<(f64, f64)> {
    vec![(0.0, 0.02), (5.0, 0.025), (30.0, 0.03)]
}

#[test]
fn test_valid_params() {
    let params = CheyetteRoughVolParams::new(
        0.03,
        make_sigma_base(),
        make_hurst(0.1),
        1.5,
        -0.3,
        &make_phi_points(),
    );
    assert!(params.is_ok());
}

#[test]
fn serde_round_trip_preserves_required_curves_and_path_state() {
    let params = CheyetteRoughVolParams::new(
        0.03,
        BaseVolatility::from_points(&[(0.0, 0.005), (2.0, 0.008)]).expect("valid curve"),
        make_hurst(0.1),
        1.5,
        -0.3,
        &make_phi_points(),
    )
    .expect("valid parameters");
    let json = serde_json::to_string(&params).expect("serialize parameters");
    let restored: CheyetteRoughVolParams =
        serde_json::from_str(&json).expect("deserialize parameters");
    for time in [0.0, 0.5, 2.0, 10.0, 35.0] {
        assert_eq!(params.phi(time), restored.phi(time));
        assert_eq!(
            params.sigma_base.get_value(time).unwrap(),
            restored.sigma_base.get_value(time).unwrap()
        );
    }
    let process = CheyetteRoughVolProcess::new(restored);
    let mut state = PathState::new(1, 5.0);
    process.populate_path_state(&[0.005, 0.001], &mut state);
    assert_eq!(
        state.get(state_keys::SHORT_RATE),
        Some(0.005 + params.phi(5.0))
    );
}

#[test]
fn serde_rejects_missing_or_invalid_required_curves() {
    let params = CheyetteRoughVolParams::new(
        0.03,
        make_sigma_base(),
        make_hurst(0.1),
        1.5,
        -0.3,
        &make_phi_points(),
    )
    .expect("valid parameters");
    let value = serde_json::to_value(params).expect("serialize parameters");
    for field in ["sigma_base", "phi_times", "phi_values"] {
        let mut missing = value.clone();
        missing.as_object_mut().expect("object").remove(field);
        assert!(serde_json::from_value::<CheyetteRoughVolParams>(missing).is_err());
    }
    for (field, invalid) in [
        ("phi_times", serde_json::json!([])),
        ("phi_values", serde_json::json!([])),
        ("phi_times", serde_json::json!([0.0, 5.0, 3.0])),
        ("kappa", serde_json::json!(-0.03)),
        ("rho", serde_json::json!(1.1)),
    ] {
        let mut malformed = value.clone();
        malformed[field] = invalid;
        assert!(serde_json::from_value::<CheyetteRoughVolParams>(malformed).is_err());
    }
}

#[test]
fn test_kappa_must_be_positive() {
    let res = CheyetteRoughVolParams::new(
        0.0,
        make_sigma_base(),
        make_hurst(0.1),
        1.5,
        -0.3,
        &make_phi_points(),
    );
    assert!(res.is_err());

    let res = CheyetteRoughVolParams::new(
        -1.0,
        make_sigma_base(),
        make_hurst(0.1),
        1.5,
        -0.3,
        &make_phi_points(),
    );
    assert!(res.is_err());
}

#[test]
fn test_kappa_must_be_finite() {
    let res = CheyetteRoughVolParams::new(
        f64::INFINITY,
        make_sigma_base(),
        make_hurst(0.1),
        1.5,
        -0.3,
        &make_phi_points(),
    );
    assert!(res.is_err());
}

#[test]
fn test_eta_must_be_positive() {
    let res = CheyetteRoughVolParams::new(
        0.03,
        make_sigma_base(),
        make_hurst(0.1),
        0.0,
        -0.3,
        &make_phi_points(),
    );
    assert!(res.is_err());

    let res = CheyetteRoughVolParams::new(
        0.03,
        make_sigma_base(),
        make_hurst(0.1),
        -1.0,
        -0.3,
        &make_phi_points(),
    );
    assert!(res.is_err());
}

#[test]
fn test_eta_must_be_finite() {
    let res = CheyetteRoughVolParams::new(
        0.03,
        make_sigma_base(),
        make_hurst(0.1),
        f64::INFINITY,
        -0.3,
        &make_phi_points(),
    );
    assert!(res.is_err());
}

#[test]
fn test_rho_must_be_in_range() {
    let res = CheyetteRoughVolParams::new(
        0.03,
        make_sigma_base(),
        make_hurst(0.1),
        1.5,
        -1.1,
        &make_phi_points(),
    );
    assert!(res.is_err());

    let res = CheyetteRoughVolParams::new(
        0.03,
        make_sigma_base(),
        make_hurst(0.1),
        1.5,
        1.1,
        &make_phi_points(),
    );
    assert!(res.is_err());
}

#[test]
fn test_rho_must_be_finite() {
    let res = CheyetteRoughVolParams::new(
        0.03,
        make_sigma_base(),
        make_hurst(0.1),
        1.5,
        f64::NAN,
        &make_phi_points(),
    );
    assert!(res.is_err());
}

#[test]
fn test_rho_boundary_values() {
    let res = CheyetteRoughVolParams::new(
        0.03,
        make_sigma_base(),
        make_hurst(0.1),
        1.5,
        -1.0,
        &make_phi_points(),
    );
    assert!(res.is_ok());

    let res = CheyetteRoughVolParams::new(
        0.03,
        make_sigma_base(),
        make_hurst(0.1),
        1.5,
        1.0,
        &make_phi_points(),
    );
    assert!(res.is_ok());
}

#[test]
fn test_phi_points_must_be_nonempty() {
    let res = CheyetteRoughVolParams::new(0.03, make_sigma_base(), make_hurst(0.1), 1.5, -0.3, &[]);
    assert!(res.is_err());
}

#[test]
fn test_phi_points_times_must_be_increasing() {
    let res = CheyetteRoughVolParams::new(
        0.03,
        make_sigma_base(),
        make_hurst(0.1),
        1.5,
        -0.3,
        &[(0.0, 0.02), (0.0, 0.03)],
    );
    assert!(res.is_err());

    let res = CheyetteRoughVolParams::new(
        0.03,
        make_sigma_base(),
        make_hurst(0.1),
        1.5,
        -0.3,
        &[(1.0, 0.02), (0.5, 0.03)],
    );
    assert!(res.is_err());
}

#[test]
fn test_phi_points_times_must_be_nonnegative() {
    let res = CheyetteRoughVolParams::new(
        0.03,
        make_sigma_base(),
        make_hurst(0.1),
        1.5,
        -0.3,
        &[(-0.1, 0.02)],
    );
    assert!(res.is_err());
}

#[test]
fn test_phi_points_values_must_be_finite() {
    let res = CheyetteRoughVolParams::new(
        0.03,
        make_sigma_base(),
        make_hurst(0.1),
        1.5,
        -0.3,
        &[(0.0, f64::NAN)],
    );
    assert!(res.is_err());
}

#[test]
fn test_phi_flat_extrapolation_left() {
    let params = CheyetteRoughVolParams::new(
        0.03,
        make_sigma_base(),
        make_hurst(0.1),
        1.5,
        -0.3,
        &[(1.0, 0.02), (5.0, 0.03)],
    )
    .expect("valid");

    assert!((params.phi(0.0) - 0.02).abs() < 1e-15);
    assert!((params.phi(0.5) - 0.02).abs() < 1e-15);
}

#[test]
fn test_phi_flat_extrapolation_right() {
    let params = CheyetteRoughVolParams::new(
        0.03,
        make_sigma_base(),
        make_hurst(0.1),
        1.5,
        -0.3,
        &[(1.0, 0.02), (5.0, 0.03)],
    )
    .expect("valid");

    assert!((params.phi(10.0) - 0.03).abs() < 1e-15);
}

#[test]
fn test_phi_linear_interpolation() {
    let params = CheyetteRoughVolParams::new(
        0.03,
        make_sigma_base(),
        make_hurst(0.1),
        1.5,
        -0.3,
        &[(0.0, 0.02), (10.0, 0.03)],
    )
    .expect("valid");

    // Midpoint: 0.02 + 0.5 * (0.03 - 0.02) = 0.025
    assert!((params.phi(5.0) - 0.025).abs() < 1e-15);
}

#[test]
fn test_phi_single_point() {
    let params = CheyetteRoughVolParams::new(
        0.03,
        make_sigma_base(),
        make_hurst(0.1),
        1.5,
        -0.3,
        &[(0.0, 0.02)],
    )
    .expect("valid");

    assert!((params.phi(0.0) - 0.02).abs() < 1e-15);
    assert!((params.phi(10.0) - 0.02).abs() < 1e-15);
}

#[test]
fn test_process_dim() {
    let params = CheyetteRoughVolParams::new(
        0.03,
        make_sigma_base(),
        make_hurst(0.1),
        1.5,
        -0.3,
        &make_phi_points(),
    )
    .expect("valid");
    let process = CheyetteRoughVolProcess::new(params);

    assert_eq!(process.dim(), 2);
    assert_eq!(process.num_factors(), 3);
}

#[test]
fn test_drift_diffusion_are_noop() {
    let params = CheyetteRoughVolParams::new(
        0.03,
        make_sigma_base(),
        make_hurst(0.1),
        1.5,
        -0.3,
        &make_phi_points(),
    )
    .expect("valid");
    let process = CheyetteRoughVolProcess::new(params);

    let x = [0.0, 0.0];
    let mut drift = [f64::NAN, f64::NAN];
    let mut diff = [f64::NAN, f64::NAN];

    process.drift(0.5, &x, &mut drift);
    process
        .diffusion(0.5, &x, &mut diff)
        .expect("valid diffusion");

    assert_eq!(drift[0], 0.0);
    assert_eq!(drift[1], 0.0);
    assert_eq!(diff[0], 0.0);
    assert_eq!(diff[1], 0.0);
}

#[test]
fn test_populate_path_state_sets_short_rate() {
    let params = CheyetteRoughVolParams::new(
        0.03,
        make_sigma_base(),
        make_hurst(0.1),
        1.5,
        -0.3,
        &[(0.0, 0.02), (10.0, 0.03)],
    )
    .expect("valid");
    let process = CheyetteRoughVolProcess::new(params);

    // At t=5.0, phi(5.0) = 0.025, x[0] = 0.005 => r = 0.03
    let x = [0.005, 0.001];
    let mut state = PathState::new(5, 5.0);
    process.populate_path_state(&x, &mut state);

    let short_rate = state.get(state_keys::SHORT_RATE).expect("short_rate set");
    assert!(
        (short_rate - 0.03).abs() < 1e-14,
        "Expected short_rate = 0.03, got {short_rate}"
    );
}

#[test]
fn test_populate_path_state_at_t_zero() {
    let params = CheyetteRoughVolParams::new(
        0.03,
        make_sigma_base(),
        make_hurst(0.1),
        1.5,
        -0.3,
        &[(0.0, 0.02), (10.0, 0.03)],
    )
    .expect("valid");
    let process = CheyetteRoughVolProcess::new(params);

    // At t=0.0, phi(0.0) = 0.02, x[0] = 0.0 => r = 0.02
    let x = [0.0, 0.0];
    let mut state = PathState::new(0, 0.0);
    process.populate_path_state(&x, &mut state);

    let short_rate = state.get(state_keys::SHORT_RATE).expect("short_rate set");
    assert!(
        (short_rate - 0.02).abs() < 1e-14,
        "Expected short_rate = 0.02, got {short_rate}"
    );
}

#[test]
fn test_metadata() {
    let params = CheyetteRoughVolParams::new(
        0.03,
        make_sigma_base(),
        make_hurst(0.1),
        1.5,
        -0.3,
        &make_phi_points(),
    )
    .expect("valid");
    let process = CheyetteRoughVolProcess::new(params);
    let meta = process.metadata();

    assert_eq!(meta.process_type, "CheyetteRoughVol");
}
