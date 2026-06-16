//! Moment matching variance reduction.
//!
//! Forces sample moments to match theoretical values exactly,
//! reducing variance for well-behaved payoffs.
//!
//! # Mathematical Note
//!
//! This implementation matches the *theoretical moments* of the standard normal:
//! - `E[Z] = 0` (first moment)
//! - `E[Z^2] = 1` (second raw moment, which equals variance for zero-mean)
//!
//! We use n (not n-1) in the variance calculation because we're matching
//! the theoretical second moment, not computing an unbiased variance estimate.
//! After transformation, the samples satisfy: (1/n) Σz_i² = 1.

/// Apply moment matching to standard normal samples.
///
/// Adjusts samples so that the empirical moments match the theoretical
/// moments of a standard normal distribution:
/// - Sample mean: (1/n) Σz_i = 0
/// - Sample second moment: (1/n) Σz_i² = 1
///
/// # Note on Variance Formula
///
/// Uses `n` (not `n - 1`) to match the theoretical second moment `E[Z^2] = 1`.
/// This is standard practice for Monte Carlo moment matching - we want the
/// sample to have the exact theoretical properties, not an unbiased estimate.
///
/// # Bias Warning
///
/// Moment matching **destroys sample independence**: the transformed samples
/// become linearly dependent (they are constrained to sum to zero and have
/// unit second moment). This induces a negative bias for convex payoffs
/// (calls, swaptions, cliquets) because the forced-zero-mean constraint
/// suppresses tail realizations that drive convexity value. The bias is
/// O(1/n) and negligible for large sample sizes but can be material for
/// small batches (n < 1000). See Glasserman (2003), §4.3.
///
/// # Arguments
///
/// * `samples` - Mutable slice of samples to adjust in-place
pub fn match_standard_normal_moments(samples: &mut [f64]) {
    if samples.is_empty() {
        return;
    }

    let n = samples.len() as f64;

    // Compute current mean (first moment)
    let mean = samples.iter().sum::<f64>() / n;

    // Compute current variance (second central moment, using n not n-1)
    // We use n to match theoretical E[Z²] = 1, not for unbiased estimation
    let var = samples.iter().map(|&x| (x - mean).powi(2)).sum::<f64>() / n;
    let std = var.sqrt();

    // Adjust to have mean=0, std=1
    // After transformation: (1/n) Σz'² = 1 (matching E[Z²] = 1)
    if std > 1e-10 {
        for x in samples {
            *x = (*x - mean) / std;
        }
    }
}

/// Apply moment matching to each time step independently.
///
/// For a matrix of samples [paths x steps], adjust each step column
/// to have exact N(0,1) moments.
///
/// # Arguments
///
/// * `samples` - Matrix of samples in row-major form: `samples[path * num_steps + step]`
/// * `num_paths` - Number of paths
/// * `num_steps` - Number of time steps
pub fn match_moments_per_step(samples: &mut [f64], num_paths: usize, num_steps: usize) {
    let mut col_buf: Vec<f64> = vec![0.0; num_paths];
    for step in 0..num_steps {
        for path in 0..num_paths {
            col_buf[path] = samples[path * num_steps + step];
        }

        match_standard_normal_moments(&mut col_buf);

        for (path, &val) in col_buf.iter().enumerate() {
            samples[path * num_steps + step] = val;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_moment_matching_basic() {
        let mut samples = vec![-1.5, -0.5, 0.5, 1.5, 2.0];
        match_standard_normal_moments(&mut samples);

        let mean = samples.iter().sum::<f64>() / samples.len() as f64;
        let var = samples.iter().map(|&x| (x - mean).powi(2)).sum::<f64>() / samples.len() as f64;

        assert!(mean.abs() < 1e-10);
        assert!((var - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_moment_matching_preserves_length() {
        let mut samples = vec![1.0, 2.0, 3.0, 4.0];
        let original_len = samples.len();

        match_standard_normal_moments(&mut samples);

        assert_eq!(samples.len(), original_len);
    }

    #[test]
    fn test_moment_matching_empty() {
        let mut samples: Vec<f64> = vec![];
        match_standard_normal_moments(&mut samples);
        // Should not panic
    }

    #[test]
    fn test_match_moments_per_step() {
        // 3 paths, 2 steps
        let mut samples = vec![
            1.0, 2.0, // path 0
            -1.0, 0.5, // path 1
            0.5, -0.5, // path 2
        ];

        match_moments_per_step(&mut samples, 3, 2);

        // Each step should have mean=0, var=1
        // Step 0: samples[0], samples[2], samples[4]
        let step0: Vec<f64> = vec![samples[0], samples[2], samples[4]];
        let mean0 = step0.iter().sum::<f64>() / 3.0;
        assert!(mean0.abs() < 1e-10);

        // Step 1: samples[1], samples[3], samples[5]
        let step1: Vec<f64> = vec![samples[1], samples[3], samples[5]];
        let mean1 = step1.iter().sum::<f64>() / 3.0;
        assert!(mean1.abs() < 1e-10);
    }
}
