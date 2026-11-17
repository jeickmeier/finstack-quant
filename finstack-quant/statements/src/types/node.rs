//! Node specification and types.

use crate::types::{AmountOrScalar, ForecastSpec};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::PeriodId;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

mod node_id {
    /// Marker for [`NodeId`].
    #[derive(Debug, Clone, Copy, Default)]
    #[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
    pub struct NodeTag;

    /// Type-safe identifier for a node in a financial model.
    ///
    /// Alias of [`finstack_quant_core::types::Id`]. Serializes as a plain string
    /// and borrows as `str`.
    pub type NodeId = finstack_quant_core::types::Id<NodeTag>;
}

pub use node_id::NodeId;

/// Specification for a single node (metric/line item) in the financial model.
///
/// A node can be:
/// - **Value**: Explicit values only
/// - **Calculated**: Formula-derived only
/// - **Mixed**: Value OR Forecast OR Formula (precedence: Value > Forecast > Formula)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
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
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "Option<IndexMap<String, AmountOrScalar>>")
    )]
    pub values: Option<IndexMap<PeriodId, AmountOrScalar>>,
    /// Publication date of each explicit period value.
    ///
    /// An evaluation with an observation cutoff reads an observation only
    /// when this date is on or before the cutoff, and needs a date for every
    /// observation in an actual period. An explicit value in a forecast
    /// period without a date is a forecast assumption, not a published
    /// observation, and is always visible.
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "IndexMap<String, finstack_quant_core::wire::DateWire>")
    )]
    pub availability_dates: IndexMap<PeriodId, finstack_quant_core::dates::Date>,

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

    /// How this line rolls up to a coarser reporting frequency. A rollup uses
    /// its own per-line method first, then this; a line declaring nothing is
    /// summed. Tags never choose a method.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rollup: Option<RollupMethod>,

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
            availability_dates: IndexMap::new(),
            formula_text: None,
            where_text: None,
            tags: Vec::new(),
            value_type: None,
            rollup: None,
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

    /// Set point-in-time availability dates for explicit observations.
    ///
    /// Dates are keyed by the same periods as [`Self::values`]. Under an
    /// observation cutoff an explicit value is visible only when its
    /// availability date is on or before the cutoff.
    ///
    /// # Arguments
    ///
    /// * `availability_dates` - Period-to-date map recording when each explicit
    ///   observation became available to the model
    #[must_use = "builder methods take self by value and return the modified value"]
    pub fn with_availability_dates(
        mut self,
        availability_dates: IndexMap<PeriodId, finstack_quant_core::dates::Date>,
    ) -> Self {
        self.availability_dates = availability_dates;
        self
    }

    /// Date from which the explicit observation for `period` is visible.
    ///
    /// A declared availability date always applies, in any period. A value
    /// without one is a forecast assumption rather than a published
    /// observation and is always visible; point-in-time evaluation rejects an
    /// undated observation in an actual period before it reaches this.
    pub(crate) fn explicit_value_available_on(
        &self,
        period: &finstack_quant_core::dates::Period,
    ) -> Option<finstack_quant_core::dates::Date> {
        self.availability_dates.get(&period.id).copied()
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

    /// Declare how this line rolls up to a coarser reporting frequency.
    ///
    /// # Arguments
    /// * `rollup` - `Sum` for flows, `PeriodEnd` for balances and trailing
    ///   metrics, `Formula` for ratios; `Formula` needs a formula on this node
    #[must_use = "builder methods take self by value and return the modified value"]
    pub fn with_rollup(mut self, rollup: RollupMethod) -> Self {
        self.rollup = Some(rollup);
        self
    }
}

/// How one node's values combine over the source periods of a target period.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum RollupMethod {
    /// Sum the source periods: flows such as revenue, expenses and cash flows.
    /// A missing source value leaves the target value missing.
    Sum,
    /// Take the last source period's value: balances such as cash, debt and
    /// equity, and trailing metrics such as last-twelve-month EBITDA.
    PeriodEnd,
    /// Re-evaluate the node's formula (and `where` clause) on the rolled-up
    /// values: ratios such as margins, leverage and coverage. Time-series
    /// functions then run at the target frequency, so `lag(x, 1)` is the
    /// prior target period.
    Formula,
}

/// Node computation type.
///
/// Determines how a node's value is computed:
/// - **Value**: Only explicit values (actuals, assumptions)
/// - **Calculated**: Only formula-derived
/// - **Mixed**: Value OR Forecast OR Formula (precedence: Value > Forecast > Formula)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum NodeType {
    /// Only explicit values
    Value,
    /// Only formula-derived
    Calculated,
    /// Value OR Forecast OR Formula (precedence: Value > Forecast > Formula)
    Mixed,
}

/// Available forecast methods.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
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

    /// Glide from the base value to a target level: linear, geometric
    /// (CAGR-to-terminal), or exponential (half-life) shape
    FadeToTarget,

    /// Mean-reverting AR(1) path:
    /// `v[t] = v[t-1] + reversion_speed * (long_run_mean - v[t-1]) + std_dev * z[t]`
    MeanReverting,

    /// Resample historical growth rates or level diffs (deterministic with seed)
    Bootstrap,
}

/// Seasonal decomposition mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
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
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
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
