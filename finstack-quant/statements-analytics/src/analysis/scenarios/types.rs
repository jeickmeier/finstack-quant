//! Sensitivity analysis types.

use finstack_quant_core::dates::PeriodId;
use finstack_quant_statements::evaluator::StatementResult;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

/// Entry in a tornado chart representing one parameter's impact on a metric.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TornadoEntry {
    /// Parameter identifier: `node@period` for statement sensitivities, or
    /// the shocked assumption name for DCF sensitivities.
    pub parameter_id: String,
    /// Downside impact (metric change when parameter is at its minimum).
    pub downside: f64,
    /// Upside impact (metric change when parameter is at its maximum).
    pub upside: f64,
}

impl TornadoEntry {
    /// Total swing magnitude: `upside - downside`.
    pub fn swing(&self) -> f64 {
        self.upside - self.downside
    }
}

/// Parameter to vary in sensitivity analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParameterSpec {
    /// Node identifier
    pub node_id: String,

    /// Period to vary
    pub period_id: PeriodId,

    /// Base value
    pub base_value: f64,

    /// Absolute values to substitute for this node/period; use
    /// [`Self::with_percentages`] to construct shocks from percentage inputs.
    pub perturbations: Vec<f64>,
}

impl ParameterSpec {
    /// Create a new parameter specification.
    ///
    /// # Arguments
    ///
    /// * `node_id` - Exact statement node identifier to vary.
    /// * `period_id` - Forecast period whose node input is replaced.
    /// * `base_value` - Finite reference value in the node's units used when
    ///   constructing percentage shocks; the model supplies the chart baseline.
    /// * `perturbations` - Non-empty finite absolute input values in the node's
    ///   units (decimal rates for rate nodes, currency amounts for monetary nodes).
    pub fn new(
        node_id: impl Into<String>,
        period_id: PeriodId,
        base_value: f64,
        perturbations: Vec<f64>,
    ) -> Self {
        Self {
            node_id: node_id.into(),
            period_id,
            base_value,
            perturbations,
        }
    }

    /// Create a parameter spec with percentage perturbations.
    ///
    /// # Arguments
    ///
    /// * `node_id` - Exact statement node identifier whose input is shocked.
    /// * `period_id` - Forecast model period whose selected node input is
    ///   replaced; execution rejects actual periods.
    /// * `base_value` - Finite reference input in the node's units, such as a
    ///   decimal rate or monetary currency amount; each shock multiplies this
    ///   value by `1 + pct / 100`. The model supplies the chart baseline.
    /// * `pct_range` - Non-empty finite percentage figures, with `10.0` meaning
    ///   a 10% increase and `-10.0` a 10% decrease. Execution rejects shocks
    ///   whose converted absolute values overflow.
    pub fn with_percentages(
        node_id: impl Into<String>,
        period_id: PeriodId,
        base_value: f64,
        pct_range: Vec<f64>,
    ) -> Self {
        let perturbations = pct_range
            .into_iter()
            .map(|pct| base_value * (1.0 + pct / 100.0))
            .collect();

        Self {
            node_id: node_id.into(),
            period_id,
            base_value,
            perturbations,
        }
    }
}

/// Sensitivity analysis mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SensitivityMode {
    /// One-at-a-time parameter variations
    Diagonal,

    /// Full factorial grid
    FullGrid,

    /// Ranked by impact magnitude
    Tornado,
}

/// Sensitivity analysis configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SensitivityConfig {
    /// Analysis mode
    pub mode: SensitivityMode,

    /// Parameters to vary
    pub parameters: Vec<ParameterSpec>,

    /// Target metrics to track
    pub target_metrics: Vec<String>,
}

impl SensitivityConfig {
    /// Create a new sensitivity configuration.
    ///
    /// # Arguments
    ///
    /// * `mode` - One-at-a-time, Cartesian-grid, or ranked one-at-a-time sweep.
    pub fn new(mode: SensitivityMode) -> Self {
        Self {
            mode,
            parameters: Vec::new(),
            target_metrics: Vec::new(),
        }
    }

    /// Add a parameter to vary.
    ///
    /// # Arguments
    ///
    /// * `param` - Node/forecast-period input specification with finite absolute
    ///   shocks; the pair must be distinct from existing parameters.
    pub fn add_parameter(&mut self, param: ParameterSpec) {
        self.parameters.push(param);
    }

    /// Add a target metric to track.
    ///
    /// # Arguments
    ///
    /// * `metric` - Exact result node identifier used to rank tornado scenarios.
    pub fn add_target_metric(&mut self, metric: impl Into<String>) {
        self.target_metrics.push(metric.into());
    }
}

/// Result of a single sensitivity scenario.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SensitivityScenario {
    /// Parameter values for this scenario keyed as `node_id@period_id`.
    pub parameter_values: IndexMap<String, f64>,

    /// Full evaluation results
    pub results: StatementResult,
}

/// Results of sensitivity analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SensitivityResult {
    /// Configuration used
    pub config: SensitivityConfig,

    /// All scenario results
    pub scenarios: Vec<SensitivityScenario>,

    /// Unperturbed baseline evaluation of the model (populated by every run).
    /// Used by tornado chart generation as the reference metric
    /// when a parameter's base value is not part of the perturbation grid.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub baseline: Option<StatementResult>,
}

impl SensitivityResult {
    /// Get scenarios count.
    pub fn len(&self) -> usize {
        self.scenarios.len()
    }

    /// Check if empty.
    pub fn is_empty(&self) -> bool {
        self.scenarios.is_empty()
    }
}
