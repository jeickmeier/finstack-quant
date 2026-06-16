//! Node specification and types.

use crate::types::AmountOrScalar;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::PeriodId;
use indexmap::IndexMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::borrow::Borrow;
use std::fmt;
use std::sync::Arc;

/// Type-safe identifier for a node in a financial model.
///
/// Wraps an `Arc<str>` so that cloning a `NodeId` is a reference-count bump
/// rather than a heap allocation — node ids are cloned heavily (evaluation
/// order, column maps, per-warning context). It serializes as a plain string
/// and is interoperable with `&str` via [`Borrow`] and [`AsRef`].
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct NodeId(Arc<str>);

impl NodeId {
    /// Create a new `NodeId` from any string-like value.
    pub fn new(id: impl Into<String>) -> Self {
        Self(Arc::from(id.into()))
    }

    /// Return the inner string slice.
    #[inline]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<&str> for NodeId {
    fn from(s: &str) -> Self {
        Self(Arc::from(s))
    }
}

impl From<String> for NodeId {
    fn from(s: String) -> Self {
        Self(Arc::from(s))
    }
}

impl From<&String> for NodeId {
    fn from(s: &String) -> Self {
        Self(Arc::from(s.as_str()))
    }
}

impl Borrow<str> for NodeId {
    fn borrow(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for NodeId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl PartialEq<&str> for NodeId {
    fn eq(&self, other: &&str) -> bool {
        &*self.0 == *other
    }
}

impl PartialEq<str> for NodeId {
    fn eq(&self, other: &str) -> bool {
        &*self.0 == other
    }
}

// Serialize as a plain string. A manual impl avoids requiring serde's `rc`
// feature flag for the `Arc<str>` inner representation.
impl Serialize for NodeId {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for NodeId {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        Ok(Self(Arc::from(s)))
    }
}

/// Specification for a single node (metric/line item) in the financial model.
///
/// A node can be:
/// - **Value**: Explicit values only
/// - **Calculated**: Formula-derived only
/// - **Mixed**: Value OR Forecast OR Formula (precedence: Value > Forecast > Formula)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NodeSpec {
    /// Unique identifier for this node
    pub node_id: NodeId,

    /// Human-readable name (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    /// Node computation type
    pub node_type: NodeType,

    /// Explicit values per period (for Value and Mixed nodes)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub values: Option<IndexMap<PeriodId, AmountOrScalar>>,

    /// Forecast specification (for Mixed nodes)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub forecast: Option<ForecastSpec>,

    /// Formula text (for Calculated and Mixed nodes)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub formula_text: Option<String>,

    /// Where clause for conditional evaluation (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub where_text: Option<String>,

    /// Tags for grouping/filtering
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,

    /// Value type (monetary with currency or scalar)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value_type: Option<NodeValueType>,

    /// Additional metadata
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    pub meta: IndexMap<String, serde_json::Value>,
}

impl NodeSpec {
    /// Create a new node specification.
    ///
    /// # Arguments
    /// * `node_id` - Unique identifier for the node
    /// * `node_type` - Computation type that defines how the node is evaluated
    pub fn new(node_id: impl Into<NodeId>, node_type: NodeType) -> Self {
        Self {
            node_id: node_id.into(),
            name: None,
            node_type,
            values: None,
            forecast: None,
            formula_text: None,
            where_text: None,
            tags: Vec::new(),
            value_type: None,
            meta: IndexMap::new(),
        }
    }

    /// Set the human-readable name.
    ///
    /// # Arguments
    /// * `name` - Display name shown in reports or UI
    #[must_use = "builder methods take self by value and return the modified value"]
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Add explicit values.
    ///
    /// # Arguments
    /// * `values` - Period-indexed map of explicit values
    #[must_use = "builder methods take self by value and return the modified value"]
    pub fn with_values(mut self, values: IndexMap<PeriodId, AmountOrScalar>) -> Self {
        self.values = Some(values);
        self
    }

    /// Set the formula text.
    ///
    /// # Arguments
    /// * `formula` - Expression written in the statements DSL
    #[must_use = "builder methods take self by value and return the modified value"]
    pub fn with_formula(mut self, formula: impl Into<String>) -> Self {
        self.formula_text = Some(formula.into());
        self
    }

    /// Set the forecast specification.
    ///
    /// # Arguments
    /// * `forecast_spec` - Forecast configuration created with [`ForecastSpec`]
    #[must_use = "builder methods take self by value and return the modified value"]
    pub fn with_forecast(mut self, forecast_spec: ForecastSpec) -> Self {
        self.forecast = Some(forecast_spec);
        self
    }

    /// Add tags.
    ///
    /// # Arguments
    /// * `tags` - Arbitrary labels used for grouping or filtering
    #[must_use = "builder methods take self by value and return the modified value"]
    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags = tags;
        self
    }
}

/// Node computation type.
///
/// Determines how a node's value is computed:
/// - **Value**: Only explicit values (actuals, assumptions)
/// - **Calculated**: Only formula-derived
/// - **Mixed**: Value OR Forecast OR Formula (precedence: Value > Forecast > Formula)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeType {
    /// Only explicit values
    Value,
    /// Only formula-derived
    Calculated,
    /// Value OR Forecast OR Formula (precedence: Value > Forecast > Formula)
    Mixed,
}

/// Forecast method specification.
///
/// Defines how to forecast future values for a node.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ForecastSpec {
    /// Forecast method
    pub method: ForecastMethod,

    /// Method-specific parameters
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    pub params: IndexMap<String, serde_json::Value>,
}

impl ForecastSpec {
    /// Create a forward-fill forecast (carry last value forward).
    pub fn forward_fill() -> Self {
        Self {
            method: ForecastMethod::ForwardFill,
            params: IndexMap::new(),
        }
    }

    /// Create a growth percentage forecast.
    ///
    /// # Arguments
    /// * `rate` - Growth rate (e.g., 0.05 for 5% growth)
    pub fn growth(rate: f64) -> Self {
        let mut params = IndexMap::new();
        params.insert("rate".into(), serde_json::json!(rate));
        Self {
            method: ForecastMethod::GrowthPct,
            params,
        }
    }

    /// Create a curve percentage forecast.
    ///
    /// # Arguments
    /// * `curve` - Vector of growth rates per period
    pub fn curve(curve: Vec<f64>) -> Self {
        let mut params = IndexMap::new();
        params.insert("curve".into(), serde_json::json!(curve));
        Self {
            method: ForecastMethod::CurvePct,
            params,
        }
    }

    /// Create an additive normal random-walk forecast.
    ///
    /// Forecasted values follow `value[t] = value[t-1] + mean + std_dev * z[t]`,
    /// where `z[t]` is a deterministic standard-normal draw derived from `seed`.
    /// Use this for additive level changes such as absolute EBITDA deltas or
    /// working-capital movements. `std_dev` is the per-period volatility and must
    /// be non-negative.
    ///
    /// # Arguments
    /// * `mean` - Per-period additive drift
    /// * `std_dev` - Per-period additive volatility
    /// * `seed` - Random seed for deterministic results
    ///
    /// # References
    ///
    /// - Monte Carlo simulation practice: `docs/REFERENCES.md#glasserman-2004-monte-carlo`
    pub fn normal(mean: f64, std_dev: f64, seed: u64) -> Self {
        let mut params = IndexMap::new();
        params.insert("mean".into(), serde_json::json!(mean));
        params.insert("std_dev".into(), serde_json::json!(std_dev));
        params.insert("seed".into(), serde_json::json!(seed));
        Self {
            method: ForecastMethod::Normal,
            params,
        }
    }

    /// Create a multiplicative log-normal path forecast.
    ///
    /// When the base value is non-zero, forecasted values follow
    /// `value[t] = value[t-1] * exp(mean - 0.5 * std_dev^2 + std_dev * z[t])`.
    /// The `-0.5 * std_dev^2` term is the standard log-normal drift adjustment
    /// so `mean` is interpreted as the expected log-return drift. When the base
    /// value is zero, the path falls back to independent
    /// `exp(mean + std_dev * z[t])` draws because multiplication by zero would
    /// otherwise collapse the whole path.
    ///
    /// # Arguments
    /// * `mean` - Per-period log-return drift
    /// * `std_dev` - Per-period log-return volatility
    /// * `seed` - Random seed for deterministic results
    ///
    /// # References
    ///
    /// - Monte Carlo simulation practice: `docs/REFERENCES.md#glasserman-2004-monte-carlo`
    pub fn lognormal(mean: f64, std_dev: f64, seed: u64) -> Self {
        let mut params = IndexMap::new();
        params.insert("mean".into(), serde_json::json!(mean));
        params.insert("std_dev".into(), serde_json::json!(std_dev));
        params.insert("seed".into(), serde_json::json!(seed));
        Self {
            method: ForecastMethod::LogNormal,
            params,
        }
    }
}

/// Available forecast methods.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ForecastMethod {
    /// Carry last value forward
    ForwardFill,

    /// Compound growth: `v[t] = v[t-1] * (1 + rate)`
    GrowthPct,

    /// Period-specific growth rates: `v[t] = v[t-1] * (1 + curve[t])`
    CurvePct,

    /// Sample from normal distribution (deterministic with seed)
    Normal,

    /// Sample from log-normal distribution (deterministic with seed)
    LogNormal,

    /// Explicit period overrides
    Override,

    /// Reference external time series
    TimeSeries,

    /// Seasonal pattern (additive/multiplicative)
    Seasonal,
}

/// Seasonal decomposition mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SeasonalMode {
    /// Additive seasonality: Y = Trend + Seasonal + Error
    Additive,
    /// Multiplicative seasonality: Y = Trend * Seasonal * Error
    Multiplicative,
}

/// Node value type classification.
///
/// Determines whether a node represents monetary values (with a specific currency)
/// or scalar values (ratios, percentages, counts, etc.).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum NodeValueType {
    /// Monetary value with a specific currency (e.g., revenue, costs, balance sheet items)
    Monetary {
        /// Currency of the monetary value
        currency: Currency,
    },
    /// Unitless scalar value (e.g., ratios, percentages, counts)
    Scalar,
}
