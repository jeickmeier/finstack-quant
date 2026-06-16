//! Market data validation and no-arbitrage constraints.
//!
//! This module provides the infrastructure for performing runtime validation
//! of market data, calibration inputs, and calibrated results. It ensures
//! that results conform to financial reality (e.g., non-negative hazard rates,
//! positive discount factors, no-arbitrage volatility surfaces).
//!
//! # Submodules
//! - [`config`]: Configuration for validation thresholds and strictness modes.
//! - [`curves`]: Runtime validators for term structures (Yield, Hazard, Inflation).
//! - [`surfaces`]: Runtime validators for volatility and correlation surfaces.
//! - [`preflight`]: Pre-execution validation for calibration steps.

mod config;
pub(crate) mod curves;
mod points;
mod preflight;
pub(crate) mod surfaces;

pub(crate) use config::default_rate_bounds_policy_for_serde;
pub use config::{RateBounds, RateBoundsPolicy, ValidationConfig, ValidationMode};
pub(crate) use preflight::preflight_step;
