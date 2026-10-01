//! Sensitivity analysis engine.

use super::types::{
    ParameterSpec, SensitivityConfig, SensitivityMode, SensitivityResult, SensitivityScenario,
    TornadoEntry,
};
use finstack_quant_core::dates::PeriodId;
use finstack_quant_statements::error::{Error, Result};
use finstack_quant_statements::evaluator::{Evaluator, StatementResult};
use finstack_quant_statements::types::{AmountOrScalar, FinancialModelSpec, NodeValueType};
use indexmap::IndexMap;

/// Sensitivity analyzer for financial models.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_statements::prelude::*;
/// use finstack_quant_statements_analytics::analysis::{SensitivityAnalyzer, SensitivityConfig, SensitivityMode, ParameterSpec};
///
/// # fn main() -> Result<()> {
/// let model = ModelBuilder::new("sensitivity_test")
///     .periods("2025Q1..Q2", None)?
///     .value("revenue", &[
///         (PeriodId::quarter(2025, 1).expect("valid period fixture"), AmountOrScalar::scalar(100_000.0)),
///         (PeriodId::quarter(2025, 2).expect("valid period fixture"), AmountOrScalar::scalar(110_000.0)),
///     ])
///     .compute("cogs", "revenue * 0.6")?
///     .compute("gross_profit", "revenue - cogs")?
///     .build()?;
///
/// let analyzer = SensitivityAnalyzer::new(&model);
/// let mut config = SensitivityConfig::new(SensitivityMode::Diagonal);
///
/// config.add_parameter(ParameterSpec::with_percentages(
///     "revenue",
///     PeriodId::quarter(2025, 1).expect("valid period fixture"),
///     100_000.0,
///     vec![-10.0, 0.0, 10.0],
/// ));
/// config.add_target_metric("gross_profit");
///
/// let result = analyzer.run(&config)?;
/// assert_eq!(result.scenarios.len(), 3);
/// # Ok(())
/// # }
/// ```
pub struct SensitivityAnalyzer<'a> {
    model: &'a FinancialModelSpec,
}

const MAX_PARAMETERS: usize = 128;
const MAX_SCENARIOS: usize = 10_000;
const MAX_EVALUATION_CELLS: usize = 10_000_000;

impl<'a> SensitivityAnalyzer<'a> {
    /// Create a new sensitivity analyzer.
    ///
    /// # Arguments
    ///
    /// * `model` - Baseline statement model; sweeps vary forecast inputs on
    ///   owned copies and leave this model unchanged.
    pub fn new(model: &'a FinancialModelSpec) -> Self {
        Self { model }
    }

    /// Run sensitivity analysis.
    ///
    /// # Parallelism
    ///
    /// On native targets, diagonal sensitivity runs each
    /// `(parameter, perturbation)` pair concurrently via Rayon. Each worker
    /// holds its own cloned `FinancialModelSpec` and `Evaluator`. Results
    /// match the serial path bit-for-bit given the same seed and model.
    ///
    /// WebAssembly builds use the serial path (no Rayon thread pool).
    ///
    /// Full-grid and tornado modes remain serial.
    ///
    /// # Errors
    ///
    /// Propagates invalid sensitivity configuration, parameter perturbation,
    /// model evaluation, and result-collection errors from the selected mode.
    /// Requests are limited to 128 distinct node/period parameters, 10,000
    /// scenarios, and 10 million model node/period cells across all evaluations
    /// (including the unperturbed baseline). Limits are checked before preparing
    /// or evaluating the model.
    ///
    /// # Arguments
    ///
    /// * `config` - Non-empty parameter sweep with finite base values and
    ///   absolute perturbation values. Each parameter must name a distinct
    ///   node/forecast-period pair with at least one perturbation.
    pub fn run(&self, config: &SensitivityConfig) -> Result<SensitivityResult> {
        let scenario_count = self.validate_config(config)?;
        match config.mode {
            // Diagonal runs in parallel on native targets; wasm32 has no
            // rayon thread pool, so fall back to the serial path.
            #[cfg(not(target_arch = "wasm32"))]
            SensitivityMode::Diagonal => self.run_diagonal_parallel(config),
            #[cfg(target_arch = "wasm32")]
            SensitivityMode::Diagonal => self.run_diagonal(config),
            SensitivityMode::FullGrid => self.run_full_grid(config, scenario_count),
            SensitivityMode::Tornado => self.run_tornado(config),
        }
    }

    fn validate_config(&self, config: &SensitivityConfig) -> Result<usize> {
        if config.parameters.is_empty() || config.parameters.len() > MAX_PARAMETERS {
            return Err(Error::invalid_input(format!(
                "Sensitivity requires 1..={MAX_PARAMETERS} parameters"
            )));
        }
        let mut count = if config.mode == SensitivityMode::FullGrid {
            1usize
        } else {
            0usize
        };
        let mut seen = std::collections::HashSet::new();
        for parameter in &config.parameters {
            if !seen.insert((parameter.node_id.as_str(), parameter.period_id)) {
                return Err(Error::invalid_input(
                    "Sensitivity parameters must name distinct node/period pairs",
                ));
            }
            if !parameter.base_value.is_finite()
                || parameter.perturbations.is_empty()
                || parameter
                    .perturbations
                    .iter()
                    .any(|value| !value.is_finite())
            {
                return Err(Error::invalid_input("Sensitivity base values and perturbations must be finite, with at least one perturbation per parameter"));
            }
            count = if config.mode == SensitivityMode::FullGrid {
                count.checked_mul(parameter.perturbations.len())
            } else {
                count.checked_add(parameter.perturbations.len())
            }
            .ok_or_else(|| Error::invalid_input("Sensitivity scenario count overflow"))?;
            if count > MAX_SCENARIOS {
                return Err(Error::invalid_input(format!(
                    "Sensitivity exceeds {MAX_SCENARIOS} scenarios"
                )));
            }
        }
        let cells = count
            .checked_add(1)
            .and_then(|evaluations| evaluations.checked_mul(self.model.nodes.len()))
            .and_then(|node_evaluations| node_evaluations.checked_mul(self.model.periods.len()))
            .ok_or_else(|| Error::invalid_input("Sensitivity evaluation size overflow"))?;
        if cells > MAX_EVALUATION_CELLS {
            return Err(Error::invalid_input(format!(
                "Sensitivity exceeds {MAX_EVALUATION_CELLS} model node/period evaluation cells"
            )));
        }
        Ok(count)
    }

    /// Run diagonal sensitivity (one-at-a-time).
    fn run_diagonal(&self, config: &SensitivityConfig) -> Result<SensitivityResult> {
        let mut evaluator = Evaluator::new();
        let prepared = evaluator.prepare(self.model)?;
        let baseline = evaluator.evaluate_prepared(self.model, &prepared)?;
        let mut model_clone = self.model.clone();
        let mut scenarios = Vec::new();

        for param in &config.parameters {
            for perturbation in &param.perturbations {
                self.apply_parameter_override(
                    &mut model_clone,
                    &param.node_id,
                    param.period_id,
                    *perturbation,
                )?;

                let results = match evaluator.evaluate_prepared(&model_clone, &prepared) {
                    Ok(r) => r,
                    Err(e) => {
                        // Restore model state before propagating the error
                        let _ = self.restore_parameter(
                            &mut model_clone,
                            &param.node_id,
                            param.period_id,
                        );
                        return Err(e);
                    }
                };

                self.restore_parameter(&mut model_clone, &param.node_id, param.period_id)?;

                let mut parameter_values = IndexMap::new();
                parameter_values.insert(
                    scenario_parameter_key(&param.node_id, param.period_id),
                    *perturbation,
                );

                scenarios.push(SensitivityScenario {
                    parameter_values,
                    results,
                });
            }
        }

        Ok(SensitivityResult {
            config: config.clone(),
            scenarios,
            baseline: Some(baseline),
        })
    }

    /// Parallel diagonal sensitivity using rayon.
    ///
    /// Each `(parameter, perturbation)` pair runs on its own worker with
    /// an independently-cloned model and evaluator, so there is no shared
    /// mutable state and no need for the serial path's apply/restore
    /// bookkeeping. Ordering is preserved to match the serial output.
    #[cfg(not(target_arch = "wasm32"))]
    fn run_diagonal_parallel(&self, config: &SensitivityConfig) -> Result<SensitivityResult> {
        use rayon::prelude::*;

        // Flatten (param, perturbation) into a work list so the parallel
        // iterator can dispatch uniformly and collect in original order.
        let work: Vec<(&ParameterSpec, f64)> = config
            .parameters
            .iter()
            .flat_map(|param| {
                param
                    .perturbations
                    .iter()
                    .map(move |perturbation| (param, *perturbation))
            })
            .collect();

        // Compile the model structure ONCE: `prepare` runs DAG construction, cycle
        // detection, and formula compilation, all identical across perturbations
        // (which change values, not topology). Workers clone the prepared evaluator
        // — cheap, since the compiled-formula cache is `Arc`-shared — instead of each
        // re-running `prepare`, and share the read-only evaluation plan.
        let mut base_evaluator = Evaluator::new();
        let prepared = base_evaluator.prepare(self.model)?;
        let baseline = base_evaluator.evaluate_prepared(self.model, &prepared)?;

        let scenarios: Result<Vec<SensitivityScenario>> = work
            .par_iter()
            .map(|(param, perturbation)| {
                // Each worker owns its own model and evaluator clone — no shared
                // mutable state, no restore bookkeeping needed because the local
                // model is dropped at the end of the closure.
                let mut local_model = self.model.clone();
                let mut local_evaluator = base_evaluator.clone();

                self.apply_parameter_override(
                    &mut local_model,
                    &param.node_id,
                    param.period_id,
                    *perturbation,
                )?;

                let results = local_evaluator.evaluate_prepared(&local_model, &prepared)?;

                let mut parameter_values = IndexMap::new();
                parameter_values.insert(
                    scenario_parameter_key(&param.node_id, param.period_id),
                    *perturbation,
                );

                Ok(SensitivityScenario {
                    parameter_values,
                    results,
                })
            })
            .collect();

        Ok(SensitivityResult {
            config: config.clone(),
            scenarios: scenarios?,
            baseline: Some(baseline),
        })
    }

    /// Run full grid sensitivity (factorial).
    fn run_full_grid(
        &self,
        config: &SensitivityConfig,
        scenario_count: usize,
    ) -> Result<SensitivityResult> {
        let mut evaluator = Evaluator::new();
        let prepared = evaluator.prepare(self.model)?;
        let baseline = evaluator.evaluate_prepared(self.model, &prepared)?;

        let mut model_clone = self.model.clone();
        let mut scenarios = Vec::with_capacity(scenario_count);
        let mut indices = vec![0usize; config.parameters.len()];
        for _ in 0..scenario_count {
            let mut parameter_values = IndexMap::new();

            for (param, &index) in config.parameters.iter().zip(&indices) {
                let perturbation = param.perturbations[index];
                self.apply_parameter_override(
                    &mut model_clone,
                    &param.node_id,
                    param.period_id,
                    perturbation,
                )?;
                parameter_values.insert(
                    scenario_parameter_key(&param.node_id, param.period_id),
                    perturbation,
                );
            }

            let results = evaluator.evaluate_prepared(&model_clone, &prepared)?;

            scenarios.push(SensitivityScenario {
                parameter_values,
                results,
            });
            // Mixed-radix enumeration preserves the original Cartesian order
            // while storing only one index per parameter.
            for (index, parameter) in indices.iter_mut().zip(&config.parameters).rev() {
                *index += 1;
                if *index < parameter.perturbations.len() {
                    break;
                }
                *index = 0;
            }
        }

        Ok(SensitivityResult {
            config: config.clone(),
            scenarios,
            baseline: Some(baseline),
        })
    }

    /// Run tornado sensitivity.
    fn run_tornado(&self, config: &SensitivityConfig) -> Result<SensitivityResult> {
        let mut result = self.run_diagonal(config)?;
        if config.target_metrics.is_empty() {
            return Ok(result);
        }

        let baseline = result
            .baseline
            .as_ref()
            .ok_or_else(|| Error::eval("Missing sensitivity baseline"))?;
        let mut ranked: Vec<_> = std::mem::take(&mut result.scenarios)
            .into_iter()
            .map(|scenario| {
                let impact =
                    max_target_impact(baseline, &scenario.results, &config.target_metrics)?;
                Ok((impact, scenario))
            })
            .collect::<Result<_>>()?;
        ranked.sort_by(|(lhs, _), (rhs, _)| descending_f64(*lhs, *rhs));
        result.scenarios = ranked.into_iter().map(|(_, scenario)| scenario).collect();
        Ok(result)
    }

    fn apply_parameter_override(
        &self,
        model: &mut FinancialModelSpec,
        node_id: &str,
        period_id: finstack_quant_core::dates::PeriodId,
        value: f64,
    ) -> Result<()> {
        // Refuse to bump historical actuals: "what if revenue was 5%
        // higher" is a forecast statement, not a rewrite of recorded
        // history. This mirrors `scenario_set::apply_overrides`, which
        // filters to `!p.is_actual` forecast periods, and prevents
        // sensitivity analyses from silently corrupting reported data.
        let Some(period) = self.model.periods.iter().find(|p| p.id == period_id) else {
            // A period outside the model grid would silently produce a
            // no-op scenario (the inserted value is never evaluated).
            return Err(Error::invalid_input(format!(
                "Cannot override parameter '{}': period '{}' is not part of the model's \
                 period grid",
                node_id, period_id
            )));
        };
        if period.is_actual {
            return Err(Error::invalid_input(format!(
                "Cannot override parameter '{}' for actual (historical) period '{}'; \
                 sensitivities may only bump forecast periods",
                node_id, period_id
            )));
        }

        if let Some(node) = model.nodes.get_mut(node_id) {
            let typed_value = match node.value_type {
                Some(NodeValueType::Monetary { currency }) => {
                    AmountOrScalar::amount(value, currency)?
                }
                Some(NodeValueType::Scalar) => AmountOrScalar::scalar(value),
                None => {
                    return Err(Error::invalid_input(format!(
                        "Cannot override parameter '{node_id}' without a declared or inferred \
                         value_type"
                    )));
                }
            };
            let mut values = node.values.clone().unwrap_or_default();
            values.insert(period_id, typed_value);
            node.values = Some(values);
            Ok(())
        } else {
            Err(Error::invalid_input(format!(
                "Node '{}' not found",
                node_id
            )))
        }
    }

    /// Restore a node's value for a specific period from the original model.
    fn restore_parameter(
        &self,
        model: &mut FinancialModelSpec,
        node_id: &str,
        period_id: finstack_quant_core::dates::PeriodId,
    ) -> Result<()> {
        let original_value = self
            .model
            .nodes
            .get(node_id)
            .and_then(|n| n.values.as_ref())
            .and_then(|v| v.get(&period_id))
            .cloned();

        if let Some(node) = model.nodes.get_mut(node_id) {
            if let Some(values) = node.values.as_mut() {
                if let Some(orig) = original_value {
                    values.insert(period_id, orig);
                } else {
                    values.swap_remove(&period_id);
                }
            }
            Ok(())
        } else {
            Err(Error::invalid_input(format!(
                "Node '{}' not found",
                node_id
            )))
        }
    }
}

fn scenario_parameter_key(
    node_id: &str,
    period_id: finstack_quant_core::dates::PeriodId,
) -> String {
    format!("{}@{}", node_id, period_id)
}

fn max_target_impact(
    baseline: &finstack_quant_statements::evaluator::StatementResult,
    scenario: &finstack_quant_statements::evaluator::StatementResult,
    target_metrics: &[String],
) -> Result<f64> {
    let mut impact = 0.0_f64;
    for metric in target_metrics {
        let periods = baseline
            .nodes
            .get(metric)
            .filter(|periods| !periods.is_empty())
            .ok_or_else(|| Error::invalid_input(format!("Missing tornado metric '{metric}'")))?;
        for period in periods.keys() {
            let baseline_value = extract_metric_value(baseline, metric, *period)?;
            let scenario_value = extract_metric_value(scenario, metric, *period)?;
            let delta = (scenario_value - baseline_value).abs();
            if !delta.is_finite() {
                return Err(Error::invalid_input(
                    "Tornado metric impacts must be finite",
                ));
            }
            impact = impact.max(delta);
        }
    }
    Ok(impact)
}

/// Descending comparator that sorts `NaN` last.
///
/// Shared by every tornado-style ranking in the crate so entry ordering
/// cannot drift between the sensitivity sweep and the DCF sensitivity.
pub(crate) fn descending_f64(lhs: f64, rhs: f64) -> std::cmp::Ordering {
    let lhs = if lhs.is_nan() { f64::NEG_INFINITY } else { lhs };
    let rhs = if rhs.is_nan() { f64::NEG_INFINITY } else { rhs };
    rhs.total_cmp(&lhs)
}

/// Generate tornado chart entries for a specific metric from sensitivity results.
///
/// Each entry represents one parameter's downside and upside impact on the
/// target metric relative to its baseline value. Entries are sorted by
/// descending absolute swing magnitude.
/// Parameter identifiers preserve `node@period` so shocks to different
/// periods of the same node remain distinguishable.
/// Only one-at-a-time diagonal/tornado results are supported. Full-grid
/// scenarios combine parameters and cannot identify individual impacts.
///
/// # Errors
///
/// Returns an invalid-input error for full-grid results, scenarios that do not
/// identify exactly one perturbed parameter, an absent baseline, a missing
/// target metric/period, or non-finite metric values and impacts.
///
/// # Arguments
///
/// * `result`      - Completed sensitivity analysis result.
/// * `metric_node` - Node identifier for the metric to inspect.
/// * `period_hint` - Optional period to look up; if `None`, uses the first
///   available baseline period for the node in every scenario.
pub fn generate_tornado_entries(
    result: &SensitivityResult,
    metric_node: &str,
    period_hint: Option<PeriodId>,
) -> Result<Vec<TornadoEntry>> {
    if result.config.mode == SensitivityMode::FullGrid {
        return Err(Error::invalid_input("Tornado entries require diagonal or tornado sensitivity results; full-grid scenarios confound parameter impacts"));
    }
    if result
        .scenarios
        .iter()
        .any(|scenario| scenario.parameter_values.len() != 1)
    {
        return Err(Error::invalid_input(
            "Tornado scenarios must identify exactly one perturbed parameter",
        ));
    }
    let baseline = result.baseline.as_ref().ok_or_else(|| {
        Error::invalid_input("Tornado entries require the unperturbed sensitivity baseline")
    })?;
    let period = period_hint
        .or_else(|| baseline.nodes.get(metric_node)?.keys().next().copied())
        .ok_or_else(|| Error::invalid_input(format!("Missing tornado metric '{metric_node}'")))?;
    let base = extract_metric_value(baseline, metric_node, period)?;
    let mut entries = Vec::new();

    for param in &result.config.parameters {
        if let Some(entry) = build_tornado_entry(result, param, metric_node, period, base)? {
            entries.push(entry);
        }
    }

    entries.sort_by(|a, b| descending_f64(a.swing().abs(), b.swing().abs()));

    Ok(entries)
}

fn build_tornado_entry(
    result: &SensitivityResult,
    param: &ParameterSpec,
    metric_node: &str,
    period: PeriodId,
    base: f64,
) -> Result<Option<TornadoEntry>> {
    let parameter_key = scenario_parameter_key(&param.node_id, param.period_id);
    let mut min_record: Option<(f64, f64)> = None;
    let mut max_record: Option<(f64, f64)> = None;

    for scenario in &result.scenarios {
        let Some(param_value) = scenario.parameter_values.get(&parameter_key) else {
            continue;
        };
        if !param_value.is_finite() {
            return Err(Error::invalid_input(
                "Tornado parameter values must be finite",
            ));
        }
        let metric_value = extract_metric_value(&scenario.results, metric_node, period)?;

        match &mut min_record {
            Some((current_value, current_metric)) => {
                if *param_value < *current_value {
                    *current_value = *param_value;
                    *current_metric = metric_value;
                }
            }
            None => {
                min_record = Some((*param_value, metric_value));
            }
        }

        match &mut max_record {
            Some((current_value, current_metric)) => {
                if *param_value > *current_value {
                    *current_value = *param_value;
                    *current_metric = metric_value;
                }
            }
            None => {
                max_record = Some((*param_value, metric_value));
            }
        }
    }

    let (Some((_, minimum)), Some((_, maximum))) = (min_record, max_record) else {
        return Ok(None);
    };
    let downside = minimum - base;
    let upside = maximum - base;
    if !downside.is_finite() || !upside.is_finite() || !(upside - downside).is_finite() {
        return Err(Error::invalid_input(
            "Tornado metric impacts must be finite",
        ));
    }

    Ok(Some(TornadoEntry {
        parameter_id: parameter_key,
        downside,
        upside,
    }))
}

fn extract_metric_value(results: &StatementResult, node_id: &str, period: PeriodId) -> Result<f64> {
    let value = results.get(node_id, &period).ok_or_else(|| {
        Error::invalid_input(format!(
            "Missing tornado metric '{node_id}' in period '{period}'"
        ))
    })?;
    if !value.is_finite() {
        return Err(Error::invalid_input(format!(
            "Tornado metric '{node_id}' in period '{period}' must be finite"
        )));
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::scenarios::types::ParameterSpec;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::PeriodId;
    use finstack_quant_core::money::Money;
    use finstack_quant_statements::builder::ModelBuilder;

    #[test]
    fn test_diagonal_sensitivity() {
        let period = PeriodId::quarter(2025, 1).expect("valid period fixture");
        let period2 = PeriodId::quarter(2025, 2).expect("valid period fixture");
        let model = ModelBuilder::new("test")
            .periods("2025Q1..Q2", None)
            .expect("valid period range")
            .value(
                "revenue",
                &[
                    (period, AmountOrScalar::scalar(100_000.0)),
                    (period2, AmountOrScalar::scalar(100_000.0)),
                ],
            )
            .compute("cogs", "revenue * 0.4")
            .expect("valid formula")
            .compute("gross_profit", "revenue - cogs")
            .expect("valid formula")
            .build()
            .expect("valid model");

        let analyzer = SensitivityAnalyzer::new(&model);

        let mut config = SensitivityConfig::new(SensitivityMode::Diagonal);
        config.add_parameter(ParameterSpec::with_percentages(
            "revenue",
            period,
            100_000.0,
            vec![-10.0, 0.0, 10.0],
        ));
        config.add_target_metric("gross_profit");

        let result = analyzer
            .run(&config)
            .expect("sensitivity analysis should succeed");
        assert_eq!(result.scenarios.len(), 3); // 3 perturbations
    }

    #[test]
    fn test_full_grid_sensitivity_builds_cartesian_product() {
        let period = PeriodId::quarter(2025, 1).expect("valid period fixture");
        let model = ModelBuilder::new("test")
            .periods("2025Q1..Q1", None)
            .expect("valid period range")
            .value("revenue", &[(period, AmountOrScalar::scalar(100_000.0))])
            .value("cogs", &[(period, AmountOrScalar::scalar(40_000.0))])
            .compute("gross_profit", "revenue - cogs")
            .expect("valid formula")
            .build()
            .expect("valid model");

        let analyzer = SensitivityAnalyzer::new(&model);
        let mut config = SensitivityConfig::new(SensitivityMode::FullGrid);
        config.add_parameter(ParameterSpec::new(
            "revenue",
            period,
            100_000.0,
            vec![90_000.0, 110_000.0],
        ));
        config.add_parameter(ParameterSpec::new(
            "cogs",
            period,
            40_000.0,
            vec![35_000.0, 45_000.0],
        ));
        config.add_target_metric("gross_profit");

        let result = analyzer.run(&config).expect("full grid should succeed");
        assert_eq!(result.scenarios.len(), 4);
        assert!(result
            .scenarios
            .iter()
            .all(|scenario| scenario.parameter_values.len() == 2));
    }

    #[test]
    fn test_tornado_orders_scenarios_by_target_metric_impact() {
        let period = PeriodId::quarter(2025, 1).expect("valid period fixture");
        let model = ModelBuilder::new("test")
            .periods("2025Q1..Q1", None)
            .expect("valid period range")
            .value("revenue", &[(period, AmountOrScalar::scalar(100_000.0))])
            .value("cogs", &[(period, AmountOrScalar::scalar(40_000.0))])
            .compute("gross_profit", "revenue - cogs")
            .expect("valid formula")
            .build()
            .expect("valid model");

        let analyzer = SensitivityAnalyzer::new(&model);
        let mut config = SensitivityConfig::new(SensitivityMode::Tornado);
        config.add_parameter(ParameterSpec::new(
            "revenue",
            period,
            100_000.0,
            vec![80_000.0, 120_000.0],
        ));
        config.add_parameter(ParameterSpec::new(
            "cogs",
            period,
            40_000.0,
            vec![30_000.0, 50_000.0],
        ));
        config.add_target_metric("gross_profit");

        let result = analyzer.run(&config).expect("tornado should succeed");
        assert_eq!(result.scenarios.len(), 4);
        assert_eq!(
            result.scenarios[0].parameter_values.keys().next(),
            Some(&"revenue@2025Q1".to_string())
        );
        assert_eq!(
            result.scenarios[1].parameter_values.keys().next(),
            Some(&"revenue@2025Q1".to_string())
        );
    }

    #[test]
    fn tornado_run_rejects_missing_requested_metric() {
        let period = PeriodId::quarter(2025, 1).expect("period");
        let model = ModelBuilder::new("missing-target")
            .periods("2025Q1..Q1", None)
            .expect("periods")
            .value("revenue", &[(period, AmountOrScalar::scalar(100.0))])
            .build()
            .expect("model");
        let mut config = SensitivityConfig::new(SensitivityMode::Tornado);
        config.add_parameter(ParameterSpec::new("revenue", period, 100.0, vec![90.0]));
        config.add_target_metric("missing");
        assert!(SensitivityAnalyzer::new(&model)
            .run(&config)
            .expect_err("missing target")
            .to_string()
            .contains("Missing tornado metric"));
    }

    #[test]
    fn tornado_run_rejects_overflowing_impact_from_finite_values() {
        let period = PeriodId::quarter(2025, 1).expect("period");
        let model = ModelBuilder::new("overflowing-impact")
            .periods("2025Q1..Q1", None)
            .expect("periods")
            .value("revenue", &[(period, AmountOrScalar::scalar(-f64::MAX))])
            .build()
            .expect("model");
        let mut config = SensitivityConfig::new(SensitivityMode::Tornado);
        config.add_parameter(ParameterSpec::new(
            "revenue",
            period,
            -f64::MAX,
            vec![f64::MAX],
        ));
        config.add_target_metric("revenue");
        assert!(SensitivityAnalyzer::new(&model)
            .run(&config)
            .expect_err("overflowing impact")
            .to_string()
            .contains("impacts must be finite"));
    }

    #[test]
    fn test_full_grid_scenario_metadata_distinguishes_period_overrides() {
        let period1 = PeriodId::quarter(2025, 1).expect("valid period fixture");
        let period2 = PeriodId::quarter(2025, 2).expect("valid period fixture");
        let model = ModelBuilder::new("test")
            .periods("2025Q1..Q2", None)
            .expect("valid period range")
            .value(
                "revenue",
                &[
                    (period1, AmountOrScalar::scalar(100_000.0)),
                    (period2, AmountOrScalar::scalar(110_000.0)),
                ],
            )
            .build()
            .expect("valid model");

        let analyzer = SensitivityAnalyzer::new(&model);
        let mut config = SensitivityConfig::new(SensitivityMode::FullGrid);
        config.add_parameter(ParameterSpec::new(
            "revenue",
            period1,
            100_000.0,
            vec![90_000.0, 110_000.0],
        ));
        config.add_parameter(ParameterSpec::new(
            "revenue",
            period2,
            110_000.0,
            vec![100_000.0, 120_000.0],
        ));

        let result = analyzer.run(&config).expect("full grid should succeed");
        assert_eq!(result.scenarios.len(), 4);
        assert!(result.scenarios.iter().all(|scenario| {
            scenario.parameter_values.contains_key("revenue@2025Q1")
                && scenario.parameter_values.contains_key("revenue@2025Q2")
        }));
    }

    #[test]
    fn test_generate_tornado_entries_rejects_nonfinite_target() {
        let period = PeriodId::quarter(2025, 1).expect("valid period fixture");
        let metric = "gross_profit".to_string();
        let mut config = SensitivityConfig::new(SensitivityMode::Tornado);
        config.add_parameter(ParameterSpec::new(
            "revenue",
            period,
            100.0,
            vec![90.0, 110.0],
        ));
        config.add_parameter(ParameterSpec::new("cogs", period, 40.0, vec![30.0, 50.0]));
        config.add_target_metric(metric.clone());

        let make_results = |value: f64| {
            let mut results = StatementResult::new();
            results
                .nodes
                .entry(metric.clone())
                .or_default()
                .insert(period, value);
            results
        };

        let scenarios = vec![
            SensitivityScenario {
                parameter_values: [("revenue@2025Q1".to_string(), 90.0)].into_iter().collect(),
                results: make_results(80.0),
            },
            SensitivityScenario {
                parameter_values: [("revenue@2025Q1".to_string(), 110.0)]
                    .into_iter()
                    .collect(),
                results: make_results(120.0),
            },
            SensitivityScenario {
                parameter_values: [("cogs@2025Q1".to_string(), 30.0)].into_iter().collect(),
                results: make_results(f64::NAN),
            },
            SensitivityScenario {
                parameter_values: [("cogs@2025Q1".to_string(), 50.0)].into_iter().collect(),
                results: make_results(f64::NAN),
            },
        ];

        let result = SensitivityResult {
            config,
            scenarios,
            baseline: Some(make_results(100.0)),
        };
        assert!(generate_tornado_entries(&result, &metric, Some(period)).is_err());
    }

    #[test]
    fn override_at_period_outside_model_grid_errors() {
        let period = PeriodId::quarter(2025, 1).expect("valid period fixture");
        let model = ModelBuilder::new("test")
            .periods("2025Q1..Q1", None)
            .expect("valid period range")
            .value("revenue", &[(period, AmountOrScalar::scalar(100_000.0))])
            .build()
            .expect("valid model");

        let analyzer = SensitivityAnalyzer::new(&model);
        let mut config = SensitivityConfig::new(SensitivityMode::Diagonal);
        // 2026Q1 is not in the model grid: previously a silent no-op scenario.
        config.add_parameter(ParameterSpec::new(
            "revenue",
            PeriodId::quarter(2026, 1).expect("valid period fixture"),
            100_000.0,
            vec![90_000.0, 110_000.0],
        ));

        let err = analyzer
            .run(&config)
            .expect_err("period outside the grid must error");
        assert!(
            err.to_string().contains("not part of the model"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn tornado_entries_anchor_on_true_baseline_when_base_not_in_grid() {
        let period = PeriodId::quarter(2025, 1).expect("valid period fixture");
        let model = ModelBuilder::new("test")
            .periods("2025Q1..Q1", None)
            .expect("valid period range")
            .value("revenue", &[(period, AmountOrScalar::scalar(100_000.0))])
            .compute("gross_profit", "revenue * 0.6")
            .expect("valid formula")
            .build()
            .expect("valid model");

        let analyzer = SensitivityAnalyzer::new(&model);
        let mut config = SensitivityConfig::new(SensitivityMode::Diagonal);
        // The base value (100k) is NOT one of the perturbations.
        config.add_parameter(ParameterSpec::new(
            "revenue",
            period,
            100_000.0,
            vec![90_000.0, 110_000.0],
        ));
        config.add_target_metric("gross_profit");

        let result = analyzer.run(&config).expect("tornado run");
        assert!(
            result.baseline.is_some(),
            "tornado runs must carry the unperturbed baseline"
        );

        let entries = generate_tornado_entries(&result, "gross_profit", Some(period))
            .expect("tornado entries");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].parameter_id, "revenue@2025Q1");
        // Baseline gross profit = 60k; downside 54k, upside 66k.
        assert!(
            (entries[0].downside - (-6_000.0)).abs() < 1e-9,
            "downside must anchor on the true baseline, got {}",
            entries[0].downside
        );
        assert!(
            (entries[0].upside - 6_000.0).abs() < 1e-9,
            "upside must anchor on the true baseline, got {}",
            entries[0].upside
        );
    }

    #[test]
    fn test_max_target_impact_rejects_nonfinite_and_overflowing_deltas() {
        let period = PeriodId::quarter(2025, 1).expect("valid period fixture");
        let metric = "gross_profit".to_string();

        let mut baseline = StatementResult::new();
        baseline
            .nodes
            .entry(metric.clone())
            .or_default()
            .insert(period, 100.0);

        let mut scenario = StatementResult::new();
        scenario
            .nodes
            .entry(metric)
            .or_default()
            .insert(period, f64::INFINITY);

        assert!(max_target_impact(&baseline, &scenario, &["gross_profit".to_string()]).is_err());
        baseline
            .nodes
            .get_mut("gross_profit")
            .expect("metric")
            .insert(period, -f64::MAX);
        scenario
            .nodes
            .get_mut("gross_profit")
            .expect("metric")
            .insert(period, f64::MAX);
        assert!(max_target_impact(&baseline, &scenario, &["gross_profit".to_string()]).is_err());
        scenario
            .nodes
            .get_mut("gross_profit")
            .expect("metric")
            .clear();
        assert!(max_target_impact(&baseline, &scenario, &["gross_profit".to_string()]).is_err());
    }

    #[test]
    fn test_descending_f64_orders_infinite_before_finite_and_nan_last() {
        let mut values = [10.0, f64::INFINITY, f64::NAN];

        values.sort_by(|lhs, rhs| descending_f64(*lhs, *rhs));

        assert!(values[0].is_infinite());
        assert_eq!(values[1], 10.0);
        assert!(values[2].is_nan());
    }

    #[test]
    fn monetary_sensitivity_preserves_node_currency() {
        let period = PeriodId::quarter(2025, 1).expect("valid period fixture");
        let model = ModelBuilder::new("money-sensitivity")
            .periods("2025Q1..Q1", None)
            .expect("periods")
            .value_money(
                "revenue",
                &[(period, Money::from((100_000_i64, Currency::USD)))],
            )
            .build()
            .expect("model");
        let analyzer = SensitivityAnalyzer::new(&model);
        let mut config = SensitivityConfig::new(SensitivityMode::Diagonal);
        config.add_parameter(ParameterSpec::new(
            "revenue",
            period,
            100_000.0,
            vec![110_000.0],
        ));
        config.add_target_metric("revenue");

        let result = analyzer.run(&config).expect("sensitivity");
        let shocked = result.scenarios[0]
            .results
            .get_money("revenue", &period)
            .expect("monetary result");
        assert_eq!(shocked.currency(), Currency::USD);
        assert_eq!(shocked.amount(), 110_000.0);
    }

    #[test]
    fn tornado_parameter_identifiers_distinguish_same_node_in_different_periods() {
        let first = PeriodId::quarter(2025, 1).expect("quarter");
        let second = PeriodId::quarter(2025, 2).expect("quarter");
        let model = ModelBuilder::new("period-identities")
            .periods("2025Q1..Q2", None)
            .expect("periods")
            .value(
                "revenue",
                &[
                    (first, AmountOrScalar::scalar(100.0)),
                    (second, AmountOrScalar::scalar(100.0)),
                ],
            )
            .build()
            .expect("model");
        let mut config = SensitivityConfig::new(SensitivityMode::Diagonal);
        for period in [first, second] {
            config.add_parameter(ParameterSpec::new(
                "revenue",
                period,
                100.0,
                vec![90.0, 110.0],
            ));
        }
        let result = SensitivityAnalyzer::new(&model)
            .run(&config)
            .expect("sensitivity");
        let entries = generate_tornado_entries(&result, "revenue", Some(first)).expect("entries");
        let ids: Vec<_> = entries
            .iter()
            .map(|entry| entry.parameter_id.as_str())
            .collect();
        assert_eq!(ids, ["revenue@2025Q1", "revenue@2025Q2"]);
    }

    #[test]
    fn rejects_oversized_cartesian_grid_before_preparing_model() {
        let period = PeriodId::quarter(2025, 1).expect("period");
        let model = ModelBuilder::new("limits")
            .periods("2025Q1..Q1", None)
            .expect("periods")
            .value("revenue", &[(period, AmountOrScalar::scalar(100.0))])
            .build()
            .expect("model");
        let mut config = SensitivityConfig::new(SensitivityMode::FullGrid);
        for index in 0..25 {
            // Missing nodes would fail during evaluation; the grid size must
            // fail first without constructing 2^25 combinations.
            config.add_parameter(ParameterSpec::new(
                format!("missing_{index}"),
                period,
                1.0,
                vec![0.0, 1.0],
            ));
        }
        let error = SensitivityAnalyzer::new(&model)
            .run(&config)
            .expect_err("oversized grid");
        assert!(error.to_string().contains("10000 scenarios"), "{error}");
    }

    #[test]
    fn rejects_oversized_diagonal_and_tornado_sweeps() {
        let period = PeriodId::quarter(2025, 1).expect("period");
        let model = ModelBuilder::new("limits")
            .periods("2025Q1..Q1", None)
            .expect("periods")
            .value("revenue", &[(period, AmountOrScalar::scalar(100.0))])
            .build()
            .expect("model");
        for mode in [SensitivityMode::Diagonal, SensitivityMode::Tornado] {
            let mut config = SensitivityConfig::new(mode);
            config.add_parameter(ParameterSpec::new(
                "revenue",
                period,
                100.0,
                vec![100.0; MAX_SCENARIOS + 1],
            ));
            assert!(SensitivityAnalyzer::new(&model)
                .run(&config)
                .expect_err("too many scenarios")
                .to_string()
                .contains("10000 scenarios"));
        }
    }

    #[test]
    fn bounds_work_for_large_models_even_when_scenario_count_is_small() {
        let period = PeriodId::quarter(2025, 1).expect("period");
        let mut builder = ModelBuilder::new("work-limit")
            .periods("2025Q1..Q1", None)
            .expect("periods");
        for index in 0..1001 {
            builder = builder.value(
                format!("node_{index}"),
                &[(period, AmountOrScalar::scalar(1.0))],
            );
        }
        let model = builder.build().expect("model");
        let mut config = SensitivityConfig::new(SensitivityMode::Diagonal);
        config.add_parameter(ParameterSpec::new(
            "node_0",
            period,
            1.0,
            vec![1.0; MAX_SCENARIOS],
        ));
        assert!(SensitivityAnalyzer::new(&model)
            .run(&config)
            .expect_err("too many evaluation cells")
            .to_string()
            .contains("evaluation cells"));
    }

    #[test]
    fn rejects_non_finite_and_duplicate_parameter_specs() {
        let period = PeriodId::quarter(2025, 1).expect("period");
        let model = ModelBuilder::new("parameters")
            .periods("2025Q1..Q1", None)
            .expect("periods")
            .value("revenue", &[(period, AmountOrScalar::scalar(100.0))])
            .build()
            .expect("model");
        let mut config = SensitivityConfig::new(SensitivityMode::Diagonal);
        config.add_parameter(ParameterSpec::new("revenue", period, 100.0, vec![f64::NAN]));
        assert!(SensitivityAnalyzer::new(&model).run(&config).is_err());
        config.parameters[0].perturbations = vec![90.0];
        config.add_parameter(config.parameters[0].clone());
        assert!(SensitivityAnalyzer::new(&model).run(&config).is_err());
    }

    #[test]
    fn full_grid_preserves_order_and_baseline_but_rejects_tornado_generation() {
        let period = PeriodId::quarter(2025, 1).expect("period");
        let model = ModelBuilder::new("grid")
            .periods("2025Q1..Q1", None)
            .expect("periods")
            .value("revenue", &[(period, AmountOrScalar::scalar(100.0))])
            .value("cost", &[(period, AmountOrScalar::scalar(20.0))])
            .compute("profit", "revenue - cost")
            .expect("formula")
            .build()
            .expect("model");
        let mut config = SensitivityConfig::new(SensitivityMode::FullGrid);
        config.add_parameter(ParameterSpec::new(
            "revenue",
            period,
            100.0,
            vec![90.0, 110.0],
        ));
        config.add_parameter(ParameterSpec::new("cost", period, 20.0, vec![10.0, 30.0]));
        let mut result = SensitivityAnalyzer::new(&model).run(&config).expect("grid");
        assert_eq!(
            result
                .baseline
                .as_ref()
                .expect("baseline")
                .get("profit", &period),
            Some(80.0)
        );
        let profits: Vec<f64> = result
            .scenarios
            .iter()
            .map(|scenario| scenario.results.get("profit", &period).expect("profit"))
            .collect();
        assert_eq!(profits, vec![80.0, 60.0, 100.0, 80.0]);
        assert!(generate_tornado_entries(&result, "profit", Some(period)).is_err());
        // Public JSON results can mislabel joint shocks as one-at-a-time runs.
        result.config.mode = SensitivityMode::Diagonal;
        let error = generate_tornado_entries(&result, "profit", Some(period))
            .expect_err("joint shocks must not masquerade as individual impacts");
        assert!(error.to_string().contains("exactly one"));
    }

    #[test]
    fn missing_baseline_is_rejected_instead_of_using_a_shocked_scenario() {
        let result = SensitivityResult {
            config: SensitivityConfig::new(SensitivityMode::Diagonal),
            scenarios: vec![],
            baseline: None,
        };
        assert!(generate_tornado_entries(&result, "profit", None).is_err());
    }

    #[test]
    fn tornado_generation_rejects_missing_targets_and_overflowing_impacts() {
        let period = PeriodId::quarter(2025, 1).expect("period");
        let later_period = PeriodId::quarter(2025, 2).expect("period");
        let make_result = |base: f64, minimum: f64, maximum: f64| {
            let make_statement = |value| {
                let mut statement = StatementResult::new();
                statement
                    .nodes
                    .entry("profit".to_string())
                    .or_default()
                    .insert(period, value);
                statement
            };
            let mut config = SensitivityConfig::new(SensitivityMode::Diagonal);
            config.add_parameter(ParameterSpec::new(
                "revenue",
                period,
                100.0,
                vec![90.0, 110.0],
            ));
            SensitivityResult {
                config,
                scenarios: [90.0, 110.0]
                    .into_iter()
                    .zip([minimum, maximum])
                    .map(|(parameter, metric)| SensitivityScenario {
                        parameter_values: [("revenue@2025Q1".to_string(), parameter)]
                            .into_iter()
                            .collect(),
                        results: make_statement(metric),
                    })
                    .collect(),
                baseline: Some(make_statement(base)),
            }
        };
        let mut result = make_result(100.0, 90.0, 110.0);
        assert!(generate_tornado_entries(&result, "missing", None).is_err());
        assert!(generate_tornado_entries(&result, "profit", Some(later_period)).is_err());
        result.scenarios[0]
            .results
            .nodes
            .get_mut("profit")
            .expect("metric")
            .clear();
        result.scenarios[0]
            .results
            .nodes
            .get_mut("profit")
            .expect("metric")
            .insert(later_period, 90.0);
        // An omitted period uses the baseline's first period consistently;
        // it must not compare different periods across scenarios.
        assert!(generate_tornado_entries(&result, "profit", None).is_err());

        for (base, minimum, maximum) in [
            (f64::NAN, 90.0, 110.0),
            (f64::INFINITY, 90.0, 110.0),
            (100.0, 90.0, f64::INFINITY),
            (-f64::MAX, f64::MAX, f64::MAX),
            (0.0, -f64::MAX, f64::MAX),
        ] {
            assert!(
                generate_tornado_entries(&make_result(base, minimum, maximum), "profit", None)
                    .is_err()
            );
        }
        result.scenarios.clear();
        assert!(generate_tornado_entries(&result, "profit", None)
            .expect("no varied parameter")
            .is_empty());
    }
}
