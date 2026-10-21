//! Sobol RNG re-exported from `finstack_quant_core` with `RandomStream` integration.
//!
//! The underlying Joe-Kuo direction table supports dimensions up to
//! [`MAX_SOBOL_DIMENSION`](finstack_quant_core::math::random::MAX_SOBOL_DIMENSION). Monte Carlo callers must count every generated
//! normal as one Sobol dimension, usually `num_steps * num_factors` before any
//! Brownian-bridge reordering. Use [`SobolRng::try_new`] so excessive
//! dimensions fail explicitly.
//!
//! The generic [`crate::monte_carlo::engine::McEngine`] rejects this stream (it reports
//! [`RandomStream::is_quasi_random`] `= true`): the engine consumes draws
//! step-by-step and reports an i.i.d. standard error, both of which are
//! invalid for a low-discrepancy sequence. Quasi-Monte Carlo pricing goes
//! through [`crate::monte_carlo::pricer::path_dependent::PathDependentPricer`], which
//! draws one Sobol point per path and estimates the error across
//! independently Owen-scrambled replicates.

use crate::monte_carlo::traits::RandomStream;

use finstack_quant_core::math::random::SobolRng;

impl RandomStream for SobolRng {
    /// Sobol sequences do not admit independent sub-streams. Always returns
    /// `None`; callers must check [`RandomStream::supports_splitting`] first
    /// and fall back to a pseudorandom generator (e.g.
    /// [`crate::monte_carlo::rng::philox::PhiloxRng`]) or serial execution.
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

    /// Sobol is a low-discrepancy sequence: successive points are dependent
    /// by construction. Always returns true, which makes the generic
    /// [`crate::monte_carlo::engine::McEngine`] reject this stream in favor of the
    /// replicate-based quasi-Monte Carlo path in
    /// [`crate::monte_carlo::pricer::path_dependent::PathDependentPricer`].
    fn is_quasi_random(&self) -> bool {
        true
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
