//! Variance-reduction utilities for Monte Carlo pricing.
//!
//! Production paths use the always-available estimators in this module:
//! [`control_variate`], plus antithetic pairing implemented inline in
//! [`crate::engine::McEngine`] and configured via
//! [`crate::engine::McEngineConfig::antithetic`].
//!
//! Each leaf module documents the estimator assumptions, the quantity being
//! reweighted or paired, and the units of the returned diagnostics.

pub mod control_variate;

pub use control_variate::{
    apply_control_variate, black_scholes_call, black_scholes_put, covariance,
};
