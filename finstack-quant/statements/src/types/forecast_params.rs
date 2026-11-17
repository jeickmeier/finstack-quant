//! The typed forecast specification.
//!
//! [`ForecastSpec`] is one closed enum: a forecast method with the typed
//! parameters that method takes. It is the form a model stores, every
//! forecast method runs on and the published schema describes. On the wire it
//! is `{ "method": ..., "params": { ... } }`; an unknown, duplicate, `null` or
//! method-inapplicable field is rejected when the document is read. Rates,
//! growth and reversion speeds are decimals per model period; levels and
//! absolute parameters are in the series' own units.

use indexmap::IndexMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::{ForecastMethod, SeasonalMode};
use crate::error::{Error, Result};
use finstack_quant_core::dates::PeriodId;

/// A forecast method with its typed parameters.
///
/// Serializes as `{ "method": ..., "params": { ... } }`. `params` may be
/// omitted when the method needs none.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[cfg_attr(
    feature = "json-schema",
    schemars(
        tag = "method",
        content = "params",
        rename_all = "snake_case",
        transform = params_optional_without_required_parameters
    )
)]
pub enum ForecastSpec {
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

/// `params` may be omitted for a method whose parameters are all optional,
/// which the derived schema would otherwise require.
#[cfg(feature = "json-schema")]
fn params_optional_without_required_parameters(schema: &mut schemars::Schema) {
    let Some(variants) = schema
        .as_object_mut()
        .and_then(|object| object.get_mut("oneOf"))
        .and_then(serde_json::Value::as_array_mut)
    else {
        return;
    };
    for variant in variants {
        let forward_fill = variant
            .pointer("/properties/method/const")
            .and_then(serde_json::Value::as_str)
            == Some("forward_fill");
        if forward_fill
            && let Some(required) = variant
                .get_mut("required")
                .and_then(serde_json::Value::as_array_mut)
        {
            required.retain(|name| name != "params");
        }
    }
}

/// The wire form of a [`ForecastSpec`]: a method tag and its parameter object.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ForecastWire {
    method: ForecastMethod,
    #[serde(
        default,
        skip_serializing_if = "IndexMap::is_empty",
        deserialize_with = "unique_parameters"
    )]
    params: IndexMap<String, serde_json::Value>,
}

/// Read a parameter object, rejecting a key that appears twice.
fn unique_parameters<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> std::result::Result<IndexMap<String, serde_json::Value>, D::Error> {
    struct Unique;

    impl<'de> serde::de::Visitor<'de> for Unique {
        type Value = IndexMap<String, serde_json::Value>;

        fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("an object of forecast parameters")
        }

        fn visit_map<A: serde::de::MapAccess<'de>>(
            self,
            mut access: A,
        ) -> std::result::Result<Self::Value, A::Error> {
            let mut params = IndexMap::new();
            while let Some((key, value)) = access.next_entry::<String, serde_json::Value>()? {
                if params.contains_key(&key) {
                    return Err(serde::de::Error::custom(format!(
                        "forecast parameter '{key}' appears more than once"
                    )));
                }
                params.insert(key, value);
            }
            Ok(params)
        }
    }

    deserializer.deserialize_map(Unique)
}

impl Serialize for ForecastSpec {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        ForecastWire {
            method: self.method(),
            params: self.params(),
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for ForecastSpec {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        let wire = ForecastWire::deserialize(deserializer)?;
        Self::from_params(wire.method, wire.params).map_err(serde::de::Error::custom)
    }
}

impl ForecastSpec {
    /// The forecast method this specification selects.
    #[must_use]
    pub fn method(&self) -> ForecastMethod {
        match self {
            Self::ForwardFill(_) => ForecastMethod::ForwardFill,
            Self::GrowthPct(_) => ForecastMethod::GrowthPct,
            Self::CurvePct(_) => ForecastMethod::CurvePct,
            Self::Normal(_) => ForecastMethod::Normal,
            Self::LogNormal(_) => ForecastMethod::LogNormal,
            Self::Override(_) => ForecastMethod::Override,
            Self::TimeSeries(_) => ForecastMethod::TimeSeries,
            Self::Seasonal(_) => ForecastMethod::Seasonal,
            Self::MeanReverting(_) => ForecastMethod::MeanReverting,
            Self::Bootstrap(_) => ForecastMethod::Bootstrap,
            Self::FadeToTarget(_) => ForecastMethod::FadeToTarget,
        }
    }

    /// The parameters as a wire object: the `params` member of the wire form,
    /// with a seed written as a decimal string.
    #[must_use]
    pub fn params(&self) -> IndexMap<String, serde_json::Value> {
        let value = match self {
            Self::ForwardFill(p) => serde_json::to_value(p),
            Self::GrowthPct(p) => serde_json::to_value(p),
            Self::CurvePct(p) => serde_json::to_value(p),
            Self::Normal(p) | Self::LogNormal(p) => serde_json::to_value(p),
            Self::Override(p) => serde_json::to_value(p),
            Self::TimeSeries(p) => serde_json::to_value(p),
            Self::Seasonal(p) => serde_json::to_value(p),
            Self::MeanReverting(p) => serde_json::to_value(p),
            Self::Bootstrap(p) => serde_json::to_value(p),
            Self::FadeToTarget(p) => serde_json::to_value(p),
        };
        match value {
            Ok(serde_json::Value::Object(map)) => map.into_iter().collect(),
            _ => IndexMap::new(),
        }
    }

    /// Build a specification from a method and its wire parameter object.
    ///
    /// # Arguments
    ///
    /// * `method` - Forecast method the parameters belong to.
    /// * `params` - The method's parameters as a wire object. A seed is a
    ///   canonical decimal string such as `"42"`.
    ///
    /// # Errors
    ///
    /// Returns a forecast error naming the offending key when a parameter is
    /// unknown to the method, missing, `null`, or of the wrong JSON type.
    pub fn from_params(
        method: ForecastMethod,
        params: IndexMap<String, serde_json::Value>,
    ) -> Result<Self> {
        fn read<T: serde::de::DeserializeOwned>(
            method: ForecastMethod,
            params: IndexMap<String, serde_json::Value>,
        ) -> Result<T> {
            if let Some((key, _)) = params.iter().find(|(_, value)| value.is_null()) {
                return Err(Error::forecast(format!(
                    "{method:?} forecast parameter '{key}' is null; omit it instead"
                )));
            }
            let object: serde_json::Map<String, serde_json::Value> = params.into_iter().collect();
            serde_path_to_error::deserialize(serde_json::Value::Object(object)).map_err(|error| {
                let path = error.path().to_string();
                let inner = error.into_inner();
                Error::forecast(if path == "." {
                    format!("Invalid {method:?} forecast parameters: {inner}")
                } else {
                    format!("Invalid {method:?} forecast parameter '{path}': {inner}")
                })
            })
        }

        Ok(match method {
            ForecastMethod::ForwardFill => Self::ForwardFill(read(method, params)?),
            ForecastMethod::GrowthPct => Self::GrowthPct(read(method, params)?),
            ForecastMethod::CurvePct => Self::CurvePct(read(method, params)?),
            ForecastMethod::Normal => Self::Normal(read(method, params)?),
            ForecastMethod::LogNormal => Self::LogNormal(read(method, params)?),
            ForecastMethod::Override => Self::Override(read(method, params)?),
            ForecastMethod::TimeSeries => Self::TimeSeries(read(method, params)?),
            ForecastMethod::Seasonal => Self::Seasonal(read(method, params)?),
            ForecastMethod::FadeToTarget => Self::FadeToTarget(read(method, params)?),
            ForecastMethod::MeanReverting => Self::MeanReverting(read(method, params)?),
            ForecastMethod::Bootstrap => Self::Bootstrap(read(method, params)?),
        })
    }

    /// The same specification with one wire parameter set, checked again as
    /// a whole.
    ///
    /// # Arguments
    ///
    /// * `key` - Parameter name in the method's wire object, such as `"min"`.
    /// * `value` - Wire value of the parameter. A seed is a canonical decimal
    ///   string.
    ///
    /// # Errors
    ///
    /// Returns a forecast error when the method has no such parameter or the
    /// value has the wrong type.
    pub fn with_param(self, key: &str, value: serde_json::Value) -> Result<Self> {
        let mut params = self.params();
        params.insert(key.to_string(), value);
        Self::from_params(self.method(), params)
    }

    /// Carry the last value forward.
    #[must_use]
    pub fn forward_fill() -> Self {
        Self::ForwardFill(BoundsParams {
            start: None,
            min: None,
            max: None,
        })
    }

    /// Compound one growth rate per period.
    ///
    /// # Arguments
    ///
    /// * `rate` - Growth per model period as a decimal (`0.05` is 5%).
    #[must_use]
    pub fn growth(rate: f64) -> Self {
        Self::GrowthPct(GrowthPctParams {
            rate,
            start: None,
            min: None,
            max: None,
        })
    }

    /// Compound a different growth rate each forecast period.
    ///
    /// # Arguments
    ///
    /// * `curve` - Growth per model period as decimals, one per forecast
    ///   period in order.
    #[must_use]
    pub fn curve(curve: Vec<f64>) -> Self {
        Self::CurvePct(CurvePctParams {
            curve,
            start: None,
            min: None,
            max: None,
        })
    }

    fn distribution(mean: f64, std_dev: f64, seed: u64) -> DistributionParams {
        DistributionParams {
            mean,
            std_dev,
            seed,
            start: None,
            correlation_with: None,
            correlation: None,
            min: None,
            max: None,
        }
    }

    /// Sample an additive random walk.
    ///
    /// # Arguments
    ///
    /// * `mean` - Mean change per model period, in the series' units.
    /// * `std_dev` - Standard deviation of the change per model period, in
    ///   the series' units; non-negative.
    /// * `seed` - Exact seed of the random stream.
    #[must_use]
    pub fn normal(mean: f64, std_dev: f64, seed: u64) -> Self {
        Self::Normal(Self::distribution(mean, std_dev, seed))
    }

    /// Sample a multiplicative random walk for positive values.
    ///
    /// # Arguments
    ///
    /// * `mean` - Mean log change per model period, as a decimal.
    /// * `std_dev` - Standard deviation of the log change per model period;
    ///   non-negative.
    /// * `seed` - Exact seed of the random stream.
    #[must_use]
    pub fn log_normal(mean: f64, std_dev: f64, seed: u64) -> Self {
        Self::LogNormal(Self::distribution(mean, std_dev, seed))
    }

    /// Set exact values for chosen forecast periods.
    ///
    /// # Arguments
    ///
    /// * `overrides` - Value per period, in the series' units. Periods without
    ///   an override carry the previous value forward.
    #[must_use]
    pub fn overrides(overrides: IndexMap<PeriodId, f64>) -> Self {
        Self::Override(OverrideParams {
            overrides: overrides
                .into_iter()
                .map(|(period, value)| (period.to_string(), value))
                .collect(),
            min: None,
            max: None,
        })
    }

    /// Project a repeating pattern from a historical series.
    ///
    /// # Arguments
    ///
    /// * `historical` - Past observations in chronological order, in the
    ///   series' units.
    /// * `season_length` - Model periods in one season; at least one.
    /// * `mode` - Whether the seasonal pattern adds to or multiplies the
    ///   trend.
    #[must_use]
    pub fn seasonal(historical: Vec<f64>, season_length: usize, mode: SeasonalMode) -> Self {
        Self::Seasonal(SeasonalParams {
            historical,
            season_length: season_length as u64,
            mode,
            growth: None,
            min: None,
            max: None,
        })
    }

    /// Project from a historical series with the chosen trend method.
    ///
    /// # Arguments
    ///
    /// * `historical` - Past observations in chronological order, in the
    ///   series' units.
    /// * `method` - Trend method; there is no default.
    #[must_use]
    pub fn time_series(historical: Vec<f64>, method: ForecastTrendMethod) -> Self {
        Self::TimeSeries(TimeSeriesParams {
            historical,
            method,
            alpha: None,
            beta: None,
            phi: None,
            window: None,
            min: None,
            max: None,
        })
    }

    /// Glide from the last value toward a target level.
    ///
    /// # Arguments
    ///
    /// * `target` - Level the series fades toward, in the series' units.
    /// * `shape` - Path of the fade; there is no default. Only the
    ///   exponential shape takes a `half_life`.
    #[must_use]
    pub fn fade_to_target(target: f64, shape: ForecastFadeShape) -> Self {
        Self::FadeToTarget(FadeToTargetParams {
            target,
            shape,
            start: None,
            half_life: None,
            min: None,
            max: None,
        })
    }

    /// Sample a mean-reverting AR(1) process.
    ///
    /// # Arguments
    ///
    /// * `long_run_mean` - Level the process reverts to, in the series'
    ///   units.
    /// * `reversion_speed` - Share of the gap closed per model period, as a
    ///   decimal.
    /// * `std_dev` - Standard deviation of the shock per model period, in
    ///   the series' units; non-negative.
    /// * `seed` - Exact seed of the random stream.
    #[must_use]
    pub fn mean_reverting(
        long_run_mean: f64,
        reversion_speed: f64,
        std_dev: f64,
        seed: u64,
    ) -> Self {
        Self::MeanReverting(MeanRevertingParams {
            long_run_mean,
            reversion_speed,
            std_dev,
            seed,
            start: None,
            correlation_with: None,
            correlation: None,
            min: None,
            max: None,
        })
    }

    /// Resample changes from a historical series.
    ///
    /// # Arguments
    ///
    /// * `historical` - Past observations in chronological order, in the
    ///   series' units.
    /// * `mode` - Whether growth rates or differences are resampled; there is
    ///   no default.
    /// * `seed` - Exact seed of the random stream.
    #[must_use]
    pub fn bootstrap(historical: Vec<f64>, mode: ForecastBootstrapMode, seed: u64) -> Self {
        Self::Bootstrap(BootstrapParams {
            historical,
            mode,
            seed,
            start: None,
            min: None,
            max: None,
        })
    }

    /// Check the static requirements of the method: finite numbers, ordered
    /// bounds, valid standard deviations and conditional parameters.
    ///
    /// These hold for any model. Checks that need the model (period keys in
    /// the plan, a correlation peer) are part of runnable-model validation.
    ///
    /// # Errors
    ///
    /// Returns a forecast error naming the method and the parameter.
    pub fn validate(&self) -> Result<()> {
        let method = self.method();
        let fail =
            |message: String| Err(Error::forecast(format!("{method:?} forecast: {message}")));
        let finite = |name: &str, value: f64| {
            if value.is_finite() {
                Ok(())
            } else {
                fail(format!("'{name}' must be a finite number"))
            }
        };
        let series = |name: &str, values: &[f64]| {
            if values.is_empty() {
                return fail(format!("'{name}' must not be empty"));
            }
            values.iter().try_for_each(|value| finite(name, *value))
        };
        let volatility = |name: &str, value: f64| {
            finite(name, value)?;
            if value < 0.0 {
                return fail(format!("'{name}' must not be negative"));
            }
            Ok(())
        };
        let correlation = |peer: &Option<String>, value: Option<f64>| match (peer, value) {
            (None, None) => Ok(()),
            (Some(_), Some(value)) if (-1.0..=1.0).contains(&value) => Ok(()),
            (Some(_), Some(_)) => fail("'correlation' must be between -1 and 1".to_string()),
            _ => fail("'correlation_with' and 'correlation' must be given together".to_string()),
        };

        let (min, max) = self.bounds();
        for (name, bound) in [("min", min), ("max", max), ("start", self.start())] {
            if let Some(bound) = bound {
                finite(name, bound)?;
            }
        }
        if let (Some(min), Some(max)) = (min, max)
            && min > max
        {
            return fail(format!("'min' ({min}) is above 'max' ({max})"));
        }

        match self {
            Self::ForwardFill(_) => Ok(()),
            Self::GrowthPct(p) => finite("rate", p.rate),
            // An empty curve is valid for a model with no forecast period;
            // the model checks its coverage.
            Self::CurvePct(p) => p.curve.iter().try_for_each(|rate| finite("curve", *rate)),
            Self::Normal(p) | Self::LogNormal(p) => {
                finite("mean", p.mean)?;
                volatility("std_dev", p.std_dev)?;
                correlation(&p.correlation_with, p.correlation)
            }
            Self::Override(p) => p.overrides.iter().try_for_each(|(period, value)| {
                if period.parse::<PeriodId>().is_err() {
                    return fail(format!(
                        "override key '{period}' is not a period identifier"
                    ));
                }
                finite("overrides", *value)
            }),
            Self::TimeSeries(p) => {
                series("historical", &p.historical)?;
                for (name, value) in [("alpha", p.alpha), ("beta", p.beta), ("phi", p.phi)] {
                    let Some(value) = value else { continue };
                    if p.method != ForecastTrendMethod::Exponential {
                        return fail(format!("'{name}' applies only to the exponential method"));
                    }
                    if !(value > 0.0 && value <= 1.0) {
                        return fail(format!("'{name}' must be in (0, 1]"));
                    }
                }
                match (p.method, p.window) {
                    (ForecastTrendMethod::MovingAverage, None) => {
                        fail("the moving_average method needs a 'window'".to_string())
                    }
                    (ForecastTrendMethod::MovingAverage, Some(0)) => {
                        fail("'window' must be at least 1".to_string())
                    }
                    (ForecastTrendMethod::MovingAverage, Some(_)) | (_, None) => Ok(()),
                    (_, Some(_)) => {
                        fail("'window' applies only to the moving_average method".to_string())
                    }
                }
            }
            Self::Seasonal(p) => {
                series("historical", &p.historical)?;
                if p.season_length == 0 {
                    return fail("'season_length' must be at least 1".to_string());
                }
                p.growth.map_or(Ok(()), |growth| finite("growth", growth))
            }
            Self::MeanReverting(p) => {
                finite("long_run_mean", p.long_run_mean)?;
                finite("reversion_speed", p.reversion_speed)?;
                volatility("std_dev", p.std_dev)?;
                correlation(&p.correlation_with, p.correlation)
            }
            Self::Bootstrap(p) => series("historical", &p.historical),
            Self::FadeToTarget(p) => {
                finite("target", p.target)?;
                match (p.shape, p.half_life) {
                    (_, None) => Ok(()),
                    (ForecastFadeShape::Exponential, Some(half_life))
                        if half_life.is_finite() && half_life > 0.0 =>
                    {
                        Ok(())
                    }
                    (ForecastFadeShape::Exponential, Some(_)) => {
                        fail("'half_life' must be positive and finite".to_string())
                    }
                    (shape, Some(_)) => fail(format!(
                        "'half_life' applies only to the exponential shape, not {shape:?}"
                    )),
                }
            }
        }
    }

    /// The explicit start level selected for the opening of the forecast
    /// suffix, when the method takes one and it is set.
    pub(crate) fn start(&self) -> Option<f64> {
        match self {
            Self::ForwardFill(p) => p.start,
            Self::GrowthPct(p) => p.start,
            Self::CurvePct(p) => p.start,
            Self::Normal(p) | Self::LogNormal(p) => p.start,
            Self::MeanReverting(p) => p.start,
            Self::Bootstrap(p) => p.start,
            Self::FadeToTarget(p) => p.start,
            Self::Override(_) | Self::TimeSeries(_) | Self::Seasonal(_) => None,
        }
    }

    /// The `(min, max)` bounds clamped onto generated values.
    pub(crate) fn bounds(&self) -> (Option<f64>, Option<f64>) {
        match self {
            Self::ForwardFill(p) => (p.min, p.max),
            Self::GrowthPct(p) => (p.min, p.max),
            Self::CurvePct(p) => (p.min, p.max),
            Self::Normal(p) | Self::LogNormal(p) => (p.min, p.max),
            Self::Override(p) => (p.min, p.max),
            Self::TimeSeries(p) => (p.min, p.max),
            Self::Seasonal(p) => (p.min, p.max),
            Self::MeanReverting(p) => (p.min, p.max),
            Self::Bootstrap(p) => (p.min, p.max),
            Self::FadeToTarget(p) => (p.min, p.max),
        }
    }

    /// The random seed of a stochastic method; `None` for deterministic methods.
    pub(crate) fn seed_mut(&mut self) -> Option<&mut u64> {
        match self {
            Self::Normal(p) | Self::LogNormal(p) => Some(&mut p.seed),
            Self::MeanReverting(p) => Some(&mut p.seed),
            Self::Bootstrap(p) => Some(&mut p.seed),
            Self::ForwardFill(_)
            | Self::GrowthPct(_)
            | Self::CurvePct(_)
            | Self::Override(_)
            | Self::TimeSeries(_)
            | Self::Seasonal(_)
            | Self::FadeToTarget(_) => None,
        }
    }
}

/// Seeds are exact `u64` values written as canonical decimal strings, so
/// every host carries all 64 bits: `"9007199254740993"` is that integer.
mod seed {
    use serde::{Deserialize, Deserializer, Serializer};

    pub(super) fn serialize<S: Serializer>(seed: &u64, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&seed.to_string())
    }

    pub(super) fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<u64, D::Error> {
        let text = String::deserialize(deserializer).map_err(|_| {
            serde::de::Error::custom("a seed is a decimal string of an unsigned 64-bit integer")
        })?;
        let canonical = !text.is_empty()
            && text.bytes().all(|byte| byte.is_ascii_digit())
            && (text == "0" || !text.starts_with('0'));
        text.parse::<u64>()
            .ok()
            .filter(|_| canonical)
            .ok_or_else(|| {
                serde::de::Error::custom(format!(
                    "'{text}' is not a canonical decimal unsigned 64-bit integer"
                ))
            })
    }
}

/// Parameters of a method with only the cross-cutting bounds.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct BoundsParams {
    /// Level at the opening of the forecast suffix, in the series' own units.
    /// When omitted, the value of the slot immediately before the suffix is
    /// the base.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start: Option<f64>,
    /// Lower bound clamped onto generated values, in the line's base units.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min: Option<f64>,
    /// Upper bound clamped onto generated values, in the line's base units.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,
}

/// Parameters of [`ForecastSpec::GrowthPct`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct GrowthPctParams {
    /// Growth per period as a decimal: 0.03 = 3%.
    pub rate: f64,
    /// Level at the opening of the forecast suffix, in the series' own units.
    /// When omitted, the value of the slot immediately before the suffix is
    /// the base.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start: Option<f64>,
    /// Lower bound clamped onto generated values, in the line's base units.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min: Option<f64>,
    /// Upper bound clamped onto generated values, in the line's base units.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,
}

/// Parameters of [`ForecastSpec::CurvePct`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CurvePctParams {
    /// Growth rate per forecast period as decimals, one per period in order.
    pub curve: Vec<f64>,
    /// Level at the opening of the forecast suffix, in the series' own units.
    /// When omitted, the value of the slot immediately before the suffix is
    /// the base.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start: Option<f64>,
    /// Lower bound clamped onto generated values, in the line's base units.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min: Option<f64>,
    /// Upper bound clamped onto generated values, in the line's base units.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,
}

/// Parameters of [`ForecastSpec::Override`].
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

/// Parameters of [`ForecastSpec::Normal`] and [`ForecastSpec::LogNormal`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct DistributionParams {
    /// Mean shock per period, as the method applies it.
    pub mean: f64,
    /// Standard deviation of the shock per period; non-negative.
    pub std_dev: f64,
    /// Random seed; the same seed reproduces the same path.
    #[serde(with = "seed")]
    #[cfg_attr(feature = "json-schema", schemars(with = "String"))]
    pub seed: u64,
    /// Node whose shocks this one correlates with; Monte Carlo evaluation only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub correlation_with: Option<String>,
    /// Correlation with `correlation_with`, in [-1, 1]; Monte Carlo evaluation only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub correlation: Option<f64>,
    /// Level at the opening of the forecast suffix, in the series' own units.
    /// When omitted, the value of the slot immediately before the suffix is
    /// the base.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start: Option<f64>,
    /// Lower bound clamped onto generated values, in the line's base units.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min: Option<f64>,
    /// Upper bound clamped onto generated values, in the line's base units.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,
}

/// Parameters of [`ForecastSpec::MeanReverting`].
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
    #[serde(with = "seed")]
    #[cfg_attr(feature = "json-schema", schemars(with = "String"))]
    pub seed: u64,
    /// Node whose shocks this one correlates with; Monte Carlo evaluation only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub correlation_with: Option<String>,
    /// Correlation with `correlation_with`, in [-1, 1]; Monte Carlo evaluation only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub correlation: Option<f64>,
    /// Level at the opening of the forecast suffix, in the series' own units.
    /// When omitted, the value of the slot immediately before the suffix is
    /// the base.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start: Option<f64>,
    /// Lower bound clamped onto generated values, in the line's base units.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min: Option<f64>,
    /// Upper bound clamped onto generated values, in the line's base units.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,
}

/// What a bootstrap resamples from its historical series.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum ForecastBootstrapMode {
    /// Period-over-period growth rates, compounded; needs a positive series.
    Growth,
    /// Additive level changes; works for a series crossing zero.
    Diff,
}

/// Parameters of [`ForecastSpec::Bootstrap`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct BootstrapParams {
    /// At least 2 observations in the line's base units, oldest first.
    pub historical: Vec<f64>,
    /// What to resample; defaults to growth rates.
    pub mode: ForecastBootstrapMode,
    /// Random seed; the same seed reproduces the same path.
    #[serde(with = "seed")]
    #[cfg_attr(feature = "json-schema", schemars(with = "String"))]
    pub seed: u64,
    /// Level at the opening of the forecast suffix, in the series' own units.
    /// When omitted, the value of the slot immediately before the suffix is
    /// the base.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start: Option<f64>,
    /// Lower bound clamped onto generated values, in the line's base units.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min: Option<f64>,
    /// Upper bound clamped onto generated values, in the line's base units.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,
}

/// Trend model of a time-series forecast.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum ForecastTrendMethod {
    /// Least-squares linear trend.
    Linear,
    /// Holt exponential smoothing; requires `alpha` and `beta`.
    Exponential,
    /// Moving average; requires `window`.
    MovingAverage,
}

/// Parameters of [`ForecastSpec::TimeSeries`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct TimeSeriesParams {
    /// Observations in the line's base units, oldest first.
    pub historical: Vec<f64>,
    /// Trend model; defaults to linear.
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

/// Parameters of [`ForecastSpec::Seasonal`].
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum ForecastFadeShape {
    /// Equal steps, reaching the target at the final period.
    Linear,
    /// Constant compound growth; base and target must be non-zero with the same sign.
    Geometric,
    /// Half-life decay toward the target; requires `half_life`.
    Exponential,
}

/// Parameters of [`ForecastSpec::FadeToTarget`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct FadeToTargetParams {
    /// Terminal level, in the line's base units.
    pub target: f64,
    /// Path to the target; defaults to linear.
    pub shape: ForecastFadeShape,
    /// Periods to close half the gap; required for, and legal only with, the exponential shape.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub half_life: Option<f64>,
    /// Level at the opening of the forecast suffix, in the series' own units.
    /// When omitted, the value of the slot immediately before the suffix is
    /// the base.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start: Option<f64>,
    /// Lower bound clamped onto generated values, in the line's base units.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min: Option<f64>,
    /// Upper bound clamped onto generated values, in the line's base units.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,
}
