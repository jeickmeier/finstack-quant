//! Random number generation for Monte Carlo simulations.
//!
//! Provides a small RNG trait, a deterministic PCG64 implementation, Sobol
//! utilities, and sampling helpers used by simulation code.
//!
//! # Components
//!
//! - [`RandomNumberGenerator`]: Trait for pluggable RNG implementations
//! - [`Pcg64Rng`]: Deterministic PCG64 generator
//! - [`box_muller`]: Normal variate generation from uniform samples
//! - [`SobolRng`]: Sobol low-discrepancy sequence with optional prefix hash scrambling
//! - [`BrownianBridge`]: Brownian bridge construction order
//! - [`poisson_inverse_cdf`]: Poisson sampling utilities
//!
//! # Quick Start
//!
//! ```rust
//! use finstack_quant_core::math::random::{Pcg64Rng, RandomNumberGenerator};
//!
//! // Create RNG with seed for reproducibility
//! let mut rng = Pcg64Rng::new(42, 0);
//!
//! // Generate samples
//! let uniform = rng.uniform();       // U(0, 1)
//! let normal = rng.normal(0.0, 1.0)?; // N(0, 1)
//! let event = rng.uniform() < 0.05;   // 5% probability
//! # Ok::<(), finstack_quant_core::Error>(())
//! ```
//!
//! # Statistical properties
//!
//! [`Pcg64Rng`] wraps `rand_pcg::Pcg64`:
//! - **Period**: 2^128
//! - **Streams**: [`Pcg64Rng::new`] uses a stream-specific increment
//! - **Deterministic**: Equal seed, stream, and call order reproduce the same sequence in the same build
//!
//! # Parallel Simulations
//!
//! For parallel Monte Carlo, use distinct stream selectors:
//!
//! ```rust
//! use finstack_quant_core::math::random::Pcg64Rng;
//!
//! // Each thread gets a distinct stream selector (e.g., 4 threads)
//! let num_threads = 4;
//! let rngs: Vec<_> = (0..num_threads)
//!     .map(|thread_id| Pcg64Rng::new(42, thread_id as u64))
//!     .collect();
//! ```
//!
//! # References
//!
//! - **PCG**: O'Neill, M. E. (2014). "PCG: A Family of Simple Fast Space-Efficient
//!   Statistically Good Algorithms for Random Number Generation." `docs/REFERENCES.md#o-neill-2014-pcg`
//! - **Box-Muller**: Box, G. E. P., & Muller, M. E. (1958). "A Note on the
//!   Generation of Random Normal Deviates." `docs/REFERENCES.md#box-muller-1958`

mod brownian_bridge;
mod poisson;
mod sobol;

pub use brownian_bridge::BrownianBridge;
pub use poisson::poisson_inverse_cdf;
pub use sobol::{MAX_SOBOL_DIMENSION, SOBOL_PERIOD, SobolRng};

/// Random number generator trait for statistical sampling.
///
/// This trait provides the basic interface needed for Monte Carlo simulations
/// and stochastic sampling algorithms.
pub trait RandomNumberGenerator: Send + Sync {
    /// Generate uniform random number in [0, 1)
    fn uniform(&mut self) -> f64;

    /// Draw a normal variate or return a domain or numerical error.
    ///
    /// # Arguments
    ///
    /// * `mean` - Finite location in the caller's units.
    /// * `std_dev` - Finite nonnegative standard deviation in the same units.
    ///   Zero returns the mean and consumes no random variate.
    fn normal(&mut self, mean: f64, std_dev: f64) -> crate::Result<f64>;
}

use rand::RngCore;
use rand_pcg::Pcg64;

/// PCG XSL-RR 128/64 with an explicit seed and stream and a cached normal companion.
/// A clone copies both the PCG state and any pending companion.
#[derive(Clone, Debug)]
pub struct Pcg64Rng {
    inner: Pcg64,
    cached_normal: Option<f64>,
}

impl Pcg64Rng {
    /// Select a deterministic PCG sequence.
    ///
    /// # Arguments
    ///
    /// * `seed` - Initial state input passed directly to the PCG constructor.
    /// * `stream` - Sequence selector. PCG uses the odd increment `2 * stream + 1`.
    #[must_use]
    pub fn new(seed: u64, stream: u64) -> Self {
        Self {
            inner: Pcg64::new(u128::from(seed), u128::from(stream)),
            cached_normal: None,
        }
    }
}

impl RandomNumberGenerator for Pcg64Rng {
    #[inline]
    fn uniform(&mut self) -> f64 {
        (self.inner.next_u64() >> 11) as f64 * (1.0 / (1_u64 << 53) as f64)
    }

    fn normal(&mut self, mean: f64, std_dev: f64) -> crate::Result<f64> {
        if !mean.is_finite() || !std_dev.is_finite() || std_dev < 0.0 {
            return Err(crate::Error::Validation(
                "normal draws require finite mean and nonnegative finite standard deviation".into(),
            ));
        }
        if std_dev == 0.0 {
            return Ok(mean);
        }
        let z = match self.cached_normal.take() {
            Some(spare) => spare,
            None => {
                let (first, spare) = box_muller(self.uniform(), self.uniform())?;
                self.cached_normal = Some(spare);
                first
            }
        };
        let value = std_dev.mul_add(z, mean);
        if !value.is_finite() {
            return Err(crate::Error::Internal(
                "normal draw exceeds the finite range".into(),
            ));
        }
        Ok(value)
    }
}

/// Transform a pair of independent half-open uniform draws into standard normal coordinates.
/// The finite binary64 grid bounds the radius by approximately 8.57.
///
/// # Arguments
///
/// * `u1` - Finite uniform coordinate in `[0,1)` selecting the radius. Zero gives a zero radius.
/// * `u2` - Finite uniform coordinate in `[0,1)` selecting the angle in turns.
#[inline]
pub fn box_muller(u1: f64, u2: f64) -> crate::Result<(f64, f64)> {
    if !(0.0..1.0).contains(&u1) || !(0.0..1.0).contains(&u2) {
        return Err(crate::Error::Validation(
            "Box–Muller coordinates must lie in [0, 1)".into(),
        ));
    }
    let radius = (-2.0 * (-u1).ln_1p()).sqrt();
    let (sin, cos) = (std::f64::consts::TAU * u2).sin_cos();
    Ok((radius * cos, radius * sin))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_box_muller() {
        let (z1, z2) = box_muller(0.5, 0.5).unwrap();
        assert!(z1.is_finite());
        assert!(z2.is_finite());

        // Test with many samples using Pcg64Rng
        let mut rng = Pcg64Rng::new(42, 0);
        let mut samples = Vec::new();
        for _ in 0..500 {
            let u1 = rng.uniform();
            let u2 = rng.uniform();
            let (z1, z2) = box_muller(u1, u2).unwrap();
            samples.push(z1);
            samples.push(z2);
        }

        let mean = samples.iter().sum::<f64>() / samples.len() as f64;
        let var =
            samples.iter().map(|&x| (x - mean).powi(2)).sum::<f64>() / (samples.len() - 1) as f64;

        assert!(mean.abs() < 0.1);
        assert!((var - 1.0).abs() < 0.2);
    }

    #[test]
    fn reflected_box_muller_obeys_the_half_open_domain() {
        assert_eq!(box_muller(0.0, 0.0).unwrap(), (0.0, 0.0));
        for bad in [-0.01, 1.0, f64::INFINITY, f64::NAN] {
            assert!(box_muller(bad, 0.0).is_err());
            assert!(box_muller(0.0, bad).is_err());
        }
        let (tail, _) = box_muller(1.0_f64.next_down(), 0.0).unwrap();
        assert!((tail - 8.571674348652905).abs() < 1e-12);
    }

    #[test]
    fn normal_cache_and_parameter_failures_preserve_draw_order() {
        let mut rng = Pcg64Rng::new(42, 7);
        let mut reference = rand_pcg::Pcg64::new(42, 7);
        let mut uniform = || (reference.next_u64() >> 11) as f64 * 2.0_f64.powi(-53);
        let (z0, z1) = box_muller(uniform(), uniform()).unwrap();
        assert_eq!(rng.normal(0.0, 1.0).unwrap(), z0);
        assert!(rng.normal(0.0, -1.0).is_err());
        assert!(rng.normal(f64::NAN, 0.0).is_err());
        assert_eq!(rng.normal(-3.0, 0.0).unwrap(), -3.0);
        assert_eq!(rng.uniform(), uniform());
        assert_eq!(rng.normal(1.0, 3.0).unwrap(), 3.0_f64.mul_add(z1, 1.0));

        rng.cached_normal = Some(1.5);
        assert_eq!(rng.normal(-f64::MAX, f64::MAX).unwrap(), f64::MAX * 0.5);
        rng.cached_normal = Some(2.0);
        assert!(rng.normal(0.0, f64::MAX).is_err());
        assert!(rng.cached_normal.is_none());
    }

    // Pcg64Rng Tests

    #[test]
    fn test_pcg64_rng_uniform() {
        let mut rng = Pcg64Rng::new(42, 0);

        // Test that values are in [0, 1)
        for _ in 0..1000 {
            let val = rng.uniform();
            assert!(
                (0.0..1.0).contains(&val),
                "Uniform sample {} out of range",
                val
            );
        }
    }

    #[test]
    fn test_pcg64_rng_deterministic() {
        let mut rng1 = Pcg64Rng::new(42, 0);
        let mut rng2 = Pcg64Rng::new(42, 0);

        // Same seed should produce same sequence
        for _ in 0..100 {
            assert_eq!(rng1.uniform(), rng2.uniform());
        }
    }

    #[test]
    fn explicit_stream_matches_xsl_rr_known_answers() {
        // Top 53 bits of the first four XSL-RR 128/64 words for state 42, sequence 7.
        let mut rng = Pcg64Rng::new(42, 7);
        for grid_index in [
            0x9881280c50f7f_u64,
            0x12c966fe585f27_u64,
            0xd4b75b6d34757_u64,
            0x190d84dcbf34a8_u64,
        ] {
            assert_eq!(rng.uniform(), grid_index as f64 / (1_u64 << 53) as f64);
        }
    }

    #[test]
    fn test_pcg64_rng_different_seeds() {
        let mut rng1 = Pcg64Rng::new(42, 0);
        let mut rng2 = Pcg64Rng::new(123, 0);

        // Different seeds should (almost certainly) produce different sequences
        let v1: Vec<f64> = (0..10).map(|_| rng1.uniform()).collect();
        let v2: Vec<f64> = (0..10).map(|_| rng2.uniform()).collect();
        assert_ne!(v1, v2);
    }

    #[test]
    fn test_pcg64_rng_streams_independent() {
        let mut rng1 = Pcg64Rng::new(42, 0);
        let mut rng2 = Pcg64Rng::new(42, 1);

        // Different streams should produce different sequences
        let v1: Vec<f64> = (0..10).map(|_| rng1.uniform()).collect();
        let v2: Vec<f64> = (0..10).map(|_| rng2.uniform()).collect();
        assert_ne!(v1, v2);
    }

    #[test]
    fn test_pcg64_rng_adjacent_streams_truly_independent() {
        // Verify that adjacent stream IDs (including even/odd pairs) use
        // distinct PCG increments and produce uncorrelated sequences.
        let pairs: &[(u64, u64)] = &[(0, 1), (2, 3), (100, 101), (0, 2), (1, 3)];
        for &(s1, s2) in pairs {
            let mut rng1 = Pcg64Rng::new(42, s1);
            let mut rng2 = Pcg64Rng::new(42, s2);

            let v1: Vec<f64> = (0..20).map(|_| rng1.uniform()).collect();
            let v2: Vec<f64> = (0..20).map(|_| rng2.uniform()).collect();
            assert_ne!(
                v1, v2,
                "Streams {s1} and {s2} must produce different sequences"
            );
        }
    }

    #[test]
    fn test_pcg64_rng_normal() {
        let mut rng = Pcg64Rng::new(42, 0);

        // Test basic properties
        let sample = rng.normal(0.0, 1.0).unwrap();
        assert!(sample.is_finite());

        // Test mean and std_dev parameters
        let sample_shifted = rng.normal(5.0, 2.0).unwrap();
        assert!(sample_shifted.is_finite());
    }

    #[test]
    fn test_pcg64_rng_normal_statistics() {
        let mut rng = Pcg64Rng::new(12345, 0);
        let n = 10_000;

        let samples: Vec<f64> = (0..n).map(|_| rng.normal(0.0, 1.0).unwrap()).collect();

        let mean = samples.iter().sum::<f64>() / n as f64;
        let variance = samples.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (n - 1) as f64;

        // Mean should be close to 0
        assert!(mean.abs() < 0.05, "Sample mean {} too far from 0", mean);

        // Variance should be close to 1
        assert!(
            (variance - 1.0).abs() < 0.1,
            "Sample variance {} too far from 1",
            variance
        );
    }

    #[test]
    fn test_pcg64_rng_uniform_statistics() {
        let mut rng = Pcg64Rng::new(98765, 0);
        let n = 10_000;

        let samples: Vec<f64> = (0..n).map(|_| rng.uniform()).collect();

        let mean = samples.iter().sum::<f64>() / n as f64;
        let variance = samples.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (n - 1) as f64;

        // Mean should be close to 0.5
        assert!(
            (mean - 0.5).abs() < 0.02,
            "Uniform mean {} too far from 0.5",
            mean
        );

        // Variance should be close to 1/12 ≈ 0.0833
        let expected_var = 1.0 / 12.0;
        assert!(
            (variance - expected_var).abs() < 0.02,
            "Uniform variance {} too far from {}",
            variance,
            expected_var
        );
    }

    #[test]
    fn test_pcg64_rng_clone() {
        let mut rng1 = Pcg64Rng::new(42, 0);

        // Advance the RNG
        for _ in 0..50 {
            let _ = rng1.uniform();
        }

        // Clone
        let mut rng2 = rng1.clone();

        // Both should produce the same sequence from this point
        for _ in 0..100 {
            assert_eq!(rng1.uniform(), rng2.uniform());
        }
    }

    #[test]
    fn test_pcg64_chi_square_uniformity() {
        // Chi-square test for uniformity
        let mut rng = Pcg64Rng::new(54321, 0);
        let n_samples = 10_000;
        let n_bins = 10;
        let mut bins = vec![0usize; n_bins];

        for _ in 0..n_samples {
            let u = rng.uniform();
            let bin = (u * n_bins as f64).floor() as usize;
            let bin = bin.min(n_bins - 1); // Handle u = 1.0 edge case
            bins[bin] += 1;
        }

        // Expected count per bin
        let expected = n_samples as f64 / n_bins as f64;

        // Chi-square statistic
        let chi_sq: f64 = bins
            .iter()
            .map(|&count| {
                let diff = count as f64 - expected;
                diff * diff / expected
            })
            .sum();

        // For 9 degrees of freedom (10 bins - 1), chi-square critical value at 99%
        // significance is about 21.67. We use a more lenient threshold.
        assert!(
            chi_sq < 30.0,
            "Chi-square test failed: {} (bins: {:?})",
            chi_sq,
            bins
        );
    }
}
