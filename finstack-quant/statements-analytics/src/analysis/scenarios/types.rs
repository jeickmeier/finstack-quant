//! Sensitivity analysis types.

use finstack_quant_core::dates::PeriodId;
use finstack_quant_statements::evaluator::StatementResult;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

/// Entry in a tornado chart representing one parameter's impact on a metric.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub struct TornadoEntry {
    /// Parameter node identifier.
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
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub struct ParameterSpec {
    /// Node identifier
    pub node_id: String,

    /// Period to vary
    pub period_id: PeriodId,

    /// Base value
    pub base_value: f64,

    /// Perturbations to apply (e.g., [-10%, 0%, +10%])
    pub perturbations: Vec<f64>,
}

impl ParameterSpec {
    /// Create a new parameter specification.
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
    /// * `node_id` - Node identifier
    /// * `period_id` - Period to vary
    /// * `base_value` - Base value
    /// * `pct_range` - Percentage range (e.g., vec![-10.0, 0.0, 10.0] for ±10%)
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
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
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
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
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
    pub fn new(mode: SensitivityMode) -> Self {
        Self {
            mode,
            parameters: Vec::new(),
            target_metrics: Vec::new(),
        }
    }

    /// Add a parameter to vary.
    pub fn add_parameter(&mut self, param: ParameterSpec) {
        self.parameters.push(param);
    }

    /// Add a target metric to track.
    pub fn add_target_metric(&mut self, metric: impl Into<String>) {
        self.target_metrics.push(metric.into());
    }
}

/// Result of a single sensitivity scenario.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub struct SensitivityScenario {
    /// Parameter values for this scenario keyed as `node_id@period_id`.
    pub parameter_values: IndexMap<String, f64>,

    /// Full evaluation results
    pub results: StatementResult,
}

/// Results of sensitivity analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub struct SensitivityResult {
    /// Configuration used
    pub config: SensitivityConfig,

    /// All scenario results
    pub scenarios: Vec<SensitivityScenario>,

    /// Unperturbed baseline evaluation of the model (populated by tornado
    /// runs). Used by tornado chart generation as the reference metric
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

    fn scenario(
        &self,
        scenario_index: usize,
    ) -> finstack_quant_statements::Result<&SensitivityScenario> {
        self.scenarios.get(scenario_index).ok_or_else(|| {
            finstack_quant_statements::Error::invalid_input(format!(
                "scenario index {scenario_index} out of range: the result holds {} scenarios",
                self.scenarios.len()
            ))
        })
    }

    /// Value a parameter took in one scenario.
    ///
    /// # Arguments
    ///
    /// * `scenario_index` - Zero-based position of the scenario in
    ///   [`scenarios`](Self::scenarios).
    /// * `parameter` - Parameter key as `node_id@period_id`, e.g.
    ///   `"revenue@2025Q1"`.
    ///
    /// # Returns
    ///
    /// The perturbed parameter value in the node's own units, or `None` when
    /// the scenario did not vary that parameter.
    ///
    /// # Errors
    ///
    /// Returns an invalid-input error when `scenario_index` is out of range.
    pub fn get_parameter_value(
        &self,
        scenario_index: usize,
        parameter: &str,
    ) -> finstack_quant_statements::Result<Option<f64>> {
        Ok(self
            .scenario(scenario_index)?
            .parameter_values
            .get(parameter)
            .copied())
    }

    /// Evaluated node value in one scenario.
    ///
    /// # Arguments
    ///
    /// * `scenario_index` - Zero-based position of the scenario in
    ///   [`scenarios`](Self::scenarios).
    /// * `node_id` - Node identifier to read from the scenario's results.
    /// * `period` - Period to read.
    ///
    /// # Returns
    ///
    /// The node value in its own units, or `None` when the node or period is
    /// absent from the scenario's results.
    ///
    /// # Errors
    ///
    /// Returns an invalid-input error when `scenario_index` is out of range.
    pub fn get_value(
        &self,
        scenario_index: usize,
        node_id: &str,
        period: &PeriodId,
    ) -> finstack_quant_statements::Result<Option<f64>> {
        Ok(self.scenario(scenario_index)?.results.get(node_id, period))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn result_with_one_scenario() -> SensitivityResult {
        let period = PeriodId::quarter(2025, 1).expect("valid period fixture");
        let mut results = StatementResult::default();
        results
            .nodes
            .entry("profit".to_string())
            .or_default()
            .insert(period, 42.0);
        SensitivityResult {
            config: SensitivityConfig::new(SensitivityMode::Diagonal),
            scenarios: vec![SensitivityScenario {
                parameter_values: IndexMap::from([("revenue@2025Q1".to_string(), 110.0)]),
                results,
            }],
            baseline: None,
        }
    }

    #[test]
    fn scenario_accessors_read_parameters_and_values() {
        let result = result_with_one_scenario();
        let period = PeriodId::quarter(2025, 1).expect("valid period fixture");

        assert_eq!(
            result
                .get_parameter_value(0, "revenue@2025Q1")
                .expect("in range"),
            Some(110.0)
        );
        assert_eq!(
            result
                .get_parameter_value(0, "cogs@2025Q1")
                .expect("in range"),
            None
        );
        assert_eq!(
            result.get_value(0, "profit", &period).expect("in range"),
            Some(42.0)
        );
        assert_eq!(
            result.get_value(0, "missing", &period).expect("in range"),
            None
        );
    }

    #[test]
    fn scenario_accessors_reject_an_out_of_range_index() {
        let result = result_with_one_scenario();
        let period = PeriodId::quarter(2025, 1).expect("valid period fixture");

        let error = result
            .get_parameter_value(1, "revenue@2025Q1")
            .expect_err("out of range");
        assert!(error.to_string().contains("scenario index 1 out of range"));
        assert!(result.get_value(3, "profit", &period).is_err());
    }
}
