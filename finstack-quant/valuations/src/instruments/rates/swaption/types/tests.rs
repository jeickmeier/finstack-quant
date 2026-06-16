//! Tests for SABR-to-normal vol conversion.
//!
//! The conversion formula is validated against:
//! 1. ATM limit: σ_N ≈ σ_LN × F (simple approximation)
//! 2. Non-ATM: σ_N ≈ σ_LN × (F-K)/ln(F/K) × [1 - σ²T/24 × (1 - ln²(F/K)/12)]
//! 3. Convergence: as K → F, the formula converges to the ATM limit

use super::lognormal_to_normal_vol;

/// Test the lognormal-to-normal vol conversion formula at ATM.
///
/// At ATM (F = K), the formula should give:
/// σ_N ≈ σ_LN × F × (1 - σ_LN²T/24)
#[test]
fn test_lognormal_to_normal_vol_atm() {
    let f: f64 = 0.03; // 3% forward rate
    let sigma_ln: f64 = 0.20; // 20% lognormal vol
    let t: f64 = 1.0; // 1 year

    // Expected: σ_N ≈ σ_LN × F × (1 - σ²T/24)
    let correction = 1.0 - (sigma_ln * sigma_ln * t) / 24.0;
    let expected_sigma_n = sigma_ln * f * correction;

    let computed_sigma_n = lognormal_to_normal_vol(sigma_ln, f, f, t, None);

    // Should be very close at ATM
    assert!(
        (computed_sigma_n - expected_sigma_n).abs() < 1e-10,
        "ATM vol conversion failed: computed={:.6}, expected={:.6}",
        computed_sigma_n,
        expected_sigma_n
    );
}

/// Test the lognormal-to-normal vol conversion formula for OTM options.
#[test]
fn test_lognormal_to_normal_vol_otm() {
    let f: f64 = 0.03; // 3% forward rate
    let k: f64 = 0.04; // 4% strike (OTM call / ITM put)
    let sigma_ln: f64 = 0.20; // 20% lognormal vol
    let t: f64 = 1.0; // 1 year

    let sigma_n = lognormal_to_normal_vol(sigma_ln, f, k, t, None);

    // Normal vol should be positive and reasonable
    assert!(sigma_n > 0.0, "Normal vol should be positive");
    // Normal vol for rates is typically in bp terms (0.001 = 10bp)
    // For 20% lognormal vol on 3% rates, expect ~60bp = 0.006
    assert!(
        sigma_n > 0.002 && sigma_n < 0.02,
        "Normal vol {} seems unreasonable for 20% lognormal on 3% rates",
        sigma_n
    );
}

/// Test that the formula converges smoothly as K → F (no discontinuity).
#[test]
fn test_lognormal_to_normal_vol_convergence() {
    let f: f64 = 0.03;
    let sigma_ln: f64 = 0.20;
    let t: f64 = 1.0;

    // Compute at exactly ATM
    let sigma_n_atm = lognormal_to_normal_vol(sigma_ln, f, f, t, None);

    // Compute at K very close to F
    for delta in [1e-6_f64, 1e-8_f64, 1e-10_f64] {
        let k = f * (1.0 + delta);
        let sigma_n = lognormal_to_normal_vol(sigma_ln, f, k, t, None);

        // Should converge to ATM value
        let diff = (sigma_n - sigma_n_atm).abs();
        assert!(
            diff < delta * 10.0,
            "Convergence failure at delta={}: diff={:.2e}",
            delta,
            diff
        );
    }
}

/// Test that the correction factor stays in reasonable bounds.
#[test]
fn test_correction_factor_bounds() {
    // High vol, long maturity: correction should be floored near 0.5
    let f: f64 = 0.03;
    let sigma_ln: f64 = 0.80; // 80% vol (extreme)
    let t: f64 = 30.0; // 30 years

    let sigma_n = lognormal_to_normal_vol(sigma_ln, f, f, t, None);

    // Even with extreme parameters, result should be positive and bounded
    assert!(sigma_n > 0.0, "Normal vol should be positive");

    // The correction should floor near 0.5, so normal vol ≈ σ_LN × F × 0.5
    let approx_floor = sigma_ln * f * 0.5;
    assert!(
        sigma_n >= approx_floor * 0.9, // Allow some tolerance from hard floor at 0.5
        "Correction floor should prevent unreasonably low vol: got {}, expected >= {}",
        sigma_n,
        approx_floor * 0.9
    );
}

/// Test shifted SABR lognormal-to-normal conversion for negative rates.
///
/// With a shift, negative rates become positive in the shifted domain,
/// allowing the standard lognormal-to-normal approximation to apply.
#[test]
fn test_lognormal_to_normal_vol_shifted_negative_rates() {
    // EUR-like scenario: negative forward and strike
    let f: f64 = -0.005; // -0.5% forward rate
    let k: f64 = -0.003; // -0.3% strike
    let sigma_ln: f64 = 0.30; // 30% lognormal vol (on shifted rates)
    let t: f64 = 1.0;
    let shift = 0.03; // 3% shift (standard for EUR)

    let sigma_n = lognormal_to_normal_vol(sigma_ln, f, k, t, Some(shift));

    // With shift: F_eff = -0.5% + 3% = 2.5%, K_eff = -0.3% + 3% = 2.7%
    // Both positive, so standard approximation applies
    assert!(sigma_n > 0.0, "Normal vol should be positive with shift");

    // For 30% lognormal vol on ~2.5% shifted rates, expect ~75bp = 0.0075
    assert!(
        sigma_n > 0.003 && sigma_n < 0.02,
        "Shifted normal vol {} seems unreasonable",
        sigma_n
    );

    // Without shift, should still produce a positive result (fallback)
    let sigma_n_no_shift = lognormal_to_normal_vol(sigma_ln, f, k, t, None);
    assert!(
        sigma_n_no_shift > 0.0,
        "Fallback should produce positive vol"
    );
}

/// Test that shifted conversion is consistent with unshifted for positive rates.
#[test]
fn test_shifted_vs_unshifted_positive_rates() {
    let f: f64 = 0.03;
    let k: f64 = 0.035;
    let sigma_ln: f64 = 0.20;
    let t: f64 = 1.0;

    // With zero shift, should give same result as no shift
    let sigma_n_none = lognormal_to_normal_vol(sigma_ln, f, k, t, None);
    let sigma_n_zero = lognormal_to_normal_vol(sigma_ln, f, k, t, Some(0.0));

    assert!(
        (sigma_n_none - sigma_n_zero).abs() < 1e-12,
        "Zero shift should match no shift: none={}, zero={}",
        sigma_n_none,
        sigma_n_zero
    );
}
