//! Control variate variance reduction using Black-Scholes.
//!
//! Uses the analytical Black-Scholes formula as a control variate
//! to reduce variance for European options under GBM.
//!
//! The control variate estimator is:
//! ```text
//! X̂ = X̄ - β(Ȳ - E[Y])
//! ```
//! where `Y` is the control (BS price), `E[Y]` is known analytically,
//! and β is the optimal coefficient.
//!
//! # Online Covariance
//!
//! For large-scale simulations where storing all samples is impractical,
//! use [`OnlineCovariance`](crate::monte_carlo::OnlineCovariance)
//! to compute covariance incrementally:
//!
//! ```
//! use finstack_quant_models::monte_carlo::OnlineCovariance;
//!
//! let mut cov = OnlineCovariance::new();
//! // Update incrementally during simulation
//! for _ in 0..10000 {
//!     let mc_value = 10.0; // simulated payoff
//!     let control_value = 9.8; // BS control value
//!     cov.update(mc_value, control_value);
//! }
//!
//! // Get optimal beta and statistics for control variate adjustment
//! let beta = cov.optimal_beta();
//! let mc_mean = cov.mean_x();
//! let control_mean = cov.mean_y();
//! ```

use crate::monte_carlo::estimate::Estimate;

/// Apply control variate adjustment to a Monte Carlo estimate.
///
/// Forms the adjusted estimator
/// ```text
/// Ŷ = X̄ − β̂ · (C̄ − E[C])
/// ```
/// with `β̂ = Cov(X, C) / Var(C)` and returns its sample mean and standard
/// error.
///
/// # Variance caveat
///
/// With a *known* optimal `β`, `Var(Ŷ) = Var(X̄)(1 − ρ²)` exactly. In practice
/// `β̂` is estimated from the same paths, so the realised sampling variance
/// has an extra `O(1/n)` term. This routine returns the plug-in variance
/// `Var(X̄) + β̂² Var(C̄) − 2β̂ Cov(X̄, C̄)`, which **underestimates** the true
/// estimator variance at small `n`. The error is empirically negligible for
/// `n ≳ 1 000` but can matter for tight confidence-interval comparisons at
/// low path counts; compute `β̂` on an independent warm-up batch if exact
/// coverage is required. See Glasserman (2003), §4.1.
///
/// # Arguments
///
/// * `mc_mean` - Monte Carlo sample mean of the target payoff `X`
/// * `mc_var`  - Monte Carlo sample variance of `X`
/// * `control_mean` - Sample mean of the control variate `C`
/// * `control_var`  - Sample variance of `C`
/// * `covariance`   - Sample covariance between `X` and `C`
/// * `control_analytical` - Known closed-form value `E[C]`
/// * `num_samples`  - Number of Monte Carlo samples
///
/// # Returns
///
/// An [`Estimate`] carrying the adjusted mean, plug-in standard error, and
/// a 95 % confidence interval. When `num_samples < 2`, returns the raw
/// `mc_mean` with `NaN` stderr and confidence bounds, and no sample standard
/// deviation, because sampling uncertainty cannot be estimated.
pub fn apply_control_variate(
    mc_mean: f64,
    mc_var: f64,
    control_mean: f64,
    control_var: f64,
    covariance: f64,
    control_analytical: f64,
    num_samples: usize,
) -> Estimate {
    if num_samples < 2 {
        return Estimate::new(mc_mean, f64::NAN, (f64::NAN, f64::NAN), num_samples);
    }

    // Optimal beta coefficient
    let beta = if control_var > 1e-10 {
        covariance / control_var
    } else {
        0.0
    };

    // Adjusted mean
    let adjusted_mean = mc_mean - beta * (control_mean - control_analytical);

    // Adjusted variance
    let adjusted_var = mc_var - 2.0 * beta * covariance + beta * beta * control_var;
    let adjusted_var = if adjusted_var < 0.0 && adjusted_var.abs() < 1e-12 {
        0.0
    } else {
        adjusted_var.max(0.0)
    };
    let adjusted_stderr = (adjusted_var / num_samples as f64).sqrt();

    // 95% confidence interval
    let z_95 = 1.96;
    let margin = z_95 * adjusted_stderr;
    let ci_95 = (adjusted_mean - margin, adjusted_mean + margin);

    Estimate::new(adjusted_mean, adjusted_stderr, ci_95, num_samples)
        .with_std_dev(adjusted_var.sqrt())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_control_variate_adjustment() {
        // Simulate some correlated samples
        let mc_samples: Vec<f64> = vec![10.0, 12.0, 11.0, 13.0, 10.5];
        let control_samples: Vec<f64> = vec![9.8, 12.2, 10.9, 13.1, 10.4];
        let control_analytical = 11.0;

        let mc_mean = mc_samples.iter().sum::<f64>() / mc_samples.len() as f64;
        let control_mean = control_samples.iter().sum::<f64>() / control_samples.len() as f64;

        let mc_var = mc_samples
            .iter()
            .map(|&x| (x - mc_mean).powi(2))
            .sum::<f64>()
            / (mc_samples.len() - 1) as f64;

        let control_var = control_samples
            .iter()
            .map(|&x| (x - control_mean).powi(2))
            .sum::<f64>()
            / (control_samples.len() - 1) as f64;

        let cov = finstack_quant_core::math::stats::covariance(&mc_samples, &control_samples);

        let result = apply_control_variate(
            mc_mean,
            mc_var,
            control_mean,
            control_var,
            cov,
            control_analytical,
            mc_samples.len(),
        );

        // Adjusted mean should be different from raw MC mean
        assert!((result.mean - mc_mean).abs() > 0.0);

        // Should have valid stderr
        assert!(result.stderr > 0.0);
    }

    #[test]
    /// Pins the covariance convention this module's control-variate maths
    /// depends on, now sourced from `core::math::stats` rather than a local copy.
    fn test_covariance() {
        let x = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let y = vec![2.0, 4.0, 6.0, 8.0, 10.0];

        let cov = finstack_quant_core::math::stats::covariance(&x, &y);

        // Perfect positive correlation: y = 2x
        // Var(x) = 2.5, Var(y) = 10, Cov(x,y) = 5
        assert!(cov > 0.0);
        assert!((cov - 5.0).abs() < 0.1);
    }

    #[test]
    /// The covariance utility retains its zero singleton convention; the
    /// control-variate estimate separately reports unknown sampling uncertainty.
    fn covariance_returns_zero_for_single_sample() {
        assert_eq!(
            finstack_quant_core::math::stats::covariance(&[1.0], &[2.0]),
            0.0
        );
    }

    #[test]
    fn control_variate_reports_unknown_uncertainty_for_insufficient_samples() {
        for num_samples in [0, 1] {
            let estimate = apply_control_variate(10.0, 1.0, 9.5, 0.5, 0.2, 9.0, num_samples);
            assert_eq!(estimate.mean, 10.0);
            assert!(estimate.stderr.is_nan());
            assert!(estimate.ci_95.0.is_nan());
            assert!(estimate.ci_95.1.is_nan());
            assert_eq!(estimate.std_dev, None);
            assert_eq!(estimate.num_paths, num_samples);
        }
    }
}
