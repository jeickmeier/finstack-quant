//! Sensitivity analysis types.

use finstack_quant_core::dates::PeriodId;
use finstack_quant_statements::evaluator::StatementResult;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

/// Entry in a tornado chart representing one parameter's impact on a metric.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
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
///
/// The wire form carries exactly one of `perturbations` (absolute values) or
/// `pct_range` (percentage moves of `base_value`, converted on deserialization
/// as by [`Self::with_percentages`]); it always serializes the absolute values.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(try_from = "ParameterSpecWire")]
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

#[derive(Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
struct ParameterSpecWire {
    /// Node identifier
    node_id: String,
    /// Period to vary
    period_id: PeriodId,
    /// Base value in the node's units; `pct_range` moves are applied to it.
    base_value: f64,
    /// Absolute values to substitute for this node/period; give exactly one of
    /// `perturbations` and `pct_range`.
    #[serde(default)]
    perturbations: Option<Vec<f64>>,
    /// Percentage moves of `base_value` (`10.0` is a 10% increase), each
    /// substituted as `base_value * (1 + pct / 100)`; give exactly one of
    /// `perturbations` and `pct_range`.
    #[serde(default)]
    pct_range: Option<Vec<f64>>,
}

impl TryFrom<ParameterSpecWire> for ParameterSpec {
    type Error = finstack_quant_statements::Error;

    fn try_from(wire: ParameterSpecWire) -> finstack_quant_statements::Result<Self> {
        match (wire.perturbations, wire.pct_range) {
            (Some(perturbations), None) => Ok(Self::new(
                wire.node_id,
                wire.period_id,
                wire.base_value,
                perturbations,
            )),
            (None, Some(pct_range)) => {
                Self::with_percentages(wire.node_id, wire.period_id, wire.base_value, pct_range)
            }
            _ => Err(finstack_quant_statements::Error::invalid_input(format!(
                "Sensitivity parameter '{}@{}' takes exactly one of `perturbations` or `pct_range`",
                wire.node_id, wire.period_id
            ))),
        }
    }
}

impl ParameterSpec {
    /// Create a new parameter specification.
    ///
    /// # Arguments
    ///
    /// * `node_id` - Exact statement node identifier to vary.
    /// * `period_id` - Forecast period whose node input is replaced. The
    ///   replacement is a point override: it does not re-anchor the node's
    ///   forecast in later periods (see [`crate::analysis::run_sensitivity`]).
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
    ///   replaced; execution rejects actual periods. The replacement is a
    ///   point override that does not re-anchor the node's forecast in later
    ///   periods (see [`crate::analysis::run_sensitivity`]).
    /// * `base_value` - Finite reference input in the node's units, such as a
    ///   decimal rate or monetary currency amount; each shock multiplies this
    ///   value by `1 + pct / 100`. The model supplies the chart baseline.
    /// * `pct_range` - Finite percentage figures, with `10.0` meaning a 10%
    ///   increase and `-10.0` a 10% decrease; a sensitivity run needs at least
    ///   one.
    ///
    /// # Errors
    ///
    /// Returns an invalid-input error when `base_value` or a percentage is
    /// not finite (`NaN` or infinite), or when a converted shock overflows to
    /// a non-finite value.
    pub fn with_percentages(
        node_id: impl Into<String>,
        period_id: PeriodId,
        base_value: f64,
        pct_range: Vec<f64>,
    ) -> finstack_quant_statements::Result<Self> {
        let node_id = node_id.into();
        if !base_value.is_finite() {
            return Err(finstack_quant_statements::Error::invalid_input(format!(
                "Sensitivity base value for '{node_id}@{period_id}' must be finite, got {base_value}"
            )));
        }
        let perturbations = pct_range
            .into_iter()
            .map(|pct| {
                let value = base_value * (1.0 + pct / 100.0);
                if value.is_finite() {
                    Ok(value)
                } else {
                    Err(finstack_quant_statements::Error::invalid_input(format!(
                        "Sensitivity shock {pct}% of '{node_id}@{period_id}' must be finite"
                    )))
                }
            })
            .collect::<finstack_quant_statements::Result<_>>()?;

        Ok(Self {
            node_id,
            period_id,
            base_value,
            perturbations,
        })
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

    #[test]
    fn parameter_spec_wire_takes_absolute_or_percentage_perturbations() {
        let absolute: ParameterSpec = serde_json::from_value(serde_json::json!({
            "node_id": "revenue", "period_id": "2025Q2", "base_value": 100.0,
            "perturbations": [90.0, 110.0],
        }))
        .expect("absolute");
        assert_eq!(absolute.perturbations, vec![90.0, 110.0]);
        let percent: ParameterSpec = serde_json::from_value(serde_json::json!({
            "node_id": "revenue", "period_id": "2025Q2", "base_value": 100.0,
            "pct_range": [-10.0, 10.0],
        }))
        .expect("percent");
        let direct = ParameterSpec::with_percentages(
            "revenue",
            PeriodId::quarter(2025, 2).expect("valid period"),
            100.0,
            vec![-10.0, 10.0],
        )
        .expect("direct");
        assert_eq!(percent.perturbations, direct.perturbations);
        let written = serde_json::to_value(&percent).expect("serialize");
        assert_eq!(
            written["perturbations"],
            serde_json::json!(direct.perturbations)
        );
        assert!(written.get("pct_range").is_none());
    }

    #[test]
    fn parameter_spec_wire_rejects_both_neither_and_unknown_fields() {
        let base = serde_json::json!({
            "node_id": "revenue", "period_id": "2025Q2", "base_value": 100.0,
        });
        let with = |extra: serde_json::Value| {
            let mut value = base.clone();
            value
                .as_object_mut()
                .expect("object")
                .extend(extra.as_object().expect("object").clone());
            serde_json::from_value::<ParameterSpec>(value)
        };
        assert!(with(serde_json::json!({})).is_err());
        assert!(with(serde_json::json!({"perturbations": [1.0], "pct_range": [1.0]})).is_err());
        assert!(with(serde_json::json!({"perturbations": [1.0], "extra": 1})).is_err());
        assert!(with(serde_json::json!({"pct_range": [-10.0]})).is_ok());
    }
}
