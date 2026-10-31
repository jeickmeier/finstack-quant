//! Typed parameter contract of every forecast method.
//!
//! [`ForecastSpec`](super::ForecastSpec) stores its parameters as a free-form
//! map, which suits long-lived model documents. [`ForecastParameters`] states
//! the same wire shape per method, so a host can read which parameters a method
//! takes from the published schema. Conditional rules, such as `half_life` being
//! legal only with the exponential shape, stay with the method and are reported
//! when the forecast is evaluated.

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use super::SeasonalMode;

/// One forecast method with its typed parameters.
///
/// Serializes as `{ "method": ..., "params": { ... } }`, the wire shape of
/// [`ForecastSpec`](super::ForecastSpec).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(
    tag = "method",
    content = "params",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum ForecastParameters {
    /// Carry the last value forward.
    ForwardFill(BoundsParams),
    /// Compound one growth rate per period.
    GrowthPct(GrowthPctParams),
    /// Compound a different growth rate each period.
    CurvePct(CurvePctParams),
    /// Sample an additive random walk.
    Normal(DistributionParams),
    /// Sample a multiplicative random walk for positive values.
    LogNormal(DistributionParams),
    /// Set exact values for chosen periods, carrying forward between them.
    Override(OverrideParams),
    /// Project from a historical series.
    TimeSeries(TimeSeriesParams),
    /// Project a repeating pattern from a historical series.
    Seasonal(SeasonalParams),
    /// Sample a mean-reverting AR(1) process.
    MeanReverting(MeanRevertingParams),
    /// Resample changes from a historical series.
    Bootstrap(BootstrapParams),
    /// Glide toward a target level over the forecast horizon.
    FadeToTarget(FadeToTargetParams),
}

/// Parameters of a method with only the cross-cutting bounds.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct BoundsParams {
    /// Lower bound clamped onto generated values, in the line's base units.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min: Option<f64>,
    /// Upper bound clamped onto generated values, in the line's base units.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,
}

/// Parameters of [`ForecastParameters::GrowthPct`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct GrowthPctParams {
    /// Growth per period as a decimal: 0.03 = 3%.
    pub rate: f64,
    /// Lower bound clamped onto generated values, in the line's base units.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min: Option<f64>,
    /// Upper bound clamped onto generated values, in the line's base units.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,
}

/// Parameters of [`ForecastParameters::CurvePct`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CurvePctParams {
    /// Growth rate per forecast period as decimals, one per period in order.
    pub curve: Vec<f64>,
    /// Lower bound clamped onto generated values, in the line's base units.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min: Option<f64>,
    /// Upper bound clamped onto generated values, in the line's base units.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,
}

/// Parameters of [`ForecastParameters::Override`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct OverrideParams {
    /// Exact levels by period id, in the line's base units; other periods carry forward.
    pub overrides: IndexMap<String, f64>,
    /// Lower bound clamped onto generated values, in the line's base units.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min: Option<f64>,
    /// Upper bound clamped onto generated values, in the line's base units.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,
}

/// Parameters of [`ForecastParameters::Normal`] and [`ForecastParameters::LogNormal`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct DistributionParams {
    /// Mean shock per period, as the method applies it.
    pub mean: f64,
    /// Standard deviation of the shock per period; non-negative.
    pub std_dev: f64,
    /// Random seed; the same seed reproduces the same path.
    pub seed: u64,
    /// Node whose shocks this one correlates with; Monte Carlo evaluation only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub correlation_with: Option<String>,
    /// Correlation with `correlation_with`, in [-1, 1]; Monte Carlo evaluation only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub correlation: Option<f64>,
    /// Lower bound clamped onto generated values, in the line's base units.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min: Option<f64>,
    /// Upper bound clamped onto generated values, in the line's base units.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,
}

/// Parameters of [`ForecastParameters::MeanReverting`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct MeanRevertingParams {
    /// Long-run level the process reverts to, in the line's base units.
    pub long_run_mean: f64,
    /// Fraction of the gap closed each period, a decimal in (0, 1].
    pub reversion_speed: f64,
    /// Standard deviation of the shock per period; non-negative.
    pub std_dev: f64,
    /// Random seed; the same seed reproduces the same path.
    pub seed: u64,
    /// Node whose shocks this one correlates with; Monte Carlo evaluation only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub correlation_with: Option<String>,
    /// Correlation with `correlation_with`, in [-1, 1]; Monte Carlo evaluation only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub correlation: Option<f64>,
    /// Lower bound clamped onto generated values, in the line's base units.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min: Option<f64>,
    /// Upper bound clamped onto generated values, in the line's base units.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,
}

/// What a bootstrap resamples from its historical series.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum ForecastBootstrapMode {
    /// Period-over-period growth rates, compounded; needs a positive series.
    #[default]
    Growth,
    /// Additive level changes; works for a series crossing zero.
    Diff,
}

/// Parameters of [`ForecastParameters::Bootstrap`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct BootstrapParams {
    /// At least 2 observations in the line's base units, oldest first.
    pub historical: Vec<f64>,
    /// What to resample; defaults to growth rates.
    #[serde(default)]
    pub mode: ForecastBootstrapMode,
    /// Random seed; the same seed reproduces the same path.
    pub seed: u64,
    /// Lower bound clamped onto generated values, in the line's base units.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min: Option<f64>,
    /// Upper bound clamped onto generated values, in the line's base units.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,
}

/// Trend model of a time-series forecast.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum ForecastTrendMethod {
    /// Least-squares linear trend.
    #[default]
    Linear,
    /// Holt exponential smoothing; requires `alpha` and `beta`.
    Exponential,
    /// Moving average; requires `window`.
    MovingAverage,
}

/// Parameters of [`ForecastParameters::TimeSeries`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct TimeSeriesParams {
    /// Observations in the line's base units, oldest first.
    pub historical: Vec<f64>,
    /// Trend model; defaults to linear.
    #[serde(default)]
    pub method: ForecastTrendMethod,
    /// Level smoothing in (0, 1]; required for the exponential method.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alpha: Option<f64>,
    /// Trend smoothing in (0, 1]; required for the exponential method.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub beta: Option<f64>,
    /// Trend damping in (0, 1]; legal only with the exponential method.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub phi: Option<f64>,
    /// Averaging window in periods; required for the moving-average method.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window: Option<u64>,
    /// Lower bound clamped onto generated values, in the line's base units.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min: Option<f64>,
    /// Upper bound clamped onto generated values, in the line's base units.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,
}

/// Parameters of [`ForecastParameters::Seasonal`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct SeasonalParams {
    /// Observations in the line's base units, oldest first; at least two full cycles.
    pub historical: Vec<f64>,
    /// Periods per seasonal cycle.
    pub season_length: u64,
    /// How the seasonal pattern combines with the trend.
    pub mode: SeasonalMode,
    /// Trend growth per period as a decimal: 0.03 = 3%; defaults to none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub growth: Option<f64>,
    /// Lower bound clamped onto generated values, in the line's base units.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min: Option<f64>,
    /// Upper bound clamped onto generated values, in the line's base units.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,
}

/// Path of a fade to target.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum ForecastFadeShape {
    /// Equal steps, reaching the target at the final period.
    #[default]
    Linear,
    /// Constant compound growth; base and target must be non-zero with the same sign.
    Geometric,
    /// Half-life decay toward the target; requires `half_life`.
    Exponential,
}

/// Parameters of [`ForecastParameters::FadeToTarget`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct FadeToTargetParams {
    /// Terminal level, in the line's base units.
    pub target: f64,
    /// Path to the target; defaults to linear.
    #[serde(default)]
    pub shape: ForecastFadeShape,
    /// Periods to close half the gap; required for, and legal only with, the exponential shape.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub half_life: Option<f64>,
    /// Lower bound clamped onto generated values, in the line's base units.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min: Option<f64>,
    /// Upper bound clamped onto generated values, in the line's base units.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,
}
