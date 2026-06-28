//! Monte Carlo execution engine and runtime configuration.
//!
//! This module contains the generic simulation loop used throughout the crate.
//! [`McEngine`] combines a [`crate::traits::RandomStream`], a
//! [`crate::traits::StochasticProcess`], a compatible
//! [`crate::traits::Discretization`], and a [`crate::traits::Payoff`] into
//! discounted Monte Carlo estimates.
//!
//! The engine provides:
//!
//! - reusable serial and parallel path loops
//! - deterministic path-to-stream mapping for reproducibility
//! - online mean / variance estimation via Welford's algorithm
//! - optional early stopping on a target 95% confidence-interval half-width
//! - optional path capture for visualization and diagnostics
//! - built-in antithetic pairing in the generic engine loop
//!
//! # Important Runtime Constraints
//!
//! Several configuration combinations are intentionally rejected at runtime:
//!
//! - parallel execution requires an RNG with deterministic stream splitting
//! - auto-stopping via `target_ci_half_width` is currently serial-only
//! - path capture cannot be combined with antithetic pairing
//! - sampled path capture uses deterministic Bernoulli sampling, so the number
//!   of captured paths is an expected count, not an exact count
//!
//! # Conventions
//!
//! - `discount_factor` is the present-value multiplier for the payoff horizon
//!   and must be finite and non-negative.
//! - Payoffs produce undiscounted [`finstack_quant_core::money::Money`] amounts in the
//!   requested currency. The engine applies `discount_factor` after each path is
//!   simulated.
//! - Confidence intervals are reported on discounted path values.
//!
//! # References
//!
//! - Online variance accumulation follows
//!   [`docs/REFERENCES.md#welford-1962`](docs/REFERENCES.md#welford-1962).
//! - Discounting conventions should be consistent with
//!   [`docs/REFERENCES.md#hull-options-futures`](docs/REFERENCES.md#hull-options-futures).

mod config;
mod path_capture;
mod pricing;
mod simulation;

#[cfg(test)]
mod tests;

pub use config::{McEngineBuilder, McEngineConfig, MAX_CAPTURED_PATHS, MAX_NUM_PATHS};
pub use path_capture::{PathCaptureConfig, PathCaptureMode};
pub use pricing::McEngine;

pub(crate) use pricing::{build_correlation_factor, validate_discounted_payoff};
pub(crate) use simulation::{run_path_loop, NoiseHook};
