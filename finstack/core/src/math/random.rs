//! Random number generation for Monte Carlo simulations.
//!
//! Provides a small RNG trait, a deterministic PCG64 implementation, Sobol
//! utilities, and sampling helpers used by simulation code.
//!
//! # Components
//!
//! - [`RandomNumberGenerator`]: Trait for pluggable RNG implementations
//! - [`Pcg64Rng`]: Deterministic PCG64 generator
//! - [`box_muller_transform`]: Normal variate generation from uniform samples
//! - [`sobol::SobolRng`]: Sobol low-discrepancy sequence with Owen scrambling
//! - [`brownian_bridge::BrownianBridge`]: Brownian bridge construction order
//! - [`poisson::poisson_inverse_cdf`]: Poisson sampling utilities
//! - [`sobol_pca::pca_ordering`]: PCA ordering for Sobol dimensions
//!
//! # Quick Start
//!
//! ```rust
//! use finstack_core::math::random::{Pcg64Rng, RandomNumberGenerator};
//!
//! // Create RNG with seed for reproducibility
//! let mut rng = Pcg64Rng::new(42);
//!
//! // Generate samples
//! let uniform = rng.uniform();       // U(0, 1)
//! let normal = rng.normal(0.0, 1.0); // N(0, 1)
//! let event = rng.bernoulli(0.05);   // 5% probability
//! ```
//!
//! # Statistical properties
//!
//! [`Pcg64Rng`] wraps `rand_pcg::Pcg64`:
//! - **Period**: 2^128
//! - **Streams**: [`Pcg64Rng::new_with_stream`] uses a stream-specific increment
//! - **Deterministic**: Same seed always produces same sequence
//!
//! # Parallel Simulations
//!
//! For parallel Monte Carlo, use independent streams:
//!
//! ```rust
//! use finstack_core::math::random::Pcg64Rng;
//!
//! // Each thread gets an independent stream (e.g., 4 threads)
//! let num_threads = 4;
//! let rngs: Vec<_> = (0..num_threads)
//!     .map(|thread_id| Pcg64Rng::new_with_stream(42, thread_id as u64))
//!     .collect();
//! ```
//!
//! # References
//!
//! - **PCG**: O'Neill, M. E. (2014). "PCG: A Family of Simple Fast Space-Efficient
//!   Statistically Good Algorithms for Random Number Generation."
//! - **Box-Muller**: Box, G. E. P., & Muller, M. E. (1958). "A Note on the
//!   Generation of Random Normal Deviates."

pub mod brownian_bridge;
pub mod poisson;
pub mod sobol;
pub mod sobol_pca;

pub use brownian_bridge::BrownianBridge;
pub use poisson::{poisson_from_normal, poisson_inverse_cdf};
pub use sobol_pca::{effective_dimension, pca_ordering, transform_pca_to_assets};

/// Random number generator trait for statistical sampling.
///
/// This trait provides the basic interface needed for Monte Carlo simulations
/// and stochastic sampling algorithms.
pub trait RandomNumberGenerator {
    /// Generate uniform random number in [0, 1)
    fn uniform(&mut self) -> f64;

    /// Generate a random 64-bit value.
    ///
    /// Implementations with native integer output should override this method.
    /// The default combines two uniforms so existing RNG implementations remain
    /// source-compatible.
    fn next_u64(&mut self) -> u64 {
        let hi = (self.uniform() * (u32::MAX as f64 + 1.0)) as u64;
        let lo = (self.uniform() * (u32::MAX as f64 + 1.0)) as u64;
        (hi << 32) | lo
    }

    /// Generate normal random number with specified mean and standard deviation
    fn normal(&mut self, mean: f64, std_dev: f64) -> f64;

    /// Generate Bernoulli random boolean with probability p
    fn bernoulli(&mut self, p: f64) -> bool;
}

// ============================================================================
// PCG64 RNG: Pcg64Rng
// ============================================================================

use rand::prelude::*;
use rand_pcg::Pcg64;

/// Deterministic random number generator backed by PCG64.
///
/// PCG64 (Permuted Congruential Generator) is used for seeded Monte Carlo
/// simulations and stochastic sampling:
///
/// - **Period**: 2^128
/// - **Streams**: [`Pcg64Rng::new_with_stream`] uses a stream-specific increment
/// - **Deterministic**: Same seed always produces same sequence
///
/// # Use Cases
///
/// - Monte Carlo option pricing
/// - Value at Risk (VaR) simulations
/// - Credit Valuation Adjustment (CVA) calculations
/// - Scenario generation for risk analysis
/// - Seeded financial simulations
///
/// # Stream Support
///
/// [`Pcg64Rng::new_with_stream`] creates deterministic stream-specific
/// sequences for:
/// - Parallel Monte Carlo paths
/// - Multi-threaded simulations
/// - Distributed computing scenarios
///
/// # Examples
///
/// ```rust
/// use finstack_core::math::random::{Pcg64Rng, RandomNumberGenerator};
///
/// // Create RNG with seed for reproducibility
/// let mut rng = Pcg64Rng::new(42);
///
/// // Generate uniform samples
/// let u = rng.uniform();
/// assert!((0.0..1.0).contains(&u));
///
/// // Generate normal samples for option pricing
/// let z = rng.normal(0.0, 1.0);
///
/// // Bernoulli for binary events
/// let default_occurred = rng.bernoulli(0.02); // 2% default probability
/// ```
///
/// # Parallel Streams
///
/// ```rust
/// use finstack_core::math::random::{Pcg64Rng, RandomNumberGenerator};
///
/// // Create independent streams for parallel simulation
/// let mut rng_path_0 = Pcg64Rng::new_with_stream(42, 0);
/// let mut rng_path_1 = Pcg64Rng::new_with_stream(42, 1);
///
/// // Each stream produces independent sequences
/// assert_ne!(rng_path_0.uniform(), rng_path_1.uniform());
/// ```
///
/// # References
///
/// - O'Neill, M. E. (2014). "PCG: A Family of Simple Fast Space-Efficient
///   Statistically Good Algorithms for Random Number Generation."
///   [https://www.pcg-random.org/](https://www.pcg-random.org/)
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Pcg64Rng {
    inner: Pcg64,
    cached_normal: Option<f64>,
    seed: u64,
    stream: u64,
}

impl Pcg64Rng {
    /// Create a new RNG with the given seed.
    ///
    /// The same seed will always produce the same sequence of random numbers,
    /// making simulations deterministic and reproducible.
    ///
    /// # Arguments
    ///
    /// * `seed` - 64-bit seed value. Different seeds produce different sequences.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_core::math::random::{Pcg64Rng, RandomNumberGenerator};
    ///
    /// let mut rng1 = Pcg64Rng::new(42);
    /// let mut rng2 = Pcg64Rng::new(42);
    ///
    /// // Same seed produces same sequence
    /// assert_eq!(rng1.uniform(), rng2.uniform());
    /// ```
    #[must_use]
    pub fn new(seed: u64) -> Self {
        Self {
            inner: Pcg64::seed_from_u64(seed),
            cached_normal: None,
            seed,
            stream: 0,
        }
    }

    /// Create a new RNG with seed and stream for parallel simulations.
    ///
    /// Each (seed, stream) pair produces a unique, independent sequence.
    /// Streams are guaranteed to be non-overlapping for at least 2^64 samples.
    ///
    /// # Arguments
    ///
    /// * `seed` - Base seed for the generator
    /// * `stream` - Stream identifier for parallel independence
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_core::math::random::{Pcg64Rng, RandomNumberGenerator};
    ///
    /// // Parallel Monte Carlo with independent paths
    /// let paths: Vec<_> = (0..100)
    ///     .map(|i| Pcg64Rng::new_with_stream(42, i))
    ///     .collect();
    /// ```
    #[must_use]
    pub fn new_with_stream(seed: u64, stream: u64) -> Self {
        // PCG64 uses a 128-bit state. We combine seed and stream to create
        // a unique starting point. The stream affects the increment, ensuring
        // different streams produce non-overlapping sequences.
        let state = ((stream as u128) << 64) | (seed as u128);
        Self {
            inner: Pcg64::new(state, ((stream as u128) << 1) | 1), // Unique odd increment per stream
            cached_normal: None,
            seed,
            stream,
        }
    }

    /// Get the current seed (for serialization/debugging).
    ///
    /// Note: This returns the original seed, not the current internal state.
    /// For full state preservation, use serde serialization.
    #[must_use]
    pub fn seed(&self) -> u64 {
        self.seed
    }

    /// Get the configured stream identifier.
    #[must_use]
    pub fn stream(&self) -> u64 {
        self.stream
    }
}

impl RandomNumberGenerator for Pcg64Rng {
    #[inline]
    fn uniform(&mut self) -> f64 {
        self.inner.random()
    }

    #[inline]
    fn next_u64(&mut self) -> u64 {
        self.inner.next_u64()
    }

    fn normal(&mut self, mean: f64, std_dev: f64) -> f64 {
        // Box-Muller transform with caching for efficiency
        if let Some(cached) = self.cached_normal.take() {
            return mean + std_dev * cached;
        }

        let u1 = self.uniform();
        let u2 = self.uniform();

        let (z0, z1) = box_muller_transform(u1, u2);
        self.cached_normal = Some(z1);
        mean + std_dev * z0
    }

    #[inline]
    fn bernoulli(&mut self, p: f64) -> bool {
        debug_assert!(
            (0.0..=1.0).contains(&p),
            "bernoulli probability must be in [0, 1], got {p}"
        );
        self.uniform() < p
    }
}

/// Box-Muller transform for generating normal random variables.
///
/// Transforms two independent uniform random variables into two independent
/// standard normal random variables using the Box-Muller method. This is the
/// classic algorithm for generating Gaussian samples in Monte Carlo simulations.
///
/// # Arguments
///
/// * `u1` - First uniform random variable in (0, 1)
/// * `u2` - Second uniform random variable in (0, 1)
///
/// # Returns
///
/// Tuple of two independent N(0,1) random variables (z₁, z₂).
///
/// # Algorithm
///
/// ```text
/// z₁ = √(-2 ln u₁) cos(2π u₂)
/// z₂ = √(-2 ln u₁) sin(2π u₂)
/// ```
///
/// # Numerical Details
///
/// Uses `f64::MIN_POSITIVE` (~2.2e-308) for clamping to allow extreme tail
/// values up to ±37σ while preventing -∞ from ln(0). The clamp at 1-ε
/// prevents numerical issues when u1 is very close to 1.
///
/// Achievable range: |z| ≤ √(-2 ln(2.2e-308)) ≈ 37.7
///
/// # Examples
///
/// ```rust
/// use finstack_core::math::random::box_muller_transform;
///
/// let (z1, z2) = box_muller_transform(0.5, 0.5);
/// // z1 and z2 are independent N(0,1) samples
/// ```
///
/// # References
///
/// - Box, G. E. P., & Muller, M. E. (1958). "A Note on the Generation of Random
///   Normal Deviates." *The Annals of Mathematical Statistics*, 29(2), 610-611.
#[inline]
pub fn box_muller_transform(u1: f64, u2: f64) -> (f64, f64) {
    use std::f64::consts::PI;

    // Use smallest positive f64 to allow extreme tail sampling (up to ~37σ)
    // while preventing -inf from ln(0). The 1.0 - EPS upper bound prevents
    // numerical issues when u1 rounds to exactly 1.0.
    //
    // With EPS = MIN_POSITIVE (~2.2e-308):
    //   -2 * ln(2.2e-308) ≈ 1418, so sqrt ≈ 37.7
    //   This allows sampling ~37σ events (probability ~1e-300)
    const EPS: f64 = f64::MIN_POSITIVE;
    let u1_safe = u1.clamp(EPS, 1.0 - f64::EPSILON);

    let r = (-2.0 * u1_safe.ln()).sqrt();
    let theta = 2.0 * PI * u2;
    let z1 = r * theta.cos();
    let z2 = r * theta.sin();
    (z1, z2)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_box_muller_transform() {
        let (z1, z2) = box_muller_transform(0.5, 0.5);
        assert!(z1.is_finite());
        assert!(z2.is_finite());

        // Test with many samples using Pcg64Rng
        let mut rng = Pcg64Rng::new(42);
        let mut samples = Vec::new();
        for _ in 0..500 {
            let u1 = rng.uniform();
            let u2 = rng.uniform();
            let (z1, z2) = box_muller_transform(u1, u2);
            samples.push(z1);
            samples.push(z2);
        }

        let mean = samples.iter().sum::<f64>() / samples.len() as f64;
        let var =
            samples.iter().map(|&x| (x - mean).powi(2)).sum::<f64>() / (samples.len() - 1) as f64;

        assert!(mean.abs() < 0.1);
        assert!((var - 1.0).abs() < 0.2);
    }

    // ── H4 documentation tests: Box-Muller clamping semantics ──
    //
    // The lower bound is f64::MIN_POSITIVE (~2.2e-308) so that deep tails
    // (up to ~37σ) remain accessible for rare-event simulation.
    // The reviewer suggested 1e-10 (truncates at ~6.8σ), but this was
    // intentionally rejected to preserve tail sampling capability.
    // These tests lock in the current intentional policy.
    #[test]
    fn test_box_muller_boundary_u1_zero_is_clamped_to_finite() {
        // u1 = 0.0 would cause ln(0) = -inf; clamping makes it finite.
        let (z1, z2) = box_muller_transform(0.0, 0.5);
        assert!(z1.is_finite(), "z1 must be finite when u1=0, got {z1}");
        assert!(z2.is_finite(), "z2 must be finite when u1=0, got {z2}");
    }

    #[test]
    fn test_box_muller_boundary_u1_near_one_is_clamped_to_finite() {
        // u1 very close to 1.0 (e.g., exactly 1.0 due to rounding) must stay finite.
        let (z1, z2) = box_muller_transform(1.0, 0.5);
        assert!(z1.is_finite(), "z1 must be finite when u1≈1, got {z1}");
        assert!(z2.is_finite(), "z2 must be finite when u1≈1, got {z2}");
    }

    #[test]
    fn test_box_muller_min_positive_clamp_allows_deep_tails() {
        // With u1 = MIN_POSITIVE the radius is sqrt(-2 ln(2.2e-308)) ≈ 37.7.
        // The tail is intentionally deep for rare-event simulation.
        let (z1, _z2) = box_muller_transform(f64::MIN_POSITIVE, 0.0);
        assert!(z1.is_finite());
        // Radius ≈ 37.7σ; accepting anything > 30 as evidence the deep tail is reachable.
        assert!(
            z1.abs() > 30.0,
            "Deep tail should be reachable with MIN_POSITIVE lower bound, got |z1|={}",
            z1.abs()
        );
    }

    // ========================================================================
    // Pcg64Rng Tests
    // ========================================================================

    #[test]
    fn test_pcg64_rng_uniform() {
        let mut rng = Pcg64Rng::new(42);

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
        let mut rng1 = Pcg64Rng::new(42);
        let mut rng2 = Pcg64Rng::new(42);

        // Same seed should produce same sequence
        for _ in 0..100 {
            assert_eq!(rng1.uniform(), rng2.uniform());
        }
    }

    #[test]
    fn test_pcg64_rng_reports_seed_and_stream() {
        let rng = Pcg64Rng::new(42);
        assert_eq!(rng.seed(), 42);
        assert_eq!(rng.stream(), 0);

        let rng = Pcg64Rng::new_with_stream(7, 11);
        assert_eq!(rng.seed(), 7);
        assert_eq!(rng.stream(), 11);
    }

    #[test]
    fn test_pcg64_rng_different_seeds() {
        let mut rng1 = Pcg64Rng::new(42);
        let mut rng2 = Pcg64Rng::new(123);

        // Different seeds should (almost certainly) produce different sequences
        let v1: Vec<f64> = (0..10).map(|_| rng1.uniform()).collect();
        let v2: Vec<f64> = (0..10).map(|_| rng2.uniform()).collect();
        assert_ne!(v1, v2);
    }

    #[test]
    fn test_pcg64_rng_streams_independent() {
        let mut rng1 = Pcg64Rng::new_with_stream(42, 0);
        let mut rng2 = Pcg64Rng::new_with_stream(42, 1);

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
            let mut rng1 = Pcg64Rng::new_with_stream(42, s1);
            let mut rng2 = Pcg64Rng::new_with_stream(42, s2);

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
        let mut rng = Pcg64Rng::new(42);

        // Test basic properties
        let sample = rng.normal(0.0, 1.0);
        assert!(sample.is_finite());

        // Test mean and std_dev parameters
        let sample_shifted = rng.normal(5.0, 2.0);
        assert!(sample_shifted.is_finite());
    }

    #[test]
    fn test_pcg64_rng_normal_statistics() {
        let mut rng = Pcg64Rng::new(12345);
        let n = 10_000;

        let samples: Vec<f64> = (0..n).map(|_| rng.normal(0.0, 1.0)).collect();

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
    fn test_pcg64_rng_bernoulli() {
        let mut rng = Pcg64Rng::new(42);

        // Test extreme probabilities
        for _ in 0..10 {
            assert!(!rng.bernoulli(0.0));
            assert!(rng.bernoulli(1.0));
        }

        // Reset RNG for frequency test
        let mut rng = Pcg64Rng::new(42);
        let mut successes = 0;
        let n = 10_000;

        for _ in 0..n {
            if rng.bernoulli(0.3) {
                successes += 1;
            }
        }

        // Should be roughly 30% successes (3000 ± 3σ where σ ≈ 45.8)
        let expected = 0.3 * n as f64;
        let std_dev = (0.3 * 0.7 * n as f64).sqrt();
        assert!(
            (successes as f64 - expected).abs() < 4.0 * std_dev,
            "Bernoulli frequency {} too far from expected {}",
            successes,
            expected
        );
    }

    #[test]
    fn test_pcg64_rng_uniform_statistics() {
        let mut rng = Pcg64Rng::new(98765);
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
    fn test_pcg64_rng_serde_roundtrip() {
        let mut rng = Pcg64Rng::new(42);

        // Advance the RNG state
        for _ in 0..100 {
            let _ = rng.uniform();
        }

        // Serialize the current state
        let json = serde_json::to_string(&rng).expect("Failed to serialize Pcg64Rng");

        // Deserialize into a new RNG
        let mut rng2: Pcg64Rng =
            serde_json::from_str(&json).expect("Failed to deserialize Pcg64Rng");

        // Both RNGs must produce identical sequences from this point forward,
        // proving the full internal PCG64 state was preserved (not just reseeded).
        for _ in 0..100 {
            assert_eq!(
                rng.uniform(),
                rng2.uniform(),
                "Deserialized RNG diverged from original — serde roundtrip broken"
            );
        }
    }

    #[test]
    fn test_pcg64_rng_clone() {
        let mut rng1 = Pcg64Rng::new(42);

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
        let mut rng = Pcg64Rng::new(54321);
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
