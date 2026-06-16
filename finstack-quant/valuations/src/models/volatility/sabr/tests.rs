#![allow(clippy::expect_used, clippy::panic)]

use super::calibration::solve_alpha_for_atm;
use super::*;

#[test]
fn test_sabr_parameters_validation() {
    // Valid parameters
    assert!(SABRParameters::new(0.2, 0.5, 0.3, 0.1).is_ok());

    // Invalid alpha
    assert!(SABRParameters::new(-0.1, 0.5, 0.3, 0.1).is_err());

    // Invalid beta
    assert!(SABRParameters::new(0.2, 1.5, 0.3, 0.1).is_err());

    // Invalid rho
    assert!(SABRParameters::new(0.2, 0.5, 0.3, 1.5).is_err());
}

#[test]
fn test_sabr_atm_volatility() {
    let params =
        SABRParameters::new(0.2, 0.5, 0.3, -0.1).expect("SABR parameters should be valid in test");
    let model = SABRModel::new(params);

    let forward = 100.0;
    let time_to_expiry = 1.0;

    let atm_vol = model
        .atm_volatility(forward, time_to_expiry)
        .expect("ATM volatility calculation should succeed in test");

    // ATM vol should be positive
    assert!(atm_vol > 0.0);

    // For ATM, implied vol should match ATM vol
    let implied_vol = model
        .implied_volatility(forward, forward, time_to_expiry)
        .expect("Volatility calculation should succeed in test");
    assert!((implied_vol - atm_vol).abs() < 1e-10);
}

#[test]
fn test_sabr_smile_shape() {
    let params =
        SABRParameters::new(0.2, 0.7, 0.4, -0.3).expect("SABR parameters should be valid in test");
    let model = SABRModel::new(params);

    let forward = 100.0;
    let time_to_expiry = 1.0;

    // Generate strikes
    let strikes = vec![80.0, 90.0, 100.0, 110.0, 120.0];
    let mut vols = Vec::new();

    for strike in &strikes {
        let vol = model
            .implied_volatility(forward, *strike, time_to_expiry)
            .expect("Volatility calculation should succeed in test");
        vols.push(vol);
    }

    // With negative rho, we expect downward sloping skew
    // Lower strikes should have higher vols
    // But the actual shape depends on all parameters
    // Just check that we get different vols (smile exists)
    let vol_range = vols
        .iter()
        .max_by(|a, b| a.total_cmp(b))
        .expect("Vols should not be empty")
        - vols
            .iter()
            .min_by(|a, b| a.total_cmp(b))
            .expect("Vols should not be empty");
    assert!(vol_range > 0.001); // There is a smile
}

#[test]
fn test_sabr_normal_model() {
    // Beta = 0 gives normal SABR
    let params =
        SABRParameters::normal(20.0, 0.3, 0.0).expect("SABR parameters should be valid in test");
    let model = SABRModel::new(params);

    let forward = 0.05; // 5% rate
    let strike = 0.06; // 6% strike
    let time_to_expiry = 2.0;

    let vol = model
        .implied_volatility(forward, strike, time_to_expiry)
        .expect("Volatility calculation should succeed in test");

    // Should produce reasonable normal vol
    assert!(vol > 0.0);
    // Normal vol can be very large for small forward rates, so we just check it's positive
}

#[test]
fn test_sabr_lognormal_model() {
    // Beta = 1 gives lognormal SABR (like Black-Scholes)
    let params =
        SABRParameters::lognormal(0.3, 0.4, 0.2).expect("SABR parameters should be valid in test");
    let model = SABRModel::new(params);

    let forward = 100.0;
    let strike = 105.0;
    let time_to_expiry = 0.5;

    let vol = model
        .implied_volatility(forward, strike, time_to_expiry)
        .expect("Volatility calculation should succeed in test");

    // Should produce reasonable lognormal vol
    assert!(vol > 0.0);
    assert!(vol < 1.0); // Less than 100% vol
}

#[test]
fn test_sabr_calibration() {
    // Create synthetic market data
    let forward = 100.0;
    let strikes = vec![90.0, 95.0, 100.0, 105.0, 110.0];
    let market_vols = vec![0.22, 0.20, 0.19, 0.195, 0.21];
    let time_to_expiry = 1.0;
    let beta = 0.5; // Fixed beta

    // minimize() now fails loudly instead of
    // silently returning the best iterate on MaxIterations. This smile is not
    // SABR-exact (positive SSE minimum), so allow a realistic gradient
    // tolerance and iteration budget for formal convergence.
    let calibrator = SABRCalibrator::new()
        .with_tolerance(1e-4)
        .with_max_iterations(2000);
    let params = calibrator
        .calibrate(forward, &strikes, &market_vols, time_to_expiry, beta)
        .expect("Volatility calculation should succeed in test");

    // Check calibrated parameters are reasonable
    assert!(params.alpha > 0.0);
    assert!(params.nu >= 0.0);
    assert!(params.rho >= -1.0 && params.rho <= 1.0);

    // Check fit quality
    let model = SABRModel::new(params);
    for (i, &strike) in strikes.iter().enumerate() {
        let model_vol = model
            .implied_volatility(forward, strike, time_to_expiry)
            .expect("Volatility calculation should succeed in test");
        let error = (model_vol - market_vols[i]).abs();
        assert!(error < 0.05); // Within 5% vol (calibration is approximate)
    }
}

#[test]
fn test_sabr_smile_generator() {
    let params = SABRParameters::new(0.25, 0.6, 0.35, -0.25)
        .expect("SABR parameters should be valid in test");
    let model = SABRModel::new(params);
    let smile = SABRSmile::new(model, 100.0, 1.0);

    let strikes = vec![85.0, 90.0, 95.0, 100.0, 105.0, 110.0, 115.0];
    let vols = smile
        .generate_smile(&strikes)
        .expect("Smile generation should succeed in test");

    // Check all vols are positive
    for vol in &vols {
        assert!(*vol > 0.0);
    }

    // Validate that smile has variation (different volatilities)
    assert!(!vols.is_empty());
    assert!(vols.iter().all(|&v| v > 0.0));
}

#[test]
fn test_sabr_negative_rates_shifted() {
    // Test shifted SABR with negative forward rates
    let forward = -0.005; // -50bps
    let strikes = vec![-0.01, -0.005, 0.0, 0.005, 0.01];
    let shift = 0.02; // 200bps shift

    let params = SABRParameters::new_with_shift(0.2, 0.5, 0.3, -0.2, shift)
        .expect("SABR parameters should be valid in test"); // Higher alpha for more reasonable vols
    let model = SABRModel::new(params);

    // Should handle negative rates correctly
    for &strike in &strikes {
        let vol = model.implied_volatility(forward, strike, 1.0);
        assert!(vol.is_ok(), "Failed for strike {}: {:?}", strike, vol);
        let vol_val = vol.expect("Volatility should be Some in test");
        assert!(
            vol_val > 0.0,
            "Non-positive volatility {} for strike {}",
            vol_val,
            strike
        );
        assert!(
            vol_val < 10.0,
            "Unreasonably high volatility {} for strike {}",
            vol_val,
            strike
        );
    }
}

#[test]
fn test_sabr_atm_stability() {
    // Test enhanced ATM stability with very close strikes
    let params =
        SABRParameters::new(0.2, 0.5, 0.3, -0.1).expect("SABR parameters should be valid in test");
    let model = SABRModel::new(params);

    let forward = 0.025;
    let strikes = vec![
        forward - 1e-10,
        forward - 1e-12,
        forward,
        forward + 1e-12,
        forward + 1e-10,
    ];

    // All should give very similar results (ATM case)
    let mut vols = Vec::new();
    for &strike in &strikes {
        let vol = model
            .implied_volatility(forward, strike, 1.0)
            .expect("Implied volatility calculation should succeed in test");
        vols.push(vol);
    }

    // Check all ATM-like volatilities are similar with practical tolerance
    let vol_range = vols
        .iter()
        .max_by(|a, b| a.total_cmp(b))
        .expect("Vols should not be empty")
        - vols
            .iter()
            .min_by(|a, b| a.total_cmp(b))
            .expect("Vols should not be empty");
    assert!(vol_range < 1e-2); // Practical tolerance for numerical precision in ATM case
}

#[test]
fn test_sabr_auto_shift_calibration() {
    // Test automatic shift detection and calibration
    let forward = -0.002; // Negative forward
    let strikes = vec![-0.005, -0.002, 0.0, 0.002, 0.005];
    let market_vols = vec![0.015, 0.012, 0.010, 0.011, 0.013]; // More reasonable vols for rates
    let time_to_expiry = 0.5;
    let beta = 0.0; // Normal model for rates

    let calibrator = SABRCalibrator::new().with_tolerance(1e-4); // Relaxed tolerance for difficult calibration
    let params = calibrator
        .calibrate_auto_shift(forward, &strikes, &market_vols, time_to_expiry, beta)
        .expect("Volatility calculation should succeed in test");

    // Should have detected need for shift
    assert!(params.is_shifted());
    assert!(params.shift().expect("Shift should be Some") > 0.0);

    // Check model works with negative rates
    let model = SABRModel::new(params);
    for &strike in &strikes {
        let vol = model.implied_volatility(forward, strike, time_to_expiry);
        assert!(vol.is_ok(), "Failed for strike {}: {:?}", strike, vol);
        let vol_val = vol.expect("Volatility should be Some in test");
        assert!(
            vol_val > 0.0,
            "Non-positive volatility {} for strike {}",
            vol_val,
            strike
        );
    }
}

#[test]
fn test_sabr_numerical_stability_extreme_parameters() {
    // Test with extreme but valid parameters
    let params =
        SABRParameters::new(0.01, 0.1, 0.1, 0.9).expect("SABR parameters should be valid in test");
    let model = SABRModel::new(params);

    let forward = 0.001; // Very low rate
    let strikes = vec![0.0005, 0.001, 0.002];

    for &strike in &strikes {
        let vol = model.implied_volatility(forward, strike, 5.0); // Long maturity
        assert!(vol.is_ok());
        let vol_val = vol.expect("Volatility should be Some in test");
        assert!(vol_val > 0.0);
        assert!(vol_val.is_finite());
    }
}

#[test]
fn test_sabr_chi_function_stability() {
    // Test chi function with various extreme cases
    let params =
        SABRParameters::new(0.2, 0.5, 0.3, 0.95).expect("SABR parameters should be valid in test"); // High rho
    let model = SABRModel::new(params);

    // Test small z values
    let small_z_values = vec![1e-8, 1e-6, 1e-4];
    for z in small_z_values {
        let chi = model.calculate_chi_robust(z);
        assert!(chi.is_ok());
        assert!(chi.expect("Chi should be Some").is_finite());
    }

    // Test rho ≈ 1 case
    let params_rho_one =
        SABRParameters::new(0.2, 0.5, 0.3, 0.999).expect("SABR parameters should be valid in test");
    let model_rho_one = SABRModel::new(params_rho_one);
    let chi_rho_one = model_rho_one.calculate_chi_robust(0.1);
    assert!(chi_rho_one.is_ok());

    // Test rho ≈ -1 case
    let params_rho_minus_one = SABRParameters::new(0.2, 0.5, 0.3, -0.999)
        .expect("SABR parameters should be valid in test");
    let model_rho_minus_one = SABRModel::new(params_rho_minus_one);
    let chi_rho_minus_one = model_rho_minus_one.calculate_chi_robust(0.1);
    assert!(chi_rho_minus_one.is_ok());
}

/// The ρ→1 branch of χ(z) must use the exact analytic limit −ln(1−z) (the
/// ρ=1 discriminant is (1−z)², making the generic formula 0/0), not the old
/// `z/(1+z/2)` Padé guess; and it must reject z ≥ 1 where the limit diverges.
#[test]
fn test_sabr_chi_rho_one_uses_exact_log_limit() {
    let params = SABRParameters::new(0.2, 0.5, 0.3, 1.0).expect("rho = 1 is a valid bound");
    let model = SABRModel::new(params);

    for &z in &[0.05_f64, 0.3, 0.7, 0.95] {
        let chi = model.calculate_chi_robust(z).expect("chi at rho=1, z<1");
        let exact = -(1.0 - z).ln();
        assert!(
            (chi - exact).abs() < 1e-12,
            "rho=1 chi({z}) = {chi}, expected -ln(1-z) = {exact}"
        );
        // The old Padé approximation deviates materially for moderate z.
        let pade = z / (1.0 + z / 2.0);
        if z >= 0.3 {
            assert!(
                (chi - pade).abs() > 1e-3,
                "rho=1 chi({z}) should not match the old Padé form"
            );
        }
    }

    // Continuity with the near-limit generic formula.
    let near =
        SABRModel::new(SABRParameters::new(0.2, 0.5, 0.3, 1.0 - 1e-9).expect("valid params"));
    let z = 0.4_f64;
    let chi_near = near.calculate_chi_robust(z).expect("chi near rho=1");
    let chi_limit = -(1.0 - z).ln();
    assert!(
        (chi_near - chi_limit).abs() < 1e-4,
        "generic formula at rho=1-1e-9 ({chi_near}) must approach the limit ({chi_limit})"
    );

    // z ≥ 1 is outside the Hagan expansion's domain at rho=1.
    assert!(model.calculate_chi_robust(1.0).is_err());
    assert!(model.calculate_chi_robust(1.5).is_err());
}

/// The `z/χ(z)` correction (`factor2`) must use the well-defined `z→0` limit
/// for small z and must NOT fabricate `1.0` for an arbitrary tiny χ.
///
/// Failure mode under test: the old `factor2` was `if x.abs() < 1e-14 { 1.0 }`.
/// `z/χ(z) → 1` only as `z → 0`; a tiny χ with a non-tiny z is a genuine
/// numerical pathology, not a `1.0` limit. The fix uses the Taylor ratio
/// `1 − (ρ/2)z + ((2−ρ²)/12)z² − (ρ/24)z³` for small z and errors otherwise.
#[test]
fn test_sabr_z_over_chi_uses_series_not_fabricated_one() {
    let rho = -0.35_f64;
    let model = SABRModel::new(SABRParameters::new(0.2, 0.5, 0.3, rho).expect("valid params"));

    // For a small but non-zero z, the ratio must follow the Taylor series,
    // i.e. it must be measurably different from a fabricated 1.0.
    let z = 1e-6_f64;
    // χ(z) for this z (exact formula) — well above the 1e-14 underflow guard.
    let chi = model
        .calculate_chi_robust(z)
        .expect("χ(z) should compute for small z");
    let ratio = model
        .z_over_chi(z, chi)
        .expect("z/χ(z) should compute for small z");

    // Expected first-order series value: 1 − (ρ/2)·z.
    let expected = 1.0 - 0.5 * rho * z + (2.0 - rho * rho) / 12.0 * z * z;
    assert!(
        (ratio - expected).abs() < 1e-13,
        "z/χ(z) small-z branch must use the Taylor ratio: got {ratio:.15}, expected {expected:.15}"
    );
    // It must NOT be the fabricated constant 1.0 (ρ≠0 ⇒ first-order term ≠ 0).
    assert!(
        (ratio - 1.0).abs() > 1e-9,
        "z/χ(z) must not fabricate 1.0 for a small but non-zero z (ratio={ratio})"
    );

    // Genuinely-pathological case: χ underflowed while z is not small ⇒ error,
    // not a fabricated 1.0.
    let pathological = model.z_over_chi(0.5, 1e-20);
    assert!(
        pathological.is_err(),
        "z/χ(z) must error when χ underflows for a non-small z, got {pathological:?}"
    );

    // Exact-division branch for a normal (non-small) z still works.
    let z_big = 0.3_f64;
    let chi_big = model
        .calculate_chi_robust(z_big)
        .expect("χ should compute for moderate z");
    let ratio_big = model
        .z_over_chi(z_big, chi_big)
        .expect("z/χ(z) should compute for moderate z");
    assert!(
        (ratio_big - z_big / chi_big).abs() < 1e-14,
        "z/χ(z) moderate-z branch must be exact division"
    );
}

// ===================================================================
// Market Standards Validation Tests (Priority 1, Task 1.2)
// ===================================================================

#[test]
fn test_sabr_rejects_negative_alpha() {
    let result = SABRParameters::new(-0.1, 0.5, 0.3, 0.1);
    assert!(result.is_err(), "Negative alpha should be rejected");

    let err = result.expect_err("should fail");
    assert!(
        matches!(err, finstack_quant_core::Error::Validation(_)),
        "Should return Validation error"
    );

    // Verify error message mentions alpha
    let err_str = format!("{}", err);
    assert!(err_str.contains("alpha") || err_str.contains("α"));
}

#[test]
fn test_sabr_rejects_zero_alpha() {
    let result = SABRParameters::new(0.0, 0.5, 0.3, 0.1);
    assert!(result.is_err(), "Zero alpha should be rejected");

    let err = result.expect_err("should fail");
    assert!(matches!(err, finstack_quant_core::Error::Validation(_)));
}

#[test]
fn test_sabr_rejects_invalid_rho() {
    // Rho > 1
    let result1 = SABRParameters::new(0.2, 0.5, 0.3, 1.5);
    assert!(result1.is_err(), "Rho > 1 should be rejected");
    assert!(matches!(
        result1.expect_err("should fail"),
        finstack_quant_core::Error::Validation(_)
    ));

    // Rho < -1
    let result2 = SABRParameters::new(0.2, 0.5, 0.3, -1.5);
    assert!(result2.is_err(), "Rho < -1 should be rejected");
    assert!(matches!(
        result2.expect_err("should fail"),
        finstack_quant_core::Error::Validation(_)
    ));

    // Rho = exactly 1.0 should be OK
    let result3 = SABRParameters::new(0.2, 0.5, 0.3, 1.0);
    assert!(result3.is_ok(), "Rho = 1.0 is valid");

    // Rho = exactly -1.0 should be OK
    let result4 = SABRParameters::new(0.2, 0.5, 0.3, -1.0);
    assert!(result4.is_ok(), "Rho = -1.0 is valid");
}

#[test]
fn test_sabr_rejects_negative_nu() {
    let result = SABRParameters::new(0.2, 0.5, -0.1, 0.1);
    assert!(result.is_err(), "Negative nu should be rejected");

    let err = result.expect_err("should fail");
    assert!(matches!(err, finstack_quant_core::Error::Validation(_)));

    // Verify error message mentions nu
    let err_str = format!("{}", err);
    assert!(err_str.contains("nu") || err_str.contains("ν"));
}

#[test]
fn test_sabr_rejects_invalid_beta() {
    // Beta > 1
    let result1 = SABRParameters::new(0.2, 1.5, 0.3, 0.1);
    assert!(result1.is_err(), "Beta > 1 should be rejected");
    assert!(matches!(
        result1.expect_err("should fail"),
        finstack_quant_core::Error::Validation(_)
    ));

    // Beta < 0
    let result2 = SABRParameters::new(0.2, -0.1, 0.3, 0.1);
    assert!(result2.is_err(), "Beta < 0 should be rejected");
    assert!(matches!(
        result2.expect_err("should fail"),
        finstack_quant_core::Error::Validation(_)
    ));

    // Beta = 0 should be OK (normal SABR)
    let result3 = SABRParameters::new(0.2, 0.0, 0.3, 0.1);
    assert!(result3.is_ok(), "Beta = 0 is valid (normal SABR)");

    // Beta = 1 should be OK (lognormal SABR)
    let result4 = SABRParameters::new(0.2, 1.0, 0.3, 0.1);
    assert!(result4.is_ok(), "Beta = 1 is valid (lognormal SABR)");
}

#[test]
fn test_sabr_accepts_boundary_values() {
    // Test that exact boundary values are accepted
    assert!(SABRParameters::new(1e-10, 0.0, 0.0, -1.0).is_ok());
    assert!(SABRParameters::new(1e-10, 1.0, 0.0, 1.0).is_ok());
    assert!(SABRParameters::new(0.001, 0.5, 0.0, 0.0).is_ok());
}

// ===================================================================
// Inverse Normal CDF Precision Tests
// ===================================================================

#[test]
fn test_normal_inverse_cdf_precision() {
    // Test that the inverse CDF has high precision for tail probabilities.
    // These golden values are from high-precision statistical tables.

    // Standard values
    assert!(
        (finstack_quant_core::math::standard_normal_inv_cdf(0.5) - 0.0).abs() < 1e-12,
        "CDF^-1(0.5) should be 0"
    );
    assert!(
        (finstack_quant_core::math::standard_normal_inv_cdf(0.84134474606854) - 1.0).abs() < 1e-8,
        "CDF^-1(0.84134...) should be ~1.0"
    );
    assert!(
        (finstack_quant_core::math::standard_normal_inv_cdf(0.97724986805182) - 2.0).abs() < 1e-8,
        "CDF^-1(0.97724...) should be ~2.0"
    );

    // Tail precision test: p = 1e-8 should give approximately -5.6120
    // (from scipy.stats.norm.ppf(1e-8) = -5.612001244174965)
    let tail_result = finstack_quant_core::math::standard_normal_inv_cdf(1e-8);
    assert!(
        (tail_result - (-5.612001244174965)).abs() < 1e-6,
        "Tail precision: CDF^-1(1e-8) = {} should be ~-5.612",
        tail_result
    );

    // Upper tail: p = 1 - 1e-8 should give approximately +5.6120
    let upper_tail_result = finstack_quant_core::math::standard_normal_inv_cdf(1.0 - 1e-8);
    assert!(
        (upper_tail_result - 5.612001244174965).abs() < 1e-6,
        "Upper tail precision: CDF^-1(1-1e-8) = {} should be ~5.612",
        upper_tail_result
    );

    // Extreme tail: p = 1e-15 should give approximately -7.941
    let extreme_tail = finstack_quant_core::math::standard_normal_inv_cdf(1e-15);
    assert!(
        (extreme_tail - (-7.941397804)).abs() < 1e-4,
        "Extreme tail: CDF^-1(1e-15) = {} should be ~-7.941",
        extreme_tail
    );
}

#[test]
fn test_normal_inverse_cdf_boundary_behavior() {
    // Edge cases: boundaries should return appropriate infinity values
    assert!(
        finstack_quant_core::math::standard_normal_inv_cdf(0.0).is_infinite()
            && finstack_quant_core::math::standard_normal_inv_cdf(0.0) < 0.0,
        "CDF^-1(0) should be -infinity"
    );
    assert!(
        finstack_quant_core::math::standard_normal_inv_cdf(1.0).is_infinite()
            && finstack_quant_core::math::standard_normal_inv_cdf(1.0) > 0.0,
        "CDF^-1(1) should be +infinity"
    );

    // Values very close to boundaries
    let near_zero = finstack_quant_core::math::standard_normal_inv_cdf(1e-300);
    assert!(near_zero < -30.0, "CDF^-1(1e-300) should be very negative");

    let near_one = finstack_quant_core::math::standard_normal_inv_cdf(1.0 - 1e-300);
    assert!(near_one > 30.0, "CDF^-1(1-1e-300) should be very positive");
}

// ===================================================================
// Arbitrage Validation Tests
// ===================================================================

#[test]
fn test_sabr_arbitrage_validation_clean_smile() {
    // Well-behaved SABR parameters should produce arbitrage-free smile
    let params = SABRParameters::new(0.2, 0.5, 0.3, -0.2).expect("Valid SABR parameters");
    let model = SABRModel::new(params);
    let smile = SABRSmile::new(model, 100.0, 1.0);

    let strikes: Vec<f64> = (70..=130).step_by(5).map(|k| k as f64).collect();
    let r = 0.05;
    let q = 0.02;

    let result = smile
        .validate_no_arbitrage(&strikes, r, q)
        .expect("Validation should succeed");

    assert!(
        result.is_arbitrage_free(),
        "Standard SABR parameters should be arbitrage-free. \
         Butterfly violations: {}, Monotonicity violations: {}",
        result.butterfly_violations.len(),
        result.monotonicity_violations.len()
    );
}

#[test]
fn test_sabr_arbitrage_check_api() {
    // Test the simplified check API
    let params = SABRParameters::new(0.2, 0.5, 0.3, -0.2).expect("Valid SABR parameters");
    let model = SABRModel::new(params);
    let smile = SABRSmile::new(model, 100.0, 1.0);

    let strikes: Vec<f64> = (80..=120).step_by(5).map(|k| k as f64).collect();

    // Should pass without error
    let check_result = smile.check_no_arbitrage(&strikes, 0.05, 0.02);
    assert!(
        check_result.is_ok(),
        "Clean smile should pass arbitrage check"
    );
}

#[test]
fn test_sabr_arbitrage_validation_result_methods() {
    // Test ArbitrageValidationResult helper methods
    let mut result = ArbitrageValidationResult::default();

    // Empty result should be arbitrage-free
    assert!(result.is_arbitrage_free());
    assert!(result.worst_butterfly_severity().is_none());

    // Add a violation
    result.butterfly_violations.push(ButterflyViolation {
        strike: 100.0,
        butterfly_value: -0.01,
        severity_pct: 0.5,
    });

    assert!(!result.is_arbitrage_free());
    assert!(
        (result
            .worst_butterfly_severity()
            .expect("severity should exist after adding violation")
            - 0.5)
            .abs()
            < 1e-10
    );
}

#[test]
fn test_sabr_arbitrage_too_few_strikes() {
    // With fewer than 3 strikes, validation should return empty result
    let params = SABRParameters::new(0.2, 0.5, 0.3, -0.2).expect("Valid SABR parameters");
    let model = SABRModel::new(params);
    let smile = SABRSmile::new(model, 100.0, 1.0);

    let strikes = vec![95.0, 100.0]; // Only 2 strikes

    let result = smile
        .validate_no_arbitrage(&strikes, 0.05, 0.02)
        .expect("Validation should succeed");

    assert!(
        result.is_arbitrage_free(),
        "With < 3 strikes, no violations should be reported"
    );
}

#[test]
fn test_sabr_arbitrage_extreme_params_may_have_violations() {
    // Extreme parameters might produce arbitrage (this tests detection, not prevention)
    // High vol-of-vol with extreme rho can sometimes produce problematic smiles
    let params = SABRParameters::new(0.5, 0.9, 1.5, 0.8).expect("Valid SABR parameters");
    let model = SABRModel::new(params);
    let smile = SABRSmile::new(model, 100.0, 0.1); // Short expiry

    let strikes: Vec<f64> = (50..=150).step_by(5).map(|k| k as f64).collect();

    // This tests that the validation runs without panicking
    // The result may or may not have violations depending on exact parameters
    let result = smile.validate_no_arbitrage(&strikes, 0.05, 0.02);
    assert!(result.is_ok(), "Validation should complete without error");
}

#[test]
fn test_sabr_new_with_shift_rejects_non_positive_shift() {
    let zero_shift = SABRParameters::new_with_shift(0.2, 0.5, 0.3, -0.2, 0.0);
    let negative_shift = SABRParameters::new_with_shift(0.2, 0.5, 0.3, -0.2, -0.01);

    for result in [zero_shift, negative_shift] {
        let err = result.expect_err("non-positive shifts should fail");
        let err_text = err.to_string();
        assert!(
            err_text.contains("shift parameter must be positive"),
            "unexpected error: {err_text}"
        );
    }
}

#[test]
fn test_sabr_validate_inputs_covers_standard_and_shifted_branches() {
    let standard =
        SABRModel::new(SABRParameters::new(0.2, 0.5, 0.3, -0.2).expect("valid standard params"));
    assert!(standard.validate_inputs(100.0, 110.0, 1.0).is_ok());

    let time_err = standard
        .validate_inputs(100.0, 110.0, 0.0)
        .expect_err("non-positive expiry should fail");
    assert!(time_err.to_string().contains("time_to_expiry"));

    let standard_rate_err = standard
        .validate_inputs(-0.01, 0.02, 1.0)
        .expect_err("unshifted SABR should reject non-positive rates");
    assert!(standard_rate_err.to_string().contains("positive rates"));

    let shifted = SABRModel::new(
        SABRParameters::new_with_shift(0.2, 0.5, 0.3, -0.2, 0.02).expect("valid shifted params"),
    );
    assert!(shifted.validate_inputs(-0.005, 0.0, 1.0).is_ok());

    let shifted_rate_err = shifted
        .validate_inputs(-0.03, -0.02, 1.0)
        .expect_err("effective non-positive shifted rates should fail");
    assert!(shifted_rate_err
        .to_string()
        .contains("effective rates must be positive"));
}

#[test]
fn test_sabr_implied_volatility_rejects_nonpositive_time_to_expiry() {
    // `implied_volatility` must wire in `validate_inputs`: a non-positive
    // `time_to_expiry` is rejected up front with a clear error rather than
    // flowing silently into the time-correction factor.
    let model =
        SABRModel::new(SABRParameters::new(0.2, 0.5, 0.3, -0.2).expect("valid standard params"));

    let forward = 100.0;
    let strike = 110.0;

    for bad_expiry in [0.0, -1.0] {
        let err = model
            .implied_volatility(forward, strike, bad_expiry)
            .expect_err("non-positive time_to_expiry must error");
        assert!(
            err.to_string().contains("time_to_expiry"),
            "error should name the degenerate input, got: {err}"
        );
    }

    // A positive expiry on the same model still succeeds.
    assert!(model.implied_volatility(forward, strike, 1.0).is_ok());
}

/// Regression: the ν→0 (pure CEV) limit must produce a **non-flat** smile for
/// β≠1.
///
/// Failure mode under test (the deleted `test_sabr_nu_zero_short_circuit_*`
/// asserted the *opposite*, incorrect behavior): the old code short-circuited
/// `implied_volatility` to `atm_volatility` for *every* strike whenever
/// `nu.abs() < 1e-14`, fabricating a perfectly flat smile. For β≠1 the SABR/CEV
/// smile is genuinely non-flat: `factor1 = α/(f_mid^(1-β)·correction)` depends
/// on the strike through `f_mid`, and the z/χ(z) ratio is a well-defined limit
/// (→1) as ν→0. Reference: Hagan et al. (2002) eq. 2.17a.
#[test]
fn test_sabr_nu_zero_smile_is_non_flat_for_beta_half() {
    // β = 0.5 (rates-standard CEV), ν exactly 0 → pure CEV smile.
    let params = SABRParameters::new(0.24, 0.5, 0.0, -0.35).expect("valid params");
    let model = SABRModel::new(params);

    let forward = 100.0;
    let expiry = 1.5;

    let vol_itm = model
        .implied_volatility(forward, 80.0, expiry)
        .expect("ITM vol should compute");
    let vol_atm = model
        .implied_volatility(forward, forward, expiry)
        .expect("ATM vol should compute");
    let vol_otm = model
        .implied_volatility(forward, 120.0, expiry)
        .expect("OTM vol should compute");

    // The CEV smile must NOT be flat: a flat-smile short-circuit would make all
    // three equal. With β=0.5 the backbone slopes — ITM > ATM > OTM here.
    let spread = vol_itm - vol_otm;
    assert!(
        spread > 1e-3,
        "ν→0 β=0.5 smile must be non-flat: itm={vol_itm}, atm={vol_atm}, otm={vol_otm}"
    );
    assert!(
        (vol_itm - vol_atm).abs() > 1e-4 && (vol_atm - vol_otm).abs() > 1e-4,
        "ν→0 smile must vary strike-to-strike: itm={vol_itm}, atm={vol_atm}, otm={vol_otm}"
    );

    // The ν→0 ATM point still coincides with `atm_volatility` (true ATM degeneracy).
    let atm_direct = model
        .atm_volatility(forward, expiry)
        .expect("ATM vol should compute");
    assert!(
        (vol_atm - atm_direct).abs() < 1e-10,
        "ATM strike should still match atm_volatility: {vol_atm} vs {atm_direct}"
    );
}

/// The β=0 (normal SABR) time correction is `(2-3ρ²)/24·ν²` ONLY — it must
/// NOT carry an `α²/(24·f_mid²)` leverage term.
///
/// Context: an audit flagged the β=0 branch as "dropping" the
/// `(1-β)²α²/(24·f_mid^(2(1-β)))` leverage term of Hagan eq. 2.17a. That is a
/// false positive — eq. 2.17a is the *lognormal/Black* σ_B expansion. This
/// β=0 code path outputs *normal* (Bachelier) vol: `factor1 = α` is the
/// normal-vol prefactor (not the Black-vol prefactor `α/F^(1-β)`). The β=0
/// *normal*-SABR time correction is the vol-of-vol term only; the α²-leverage
/// term is the normal→Black convexity conversion and lives solely in the
/// Black σ_B formula. Including it would break the exact Bachelier identity
/// (next test) and over-state the normal smile.
///
/// Reference (β=0 normal-SABR, Hagan/Obloj normal-vol expansion), computed
/// independently in Python for α=0.012, β=0, ν=0.25, ρ=-0.20, F=0.03, T=2.0:
///
///   OFF-ATM K=0.045:
///     z       = (ν/α)·(F−K)            = -0.3125
///     χ(z)    = ln((√disc+z−ρ)/(1−ρ))  = -0.317301581174726
///     factor1 = α (f_mid^(1-β)=1)      = 0.012
///     factor2 = z/χ(z)                 = 0.984867452733927
///     tc      = (2-3ρ²)/24·ν²          = 0.004895833333333  (no leverage term)
///     vol     = factor1·factor2·(1+T·tc) = 0.011934131358503
///   ATM K=F=0.03:
///     vol_atm = α·(1+T·tc)             = 0.012117500000000
#[test]
fn test_sabr_beta_zero_time_correction_is_vol_of_vol_only() {
    let params = SABRParameters::new(0.012, 0.0, 0.25, -0.20).expect("valid β=0 params");
    let model = SABRModel::new(params);

    let forward = 0.03_f64;
    let expiry = 2.0_f64;

    // Off-ATM: locks the β=0 branch of `implied_volatility`.
    let off_atm = model
        .implied_volatility(forward, 0.045, expiry)
        .expect("β=0 off-ATM vol should compute");
    assert!(
        (off_atm - 0.011_934_131_358_503).abs() < 1e-12,
        "β=0 off-ATM normal-SABR time correction is vol-of-vol only \
         (no α²/(24·f_mid²) leverage term): got {off_atm:.15}, expected 0.011934131358503"
    );

    // ATM: locks the β=0 branch of `atm_volatility`.
    let atm = model
        .atm_volatility(forward, expiry)
        .expect("β=0 ATM vol should compute");
    assert!(
        (atm - 0.012_117_500_000_000).abs() < 1e-12,
        "β=0 ATM normal-SABR time correction is vol-of-vol only: \
         got {atm:.15}, expected 0.012117500000000"
    );

    // The off-ATM β=0 path and `atm_volatility` must agree at the ATM strike.
    let at_strike = model
        .implied_volatility(forward, forward, expiry)
        .expect("β=0 ATM-strike vol should compute");
    assert!(
        (at_strike - atm).abs() < 1e-12,
        "β=0 ATM-strike implied vol {at_strike} must match atm_volatility {atm}"
    );
}

/// Bachelier identity: β=0, ν=0 ⇒ normal implied vol equals α exactly, flat
/// across strikes and maturities.
///
/// The β=0 SABR SDE is `dF = α·F^0·dW = α·dW` — pure arithmetic Brownian
/// motion with *constant* normal volatility α. Its normal implied vol is
/// therefore exactly α everywhere, with NO time correction. This identity is
/// what makes the (false-positive) "add the α² leverage term to β=0" audit
/// item provably wrong: a leverage term would push β=0/ν=0 ATM vol away from
/// α. This test pins the identity so the β=0 normal branch cannot regress.
#[test]
fn test_sabr_beta_zero_nu_zero_is_flat_bachelier_alpha() {
    let alpha = 0.012_f64;
    let model = SABRModel::new(
        SABRParameters::new(alpha, 0.0, 0.0, 0.0).expect("β=0, ν=0 params are valid"),
    );

    for &forward in &[0.02_f64, 0.03, 0.05] {
        for &expiry in &[0.5_f64, 2.0, 10.0] {
            // ATM.
            let atm = model
                .atm_volatility(forward, expiry)
                .expect("β=0,ν=0 ATM vol should compute");
            assert!(
                (atm - alpha).abs() < 1e-12,
                "β=0,ν=0 ATM normal vol must equal α={alpha} (Bachelier): \
                 got {atm} at F={forward}, T={expiry}"
            );
            // Off-ATM strikes: the normal smile is flat at α.
            for &strike in &[forward * 0.7, forward * 1.4] {
                let vol = model
                    .implied_volatility(forward, strike, expiry)
                    .expect("β=0,ν=0 off-ATM vol should compute");
                assert!(
                    (vol - alpha).abs() < 1e-12,
                    "β=0,ν=0 normal vol must be flat at α={alpha}: got {vol} \
                     at F={forward}, K={strike}, T={expiry}"
                );
            }
        }
    }
}

#[test]
fn test_solve_alpha_for_atm_round_trips_target_vol() {
    let forward = 100.0;
    let time_to_expiry = 2.0;
    let beta = 0.55;
    let nu = 0.42;
    let rho = -0.18;
    let original_alpha = 0.28;

    let original =
        SABRModel::new(SABRParameters::new(original_alpha, beta, nu, rho).expect("valid params"));
    let target_atm = original
        .atm_volatility(forward, time_to_expiry)
        .expect("ATM vol should compute");

    let solved_alpha =
        solve_alpha_for_atm(forward, target_atm, time_to_expiry, beta, nu, rho, 1e-12)
            .expect("alpha solve should succeed");

    let solved = SABRModel::new(
        SABRParameters::new(solved_alpha, beta, nu, rho).expect("solved params should be valid"),
    );
    let solved_atm = solved
        .atm_volatility(forward, time_to_expiry)
        .expect("ATM vol should compute");

    assert!((solved_alpha - original_alpha).abs() < 1e-8);
    assert!((solved_atm - target_atm).abs() < 1e-10);
}

/// `calibrate_with_atm_pinning` must pin to the volatility *interpolated to the
/// forward*, not to the market quote at whatever strike happens to be nearest.
///
/// Failure mode under test: the old `find_atm_vol` returned `vols[i]` for the
/// strike with the smallest `|K − F|`. When the strike grid does not contain F,
/// that nearest quote is genuinely off-ATM, so the "ATM pin" pinned the model
/// to the wrong volatility level. The fix interpolates the smile to F linearly
/// in total variance (σ²·T, equivalently σ² for a fixed-expiry slice).
///
/// Construction: synthesize a smile from a known SABR model and drop the ATM
/// strike from the calibration grid, keeping a *tight* bracket around F so the
/// linear-in-variance interpolation is accurate. The ATM-pinned calibration
/// must then land on a model ATM vol that is substantially closer to the true
/// ATM than the nearest off-ATM market quote — exactly the improvement the
/// interpolation delivers over the old nearest-strike rule.
#[test]
fn test_sabr_atm_pinning_interpolates_when_grid_lacks_forward() {
    let true_params = SABRParameters::new(0.20, 0.5, 0.30, -0.25).expect("valid params");
    let true_model = SABRModel::new(true_params);

    let forward = 100.0_f64;
    let expiry = 1.0_f64;

    // Strike grid deliberately OMITS the forward (100). The inner strikes 98
    // and 102 bracket F tightly; the nearest quote (98 or 102) is still
    // off-ATM, which is what the old nearest-strike pin would have used.
    let strikes = vec![90.0, 98.0, 102.0, 110.0];
    let market_vols: Vec<f64> = strikes
        .iter()
        .map(|&k| {
            true_model
                .implied_volatility(forward, k, expiry)
                .expect("synthetic vol should compute")
        })
        .collect();

    let true_atm = true_model
        .implied_volatility(forward, forward, expiry)
        .expect("true ATM vol should compute");

    // The nearest-strike quote (K=98) — what the OLD `find_atm_vol` would pin
    // to. Assert it is measurably off the true ATM so the test is meaningful.
    let nearest_vol = market_vols[1]; // K = 98
    let nearest_err = (nearest_vol - true_atm).abs();
    assert!(
        nearest_err > 5e-4,
        "test setup: nearest-strike quote {nearest_vol} must be off true ATM {true_atm}"
    );

    // non-convergence is now a hard error, and
    // a 1e-10 gradient tolerance is unattainable for the scalar LM
    // formulation (the vega-weighted SSE stagnates around 6e-7); use an
    // attainable tolerance with a larger budget.
    let calibrated = SABRCalibrator::new()
        .with_tolerance(1e-5)
        .with_max_iterations(1000)
        .calibrate_with_atm_pinning(forward, &strikes, &market_vols, expiry, 0.5)
        .expect("ATM-pinned calibration should succeed");
    let calibrated_model = SABRModel::new(calibrated);

    let calibrated_atm = calibrated_model
        .atm_volatility(forward, expiry)
        .expect("calibrated ATM vol should compute");
    let interp_err = (calibrated_atm - true_atm).abs();

    // The interpolated pin must be substantially closer to the true ATM than
    // the nearest-strike quote — the concrete improvement from the fix.
    assert!(
        interp_err < nearest_err * 0.5,
        "interpolated ATM pin must beat the nearest-strike quote: \
         calibrated_atm={calibrated_atm} (err {interp_err:.6}), \
         nearest quote={nearest_vol} (err {nearest_err:.6}), true_atm={true_atm}"
    );
}

#[test]
fn test_sabr_calibrate_with_atm_pinning_matches_synthetic_smile() {
    let true_params = SABRParameters::new(0.22, 0.6, 0.35, -0.25).expect("valid params");
    let true_model = SABRModel::new(true_params);

    let forward = 100.0;
    let expiry = 1.25;
    let beta = 0.6;
    let strikes = vec![80.0, 90.0, 100.0, 110.0, 120.0];
    let market_vols: Vec<f64> = strikes
        .iter()
        .map(|&strike| {
            true_model
                .implied_volatility(forward, strike, expiry)
                .expect("synthetic vol should compute")
        })
        .collect();

    let calibrated = SABRCalibrator::new()
        .with_tolerance(1e-10)
        .with_max_iterations(200)
        .calibrate_with_atm_pinning(forward, &strikes, &market_vols, expiry, beta)
        .expect("ATM-pinned calibration should succeed");
    let calibrated_model = SABRModel::new(calibrated);

    let atm_idx = strikes
        .iter()
        .position(|&strike| strike == forward)
        .expect("ATM strike should be present");
    let atm_market = market_vols[atm_idx];
    let calibrated_atm = calibrated_model
        .atm_volatility(forward, expiry)
        .expect("ATM vol should compute");
    assert!((calibrated_atm - atm_market).abs() < 1e-8);

    for (strike, market_vol) in strikes.iter().zip(market_vols.iter()) {
        let fitted = calibrated_model
            .implied_volatility(forward, *strike, expiry)
            .expect("fitted vol should compute");
        assert!(
            (fitted - market_vol).abs() < 1e-3,
            "bad fit at strike {strike}: fitted={fitted}, market={market_vol}"
        );
    }
}

#[test]
fn test_sabr_calibrate_with_derivatives_recovers_known_smile() {
    let true_params = SABRParameters::new(0.25, 0.5, 0.45, -0.3).expect("valid params");
    let true_model = SABRModel::new(true_params);

    let forward = 100.0;
    let expiry = 0.75;
    let beta = 0.5;
    let strikes = vec![85.0, 95.0, 100.0, 105.0, 115.0];
    let market_vols: Vec<f64> = strikes
        .iter()
        .map(|&strike| {
            true_model
                .implied_volatility(forward, strike, expiry)
                .expect("synthetic vol should compute")
        })
        .collect();

    // non-convergence is now a hard error;
    // 1e-9 within 200 evals previously "passed" via the silent best-guess
    // fallback (the vega-weighted SSE stagnates around 2e-6). Use an
    // attainable tolerance and budget.
    let params = SABRCalibrator::new()
        .with_tolerance(1e-5)
        .with_max_iterations(1000)
        .calibrate_with_derivatives(forward, &strikes, &market_vols, expiry, beta)
        .expect("derivative calibration should succeed");

    let model = SABRModel::new(params);
    for (strike, market_vol) in strikes.into_iter().zip(market_vols.into_iter()) {
        let fitted = model
            .implied_volatility(forward, strike, expiry)
            .expect("model vol should compute");
        assert!(
            (fitted - market_vol).abs() < 2e-2,
            "fit too loose at strike {strike}: fitted={fitted}, market={market_vol}"
        );
    }
}

/// At Δ=0.5 (forward delta) the call and put strikes coincide at the
/// delta-neutral point `F·exp(σ²T/2)` — slightly above the forward, not at
/// it (N⁻¹(0.5) = 0, leaving only the σ²T/2 convexity term).
#[test]
fn test_sabr_strike_from_delta_half_delta_is_delta_neutral_strike() {
    let params = SABRParameters::new(0.2, 0.5, 0.3, -0.2).expect("valid params");
    let forward = 100.0;
    let expiry = 1.0;
    let model = SABRModel::new(params);
    let atm_vol = model
        .atm_volatility(forward, expiry)
        .expect("ATM vol should compute");
    let smile = SABRSmile::new(model, forward, expiry);

    let call_strike = smile
        .strike_from_delta(0.5, true)
        .expect("call strike should compute");
    let put_strike = smile
        .strike_from_delta(0.5, false)
        .expect("put strike should compute");

    let expected = forward * (0.5 * atm_vol * atm_vol * expiry).exp();
    assert!((call_strike - expected).abs() < 1e-12);
    assert!((put_strike - expected).abs() < 1e-12);
}

/// Round-trip at Δ=0.25: the strike returned by `strike_from_delta` must
/// reproduce the requested forward delta under Black-76 with the same vol,
/// and land on the correct side of the forward (25Δ call above, 25Δ put
/// below). Regression for the inverted formula `K = F·exp(N⁻¹(Δ)·σ√T)`,
/// which put the 25Δ call strike *below* the forward with an actual delta
/// of ~0.78.
#[test]
fn test_sabr_strike_from_delta_round_trip_at_25_delta() {
    use finstack_quant_core::math::norm_cdf;

    let params = SABRParameters::new(0.2, 0.5, 0.3, -0.2).expect("valid params");
    let forward = 100.0;
    let expiry = 1.0;
    let model = SABRModel::new(params);
    let atm_vol = model
        .atm_volatility(forward, expiry)
        .expect("ATM vol should compute");
    let smile = SABRSmile::new(model, forward, expiry);

    let sigma_sqrt_t = atm_vol * expiry.sqrt();
    let d1 =
        |strike: f64| ((forward / strike).ln() + 0.5 * atm_vol * atm_vol * expiry) / sigma_sqrt_t;

    let call_strike = smile
        .strike_from_delta(0.25, true)
        .expect("call strike should compute");
    assert!(
        call_strike > forward,
        "25Δ call strike {call_strike} must be above the forward {forward}"
    );
    let call_delta = norm_cdf(d1(call_strike));
    assert!(
        (call_delta - 0.25).abs() < 1e-10,
        "25Δ call round-trip failed: actual delta {call_delta}"
    );

    let put_strike = smile
        .strike_from_delta(0.25, false)
        .expect("put strike should compute");
    assert!(
        put_strike < forward,
        "25Δ put strike {put_strike} must be below the forward {forward}"
    );
    let put_delta = norm_cdf(-d1(put_strike));
    assert!(
        (put_delta - 0.25).abs() < 1e-10,
        "25Δ put round-trip failed: actual |delta| {put_delta}"
    );
}

// ===================================================================
// Hagan (1-beta) exponent in the vol denominator
// ===================================================================

/// At β=1 (lognormal SABR), ν=0 (no vol-of-vol), ρ=0, the model reduces to
/// dF = α·F·dW, i.e. a pure GBM with constant log-vol α.
///
/// Hagan et al. (2002) eq. 2.18 (ATM formula):
///
///   σ_ATM = α / F^(1-β) · [1 + ((1-β)²α²/(24·F^(2(1-β))) + ρβνα/(4·F^(1-β)) + (2-3ρ²)ν²/24)·T]
///
/// At β=1: F^(1-β) = F^0 = 1, (1-β)² = 0, ν=0 so all time-correction terms vanish:
///
///   σ_ATM = α · 1 · [1 + 0] = α   (for any forward F)
///
/// The buggy code uses F^β = F^1 = F in the denominator instead of F^(1-β) = 1,
/// so it returns α/F, which equals α only at F=1.
#[test]
fn sabr_beta_one_atm_recovers_alpha() {
    let alpha = 0.20_f64;
    let beta = 1.0_f64;
    let nu = 0.0_f64; // no vol-of-vol: forces ATM path, pure GBM
    let rho = 0.0_f64;

    let params = SABRParameters::new(alpha, beta, nu, rho)
        .expect("β=1 lognormal SABR params should be valid");
    let model = SABRModel::new(params);

    for &fwd in &[0.01_f64, 1.0, 100.0, 4000.0] {
        let vol = model
            .implied_volatility(fwd, fwd, 1.0)
            .expect("ATM vol should compute for β=1, ν=0");
        assert!(
            (vol - alpha).abs() < 1e-10,
            "β=1, ν=0 ATM vol should equal α={alpha} for F={fwd}, got {vol}"
        );
    }
}

/// Reference value derivation for α=0.20, β=1, ν=0.30, ρ=-0.30, F=100, K=120, T=1.
///
/// Hagan et al. (2002) eq. 2.17a (off-ATM, with Obloj correction absorbed into
/// the β=1 exact formula):
///
///   σ_B = (α / f_mid^(1-β)) · (z / χ(z)) · factor3
///
///   where:
///     f_mid = sqrt(F·K)    = sqrt(100·120) = 109.5445115...
///     At β=1: f_mid^(1-β) = f_mid^0       = 1.0
///     z      = (ν/α)·ln(F/K)               = (0.30/0.20)·ln(100/120)
///            = 1.5·(-0.18232155...)         = -0.27348233519...
///     disc   = 1 - 2ρz + z²  = 1 - 2(-0.30)(-0.27348...) + (-0.27348...)²
///            = 1 - 0.16409...  + 0.07479... = 0.91071...
///     χ(z)   = ln((sqrt(disc) + z - ρ)/(1-ρ))
///            = ln((0.95431...  + (-0.27348...) - (-0.30)) / (1 - (-0.30)))
///            = ln(0.98083... / 1.30)          = ln(0.75448...)
///            = -0.28173...
///     z/χ(z) = (-0.27348...) / (-0.28173...) = 0.97074...
///
///   factor3 at β=1 (the (1-β)² and (1-β)⁴ log-moneyness terms vanish):
///     term_a = (1-β)²α²/(24·f_mid^(2(1-β))) = 0
///     term_b = 0.25·ρ·β·ν·α / f_mid^(1-β)   = 0.25·(-0.30)·1·0.30·0.20 / 1
///            = -0.0045
///     term_c = (2 - 3ρ²)·ν² / 24             = (2 - 3·0.09)·0.09/24
///            = 1.73·0.09/24                   = 0.0064875
///     time_correction = 0 + (-0.0045) + 0.0064875 = 0.0019875
///     factor3 = 1 + 1.0 · 0.0019875           = 1.0019875
///
///   σ_B = (0.20 / 1.0) · 0.97074... · 1.0019875 = 0.19453422...
///
/// Computed independently in Python (no codebase dependency):
///   >>> import math
///   >>> alpha,beta,nu,rho,F,K,T = 0.20,1.0,0.30,-0.30,100.0,120.0,1.0
///   >>> f_mid=math.sqrt(F*K); z=(nu/alpha)*math.log(F/K)
///   >>> disc=1-2*rho*z+z*z; chi=math.log((disc**0.5+z-rho)/(1-rho))
///   >>> tc=0.25*rho*beta*nu*alpha+(2-3*rho**2)*nu**2/24
///   >>> vol=(alpha/(f_mid**(1-beta)))*(z/chi)*(1+T*tc)
///   >>> round(vol,10)   →  0.1945342213
#[test]
fn sabr_beta_one_smile_matches_hagan_reference() {
    let alpha = 0.20_f64;
    let beta = 1.0_f64;
    let nu = 0.30_f64;
    let rho = -0.30_f64;
    let forward = 100.0_f64;
    let strike = 120.0_f64;
    let expiry = 1.0_f64;

    // Reference: 0.1945342213 (see derivation in doc-comment above)
    let reference_vol = 0.194_534_221_258_664_37_f64;

    let params = SABRParameters::new(alpha, beta, nu, rho)
        .expect("β=1 lognormal SABR params should be valid");
    let model = SABRModel::new(params);

    let vol = model
        .implied_volatility(forward, strike, expiry)
        .expect("OTM vol should compute for β=1");

    assert!(
        (vol - reference_vol).abs() < 1e-6,
        "β=1 Hagan reference mismatch: got {vol:.10}, expected {reference_vol:.10}"
    );
}

/// `calibrate` and `calibrate_with_derivatives` minimize the same
/// vega-weighted objective, so on a skewed smile they must agree on the
/// calibrated (α, ν, ρ).
///
/// Regression for the bug where `calibrate_with_derivatives` fed LM the
/// gradient of the *unweighted* SSE while the objective was vega-weighted,
/// converging to the wrong problem's stationary point.
#[test]
fn test_sabr_calibrate_and_calibrate_with_derivatives_agree() {
    // ATM lognormal vol ≈ α/√F = 0.20, so the ±20% wings sit ~1σ out and
    // carry genuine vega weight.
    let true_params = SABRParameters::new(2.0, 0.5, 0.5, -0.35).expect("valid params");
    let true_model = SABRModel::new(true_params);

    let forward = 100.0;
    let expiry = 1.0;
    let beta = 0.5;
    let strikes = vec![80.0, 90.0, 100.0, 110.0, 120.0];
    let market_vols: Vec<f64> = strikes
        .iter()
        .map(|&strike| {
            true_model
                .implied_volatility(forward, strike, expiry)
                .expect("synthetic vol should compute")
        })
        .collect();

    // non-convergence is now a hard error;
    // 1e-10 previously "passed" via the silent best-guess fallback. Use an
    // attainable tolerance and budget.
    let calibrator = SABRCalibrator::new()
        .with_tolerance(1e-7)
        .with_max_iterations(1000);

    let fd_free = calibrator
        .calibrate(forward, &strikes, &market_vols, expiry, beta)
        .expect("calibrate should succeed");
    let with_derivs = calibrator
        .calibrate_with_derivatives(forward, &strikes, &market_vols, expiry, beta)
        .expect("calibrate_with_derivatives should succeed");

    assert!(
        (fd_free.alpha - with_derivs.alpha).abs() < 1e-3,
        "alpha disagreement: {} vs {}",
        fd_free.alpha,
        with_derivs.alpha
    );
    assert!(
        (fd_free.nu - with_derivs.nu).abs() < 1e-2,
        "nu disagreement: {} vs {}",
        fd_free.nu,
        with_derivs.nu
    );
    assert!(
        (fd_free.rho - with_derivs.rho).abs() < 1e-2,
        "rho disagreement: {} vs {}",
        fd_free.rho,
        with_derivs.rho
    );

    // Both must reprice the synthetic smile.
    let model = SABRModel::new(with_derivs);
    for (strike, market_vol) in strikes.iter().zip(market_vols.iter()) {
        let fitted = model
            .implied_volatility(forward, *strike, expiry)
            .expect("fitted vol should compute");
        assert!(
            (fitted - market_vol).abs() < 5e-4,
            "calibrate_with_derivatives misfit at strike {strike}: \
             fitted={fitted:.6}, market={market_vol:.6}"
        );
    }
}

/// Normal-convention (β=0) calibration must actually fit the smile wings.
///
/// Regression for the vega-weighting convention bug: ~1% normal vols fed to
/// the *lognormal* Black vega collapse every wing weight to the 1e-10 floor,
/// so LM declares convergence at the initial guess (ν=0.3, ρ=0.0) without
/// fitting anything. With Bachelier vega the calibration must recover a
/// synthetic skewed β=0 smile to sub-bp accuracy **unweighted**.
#[test]
fn test_sabr_normal_convention_calibration_reprices_wings_unweighted() {
    // Skewed normal smile generated from known β=0 parameters far from the
    // optimizer's initial guess (ν=0.3, ρ=0.0).
    let true_params = SABRParameters::new(0.0085, 0.0, 0.55, -0.4).expect("valid β=0 params");
    let true_model = SABRModel::new(true_params);

    let forward = 0.03_f64;
    let expiry = 1.0_f64;
    let strikes = vec![0.01, 0.02, 0.03, 0.04, 0.05];
    let market_vols: Vec<f64> = strikes
        .iter()
        .map(|&strike| {
            true_model
                .implied_volatility(forward, strike, expiry)
                .expect("synthetic normal vol should compute")
        })
        .collect();

    // Sanity: these are normal vols (~80–120bp), not lognormal levels.
    assert!(market_vols.iter().all(|&v| v > 0.004 && v < 0.02));
    // Sanity: the smile is genuinely skewed — the initial guess can't fit it.
    assert!((market_vols[0] - market_vols[4]).abs() > 5e-4);

    let calibrated = SABRCalibrator::new()
        .with_tolerance(1e-10)
        .with_max_iterations(300)
        .calibrate_with_atm_pinning(forward, &strikes, &market_vols, expiry, 0.0)
        .expect("normal-convention calibration should succeed");

    // The optimizer must have moved off the initial guess.
    assert!(
        (calibrated.nu - 0.3).abs() > 0.05 || calibrated.rho.abs() > 0.05,
        "calibration did not move off the initial guess: nu={}, rho={}",
        calibrated.nu,
        calibrated.rho
    );

    // Unweighted wing repricing: every strike within 0.5 normal bp.
    let calibrated_model = SABRModel::new(calibrated);
    for (strike, market_vol) in strikes.iter().zip(market_vols.iter()) {
        let fitted = calibrated_model
            .implied_volatility(forward, *strike, expiry)
            .expect("fitted normal vol should compute");
        assert!(
            (fitted - market_vol).abs() < 5e-5,
            "wing not repriced at strike {strike}: fitted={fitted:.8}, market={market_vol:.8}"
        );
    }
}

/// The χ(z) Taylor series must agree with the exact formula near the series
/// crossover (`|z| ≈ 1e-5`) and through the blend region, for a range of ρ.
/// Guards the c3 = (3ρ²−1)/6 and c4 = ρ(5ρ²−3)/8 coefficients.
#[test]
fn test_chi_series_matches_exact_near_crossover() {
    for &rho in &[-0.9, -0.5, -0.1, 0.0, 0.1, 0.5, 0.9] {
        let params =
            SABRParameters::new(0.2, 0.5, 0.3, rho).expect("SABR parameters should be valid");
        let model = SABRModel::new(params);

        for &z in &[-1e-3, -1e-4, -2e-5, -9e-6, 9e-6, 2e-5, 1e-4, 1e-3] {
            // Exact χ(z) = ln((√(1−2ρz+z²)+z−ρ)/(1−ρ)), well-conditioned here.
            let disc = (1.0 - 2.0 * rho * z + z * z).sqrt();
            let exact = ((disc + z - rho) / (1.0 - rho)).ln();
            let robust = model
                .calculate_chi_robust(z)
                .expect("chi should compute for small z");
            // Tolerance: series truncation is O(z⁵); the dominant term is the
            // ~1e-16/(1−ρ) floating-point noise of the exact reference itself.
            assert!(
                (robust - exact).abs() < 1e-10 * z.abs() + 1e-14,
                "chi mismatch at rho={rho}, z={z:e}: robust={robust:e}, exact={exact:e}"
            );

            // z/χ(z) Taylor ratio must be consistent with the same expansion.
            let ratio = model
                .z_over_chi(z, robust)
                .expect("z/chi should compute for small z");
            assert!(
                (ratio - z / exact).abs() < 1e-10,
                "z/chi mismatch at rho={rho}, z={z:e}: ratio={ratio:e}, exact={:e}",
                z / exact
            );
        }
    }
}
