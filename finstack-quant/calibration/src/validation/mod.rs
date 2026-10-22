//! Market data validation and no-arbitrage constraints.
//!
//! This module provides the infrastructure for performing runtime validation
//! of market data, calibration inputs, and calibrated results. It ensures
//! that results conform to financial reality (e.g., non-negative hazard rates,
//! positive discount factors, no-arbitrage volatility surfaces).
//!
//! Configuration, curve and surface validators, and calibration-step preflight
//! checks are implemented in private submodules and exposed here through the
//! public validation types and functions below.

mod config;
pub(crate) mod curves;
mod points;
mod preflight;
pub(crate) mod surfaces;

pub use config::{RateBounds, RateBoundsPolicy, ValidationConfig, ValidationMode};
pub use curves::CurveValidator;
pub(crate) use preflight::preflight_step;
pub use surfaces::{
    validate_butterfly_call_convexity, validate_butterfly_spread, validate_calendar_spread,
    validate_calendar_spread_with_forwards, validate_surface, validate_surface_with_forwards,
    validate_vol_bounds,
};
