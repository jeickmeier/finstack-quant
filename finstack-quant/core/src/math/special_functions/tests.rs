#[test]
fn logarithmic_normal_cdf_preserves_underflowed_tails() {
    // High-precision log(erfc(-x/sqrt(2))/2) reference values.
    for (x, expected) in [
        (-10.0, -53.231_285_150_512_47),
        (-40.0, -804.608_442_013_753_8),
        (-100.0, -5_005.524_208_694_205),
    ] {
        assert!((super::log_norm_cdf(x).expect("valid variate") - expected).abs() < 2e-12);
    }
    for x in [-9.999, -5.0, -1.0, 0.0, 1.0, 5.0] {
        assert!(
            (super::log_norm_cdf(x).expect("valid variate").exp()
                - super::norm_cdf(x).expect("valid variate"))
            .abs()
                < 2e-15
        );
    }
    assert_eq!(
        super::log_norm_cdf(f64::NEG_INFINITY).expect("valid variate"),
        f64::NEG_INFINITY
    );
    assert_eq!(
        super::log_norm_cdf(f64::INFINITY).expect("valid variate"),
        0.0
    );
    assert!(super::log_norm_cdf(f64::NAN).is_err());
    // The probability itself is unrepresentable, but its reflected
    // product is an ordinary finite number.
    let reflected = (800.0 + super::log_norm_cdf(-40.0).expect("valid variate")).exp();
    assert!((reflected - 0.009_967_335_188_301_31).abs() < 2e-15);
}

use super::*;

#[test]
fn student_t_cdf_rejects_nan() {
    // NaN is outside the Student-t variate domain.
    assert!(student_t_cdf(f64::NAN, 5.0).is_err());
    assert!(student_t_cdf(f64::NAN, 0.0).is_err());
}

#[test]
fn test_standard_normal_inv_cdf_domain_boundaries_and_out_of_domain() {
    assert!(standard_normal_inv_cdf(f64::NAN).is_err());
    assert!(standard_normal_inv_cdf(-0.1).is_err());
    assert!(standard_normal_inv_cdf(-1e-15).is_err());
    assert_eq!(
        standard_normal_inv_cdf(0.0).expect("valid probability"),
        f64::NEG_INFINITY
    );
    assert_eq!(
        standard_normal_inv_cdf(1.0).expect("valid probability"),
        f64::INFINITY
    );
    assert!(standard_normal_inv_cdf(1.000_000_000_000_000_2).is_err());
}

#[test]
fn test_student_t_inv_cdf_domain_boundaries_and_out_of_domain() {
    for df in [5.0, 1000.0] {
        assert!(student_t_inv_cdf(f64::NAN, df).is_err(), "df={df}");
        assert!(student_t_inv_cdf(-0.1, df).is_err(), "df={df}");
        assert!(
            student_t_inv_cdf(1.000_000_000_000_000_2, df).is_err(),
            "df={df}"
        );
        assert_eq!(
            student_t_inv_cdf(0.0, df).expect("valid boundary 0.0"),
            f64::NEG_INFINITY,
            "df={df}"
        );
        assert_eq!(
            student_t_inv_cdf(1.0, df).expect("valid boundary 1.0"),
            f64::INFINITY,
            "df={df}"
        );
    }
}

#[test]
fn invalid_student_t_df_is_consistent_across_build_profiles() {
    for df in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        assert!(student_t_cdf(1.0, df).is_err());
        assert!(student_t_inv_cdf(0.95, df).is_err());
    }
}

#[test]
fn student_t_large_df_uses_exact_tail_not_normal_cutoff() {
    use statrs::distribution::{ContinuousCDF, StudentsT};

    let dist = StudentsT::new(0.0, 1.0, 101.0).unwrap();
    let exact_cdf = dist.cdf(-3.0);
    let exact_quantile = dist.inverse_cdf(0.001);

    assert!(
        (student_t_cdf(-3.0, 101.0).expect("valid checked inputs") - exact_cdf).abs() < 1e-14,
        "df=101 CDF should use exact Student-t tail"
    );
    assert!(
        (student_t_inv_cdf(0.001, 101.0).expect("valid checked inputs") - exact_quantile).abs()
            < 1e-12,
        "df=101 inverse CDF should use exact Student-t tail"
    );
}

#[test]
fn test_erf() {
    // Test known values with reasonable tolerance for approximation
    assert!(
        (erf(0.0).expect("valid variate") - 0.0).abs() < 1e-6,
        "erf(0) should be 0, got {}",
        erf(0.0).expect("valid variate")
    );
    assert!(
        (erf(1.0).expect("valid variate") - 0.8427007929).abs() < 1e-4,
        "erf(1) should be ~0.8427, got {}",
        erf(1.0).expect("valid variate")
    );
    assert!(
        (erf(-1.0).expect("valid variate") - (-0.8427007929)).abs() < 1e-4,
        "erf(-1) should be ~-0.8427, got {}",
        erf(-1.0).expect("valid variate")
    );
}

#[test]
fn test_norm_cdf() {
    // Test known values
    assert!((norm_cdf(0.0).expect("valid variate") - 0.5).abs() < 1e-6);
    assert!((norm_cdf(1.0).expect("valid variate") - 0.8413447460685429).abs() < 1e-6);
    assert!((norm_cdf(-1.0).expect("valid variate") - 0.15865525393145705).abs() < 1e-6);

    // Test extreme values
    assert!(norm_cdf(-10.0).expect("valid variate") < 1e-10);
    assert!(norm_cdf(10.0).expect("valid variate") > 1.0 - 1e-10);
}

#[test]
fn test_norm_pdf() {
    use std::f64::consts::PI;
    // Test known values
    assert!((norm_pdf(0.0).expect("valid variate") - (1.0 / (2.0 * PI).sqrt())).abs() < 1e-12);

    // Test symmetry
    assert!(
        (norm_pdf(1.0).expect("valid variate") - norm_pdf(-1.0).expect("valid variate")).abs()
            < 1e-12
    );

    // Test non-negativity
    assert!(norm_pdf(5.0).expect("valid variate") >= 0.0);
}

#[test]
fn test_standard_normal_inv_cdf() {
    // Known values at the precision the Boost-derived erfc_inv delivers.
    assert!((standard_normal_inv_cdf(0.5).expect("valid probability") - 0.0).abs() < 1e-12);
    assert!(
        (standard_normal_inv_cdf(0.8413447460685429).expect("valid probability") - 1.0).abs()
            < 1e-9
    );
    assert!(
        (standard_normal_inv_cdf(0.15865525393145705).expect("valid probability") - (-1.0)).abs()
            < 1e-9
    );

    // Tail quantiles: reference values from scipy.special.ndtri
    // (double-precision evaluation of Φ⁻¹). The implementation delivers
    // ~1e-6 *relative* accuracy in the far tails; assert at 1e-7 relative
    // so a precision regression would be caught.
    let tail_cases: [(f64, f64); 4] = [
        (1e-12, -7.034483825301131),
        (1e-10, -6.361340902404056),
        (1e-8, -5.612001244174789),
        (1e-6, -4.753424308822899),
    ];
    for &(p, x_ref) in &tail_cases {
        let x = standard_normal_inv_cdf(p).expect("valid probability");
        let rel_err = ((x - x_ref) / x_ref).abs();
        assert!(
            rel_err < 1e-7,
            "tail quantile regression: p={p}, x={x}, ref={x_ref}, rel_err={rel_err:.3e}"
        );
    }
}

#[test]
fn test_normal_cdf_inv_cdf_roundtrip() {
    // Central region: the implementation delivers ~5e-12 absolute
    // round-trip accuracy (measured: 4.6e-12 at p = 0.1); pin at 1e-11
    // so precision regressions are visible.
    let test_values = [0.1, 0.25, 0.5, 0.75, 0.9];

    for &p in &test_values {
        let x = standard_normal_inv_cdf(p).expect("valid probability");
        let p_back = norm_cdf(x).expect("valid variate");
        assert!(
            (p - p_back).abs() < 1e-11,
            "Failed roundtrip for p={}, got x={}, p_back={}",
            p,
            x,
            p_back
        );
    }
}

#[test]
fn test_extreme_tail_behavior() {
    // Test enhanced tail behavior for extreme values critical to copula models
    let extreme_values = [1e-12, 1e-10, 1e-8, 1e-6];

    for &p in &extreme_values {
        let x_low = standard_normal_inv_cdf(p).expect("valid probability");
        let x_high = standard_normal_inv_cdf(1.0 - p).expect("valid probability");

        // Inverse CDF should be finite and reasonable
        assert!(
            x_low.is_finite(),
            "Inverse CDF should be finite for p={}",
            p
        );
        assert!(
            x_high.is_finite(),
            "Inverse CDF should be finite for p={}",
            1.0 - p
        );

        // Symmetry Φ⁻¹(p) = −Φ⁻¹(1−p): limited by the representation of
        // 1−p in f64 (absolute error ~ε/φ(x) ≈ 1e-16·1e12 ≈ 1e-4 at
        // p = 1e-12), not by the algorithm. Assert proportionally to the
        // quantile's tail sensitivity rather than a flat 0.01.
        let symmetry_error = (x_low + x_high).abs();
        let one_minus_p_resolution = f64::EPSILON
            / norm_pdf(x_low)
                .expect("valid variate")
                .max(f64::MIN_POSITIVE);
        let symmetry_tol = (1e-6 * x_low.abs()).max(4.0 * one_minus_p_resolution);
        assert!(
            symmetry_error < symmetry_tol,
            "Symmetry violated: x_low={}, x_high={} for p={}, error={}, tol={}",
            x_low,
            x_high,
            p,
            symmetry_error,
            symmetry_tol
        );

        // CDF should be stable in extreme tails
        let p_back_low = norm_cdf(x_low).expect("valid variate");
        let p_back_high = norm_cdf(x_high).expect("valid variate");

        assert!(
            p_back_low.is_finite(),
            "CDF should be finite for x={}",
            x_low
        );
        assert!(
            p_back_high.is_finite(),
            "CDF should be finite for x={}",
            x_high
        );

        // Should be bounded properly
        assert!((0.0..=1.0).contains(&p_back_low));
        assert!((0.0..=1.0).contains(&p_back_high));

        // Round-trip accuracy in tail regions. A quantile error of δx
        // amplifies to a relative probability error of ~|x|·δx, so the
        // ~1e-6 relative quantile accuracy implies a p-relative round-trip
        // error of ~|x|²·1e-6 ≈ 5e-5 at p = 1e-12. Assert at 1e-4 so a
        // precision regression (e.g. to the old 10% level) is caught.
        let roundtrip_error_low = (p - p_back_low).abs() / p; // Relative error
        let roundtrip_error_high = ((1.0 - p) - p_back_high).abs() / (1.0 - p);

        assert!(
            roundtrip_error_low < 1e-4,
            "Poor roundtrip accuracy in tail: p={}, x={}, p_back={}, rel_error={}",
            p,
            x_low,
            p_back_low,
            roundtrip_error_low
        );
        assert!(
            roundtrip_error_high < 1e-4,
            "Poor roundtrip accuracy in tail: p={}, x={}, p_back={}, rel_error={}",
            1.0 - p,
            x_high,
            p_back_high,
            roundtrip_error_high
        );
    }
}

#[test]
fn test_numerical_stability_correlations() {
    // Test numerical stability for extreme correlation values used in copula models
    let extreme_correlations: [f64; 8] = [
        1e-10,
        1e-8,
        1e-6,
        0.001,
        0.999,
        1.0 - 1e-6,
        1.0 - 1e-8,
        1.0 - 1e-10,
    ];

    for &rho in &extreme_correlations {
        let sqrt_rho = rho.sqrt();
        let sqrt_one_minus_rho = (1.0 - rho).sqrt();

        // These should be finite and reasonable
        assert!(
            sqrt_rho.is_finite(),
            "sqrt(ρ) should be finite for ρ={}",
            rho
        );
        assert!(
            sqrt_one_minus_rho.is_finite(),
            "sqrt(1-ρ) should be finite for ρ={}",
            rho
        );

        // Test conditional probability calculation stability
        let default_threshold = standard_normal_inv_cdf(0.05).expect("valid probability"); // 5% default prob
        for market_factor in [-3.0, -1.0, 0.0, 1.0, 3.0] {
            let conditional_threshold =
                (default_threshold - sqrt_rho * market_factor) / sqrt_one_minus_rho;
            let cond_prob = norm_cdf(conditional_threshold).expect("valid variate");

            assert!(
                cond_prob.is_finite(),
                "Conditional probability should be finite for ρ={}, Z={}",
                rho,
                market_factor
            );
            assert!(
                (0.0..=1.0).contains(&cond_prob),
                "Conditional probability should be in [0,1]: got {} for ρ={}, Z={}",
                cond_prob,
                rho,
                market_factor
            );
        }
    }
}

#[test]
fn test_student_t_cdf() {
    // Test CDF at zero (should be 0.5 for symmetric distribution)
    assert!((student_t_cdf(0.0, 5.0).expect("valid checked inputs") - 0.5).abs() < 1e-6);
    assert!((student_t_cdf(0.0, 10.0).expect("valid checked inputs") - 0.5).abs() < 1e-6);

    // Test known values from statistical tables
    // t-distribution with df=5, x=-2.0 should give CDF ≈ 0.051
    let cdf = student_t_cdf(-2.0, 5.0).expect("valid checked inputs");
    assert!(
        (cdf - 0.051).abs() < 0.002,
        "CDF(-2.0, df=5) = {}, expected ~0.051",
        cdf
    );

    // Test symmetry: F(-x) = 1 - F(x)
    let x = 1.5;
    let df = 5.0;
    let cdf_neg = student_t_cdf(-x, df).expect("valid checked inputs");
    let cdf_pos = student_t_cdf(x, df).expect("valid checked inputs");
    assert!(
        (cdf_neg + cdf_pos - 1.0).abs() < 1e-10,
        "CDF symmetry violated: F(-{}) + F({}) = {} + {} ≠ 1",
        x,
        x,
        cdf_neg,
        cdf_pos
    );
}

#[test]
fn test_student_t_cdf_approaches_normal() {
    // With high df, Student-t approaches Normal
    let x = -1.5;
    let t_cdf = student_t_cdf(x, 200.0).expect("valid checked inputs");
    let normal_cdf = norm_cdf(x).expect("valid variate");

    assert!(
        (t_cdf - normal_cdf).abs() < 0.01,
        "High df t-distribution should approximate normal: t={}, normal={}",
        t_cdf,
        normal_cdf
    );

    // Test fallback to normal for very high df
    let t_cdf_high = student_t_cdf(x, 1000.0).expect("valid checked inputs");
    assert!(
        (t_cdf_high - normal_cdf).abs() < 0.001,
        "Very high df should be almost identical to normal"
    );
}

#[test]
fn test_student_t_inv_cdf() {
    // Test inverse of median should be zero
    assert!((student_t_inv_cdf(0.5, 5.0).expect("valid checked inputs") - 0.0).abs() < 1e-6);
    assert!((student_t_inv_cdf(0.5, 10.0).expect("valid checked inputs") - 0.0).abs() < 1e-6);

    // Test round-trip
    let p = 0.95;
    let df = 10.0;
    let x = student_t_inv_cdf(p, df).expect("valid checked inputs");
    let p_back = student_t_cdf(x, df).expect("valid checked inputs");
    assert!(
        (p_back - p).abs() < 1e-6,
        "Round-trip failed: p={}, x={}, p_back={}",
        p,
        x,
        p_back
    );
}

#[test]
fn test_student_t_roundtrip() {
    let test_probs = [0.1, 0.25, 0.5, 0.75, 0.9, 0.95];
    let test_dfs = [3.0, 5.0, 10.0, 30.0];

    for &p in &test_probs {
        for &df in &test_dfs {
            let x = student_t_inv_cdf(p, df).expect("valid checked inputs");
            let p_back = student_t_cdf(x, df).expect("valid checked inputs");
            assert!(
                (p - p_back).abs() < 1e-5,
                "Round-trip failed for p={}, df={}: got x={}, p_back={}",
                p,
                df,
                x,
                p_back
            );
        }
    }
}

#[test]
fn test_general_normal_helpers_match_standardized_formula() {
    let x = 1.75;
    let mean = 1.2;
    let std_dev = 0.4;
    let z = (x - mean) / std_dev;

    let expected_cdf = norm_cdf(z).expect("valid variate");
    let expected_pdf = norm_pdf(z).expect("valid variate") / std_dev;
    let actual_cdf = match norm_cdf_with_params(x, mean, std_dev) {
        Ok(value) => value,
        Err(err) => panic!("norm_cdf_with_params should succeed for positive std_dev: {err}"),
    };
    let actual_pdf = match norm_pdf_with_params(x, mean, std_dev) {
        Ok(value) => value,
        Err(err) => panic!("norm_pdf_with_params should succeed for positive std_dev: {err}"),
    };

    assert!((actual_cdf - expected_cdf).abs() < 1e-12);
    assert!((actual_pdf - expected_pdf).abs() < 1e-12);
}

#[test]
fn test_general_normal_helpers_reject_non_positive_std_dev() {
    assert!(norm_cdf_with_params(0.0, 0.0, 0.0).is_err());
    assert!(norm_pdf_with_params(0.0, 0.0, -1.0).is_err());
    assert!(norm_cdf_with_params(f64::NAN, 0.0, 1.0).is_err());
    assert!(norm_pdf_with_params(f64::NAN, 0.0, 1.0).is_err());
    assert!(norm_cdf_with_params(0.0, f64::NAN, 1.0).is_err());
    assert!(norm_pdf_with_params(0.0, f64::INFINITY, 1.0).is_err());
    assert_eq!(norm_cdf_with_params(f64::INFINITY, 0.0, 1.0).unwrap(), 1.0);
    assert_eq!(norm_pdf_with_params(f64::INFINITY, 0.0, 1.0).unwrap(), 0.0);
    // Overflow-safe standardization when `x - mean` would overflow f64
    assert!(
        (norm_cdf_with_params(f64::MAX, -f64::MAX, f64::MAX).unwrap()
            - norm_cdf(2.0).expect("valid variate"))
        .abs()
            < 1e-12
    );
}
