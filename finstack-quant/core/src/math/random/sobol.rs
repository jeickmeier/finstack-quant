//! Finite 32-bit Sobol points in natural binary index order using Joe–Kuo directions.
//!
//! An optional seeded prefix hash permutes each coordinate's binary digits.
//! This permutation preserves dyadic box structure. Its seed law and replicate
//! uncertainty are considerations for the estimator using the sequence.
//!
//! # References
//!
//! - Joe, S., & Kuo, F. Y. (2008). "Constructing Sobol Sequences with Better
//!   Two-Dimensional Projections." SIAM J. Sci. Comput., 30(5), 2635-2654. `docs/REFERENCES.md#joe-kuo-2008-sobol`
//!
//! - Sobol, I.M. (1967). "Distribution of points in a cube and approximate
//!   evaluation of integrals." USSR Comp. Math. and Math. Physics, 7(4), 86-112. `docs/REFERENCES.md#sobol-1967`
//!
//! - Owen, A. B. (1995). "Randomly Permuted (t,m,s)-Nets and (t,s)-Sequences."
//!   Monte Carlo and Quasi-Monte Carlo Methods in Scientific Computing, 299-317. `docs/REFERENCES.md#owen-1995-scrambling`
//!
//! - Owen, A. B. (1997). "Scrambled Net Variance for Integrals of Smooth Functions."
//!   Annals of Statistics, 25(4), 1541-1562. `docs/REFERENCES.md#owen-1995-scrambling`

use crate::math::special_functions::standard_normal_inv_cdf;

/// Maximum supported dimension for this Sobol implementation.
///
/// Higher dimensions require additional direction numbers from Joe & Kuo's tables.
/// See <https://web.maths.unsw.edu.au/~fkuo/sobol/> for tables up to 21201 dimensions.
pub const MAX_SOBOL_DIMENSION: usize = 40;

/// Distinct points in one Sobol period given 32-bit direction numbers.
pub const SOBOL_PERIOD: u64 = 1 << 32;

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

/// Low-discrepancy vector sequence with up to 40 dimensions and 2^32 points per run.
/// The cursor selects the next point. Cloning copies that position and all digit permutations.
#[derive(Clone, Debug)]
pub struct SobolRng {
    index: u64,
    dimension: usize,
    bit_seeds: Vec<[u32; 32]>,
    direction_numbers: Vec<[u32; 32]>,
    state: [u32; MAX_SOBOL_DIMENSION],
}

impl SobolRng {
    /// Construct a generator at point zero.
    ///
    /// # Arguments
    ///
    /// * `dimension` - Number of coordinates per point, from 1 through 40.
    /// * `scramble_seed` - Optional seed for the prefix hash digit permutation.
    ///   `None` preserves the raw Sobol directions. Every `Some` value selects a permutation.
    pub fn new(dimension: usize, scramble_seed: Option<u64>) -> crate::Result<Self> {
        if !(1..=MAX_SOBOL_DIMENSION).contains(&dimension) {
            return Err(crate::Error::Validation(format!(
                "Sobol dimension must be 1-{MAX_SOBOL_DIMENSION}, got {dimension}"
            )));
        }
        let direction_numbers = initialize_direction_numbers(dimension)?;
        let mut bit_seeds = Vec::new();
        if let Some(seed) = scramble_seed {
            bit_seeds
                .try_reserve_exact(dimension)
                .map_err(|e| crate::Error::Internal(format!("Sobol digit seed allocation: {e}")))?;
            for d in 0..dimension {
                let base = hash_combine(seed, d as u64);
                bit_seeds.push(std::array::from_fn(|bit| {
                    hash_combine(u64::from(base), bit as u64)
                }));
            }
        }
        Ok(Self {
            index: 0,
            dimension,
            bit_seeds,
            direction_numbers,
            state: [0; MAX_SOBOL_DIMENSION],
        })
    }

    /// Raw Sobol integer for dimension `d` at an arbitrary sequence index.
    ///
    /// The reference (from-scratch) definition: XOR of the direction numbers
    /// selected by the set bits of `index`. Used by [`Self::skip`] and by the
    /// equivalence tests that pin the incremental cache to this definition.
    fn value_at(&self, index: u64, d: usize) -> u32 {
        let mut value = 0u32;
        let mut index = index;
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

    /// Apply the seeded prefix hash from the most significant digit down.
    fn permute_digits(&self, value: u32, d: usize) -> u32 {
        let Some(matrix) = self.bit_seeds.get(d) else {
            return value;
        };
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
        for slot in &mut self.state {
            *slot = 0;
        }
    }

    /// Skip whole points relative to the current cursor.
    ///
    /// # Arguments
    ///
    /// * `n` - Number of points to consume, at most the remaining period.
    ///   Landing exactly at exhaustion is allowed.
    pub fn skip(&mut self, n: u64) -> crate::Result<()> {
        let target = self.target_index(n)?;
        if n == 0 {
            return Ok(());
        }
        if target == SOBOL_PERIOD {
            self.state.fill(0);
        } else {
            for d in 0..self.dimension {
                self.state[d] = self.value_at(target, d);
            }
        }
        self.index = target;
        Ok(())
    }

    fn target_index(&self, count: u64) -> crate::Result<u64> {
        if count > SOBOL_PERIOD - self.index {
            return Err(crate::Error::Validation(format!(
                "Sobol cursor {} cannot consume {count} points in a period of {SOBOL_PERIOD}",
                self.index
            )));
        }
        Ok(self.index + count)
    }

    /// Emit complete consecutive points on the interior grid `(k + 0.5)/2^32`.
    ///
    /// # Arguments
    ///
    /// * `out` - Row-major buffer, one coordinate per dimension per point.
    ///   Its length must be a multiple of the dimension within the remaining period.
    pub fn fill_u01(&mut self, out: &mut [f64]) -> crate::Result<()> {
        self.fill(out, Ok)
    }

    /// Emit inverse-normal quantiles of complete consecutive interior-grid points.
    ///
    /// # Arguments
    ///
    /// * `out` - Row-major buffer of standard normal coordinates. Its length
    ///   must be a multiple of the dimension within the remaining period.
    pub fn fill_std_normals(&mut self, out: &mut [f64]) -> crate::Result<()> {
        self.fill(out, standard_normal_inv_cdf)
    }

    fn fill(
        &mut self,
        out: &mut [f64],
        transform: impl Fn(f64) -> crate::Result<f64>,
    ) -> crate::Result<()> {
        if !out.len().is_multiple_of(self.dimension) {
            return Err(crate::Error::Validation(format!(
                "Sobol output length {} is not a multiple of dimension {}",
                out.len(),
                self.dimension
            )));
        }
        let count = (out.len() / self.dimension) as u64;
        let target = self.target_index(count)?;
        let mut state = self.state;
        for (index, row) in (self.index..target).zip(out.chunks_exact_mut(self.dimension)) {
            for (d, slot) in row.iter_mut().enumerate() {
                let k = self.permute_digits(state[d], d);
                *slot = transform((f64::from(k) + 0.5) / SOBOL_PERIOD as f64)?;
                if !slot.is_finite() {
                    return Err(crate::Error::Internal("non-finite Sobol transform".into()));
                }
            }
            let next = index + 1;
            if next == SOBOL_PERIOD {
                state.fill(0);
            } else {
                let carry = next.trailing_zeros() as usize;
                for (slot, directions) in state.iter_mut().zip(&self.direction_numbers) {
                    for direction in &directions[..=carry] {
                        *slot ^= direction;
                    }
                }
            }
        }
        self.index = target;
        self.state = state;
        Ok(())
    }
}

/// Initialize direction numbers for Sobol sequence.
///
/// Uses direction numbers from Joe & Kuo (2008) for up to 40 dimensions.
/// These are the "new-joe-kuo-6.21201" direction numbers.
///
/// For more dimensions, download the full table from:
/// <https://web.maths.unsw.edu.au/~fkuo/sobol/>
fn initialize_direction_numbers(max_dim: usize) -> crate::Result<Vec<[u32; 32]>> {
    let mut all_directions = Vec::new();
    all_directions
        .try_reserve_exact(max_dim)
        .map_err(|e| crate::Error::Internal(format!("Sobol direction table allocation: {e}")))?;
    all_directions.push(std::array::from_fn(|i| 1u32 << (31 - i)));

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
        // Dimensions 18-40: values below are taken verbatim from the published
        // Joe & Kuo "new-joe-kuo-6.21201" table
        // (<https://web.maths.unsw.edu.au/~fkuo/sobol/new-joe-kuo-6.21201>,
        // retrieved 2026-06-11; golden copy at
        // `finstack-quant/core/tests/data/sobol_joe_kuo_d2_40.txt`). A previous
        // revision embedded incorrect m_i values for d=18-39 and was missing
        // d=40 entirely (out-of-bounds panic for 40-dimensional sequences);
        // validated by `tests/sobol_golden.rs`.
        // Dimension 18: s=6, a=22, m=[1,3,1,15,13,25]
        (6, 22, &[1, 3, 1, 15, 13, 25]),
        // Dimension 19: s=6, a=25, m=[1,1,5,5,19,61]
        (6, 25, &[1, 1, 5, 5, 19, 61]),
        // Dimension 20: s=7, a=1, m=[1,3,7,11,23,15,103]
        (7, 1, &[1, 3, 7, 11, 23, 15, 103]),
        // Dimension 21
        (7, 4, &[1, 3, 7, 13, 13, 15, 69]),
        // Dimension 22
        (7, 7, &[1, 1, 3, 13, 7, 35, 63]),
        // Dimension 23
        (7, 8, &[1, 3, 5, 9, 1, 25, 53]),
        // Dimension 24
        (7, 14, &[1, 3, 1, 13, 9, 35, 107]),
        // Dimension 25
        (7, 19, &[1, 3, 1, 5, 27, 61, 31]),
        // Dimension 26
        (7, 21, &[1, 1, 5, 11, 19, 41, 61]),
        // Dimension 27
        (7, 28, &[1, 3, 5, 3, 3, 13, 69]),
        // Dimension 28
        (7, 31, &[1, 1, 7, 13, 1, 19, 1]),
        // Dimension 29
        (7, 32, &[1, 3, 7, 5, 13, 19, 59]),
        // Dimension 30
        (7, 37, &[1, 1, 3, 9, 25, 29, 41]),
        // Dimension 31
        (7, 41, &[1, 3, 5, 13, 23, 1, 55]),
        // Dimension 32
        (7, 42, &[1, 3, 7, 3, 13, 59, 17]),
        // Dimension 33
        (7, 50, &[1, 3, 1, 3, 5, 53, 69]),
        // Dimension 34
        (7, 55, &[1, 1, 5, 5, 23, 33, 13]),
        // Dimension 35
        (7, 56, &[1, 1, 7, 7, 1, 61, 123]),
        // Dimension 36
        (7, 59, &[1, 1, 7, 9, 13, 61, 49]),
        // Dimension 37
        (7, 62, &[1, 3, 3, 5, 3, 55, 33]),
        // Dimension 38
        (8, 14, &[1, 3, 1, 15, 31, 13, 49, 245]),
        // Dimension 39
        (8, 21, &[1, 3, 5, 15, 31, 59, 63, 97]),
        // Dimension 40
        (8, 22, &[1, 3, 1, 11, 11, 11, 77, 249]),
    ];

    for &(s, a, initial_m) in joe_kuo_data.iter().take(max_dim - 1) {
        let mut directions = [0; 32];

        // Set initial direction numbers (scaled to 32 bits)
        for (i, &m) in initial_m.iter().enumerate() {
            directions[i] = m << (31 - i);
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
            directions[i] = v;
        }

        all_directions.push(directions);
    }

    Ok(all_directions)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Next Sobol point as an owned vector.
    fn point(sobol: &mut SobolRng) -> Vec<f64> {
        let mut out = vec![0.0; sobol.dimension];
        sobol.fill_u01(&mut out).unwrap();
        out
    }

    #[test]
    fn constructor_rejects_invalid_dimension() {
        assert!(SobolRng::new(0, None).is_err());
        assert!(SobolRng::new(MAX_SOBOL_DIMENSION + 1, None).is_err());
    }

    #[test]
    fn test_sobol_basic() {
        let mut sobol = SobolRng::new(2, None).expect("valid dimension");

        // First few points should be deterministic
        let p1 = point(&mut sobol);
        assert_eq!(p1.len(), 2);

        let p2 = point(&mut sobol);
        assert_eq!(p2.len(), 2);

        // Points should be different
        assert_ne!(p1[0], p2[0]);
    }

    #[test]
    fn test_sobol_range() {
        let mut sobol = SobolRng::new(3, None).expect("valid dimension");

        for _ in 0..100 {
            let point = point(&mut sobol);
            for &val in &point {
                assert!((0.0..1.0).contains(&val));
            }
        }
    }

    #[test]
    fn test_fill_u01_matches_consecutive_points() {
        let mut sobol_fill = SobolRng::new(2, Some(123)).expect("valid dimension");
        let mut out = vec![0.0; 4];
        sobol_fill.fill_u01(&mut out).unwrap();

        let mut sobol_points = SobolRng::new(2, Some(123)).expect("valid dimension");
        let p1 = point(&mut sobol_points);
        let p2 = point(&mut sobol_points);

        assert_eq!(out, vec![p1[0], p1[1], p2[0], p2[1]]);
        assert!(out.iter().all(|&u| (0.0..1.0).contains(&u)));
    }

    #[test]
    fn zero_seed_matches_prefix_hash_reference_words() {
        // Integer-only reference applies the two fixed hashes to the running
        // output prefix, then forms the exact midpoint of each scrambled word.
        let mut sobol = SobolRng::new(2, Some(0)).unwrap();
        let mut values = [0.0; 4];
        sobol.fill_u01(&mut values).unwrap();
        let expected = [
            0x992bca72_u32,
            0x7652bbf5_u32,
            0x32c6c857_u32,
            0xa114a27a_u32,
        ];
        for (value, word) in values.into_iter().zip(expected) {
            assert_eq!(value, (f64::from(word) + 0.5) / SOBOL_PERIOD as f64);
        }
    }

    #[test]
    fn prefix_hash_changes_points() {
        let mut sobol_no_scramble = SobolRng::new(2, None).expect("valid dimension");
        let mut sobol_scrambled = SobolRng::new(2, Some(12345)).expect("valid dimension");

        // Different scrambling should give different sequences
        let p1 = point(&mut sobol_no_scramble);
        let p2 = point(&mut sobol_scrambled);

        assert_ne!(p1[0], p2[0]);
    }

    #[test]
    fn test_scrambled_sobol_is_deterministic_for_same_seed() {
        let mut sobol1 = SobolRng::new(3, Some(12345)).expect("valid dimension");
        let mut sobol2 = SobolRng::new(3, Some(12345)).expect("valid dimension");
        let mut different_seed = SobolRng::new(3, Some(54321)).expect("valid dimension");

        for _ in 0..32 {
            assert_eq!(point(&mut sobol1), point(&mut sobol2));
        }

        assert_ne!(point(&mut sobol1), point(&mut different_seed));
    }

    #[test]
    fn test_sobol_reset_and_skip() {
        let mut sobol = SobolRng::new(2, None).expect("valid dimension");

        let p1_first = point(&mut sobol);
        let _p2 = point(&mut sobol);

        sobol.reset();
        let p1_after_reset = point(&mut sobol);

        // After reset, should get same first point
        assert_eq!(p1_first[0], p1_after_reset[0]);
        assert_eq!(p1_first[1], p1_after_reset[1]);
    }

    #[test]
    fn test_fill_std_normals() {
        let mut sobol = SobolRng::new(1, None).expect("valid dimension");
        let mut normals = vec![0.0; 100];
        sobol.fill_std_normals(&mut normals).unwrap();

        // All values should be finite, including the first Sobol point.
        for &n in &normals {
            assert!(n.is_finite(), "Non-finite value: {}", n);
        }

        // Mean should be reasonable (QMC doesn't guarantee mean=0)
        let mean = normals.iter().sum::<f64>() / normals.len() as f64;
        assert!(mean.abs() < 2.0);
    }

    #[test]
    fn first_normal_is_the_quantile_of_the_first_midpoint() {
        let mut sobol = SobolRng::new(1, None).unwrap();
        let mut out = [0.0];
        sobol.fill_std_normals(&mut out).unwrap();
        assert!((out[0] + 6.337957754553789).abs() < 1e-7);
    }

    #[test]
    fn transform_failure_leaves_cursor_and_cache_unmodified() {
        let mut sobol = SobolRng::new(2, Some(9)).unwrap();
        let previous = sobol.clone();
        let mut out = [0.0; 4];
        let calls = std::cell::Cell::new(0);
        let result = sobol.fill(&mut out, |value| {
            calls.set(calls.get() + 1);
            if calls.get() == 3 {
                return Err(crate::Error::Internal("test transform failure".into()));
            }
            Ok(value)
        });
        assert!(result.is_err());
        assert_eq!(sobol.index, previous.index);
        assert_eq!(sobol.state, previous.state);
    }

    #[test]
    fn incremental_state_matches_from_scratch_definition() {
        // The cached per-dimension state must equal the reference definition
        // value_at(index, d) at every step — including across skip and reset.
        let mut rng = SobolRng::new(8, None).expect("valid dimension");

        let check = |rng: &SobolRng| {
            for d in 0..8 {
                assert_eq!(rng.state[d], rng.value_at(rng.index, d), "dim {d}");
            }
        };

        for _ in 0..300 {
            check(&rng);
            let mut buf = [0.0f64; 8];
            rng.fill_u01(&mut buf).unwrap();
        }
        // Cross a power-of-two boundary where many bits flip at once.
        rng.skip(4812).unwrap(); // -> index 5112
        check(&rng);
        rng.skip(100_000).unwrap();
        check(&rng);
        rng.reset();
        check(&rng);
        for _ in 0..257 {
            let mut buf = [0.0f64; 8];
            rng.fill_u01(&mut buf).unwrap();
            check(&rng);
        }
    }

    #[test]
    fn partial_points_and_exhausted_requests_preserve_state_and_output() {
        let mut rng = SobolRng::new(2, Some(0)).unwrap();
        let mut raw = SobolRng::new(2, None).unwrap();
        assert_ne!(point(&mut rng), point(&mut raw));
        rng.reset();
        rng.skip(SOBOL_PERIOD - 1).unwrap();
        let mut too_many = [123.0; 4];
        assert!(rng.fill_std_normals(&mut too_many).is_err());
        assert!(rng.fill_u01(&mut too_many[..3]).is_err());
        assert_eq!(too_many, [123.0; 4]);
        assert!(rng.skip(u64::MAX).is_err());
        assert_eq!(rng.index, SOBOL_PERIOD - 1);
        rng.fill_u01(&mut too_many[..2]).unwrap();
        assert_eq!(rng.index, SOBOL_PERIOD);
        assert!(rng.state.iter().all(|&x| x == 0));
        rng.fill_u01(&mut []).unwrap();
        rng.skip(0).unwrap();
        assert!(rng.fill_u01(&mut too_many[..2]).is_err());
        rng.reset();
        rng.skip(SOBOL_PERIOD).unwrap();
        assert!(rng.skip(1).is_err());
    }
}
