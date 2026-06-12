//! Philox 4x32-10 counter-based random number generator.
//!
//! Philox is a counter-based PRNG that enables:
//! - Deterministic parallel execution (each path gets independent stream)
//! - No shared state between threads
//! - Reproducible results regardless of thread count
//!
//! # Algorithm
//!
//! Philox uses a simplified Feistel network structure with S-box substitution
//! via multiplication. The "4x32-10" variant produces 4 32-bit outputs per
//! iteration using 10 rounds of mixing.
//!
//! # Why Philox for Monte Carlo?
//!
//! Counter-based PRNGs like Philox are ideal for Monte Carlo simulation:
//!
//! 1. **Perfect parallelization**: No mutable shared state between threads
//! 2. **Reproducibility**: Same seed + counter always gives same result
//! 3. **Stream splitting**: Each path can have an independent stream
//! 4. **Fast**: Optimized for modern CPUs, no memory lookups
//!
//! # Statistical Quality
//!
//! Philox passes all tests in the TestU01 BigCrush suite, the most stringent
//! battery of statistical tests for PRNGs. It has been validated for use in
//! high-stakes scientific computing applications.
//!
//! # Industry Adoption
//!
//! Philox is the default or recommended PRNG in several major frameworks:
//! - **TensorFlow**: `tf.random.Generator` uses Philox
//! - **JAX**: `jax.random` uses Threefry/Philox
//! - **NVIDIA cuRAND**: Philox available as `CURAND_RNG_PHILOX4_32_10`
//! - **NumPy**: Available via `numpy.random.Philox`
//!
//! # References
//!
//! - Salmon, J.K., Moraes, M.A., Dror, R.O., & Shaw, D.E. (2011).
//!   "Parallel Random Numbers: As Easy as 1, 2, 3."
//!   Proceedings of SC '11 (International Conference for High Performance
//!   Computing, Networking, Storage and Analysis).
//!   DOI: 10.1145/2063384.2063405
//!
//! - TestU01: L'Ecuyer, P., & Simard, R. (2007).
//!   "TestU01: A C Library for Empirical Testing of Random Number Generators."
//!   ACM Transactions on Mathematical Software, 33(4), Article 22.

use super::super::traits::RandomStream;
use finstack_core::math::random::box_muller_transform;

/// Philox 4x32-10 random number generator.
///
/// Uses a 64-bit counter and 64-bit key for generating random numbers.
/// The algorithm is a Feistel-like construction that provides strong
/// statistical properties while being fast and parallel-friendly.
#[derive(Debug, Clone)]
pub struct PhiloxRng {
    /// Global seed (forms the key)
    key: u64,
    /// Stream ID (for splitting)
    stream_id: u64,
    /// Current counter value
    counter: u64,
    /// Index within current block (0-3)
    idx: usize,
    /// Current block of random values
    block: [u32; 4],
    /// Spare normal variate from the most recent Box-Muller pair.
    spare_normal: Option<f64>,
}

// Philox constants
const PHILOX_M0: u32 = 0xD2511F53;
const PHILOX_M1: u32 = 0xCD9E8D57;
const PHILOX_W0: u32 = 0x9E3779B9;
const PHILOX_W1: u32 = 0xBB67AE85;

impl PhiloxRng {
    /// Create a new Philox RNG with the given seed.
    #[inline]
    pub fn new(seed: u64) -> Self {
        let mut rng = Self {
            key: seed,
            stream_id: 0,
            counter: 0,
            idx: 4, // Placeholder; the eager generate_block() below resets it.
            block: [0; 4],
            spare_normal: None,
        };
        rng.generate_block();
        rng
    }

    /// Infallible substream construction: returns a fresh `PhiloxRng` seeded
    /// from the same key but with the given `stream_id`.
    ///
    /// Equivalent to the trait method [`RandomStream::split`] but with a
    /// `Self` return type, avoiding `Option`-unwrapping at concrete call sites.
    #[inline]
    #[must_use]
    pub fn substream(&self, stream_id: u64) -> Self {
        Self::with_stream(self.key, stream_id)
    }

    /// Create with explicit stream ID (for splitting).
    #[inline]
    pub fn with_stream(seed: u64, stream_id: u64) -> Self {
        let mut rng = Self {
            key: seed,
            stream_id,
            counter: 0,
            idx: 4,
            block: [0; 4],
            spare_normal: None,
        };
        rng.generate_block();
        rng
    }

    /// Generate a new block of random values.
    ///
    /// This is a hot path method called frequently during simulation.
    /// The loop is unrolled by the compiler for optimal performance.
    #[inline]
    fn generate_block(&mut self) {
        // Combine stream_id and counter to form the full counter
        let ctr0 = (self.counter & 0xFFFFFFFF) as u32;
        let ctr1 = ((self.counter >> 32) & 0xFFFFFFFF) as u32;
        let ctr2 = (self.stream_id & 0xFFFFFFFF) as u32;
        let ctr3 = ((self.stream_id >> 32) & 0xFFFFFFFF) as u32;

        // Extract key parts
        let key0 = (self.key & 0xFFFFFFFF) as u32;
        let key1 = ((self.key >> 32) & 0xFFFFFFFF) as u32;

        // Apply Philox-4x32-10 rounds
        let (mut v0, mut v1, mut v2, mut v3) = (ctr0, ctr1, ctr2, ctr3);
        let (mut k0, mut k1) = (key0, key1);

        for _ in 0..10 {
            // Feistel-like round
            let (hi0, lo0) = mulhilo(v0, PHILOX_M0);
            let (hi1, lo1) = mulhilo(v2, PHILOX_M1);

            v0 = hi1 ^ v1 ^ k0;
            v1 = lo1;
            v2 = hi0 ^ v3 ^ k1;
            v3 = lo0;

            // Update key
            k0 = k0.wrapping_add(PHILOX_W0);
            k1 = k1.wrapping_add(PHILOX_W1);
        }

        self.block = [v0, v1, v2, v3];
        self.idx = 0;
        self.counter = self.counter.wrapping_add(1);
    }

    /// Get next u32 value.
    ///
    /// Hot path method - called very frequently during simulation.
    #[inline]
    fn next_u32(&mut self) -> u32 {
        if self.idx >= 4 {
            self.generate_block();
        }
        let val = self.block[self.idx];
        self.idx += 1;
        val
    }

    /// Get next u64 value (combines two u32s).
    ///
    /// Hot path method - used for generating uniforms.
    #[inline]
    fn next_u64(&mut self) -> u64 {
        let lo = self.next_u32() as u64;
        let hi = self.next_u32() as u64;
        (hi << 32) | lo
    }
}

impl RandomStream for PhiloxRng {
    #[inline]
    fn split(&self, stream_id: u64) -> Option<Self> {
        // Create a new stream with a different stream_id
        // This ensures independence between streams
        Some(PhiloxRng::with_stream(self.key, stream_id))
    }

    /// Fill buffer with uniform random numbers in the open interval (0, 1).
    ///
    /// Hot path method - called on every timestep of every path.
    ///
    /// The grid-centred mapping `(bits + 0.5)·2⁻⁵³` never produces exactly
    /// 0 or 1 (matching core's Sobol normal mapping), so inverse-CDF
    /// consumers and the antithetic mirror `1 − u` are safe at both
    /// boundaries.
    #[inline]
    fn fill_u01(&mut self, out: &mut [f64]) {
        for x in out {
            // Convert u64 to (0, 1) using the upper 53 bits, centred.
            let bits = self.next_u64() >> 11;
            *x = (bits as f64 + 0.5) * (1.0 / (1u64 << 53) as f64);
        }
    }

    /// Fill buffer with standard normal random numbers.
    ///
    /// Hot path method - called on every timestep of every path.
    /// Uses Box-Muller transform in pairs for efficiency.
    #[inline]
    fn fill_std_normals(&mut self, out: &mut [f64]) {
        let mut i = 0;
        if let Some(spare) = self.spare_normal.take() {
            if let Some(first) = out.first_mut() {
                *first = spare;
                i = 1;
            }
        }

        while i < out.len() {
            let u1 = self.next_u01();
            let u2 = self.next_u01();

            let (z1, z2) = box_muller_transform(u1, u2);
            out[i] = z1;
            i += 1;

            if i < out.len() {
                out[i] = z2;
                i += 1;
            } else {
                self.spare_normal = Some(z2);
            }
        }
    }
}

/// Multiply two u32 values and return high and low parts of 64-bit result.
#[inline(always)]
fn mulhilo(a: u32, b: u32) -> (u32, u32) {
    let product = (a as u64) * (b as u64);
    let hi = (product >> 32) as u32;
    let lo = (product & 0xFFFFFFFF) as u32;
    (hi, lo)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_philox_basic() {
        let mut rng = PhiloxRng::new(42);
        let val = rng.next_u01();
        assert!((0.0..1.0).contains(&val));
    }

    /// Known-answer test against the Random123 reference distribution
    /// (Salmon et al. 2011, `kat_vectors`): philox4x32-10 with
    /// `ctr = {0,0,0,0}`, `key = {0,0}` must produce
    /// `{0x6627e8d5, 0xe169c58d, 0xbc57ac4c, 0x9b00dbd8}`.
    ///
    /// `PhiloxRng::new(0)` maps exactly onto that input: key = seed = 0,
    /// counter words = [counter_lo, counter_hi, stream_lo, stream_hi] =
    /// {0,0,0,0} for the first block. Moment tests cannot detect a
    /// transposed multiplier or a swapped Weyl constant; this can.
    #[test]
    fn test_philox_matches_random123_reference_vector() {
        let mut rng = PhiloxRng::new(0);
        let expected: [u32; 4] = [0x6627_e8d5, 0xe169_c58d, 0xbc57_ac4c, 0x9b00_dbd8];
        for (i, &e) in expected.iter().enumerate() {
            let got = rng.next_u32();
            assert_eq!(got, e, "word {i}: got {got:#010x}, expected {e:#010x}");
        }
    }

    /// Adjacent streams must be statistically uncorrelated, not merely
    /// "first draws differ". A 4σ bound on the sample correlation of 20k
    /// normal pairs is ≈ 0.028.
    #[test]
    fn test_adjacent_streams_uncorrelated() {
        let n = 20_000;
        let mut a = PhiloxRng::with_stream(42, 1);
        let mut b = PhiloxRng::with_stream(42, 2);
        let mut za = vec![0.0; n];
        let mut zb = vec![0.0; n];
        a.fill_std_normals(&mut za);
        b.fill_std_normals(&mut zb);

        let mean = |v: &[f64]| v.iter().sum::<f64>() / v.len() as f64;
        let (ma, mb) = (mean(&za), mean(&zb));
        let mut cov = 0.0;
        let mut va = 0.0;
        let mut vb = 0.0;
        for (x, y) in za.iter().zip(&zb) {
            cov += (x - ma) * (y - mb);
            va += (x - ma) * (x - ma);
            vb += (y - mb) * (y - mb);
        }
        let corr = cov / (va.sqrt() * vb.sqrt());
        let bound = 4.0 / (n as f64).sqrt();
        assert!(
            corr.abs() < bound,
            "adjacent streams correlated: |{corr}| >= {bound}"
        );
    }

    #[test]
    fn test_philox_reproducibility() {
        let mut rng1 = PhiloxRng::new(42);
        let mut rng2 = PhiloxRng::new(42);

        for _ in 0..100 {
            assert_eq!(rng1.next_u32(), rng2.next_u32());
        }
    }

    #[test]
    fn test_philox_split_independence() {
        let rng = PhiloxRng::new(42);
        let mut stream1 = rng.substream(1);
        let mut stream2 = rng.substream(2);

        // Different streams should produce different values
        let val1 = stream1.next_u01();
        let val2 = stream2.next_u01();
        assert_ne!(val1, val2);
    }

    #[test]
    fn test_philox_split_reproducibility() {
        let rng = PhiloxRng::new(42);
        let mut stream1a = rng.substream(1);
        let mut stream1b = rng.substream(1);

        // Same stream ID should produce same values
        for _ in 0..100 {
            assert_eq!(stream1a.next_u32(), stream1b.next_u32());
        }
    }

    #[test]
    fn test_philox_normals() {
        let mut rng = PhiloxRng::new(42);
        let mut normals = vec![0.0; 1000];
        rng.fill_std_normals(&mut normals);

        // Check basic statistical properties
        let mean: f64 = normals.iter().sum::<f64>() / normals.len() as f64;
        let variance: f64 =
            normals.iter().map(|&x| (x - mean).powi(2)).sum::<f64>() / (normals.len() - 1) as f64;

        // Mean should be close to 0, variance close to 1
        assert!(mean.abs() < 0.1);
        assert!((variance - 1.0).abs() < 0.1);
    }

    #[test]
    fn test_fill_u01_range() {
        let mut rng = PhiloxRng::new(12345);
        let mut values = vec![0.0; 1000];
        rng.fill_u01(&mut values);

        for &val in &values {
            assert!((0.0..1.0).contains(&val));
        }
    }

    #[test]
    fn test_philox_reuses_spare_normal_after_odd_request() {
        let mut rng_odd = PhiloxRng::new(42);
        let mut rng_even = PhiloxRng::new(42);

        let mut odd = [0.0; 1];
        rng_odd.fill_std_normals(&mut odd);
        let next_from_odd = rng_odd.next_std_normal();

        let mut even = [0.0; 2];
        rng_even.fill_std_normals(&mut even);

        assert_eq!(odd[0], even[0]);
        assert_eq!(next_from_odd, even[1]);
    }
}
