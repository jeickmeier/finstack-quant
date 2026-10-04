//! Credit factor hierarchy artifacts, calibration, and decomposition.

/// Credit hierarchy calibration from issuer spread histories.
pub mod calibration;
/// Credit factor decomposition across hierarchy levels.
pub mod decomposition;
/// Credit factor covariance and idiosyncratic-volatility forecasts.
mod forecast;
/// Credit factor hierarchy artifact types.
pub mod hierarchy;
mod peel;
/// Decimal-spread input convention and bp conversion.
pub(crate) mod units;

pub use forecast::{FactorCovarianceForecast, VolHorizon};
