//! Core [`RandomNumberGenerator`] trait, [`Pcg64Rng`], and [`box_muller`] sampling.

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
    pub(super) cached_normal: Option<f64>,
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

impl Pcg64Rng {
    /// Draw an index uniformly from `0..len` without modulo or rounding bias.
    ///
    /// Takes 64-bit words from the generator and rejects those in the
    /// incomplete final block of `len` values, so every index is exactly
    /// equally likely and the result is the same on 32-bit and 64-bit
    /// targets.
    ///
    /// # Arguments
    ///
    /// * `len` - Number of equally likely outcomes; must be at least 1. The
    ///   result is in `0..len`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_core::math::random::Pcg64Rng;
    ///
    /// let mut rng = Pcg64Rng::new(42, 0);
    /// assert!(rng.index(3) < 3);
    /// ```
    ///
    /// # Panics
    ///
    /// Panics when `len` is zero.
    pub fn index(&mut self, len: usize) -> usize {
        assert!(len > 0, "cannot draw an index from an empty range");
        let len = len as u64;
        // The number of words in the incomplete final block: 2^64 mod len.
        let excess = (u64::MAX % len + 1) % len;
        loop {
            let word = self.inner.next_u64();
            if word <= u64::MAX - excess {
                return (word % len) as usize;
            }
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
