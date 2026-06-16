//! Poisson distribution sampling for jump processes.
//!
//! Provides methods to sample from Poisson distribution for modeling
//! jump arrivals in jump-diffusion processes.

/// Sample from Poisson distribution using inverse CDF method.
///
/// # Arguments
///
/// * `lambda` - Mean number of events (λ)
/// * `u` - Uniform random variable in [0, 1)
///
/// # Returns
///
/// Number of Poisson events
///
/// # Algorithm
///
/// Uses inverse CDF: finds smallest k such that P(N ≤ k) ≥ u
///
/// For small λ (< 30), uses direct summation of the Poisson pmf. The search
/// is truncated at k = 200; for λ < 30 the tail mass P(N > 200) is below
/// 1e-100, so this only matters for u within rounding distance of 1.0,
/// where the sample saturates at 200 instead of growing without bound.
///
/// For large λ (≥ 30), uses the normal approximation N(λ, λ) with the
/// standard 0.5 continuity correction: the smallest integer k with
/// `Φ((k + 0.5 − λ)/√λ) ≥ u` is `k = ⌈λ + √λ·Φ⁻¹(u) − 0.5⌉`. The remaining
/// bias is the uncorrected skewness of the Poisson distribution
/// (skew = 1/√λ ≤ 0.18 at the λ = 30 threshold): the approximation slightly
/// under-weights the right tail and over-weights the left tail relative to
/// the exact distribution. A Cornish–Fisher term would reduce this further
/// but is not applied here.
pub fn poisson_inverse_cdf(lambda: f64, u: f64) -> usize {
    if lambda <= 0.0 {
        return 0;
    }

    // Threshold 30.0: normal approximation skewness = 1/√λ < 0.18
    if lambda < 30.0 {
        let mut p = (-lambda).exp(); // P(N = 0)
        let mut cdf = p;
        let mut k = 0;

        // Cap at 200 to prevent infinite loops for extreme u values.
        // P(N > 200 | λ < 30) < 1e-100, so this truncation is unreachable
        // except for u ≈ 1.0 within floating-point rounding.
        while cdf < u && k < 200 {
            k += 1;
            p *= lambda / k as f64;
            cdf += p;
        }

        k
    } else {
        // For large lambda, use normal approximation with continuity
        // correction: P(N ≤ k) ≈ Φ((k + 0.5 − λ)/√λ), so the inverse-CDF
        // sample is the smallest integer k ≥ λ + √λ·z − 0.5.
        use crate::math::special_functions::standard_normal_inv_cdf;

        let std_dev = lambda.sqrt();
        let z = standard_normal_inv_cdf(u);
        let n_approx = lambda + std_dev * z - 0.5;

        n_approx.ceil().max(0.0) as usize
    }
}

/// Sample from Poisson using standard normal input.
///
/// Converts a standard normal variate to Poisson via CDF transform.
///
/// # Arguments
///
/// * `lambda` - Mean number of events
/// * `z` - Standard normal variate
///
/// # Returns
///
/// Number of Poisson events
pub fn poisson_from_normal(lambda: f64, z: f64) -> usize {
    use crate::math::special_functions::norm_cdf;

    let u = norm_cdf(z);
    poisson_inverse_cdf(lambda, u)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_poisson_zero_lambda() {
        assert_eq!(poisson_inverse_cdf(0.0, 0.5), 0);
        assert_eq!(poisson_inverse_cdf(0.0, 0.9), 0);
    }

    #[test]
    fn test_poisson_small_lambda() {
        // For λ = 1, P(N=0) = e^{-1} ≈ 0.368
        let lambda = 1.0;

        // u < e^{-1} should give 0
        assert_eq!(poisson_inverse_cdf(lambda, 0.3), 0);

        // u > e^{-1} should give 1 or more
        assert!(poisson_inverse_cdf(lambda, 0.5) >= 1);
    }

    #[test]
    fn test_poisson_mean() {
        // Test that empirical mean approaches lambda
        let lambda = 3.0;
        let n_samples = 1000;

        let mut sum = 0;
        for i in 0..n_samples {
            let u = (i as f64 + 0.5) / n_samples as f64; // Uniform grid
            let k = poisson_inverse_cdf(lambda, u);
            sum += k;
        }

        let empirical_mean = sum as f64 / n_samples as f64;

        // Should be close to lambda
        assert!((empirical_mean - lambda).abs() / lambda < 0.2);
    }

    #[test]
    fn test_poisson_large_lambda_continuity_correction() {
        let lambda = 100.0;

        // Median of Poisson(100) is 100; the corrected inverse CDF at u = 0.5
        // gives ceil(100 + 0 - 0.5) = 100.
        assert_eq!(poisson_inverse_cdf(lambda, 0.5), 100);

        // Mean over a uniform grid should be close to lambda. Without the
        // continuity correction the discretization bias is ~0.5.
        let n_samples = 20_000;
        let mut sum = 0usize;
        for i in 0..n_samples {
            let u = (i as f64 + 0.5) / n_samples as f64;
            sum += poisson_inverse_cdf(lambda, u);
        }
        let empirical_mean = sum as f64 / n_samples as f64;
        assert!(
            (empirical_mean - lambda).abs() < 0.25,
            "empirical mean {empirical_mean} too far from {lambda}"
        );
    }

    #[test]
    fn test_poisson_from_normal() {
        let lambda = 2.0;

        // z = 0 (median) should give around lambda
        let k = poisson_from_normal(lambda, 0.0);
        assert!(k <= 4); // Should be close to 2

        // Very negative z should give 0 or low value
        let k_low = poisson_from_normal(lambda, -3.0);
        assert!(k_low <= 2);

        // Very positive z should give higher value
        let k_high = poisson_from_normal(lambda, 3.0);
        assert!(k_high >= 2);
    }
}
