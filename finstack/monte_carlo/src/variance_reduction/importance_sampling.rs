//! Importance sampling variance reduction.
//!
//! Uses exponential tilting to shift the sampling distribution toward
//! rare events (deep OTM, barriers, tail risks).
//!
//! The tilted estimator is `E[X] ≈ E*[X * L(ω)]`,
//! where `L` is the likelihood ratio and `E*` is under the tilted measure.
//!
//! # ESS Threshold Warning
//!
//! The Effective Sample Size (ESS) measures the efficiency of importance sampling.
//! When ESS/N drops below 10% (typically 0.1), the variance of the estimator
//! becomes unreliable and results should be treated with caution.
//!
//! Reference: Kong et al. (1994) "Sequential Imputations and Bayesian Missing Data Problems"

/// Default ESS ratio threshold below which a warning is triggered.
/// An ESS ratio of 0.1 (10%) is a common threshold in practice.
pub const DEFAULT_ESS_THRESHOLD: f64 = 0.1;

/// Compute exponential tilting for barrier options.
///
/// Shifts the drift toward the barrier to increase hit probability,
/// then re-weights samples using likelihood ratios.
///
/// # Convention
///
/// - **Input `z`** is a standard-normal draw under the **target** measure
///   `N(0, 1)` (the measure under which the payoff is to be priced).
/// - **Output `tilted_z = z + θ`** is the corresponding draw under the
///   **tilted** proposal measure `N(θ, 1)`. Callers feed `tilted_z` into the
///   discretization/step in place of the untilted shock, so every path is
///   simulated under the proposal measure.
/// - **Output `likelihood_ratio`** is the target-over-proposal Radon-Nikodym
///   derivative evaluated at the realized sample:
///
///   ```text
///   L(Z*) = φ_target(Z*) / φ_tilted(Z*)
///         = exp(-θ · Z* + ½ θ²)
///   ```
///
///   where `φ_target` is the N(0,1) pdf and `φ_tilted` is the N(θ,1) pdf.
///   Use this weight with [`weighted_estimate`] to recover the unbiased
///   target-measure expectation `E_target[X] = E_tilted[X · L]`.
///
/// # Arguments
///
/// * `theta` - Tilting parameter (drift shift under the proposal measure)
/// * `z` - Standard normal sample under the **target** measure
///
/// # Returns
///
/// `(tilted_z, likelihood_ratio)` — the draw to use in simulation under the
/// tilted proposal, and the target-over-proposal weight to attach to the
/// resulting path value.
pub fn exponential_tilt(theta: f64, z: f64) -> (f64, f64) {
    let tilted_z = z + theta;
    let log_likelihood = -theta * tilted_z + 0.5 * theta * theta;
    let likelihood_ratio = log_likelihood.exp();

    (tilted_z, likelihood_ratio)
}

/// Result of importance sampling estimation with ESS diagnostics.
#[derive(Debug, Clone)]
pub struct ImportanceSamplingResult {
    /// Importance-sampling mean estimate
    pub mean: f64,
    /// Standard error of the estimate
    pub stderr: f64,
    /// Effective sample size
    pub ess: f64,
    /// ESS ratio (ESS / N), ranges from 0 to 1
    pub ess_ratio: f64,
    /// True if ESS ratio is below threshold (estimate may be unreliable)
    pub low_ess_warning: bool,
}

/// Compute the ordinary importance-sampling estimate.
///
/// # Arguments
///
/// * `values` - Sampled values under the tilted measure
/// * `weights` - Likelihood ratios
///
/// # Returns
///
/// `(is_mean, is_stderr)`
pub fn weighted_estimate(values: &[f64], weights: &[f64]) -> (f64, f64) {
    let result = weighted_estimate_with_diagnostics(values, weights, DEFAULT_ESS_THRESHOLD);
    (result.mean, result.stderr)
}

/// Compute the ordinary importance-sampling estimate with full diagnostics.
///
/// Returns detailed results including ESS and warning flag when ESS is low.
///
/// # Arguments
///
/// * `values` - Sampled values under the tilted measure
/// * `weights` - Likelihood ratios
/// * `ess_threshold` - ESS ratio threshold for warning (typically 0.1)
///
/// # Returns
///
/// `ImportanceSamplingResult` with mean, stderr, ESS, and warning flag
///
/// # Example
///
/// ```ignore
/// use finstack_monte_carlo::variance_reduction::importance_sampling::{
///     weighted_estimate_with_diagnostics, DEFAULT_ESS_THRESHOLD
/// };
///
/// let values = vec![1.0, 2.0, 3.0, 4.0];
/// let weights = vec![1.0, 1.0, 1.0, 1.0]; // Uniform weights
///
/// let result = weighted_estimate_with_diagnostics(&values, &weights, DEFAULT_ESS_THRESHOLD);
/// assert!(!result.low_ess_warning); // Uniform weights = good ESS
/// ```
pub fn weighted_estimate_with_diagnostics(
    values: &[f64],
    weights: &[f64],
    ess_threshold: f64,
) -> ImportanceSamplingResult {
    assert_eq!(values.len(), weights.len());

    if values.is_empty() {
        return ImportanceSamplingResult {
            mean: 0.0,
            stderr: 0.0,
            ess: 0.0,
            ess_ratio: 0.0,
            low_ess_warning: true,
        };
    }

    let n = values.len() as f64;

    // Compute ESS
    let ess = effective_sample_size(weights);
    let ess_ratio = ess / n;
    let low_ess_warning = ess_ratio < ess_threshold;

    let weighted_payoffs: Vec<f64> = values.iter().zip(weights).map(|(v, w)| v * w).collect();

    let mean = weighted_payoffs.iter().sum::<f64>() / n;

    let variance = if weighted_payoffs.len() > 1 {
        weighted_payoffs
            .iter()
            .map(|x| (x - mean).powi(2))
            .sum::<f64>()
            / (n - 1.0)
    } else {
        0.0
    };

    let stderr = (variance / n).sqrt();

    ImportanceSamplingResult {
        mean,
        stderr,
        ess,
        ess_ratio,
        low_ess_warning,
    }
}

/// Effective sample size for weighted samples.
///
/// ESS = (Σw_i)² / Σ(w_i²)
///
/// Measures the efficiency of importance sampling.
/// ESS = N means perfect (uniform weights), ESS << N means poor weighting.
#[must_use]
pub fn effective_sample_size(weights: &[f64]) -> f64 {
    if weights.is_empty() {
        return 0.0;
    }

    let sum_w: f64 = weights.iter().sum();
    let sum_w2: f64 = weights.iter().map(|w| w * w).sum();

    if sum_w2 > 1e-10 {
        (sum_w * sum_w) / sum_w2
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exponential_tilt() {
        let (tilted, lr) = exponential_tilt(0.5, 1.0);

        // Tilted should be shifted
        assert_eq!(tilted, 1.5);

        // Likelihood ratio should be positive
        assert!(lr > 0.0);
    }

    #[test]
    fn test_weighted_estimate_uniform() {
        let values = vec![1.0, 2.0, 3.0, 4.0];
        let weights = vec![1.0, 1.0, 1.0, 1.0]; // Uniform weights

        let (mean, _) = weighted_estimate(&values, &weights);

        // Should equal simple mean
        assert!((mean - 2.5).abs() < 1e-10);
    }

    #[test]
    fn test_weighted_estimate_non_uniform() {
        let values = vec![1.0, 2.0, 3.0];
        let weights = vec![1.0, 2.0, 1.0]; // Higher weight on middle value

        let (mean, _) = weighted_estimate(&values, &weights);

        // Ordinary IS mean: (1*1 + 2*2 + 3*1) / 3 = 8/3
        assert!((mean - (8.0 / 3.0)).abs() < 1e-10);
    }

    #[test]
    fn test_effective_sample_size_uniform() {
        let weights = vec![1.0, 1.0, 1.0, 1.0];
        let ess = effective_sample_size(&weights);

        // Uniform weights should give ESS = N
        assert!((ess - 4.0).abs() < 1e-10);
    }

    #[test]
    fn test_effective_sample_size_concentrated() {
        let weights = vec![1.0, 0.1, 0.1, 0.1];
        let ess = effective_sample_size(&weights);

        // One dominant weight should give ESS < N
        assert!(ess < 4.0);
        assert!(ess > 1.0);
    }

    #[test]
    fn test_tilting_increases_likelihood() {
        // Tilting toward positive values
        let theta = 1.0;
        let z_positive = 2.0;
        let z_negative = -2.0;

        let (_, lr_pos) = exponential_tilt(theta, z_positive);
        let (_, lr_neg) = exponential_tilt(theta, z_negative);

        // Positive shifted samples should have lower target-over-tilted likelihood
        // under a positive tilt.
        assert!(lr_pos < lr_neg);
    }

    #[test]
    fn test_exponential_tilt_uses_target_over_tilted_density_ratio() {
        let theta = 1.0;
        let z = 2.0;
        let (tilted_z, likelihood_ratio) = exponential_tilt(theta, z);

        assert_eq!(tilted_z, 3.0);

        let expected = (-theta * tilted_z + 0.5 * theta * theta).exp();
        assert!(
            (likelihood_ratio - expected).abs() < 1e-12,
            "expected likelihood ratio {} but got {}",
            expected,
            likelihood_ratio
        );
    }

    #[test]
    fn test_weighted_estimate_uses_plain_mean_of_weighted_payoffs() {
        let values = vec![1.0, 1.0];
        let weights = vec![2.0, 1.0];

        let (mean, _) = weighted_estimate(&values, &weights);

        assert!(
            (mean - 1.5).abs() < 1e-12,
            "expected ordinary IS mean 1.5 but got {}",
            mean
        );
    }

    #[test]
    fn test_weighted_estimate_with_diagnostics_uniform() {
        let values = vec![1.0, 2.0, 3.0, 4.0];
        let weights = vec![1.0, 1.0, 1.0, 1.0];

        let result =
            super::weighted_estimate_with_diagnostics(&values, &weights, DEFAULT_ESS_THRESHOLD);

        // Uniform weights should give ESS ratio of 1.0 (perfect)
        assert!((result.ess_ratio - 1.0).abs() < 1e-10);
        assert!(!result.low_ess_warning);
    }

    #[test]
    fn test_weighted_estimate_with_diagnostics_low_ess() {
        // Create weights where one sample dominates
        let values = vec![1.0; 100];
        let mut weights = vec![0.001; 100];
        weights[0] = 100.0; // One sample dominates

        let result =
            super::weighted_estimate_with_diagnostics(&values, &weights, DEFAULT_ESS_THRESHOLD);

        // ESS ratio should be very low
        assert!(result.ess_ratio < DEFAULT_ESS_THRESHOLD);
        assert!(result.low_ess_warning);
    }

    #[test]
    fn test_ess_ratio_boundary() {
        // Create weights that give ESS ratio just above and below threshold
        let n = 100;
        let values = vec![1.0; n];

        // Uniform weights -> ESS ratio = 1.0
        let uniform_weights = vec![1.0; n];
        let result_uniform = super::weighted_estimate_with_diagnostics(
            &values,
            &uniform_weights,
            DEFAULT_ESS_THRESHOLD,
        );
        assert!(!result_uniform.low_ess_warning);

        // Highly concentrated weights -> low ESS
        let mut concentrated = vec![1.0; n];
        concentrated[0] = 1000.0;
        let result_concentrated = super::weighted_estimate_with_diagnostics(
            &values,
            &concentrated,
            DEFAULT_ESS_THRESHOLD,
        );
        assert!(result_concentrated.low_ess_warning);
    }
}
