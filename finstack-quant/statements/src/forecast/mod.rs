//! Forecast methods for time-series projection.
//!
//! This module provides various forecast methods for projecting values into
//! future periods, including:
//! - **Deterministic**: ForwardFill, GrowthPct, CurvePct, Override,
//!   FadeToTarget (linear / geometric / exponential glide to a target level)
//! - **Statistical**: Normal, LogNormal, MeanReverting (AR(1)), Bootstrap
//!   (historical resampling) — all with deterministic seeding
//! - **TimeSeries**: trend detection (linear, Holt / damped Holt via `phi`,
//!   moving average) and Seasonal patterns
//!
//! A node has one forecast suffix per run: the model's forecast periods, one
//! step per period whether or not an explicit value is published there.
//! Methods that step from a base use the `start` level of the specification,
//! or else the visible value in the period just before the suffix; without
//! either the forecast is unavailable.
//!
//! # Bounds
//!
//! Every method accepts optional `min` / `max` parameters that clamp the
//! generated values to a band: ratios that must stay
//! in a range, levels that cannot go negative, or trend extrapolations that
//! must not explode. The clamp applies to published values only and each
//! clipped cell is recorded as a warning — every recurrence evolves
//! unclamped, and Monte Carlo evaluation records correlation Z-scores before
//! clamping.
//!
//! # Random Number Generation
//!
//! Statistical forecast methods (Normal, LogNormal, MeanReverting, Bootstrap)
//! require a `seed` parameter
//! for deterministic random number generation. This ensures reproducibility:
//! - Same seed → identical forecast values across runs
//! - Different seeds → different (but still deterministic) values
//!
//! Each stochastic node draws from its own versioned stream, keyed by the
//! configured seed, the node identifier and, in Monte Carlo, the path (see
//! `forecast::stream`), so independent nodes never share shock draws.
//! Optional `correlation_with` / `correlation` parameters pair Normal,
//! LogNormal, and MeanReverting nodes for correlated
//! shocks (see `forecast::statistical::correlation`); the peer's run is
//! generated on demand when the dependent node is simulated.
//!
//! The RNG uses the Box-Muller transform for normal distribution sampling,
//! with guards against edge cases (e.g., ln(0)).
//!
//! # Parameter Validation
//!
//! - **std_dev**: Must be non-negative. Zero produces a degenerate distribution.
//! - **rate** (GrowthPct): Rates > 100% per period produce warnings.
//! - **seed**: Required for statistical methods (ensures reproducibility).
//!
//! # Overflow Protection
//!
//! Compound growth methods (GrowthPct, CurvePct) detect and error on overflow
//! conditions to prevent silent numerical failures.
//!
//! # Warnings
//!
//! The following conditions produce log warnings (but not errors):
//! - Growth rates exceeding 100% per period
//! - std_dev = 0.0 in LogNormal (degenerate distribution)
//!
//! For forecast analysis tools (backtesting, covenant breach detection), see
//! the `finstack-quant-statements-analytics` crate.

mod deterministic;
mod dispatch;
mod fade;
mod override_method;
pub(crate) mod statistical;
pub(crate) mod stream;
mod timeseries;

#[cfg(test)]
mod tests;

#[cfg(test)]
pub(crate) use dispatch::apply_forecast_for_node;
pub(crate) use dispatch::{apply_bounds, apply_forecast, apply_forecast_unbounded};
