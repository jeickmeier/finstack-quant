//! Variance-reduction utilities for Monte Carlo pricing.
//!
//! Production paths use the always-available estimators in this module:
//! [`control_variate`], plus antithetic pairing implemented inline in
//! [`crate::engine::McEngine`] and configured via
//! [`crate::engine::McEngineConfig::antithetic`].
//!
//! Experimental estimators (`moment_matching` and `importance_sampling`) are
//! gated behind the `vr-experimental` Cargo feature. They are not yet wired
//! into the engine or pricers — enable the feature only when developing or
//! evaluating the underlying estimators.
//!
//! Each leaf module documents the estimator assumptions, the quantity being
//! reweighted or paired, and the units of the returned diagnostics.

pub mod control_variate;

#[cfg(feature = "vr-experimental")]
pub mod moment_matching;

#[cfg(feature = "vr-experimental")]
pub mod importance_sampling;

pub use control_variate::{
    apply_control_variate, black_scholes_call, black_scholes_put, covariance,
};
