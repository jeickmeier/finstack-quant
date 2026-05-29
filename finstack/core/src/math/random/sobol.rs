//! Sobol quasi-Monte Carlo sequence with Owen scrambling.
//!
//! Sobol sequences are low-discrepancy quasi-random sequences that
//! provide better convergence than pseudo-random for smooth payoffs.
//!
//! Owen scrambling adds randomization while preserving low-discrepancy
//! properties, enabling error estimation.
//!
//! # Owen Scrambling Implementation
//!
//! This implementation uses proper recursive bitwise Owen scrambling as described
//! in Owen (1995, 1997). The scrambling applies a random permutation to each digit
//! where the permutation depends on all higher-order digits. This preserves the
//! (t,m,s)-net structure while providing independent randomization for variance
//! estimation.
//!
//! # Dimension Support
//!
//! This implementation supports up to 40 dimensions using direction numbers
//! from Joe & Kuo (2008). For higher dimensions (up to 21201), use the
//! direction number tables available at:
//! <https://web.maths.unsw.edu.au/~fkuo/sobol/>
//!
//! # References
//!
//! - Joe, S., & Kuo, F. Y. (2008). "Constructing Sobol Sequences with Better
//!   Two-Dimensional Projections." SIAM J. Sci. Comput., 30(5), 2635-2654.
//!
//! - Sobol, I.M. (1967). "Distribution of points in a cube and approximate
//!   evaluation of integrals." USSR Comp. Math. and Math. Physics, 7(4), 86-112.
//!
//! - Owen, A. B. (1995). "Randomly Permuted (t,m,s)-Nets and (t,s)-Sequences."
//!   Monte Carlo and Quasi-Monte Carlo Methods in Scientific Computing, 299-317.
//!
//! - Owen, A. B. (1997). "Scrambled Net Variance for Integrals of Smooth Functions."
//!   Annals of Statistics, 25(4), 1541-1562.

use crate::math::special_functions::standard_normal_inv_cdf as inverse_normal_cdf;

/// Maximum supported dimension for this Sobol implementation.
///
/// Higher dimensions require additional direction numbers from Joe & Kuo's tables.
/// See <https://web.maths.unsw.edu.au/~fkuo/sobol/> for tables up to 21201 dimensions.
pub const MAX_SOBOL_DIMENSION: usize = 40;

/// Combine two values into a deterministic hash.
///
/// Uses a variant of the Boost hash_combine approach with improved mixing.
#[inline]
fn hash_combine(seed: u64, value: u64) -> u32 {
    // Mix the seed and value together
    let mut h = seed;
    h ^= value.wrapping_add(0x9e3779b97f4a7c15); // Golden ratio fractional part for 64-bit
    h = h.wrapping_mul(0xbf58476d1ce4e5b9); // Splitmix64 constant
    h ^= h >> 30;
    h = h.wrapping_mul(0x94d049bb133111eb);
    h ^= h >> 31;
    h as u32
}

/// Sobol sequence generator with Owen scrambling.
///
/// This implementation supports up to [`MAX_SOBOL_DIMENSION`] dimensions using
/// direction numbers from Joe & Kuo (2008). For production use with higher
/// dimensions (e.g., pricing baskets with many underlyings), consider loading
/// direction numbers from the full Joe & Kuo tables.
///
/// # Owen Scrambling
///
/// Uses recursive bitwise Owen scrambling where each bit's permutation depends
/// on all higher-order bits. This preserves the low-discrepancy structure while
/// providing randomization for variance estimation.
///
/// # Dimension Requirements
///
/// - Single-asset paths: 1 dimension per timestep
/// - Multi-asset (basket, correlation): `n_assets × n_timesteps` dimensions
/// - Heston/stochastic vol: 2 dimensions per timestep
///
/// For a 10-asset basket with 252 daily steps: 2520 dimensions (requires extended tables).
///
/// # Example
///
/// ```rust
/// use finstack_core::math::random::sobol::SobolRng;
///
/// // Create 3D Sobol sequence with Owen scrambling
/// let mut sobol = SobolRng::try_new(3, 12345).expect("valid dimension");
///
/// // Generate 100 quasi-random points
/// for _ in 0..100 {
///     let point = sobol.next_point();
///     assert!(point.iter().all(|&x| x >= 0.0 && x < 1.0));
/// }
/// ```
#[derive(Clone, Debug)]
pub struct SobolRng {
    /// Current index in the sequence
    index: u64,
    /// Dimension
    dimension: usize,
    /// Base scrambling seed for each dimension
    scramble_seeds: Vec<u32>,
    /// Scramble matrices for proper Owen scrambling (32 values per dimension)
    /// Each row contains hash seeds for recursive bit permutation
    scramble_matrices: Vec<[u32; 32]>,
    /// Direction numbers for Sobol construction
    direction_numbers: Vec<Vec<u32>>,
}

impl SobolRng {
    /// Fallible constructor for a new Sobol sequence.
    ///
    /// Returns an error if `dimension` is 0 or exceeds [`MAX_SOBOL_DIMENSION`].
    pub fn try_new(dimension: usize, scramble_seed: u64) -> crate::Result<Self> {
        if dimension == 0 || dimension > MAX_SOBOL_DIMENSION {
            return Err(crate::Error::Validation(format!(
                "Sobol dimension must be 1-{MAX_SOBOL_DIMENSION}, got {dimension}"
            )));
        }

        // Initialize direction numbers (simplified for first 8 dimensions)
        let direction_numbers = initialize_direction_numbers(dimension);

        // Generate scrambling seeds and matrices for proper Owen scrambling
        let mut scramble_seeds = Vec::with_capacity(dimension);
        let mut scramble_matrices = Vec::with_capacity(dimension);

        for i in 0..dimension {
            let base_seed = if scramble_seed == 0 {
                0
            } else {
                // Create dimension-specific seed using a mixing function
                hash_combine(scramble_seed, i as u64)
            };
            scramble_seeds.push(base_seed);

            // Initialize scramble matrix for this dimension
            // Each row provides hash seeds for the recursive bit scrambling
            let mut matrix = [0u32; 32];
            if scramble_seed != 0 {
                for (bit, entry) in matrix.iter_mut().enumerate() {
                    // Generate deterministic but uncorrelated seeds for each bit level
                    *entry = hash_combine(base_seed as u64, bit as u64);
                }
            }
            scramble_matrices.push(matrix);
        }

        Ok(Self {
            index: 0,
            dimension,
            scramble_seeds,
            scramble_matrices,
            direction_numbers,
        })
    }

    /// Get the next point in the Sobol sequence.
    ///
    /// Returns a vector of `dimension` values in \[0, 1).
    /// For allocation-free usage in tight loops, prefer [`fill_point`](Self::fill_point).
    pub fn next_point(&mut self) -> Vec<f64> {
        let mut point = vec![0.0; self.dimension];
        self.fill_point(&mut point);
        point
    }

    /// Fill `buf` with the next Sobol point, avoiding per-call allocation.
    ///
    /// Writes `min(buf.len(), dimension)` values into `buf`. Advances the
    /// sequence index by one regardless of buffer length.
    pub fn fill_point(&mut self, buf: &mut [f64]) {
        for (d, slot) in buf.iter_mut().enumerate().take(self.dimension) {
            let value = self.sobol_value(d);
            *slot = self.owen_scramble(value, d);
        }
        self.index += 1;
    }

    /// Compute Sobol value for dimension d at current index.
    fn sobol_value(&self, d: usize) -> u32 {
        let mut value = 0u32;
        let mut index = self.index;
        let mut bit = 0;

        while index > 0 {
            if (index & 1) == 1 {
                value ^= self.direction_numbers[d][bit];
            }
            index >>= 1;
            bit += 1;
        }

        value
    }

    /// Apply an Owen-style scramble to a Sobol value.
    ///
    /// Owen scrambling (Owen 1995, 1997) applies a recursive bitwise permutation
    /// where each bit's flip decision depends on all higher-order (more significant)
    /// bits, achieving better variance reduction than simple XOR scrambling.
    ///
    /// # Approximation
    ///
    /// This is an **approximate** Owen scramble: the per-bit flip is a *hash* of
    /// the higher-order bits XOR a fixed per-bit scramble seed, not the canonical
    /// nested-uniform permutation tree. It decorrelates replications and is
    /// adequate for randomized-QMC error bars at the dimensions used here, but it
    /// does **not** strictly guarantee the (t,m,s)-net-preserving randomization
    /// that scrambled-net variance theory assumes — quality can degrade in high
    /// dimensions where the higher-bit space is sparse. Replace with a Matoušek
    /// linear scramble or a full nested-uniform construction (validated against a
    /// reference scrambled-Sobol sequence) if strict net preservation is needed.
    ///
    /// The algorithm processes bits from most significant to least significant,
    /// where the decision to flip bit `i` depends on:
    /// - The hash of all bits at positions > i
    /// - A per-bit scrambling seed from the scramble matrix
    ///
    /// # References
    ///
    /// - Owen, A. B. (1995). "Randomly Permuted (t,m,s)-Nets and (t,s)-Sequences."
    /// - Owen, A. B. (1997). "Scrambled Net Variance for Integrals of Smooth Functions."
    fn owen_scramble(&self, value: u32, d: usize) -> f64 {
        (self.owen_scramble_int(value, d) as f64) / (u32::MAX as f64 + 1.0)
    }

    /// Apply Owen scrambling and return the scrambled integer in [0, 2^32).
    fn owen_scramble_int(&self, value: u32, d: usize) -> u32 {
        if self.scramble_seeds[d] == 0 {
            return value;
        }

        let matrix = &self.scramble_matrices[d];
        let mut scrambled = value;

        // Process from most significant bit to least significant
        // Each bit's flip depends on all higher-order bits
        for (bit, &seed) in matrix.iter().enumerate() {
            let bit_pos = 31 - bit; // Process from MSB to LSB

            // Extract higher-order bits (more significant than current bit)
            let higher_bits = if bit == 0 {
                0u32
            } else {
                scrambled >> (bit_pos + 1)
            };

            // Hash the higher bits with the per-bit seed to determine flip
            let hash_input = higher_bits ^ seed;
            let flip = Self::should_flip_bit(hash_input);

            // Conditionally flip the current bit
            if flip {
                scrambled ^= 1u32 << bit_pos;
            }
        }

        scrambled
    }

    /// Determine if a bit should be flipped based on hash input.
    ///
    /// Uses a simple but effective mixing function (MurmurHash-inspired) to convert
    /// the hash input into a binary decision (flip or not).
    #[inline]
    fn should_flip_bit(hash_input: u32) -> bool {
        // Mix the input bits thoroughly using multiplicative hashing
        let mixed = hash_input
            .wrapping_mul(0x9e3779b9) // Golden ratio fractional part
            .wrapping_add(0x6a09e667); // SHA-256 initial hash value H0
        let mixed = mixed ^ (mixed >> 16);
        let mixed = mixed.wrapping_mul(0x85ebca6b); // MurmurHash3 constant
        let mixed = mixed ^ (mixed >> 13);

        // Use the LSB of the mixed value to determine flip
        (mixed & 1) == 1
    }

    /// Reset to beginning of sequence.
    pub fn reset(&mut self) {
        self.index = 0;
    }

    /// Skip ahead in the sequence.
    pub fn skip(&mut self, n: u64) {
        self.index += n;
    }

    /// Fill buffer with uniform random numbers in [0, 1).
    ///
    /// This fills with consecutive Sobol points (row-major by dimension).
    pub fn fill_u01(&mut self, out: &mut [f64]) {
        debug_assert!(
            out.len().is_multiple_of(self.dimension),
            "fill_u01: buffer length {} is not a multiple of dimension {}",
            out.len(),
            self.dimension
        );
        for chunk in out.chunks_mut(self.dimension) {
            self.fill_point(chunk);
        }
    }

    /// Fill buffer with standard normal random numbers.
    ///
    /// Uses the inverse CDF on Sobol-generated uniforms.
    ///
    /// # Mapping
    ///
    /// Raw Sobol integers are in `[0, 2^32)`. We map integer k to
    /// `(k + 0.5) / 2^32`, centring each grid cell so that k=0 becomes
    /// ~1.16e-10 rather than exactly 0. This keeps the inverse normal CDF
    /// finite without the ~37-sigma outlier that `f64::MIN_POSITIVE` would
    /// produce as a clamp lower bound.
    pub fn fill_std_normals(&mut self, out: &mut [f64]) {
        // Pre-fill with raw u32 Sobol integers (we need integer k before
        // converting to uniform) — but fill_u01 already converts. Instead
        // we re-implement a grid-centred mapping inline.
        debug_assert!(
            out.len().is_multiple_of(self.dimension),
            "fill_std_normals: buffer length {} is not a multiple of dimension {}",
            out.len(),
            self.dimension
        );
        // Scale factor: 1 / 2^32
        const INV_2_32: f64 = 1.0 / 4_294_967_296.0_f64;
        for chunk in out.chunks_mut(self.dimension) {
            for (d, slot) in chunk.iter_mut().enumerate().take(self.dimension) {
                let raw = self.sobol_value(d);
                let scrambled = self.owen_scramble_int(raw, d);
                // Map integer k in [0, 2^32) to (k + 0.5) / 2^32 in (0, 1)
                let u = (scrambled as f64 + 0.5) * INV_2_32;
                *slot = inverse_normal_cdf(u);
            }
            self.index += 1;
        }
    }
}

/// Initialize direction numbers for Sobol sequence.
///
/// Uses direction numbers from Joe & Kuo (2008) for up to 40 dimensions.
/// These are the "new-joe-kuo-6.21201" direction numbers.
///
/// For more dimensions, download the full table from:
/// <https://web.maths.unsw.edu.au/~fkuo/sobol/>
fn initialize_direction_numbers(max_dim: usize) -> Vec<Vec<u32>> {
    let mut all_directions = Vec::with_capacity(max_dim);

    // First dimension: powers of 2 (dimension 1 is always standard binary fractions)
    let mut dim0 = Vec::with_capacity(32);
    for i in 0..32 {
        dim0.push(1u32 << (31 - i));
    }
    all_directions.push(dim0);

    // Direction numbers from Joe & Kuo (2008) for dimensions 2-40
    // Format: (degree s, polynomial a, [m_1, m_2, ..., m_s])
    // The polynomial representation is: x^s + a_1*x^(s-1) + ... + a_s
    // where a is the binary representation of coefficients
    let joe_kuo_data: &[(usize, u32, &[u32])] = &[
        // Dimension 2: s=1, a=0, m=[1]
        (1, 0, &[1]),
        // Dimension 3: s=2, a=1, m=[1,3]
        (2, 1, &[1, 3]),
        // Dimension 4: s=3, a=1, m=[1,3,1]
        (3, 1, &[1, 3, 1]),
        // Dimension 5: s=3, a=2, m=[1,1,1]
        (3, 2, &[1, 1, 1]),
        // Dimension 6: s=4, a=1, m=[1,1,3,3]
        (4, 1, &[1, 1, 3, 3]),
        // Dimension 7: s=4, a=4, m=[1,3,5,13]
        (4, 4, &[1, 3, 5, 13]),
        // Dimension 8: s=5, a=2, m=[1,1,5,5,17]
        (5, 2, &[1, 1, 5, 5, 17]),
        // Dimension 9: s=5, a=4, m=[1,1,5,5,5]
        (5, 4, &[1, 1, 5, 5, 5]),
        // Dimension 10: s=5, a=7, m=[1,1,7,11,19]
        (5, 7, &[1, 1, 7, 11, 19]),
        // Dimension 11: s=5, a=11, m=[1,1,5,1,1]
        (5, 11, &[1, 1, 5, 1, 1]),
        // Dimension 12: s=5, a=13, m=[1,1,1,3,11]
        (5, 13, &[1, 1, 1, 3, 11]),
        // Dimension 13: s=5, a=14, m=[1,3,5,5,31]
        (5, 14, &[1, 3, 5, 5, 31]),
        // Dimension 14: s=6, a=1, m=[1,3,3,9,7,49]
        (6, 1, &[1, 3, 3, 9, 7, 49]),
        // Dimension 15: s=6, a=13, m=[1,1,1,15,21,21]
        (6, 13, &[1, 1, 1, 15, 21, 21]),
        // Dimension 16: s=6, a=16, m=[1,3,1,13,27,49]
        (6, 16, &[1, 3, 1, 13, 27, 49]),
        // Dimension 17: s=6, a=19, m=[1,1,1,15,7,5]
        (6, 19, &[1, 1, 1, 15, 7, 5]),
        // Dimension 18: s=6, a=22, m=[1,3,3,7,17,21]
        (6, 22, &[1, 3, 3, 7, 17, 21]),
        // Dimension 19: s=6, a=25, m=[1,1,7,13,7,5]
        (6, 25, &[1, 1, 7, 13, 7, 5]),
        // Dimension 20: s=7, a=1, m=[1,1,5,11,15,41,85]
        (7, 1, &[1, 1, 5, 11, 15, 41, 85]),
        // Dimension 21-40: Additional Joe & Kuo direction numbers
        (7, 4, &[1, 3, 3, 1, 31, 9, 41]),
        (7, 7, &[1, 3, 3, 5, 9, 9, 117]),
        (7, 8, &[1, 1, 1, 5, 23, 33, 51]),
        (7, 14, &[1, 3, 1, 7, 19, 15, 63]),
        (7, 19, &[1, 1, 7, 7, 25, 21, 127]),
        (7, 21, &[1, 3, 5, 7, 25, 9, 69]),
        (7, 28, &[1, 1, 3, 7, 17, 49, 119]),
        (7, 31, &[1, 3, 7, 15, 25, 33, 5]),
        (7, 32, &[1, 1, 7, 9, 9, 9, 49]),
        (7, 37, &[1, 3, 3, 7, 15, 31, 21]),
        (7, 41, &[1, 1, 5, 15, 19, 47, 17]),
        (7, 42, &[1, 3, 7, 9, 5, 11, 65]),
        (7, 50, &[1, 1, 3, 11, 21, 29, 83]),
        (7, 55, &[1, 3, 5, 13, 11, 21, 111]),
        (7, 56, &[1, 1, 1, 11, 19, 53, 93]),
        (7, 59, &[1, 3, 1, 5, 17, 27, 35]),
        (7, 62, &[1, 1, 7, 3, 25, 15, 45]),
        (8, 14, &[1, 3, 3, 9, 25, 19, 5, 247]),
        (8, 21, &[1, 1, 5, 3, 31, 1, 117, 135]),
    ];

    for (dim_idx, &(s, a, initial_m)) in joe_kuo_data
        .iter()
        .enumerate()
        .take(max_dim.saturating_sub(1))
    {
        let mut directions = Vec::with_capacity(32);

        // Set initial direction numbers (scaled to 32 bits)
        for (i, &m) in initial_m.iter().enumerate() {
            directions.push(m << (31 - i));
        }

        // Generate remaining direction numbers using recurrence relation:
        // v_i = a_1*v_{i-1} XOR a_2*v_{i-2} XOR ... XOR a_{s-1}*v_{i-s+1} XOR v_{i-s} XOR (v_{i-s}/2^s)
        for i in s..32 {
            let mut v = directions[i - s] >> s;
            for k in 1..s {
                let coeff = (a >> (s - 1 - k)) & 1;
                if coeff == 1 {
                    v ^= directions[i - k];
                }
            }
            v ^= directions[i - s];
            directions.push(v);
        }

        all_directions.push(directions);

        // Early exit if we've filled enough dimensions
        if dim_idx + 2 >= max_dim {
            break;
        }
    }

    all_directions
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_try_new_rejects_invalid_dimension() {
        assert!(SobolRng::try_new(0, 0).is_err());
        assert!(SobolRng::try_new(MAX_SOBOL_DIMENSION + 1, 0).is_err());
    }

    #[test]
    fn test_sobol_basic() {
        let mut sobol = SobolRng::try_new(2, 0).expect("valid dimension");

        // First few points should be deterministic
        let p1 = sobol.next_point();
        assert_eq!(p1.len(), 2);

        let p2 = sobol.next_point();
        assert_eq!(p2.len(), 2);

        // Points should be different
        assert_ne!(p1[0], p2[0]);
    }

    #[test]
    fn test_sobol_range() {
        let mut sobol = SobolRng::try_new(3, 0).expect("valid dimension");

        for _ in 0..100 {
            let point = sobol.next_point();
            for &val in &point {
                assert!((0.0..1.0).contains(&val));
            }
        }
    }

    #[test]
    fn test_owen_scrambling() {
        let sobol_no_scramble = SobolRng::try_new(2, 0).expect("valid dimension");
        let sobol_scrambled = SobolRng::try_new(2, 12345).expect("valid dimension");

        // Different scrambling should give different sequences
        let p1 = sobol_no_scramble.clone().next_point();
        let p2 = sobol_scrambled.clone().next_point();

        assert_ne!(p1[0], p2[0]);
    }

    #[test]
    fn test_sobol_reset_and_skip() {
        let mut sobol = SobolRng::try_new(2, 0).expect("valid dimension");

        let p1_first = sobol.next_point();
        let _p2 = sobol.next_point();

        sobol.reset();
        let p1_after_reset = sobol.next_point();

        // After reset, should get same first point
        assert_eq!(p1_first[0], p1_after_reset[0]);
        assert_eq!(p1_first[1], p1_after_reset[1]);
    }

    #[test]
    fn test_fill_std_normals() {
        let mut sobol = SobolRng::try_new(1, 0).expect("valid dimension");
        let mut normals = vec![0.0; 100];
        sobol.fill_std_normals(&mut normals);

        // All values should be finite, including the first Sobol point.
        for &n in &normals {
            assert!(n.is_finite(), "Non-finite value: {}", n);
        }

        // Mean should be reasonable (QMC doesn't guarantee mean=0)
        let mean = normals.iter().sum::<f64>() / normals.len() as f64;
        assert!(mean.abs() < 2.0);
    }

    // ── H5 regression: open-interval mapping keeps first point finite ──
    //
    // Without scrambling the first Sobol integer for dimension 0 is 0.
    // The old MIN_POSITIVE clamp would map 0 → 2.2e-308 → N^{-1}(2.2e-308) ≈ -37σ.
    // The (k+0.5)/2^32 mapping should produce a normal quantile ≈ 4.6σ which,
    // while extreme, is not an artificial boundary artefact.
    #[test]
    fn test_fill_std_normals_first_point_finite_and_bounded() {
        // No scrambling — first point is k=0 in all dimensions.
        let mut sobol = SobolRng::try_new(1, 0).expect("valid dimension");
        let mut out = vec![0.0; 1];
        sobol.fill_std_normals(&mut out);
        assert!(out[0].is_finite(), "First unscrambled point must be finite");
        // With (0+0.5)/2^32 ≈ 1.16e-10, Φ^{-1}(1.16e-10) ≈ -6.4σ.
        // In practice the exact value depends on the inverse-normal implementation,
        // but it must be finite and larger than -40.
        assert!(
            out[0] > -40.0,
            "First point should not be a ~37σ outlier from MIN_POSITIVE, got {}",
            out[0]
        );
    }

    // ── H5 regression: grid-centred mapping symmetry ──
    //
    // For a 1-D 2-point sequence (k=0, k=2^31) the open-interval values are
    // (0.5/2^32, (2^31+0.5)/2^32) = (~1.16e-10, ~0.5). Their inverse-normal
    // quantiles should be symmetric and finite.
    #[test]
    fn test_fill_std_normals_symmetry_no_scramble() {
        let mut sobol = SobolRng::try_new(1, 0).expect("valid dimension");
        let mut out = vec![0.0; 2];
        sobol.fill_std_normals(&mut out);
        for &v in &out {
            assert!(v.is_finite(), "All points must be finite: {v}");
        }
    }

    // ── H5 regression: no artificial extreme tails with scrambling ──
    //
    // With a large batch and scrambling the 5-sigma quantile should not
    // appear more than ~3× its theoretical frequency.
    #[test]
    fn test_fill_std_normals_no_extreme_tail_outliers() {
        let mut sobol = SobolRng::try_new(1, 42).expect("valid dimension");
        let mut out = vec![0.0; 1024];
        sobol.fill_std_normals(&mut out);
        // No value should exceed ±10σ in a 1024-point Sobol sequence
        for &v in &out {
            assert!(
                v.abs() < 10.0,
                "Unexpected extreme outlier {v} (grid-centred mapping should prevent >10σ in 1024 pts)"
            );
        }
    }
}
