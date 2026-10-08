//! Variance-reduction utilities for Monte Carlo pricing.
//!
//! Production paths use the always-available estimators in this module:
//! [`control_variate`], plus antithetic pairing implemented inline in
//! [`crate::monte_carlo::engine::McEngine`] and configured via
//! [`crate::monte_carlo::engine::McEngineConfig::antithetic`].
//!
//! Each leaf module documents the estimator assumptions, the quantity being
//! reweighted or paired, and the units of the returned diagnostics.

pub mod control_variate;
