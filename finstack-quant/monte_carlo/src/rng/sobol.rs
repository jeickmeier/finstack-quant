//! Sobol RNG re-exported from `finstack_quant_core` with `RandomStream` integration.
//!
//! The underlying Joe-Kuo direction table supports dimensions up to
//! [`MAX_SOBOL_DIMENSION`]. Monte Carlo callers must count every generated
//! normal as one Sobol dimension, usually `num_steps * num_factors` before any
//! Brownian-bridge reordering. Use [`SobolRng::try_new`] so excessive
//! dimensions fail explicitly.

use crate::traits::RandomStream;

pub use finstack_quant_core::math::random::sobol::{SobolRng, MAX_SOBOL_DIMENSION};

impl RandomStream for SobolRng {
    /// Sobol sequences do not admit independent sub-streams. Always returns
    /// `None`; callers must check [`RandomStream::supports_splitting`] first
    /// and fall back to a pseudorandom generator (e.g.
    /// [`crate::rng::philox::PhiloxRng`]) or serial execution.
    fn split(&self, _stream_id: u64) -> Option<Self> {
        None
    }

    fn fill_u01(&mut self, out: &mut [f64]) {
        SobolRng::fill_u01(self, out);
    }

    fn fill_std_normals(&mut self, out: &mut [f64]) {
        SobolRng::fill_std_normals(self, out);
    }

    /// Sobol sequences cannot be split into independent streams.
    /// Always returns false.
    fn supports_splitting(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_zero_returns_none() {
        let rng = SobolRng::try_new(4, 0).expect("valid Sobol dimension");
        assert!(rng.split(0).is_none());
    }

    #[test]
    fn split_nonzero_returns_none() {
        let rng = SobolRng::try_new(4, 0).expect("valid Sobol dimension");
        assert!(rng.split(1).is_none());
    }
}
