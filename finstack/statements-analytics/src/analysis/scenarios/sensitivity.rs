//! Sensitivity analysis engine.

use super::types::{
    ParameterSpec, SensitivityConfig, SensitivityMode, SensitivityResult, SensitivityScenario,
    TornadoEntry,
};
use finstack_core::dates::PeriodId;
use finstack_statements::error::{Error, Result};
use finstack_statements::evaluator::{Evaluator, StatementResult};
use finstack_statements::types::{AmountOrScalar, FinancialModelSpec};
use indexmap::IndexMap;

/// Sensitivity analyzer for financial models.
///
/// # Examples
///
/// ```rust
/// use finstack_statements::prelude::*;
/// use finstack_statements_analytics::analysis::{SensitivityAnalyzer, SensitivityConfig, SensitivityMode, ParameterSpec};
///
/// # fn main() -> Result<()> {
/// let model = ModelBuilder::new("sensitivity_test")
///     .periods("2025Q1..Q2", None)?
///     .value("revenue", &[
///         (PeriodId::quarter(2025, 1), AmountOrScalar::scalar(100_000.0)),
///         (PeriodId::quarter(2025, 2), AmountOrScalar::scalar(110_000.0)),
///     ])
///     .compute("cogs", "revenue * 0.6")?
///     .compute("gross_profit", "revenue - cogs")?
///     .build()?;
///
/// let analyzer = SensitivityAnalyzer::new(&model);
/// let mut config = SensitivityConfig::new(SensitivityMode::Diagonal);
///
/// // Define parameter to vary
/// config.add_parameter(ParameterSpec::with_percentages(
///     "revenue",
///     PeriodId::quarter(2025, 1),
///     100_000.0,
///     vec![-10.0, 0.0, 10.0],
/// ));
///
/// // Define target metric to observe
/// config.add_target_metric("gross_profit");
///
/// // Run sensitivity analysis
/// let result = analyzer.run(&config)?;
/// assert_eq!(result.scenarios.len(), 3); // One for each perturbation
/// # Ok(())
/// # }
/// ```
pub struct SensitivityAnalyzer<'a> {
    model: &'a FinancialModelSpec,
}

impl<'a> SensitivityAnalyzer<'a> {
    /// Create a new sensitivity analyzer.
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
    pub fn run(&self, config: &SensitivityConfig) -> Result<SensitivityResult> {
        match config.mode {
            // Diagonal runs in parallel on native targets; wasm32 has no
            // rayon thread pool, so fall back to the serial path.
            #[cfg(not(target_arch = "wasm32"))]
            SensitivityMode::Diagonal => self.run_diagonal_parallel(config),
            #[cfg(target_arch = "wasm32")]
            SensitivityMode::Diagonal => self.run_diagonal(config),
            SensitivityMode::FullGrid => self.run_full_grid(config),
            SensitivityMode::Tornado => self.run_tornado(config),
        }
    }

    /// Run diagonal sensitivity (one-at-a-time).
    fn run_diagonal(&self, config: &SensitivityConfig) -> Result<SensitivityResult> {
        let mut evaluator = Evaluator::new();
        let prepared = evaluator.prepare(self.model)?;
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

        let scenarios: Result<Vec<SensitivityScenario>> = work
            .par_iter()
            .map(|(param, perturbation)| {
                // Each worker owns its own model and evaluator — no shared
                // mutable state, no restore bookkeeping needed because the
                // local model is dropped at the end of the closure.
                let mut local_model = self.model.clone();
                let mut local_evaluator = Evaluator::new();
                let prepared = local_evaluator.prepare(&local_model)?;

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
        })
    }

    /// Run full grid sensitivity (factorial).
    fn run_full_grid(&self, config: &SensitivityConfig) -> Result<SensitivityResult> {
        if config.parameters.is_empty() {
            return Err(Error::invalid_input(
                "Full grid sensitivity requires at least one parameter",
            ));
        }

        let mut evaluator = Evaluator::new();
        let prepared = evaluator.prepare(self.model)?;

        let mut combinations = Vec::new();
        let mut current = Vec::new();
        build_parameter_grid(&config.parameters, 0, &mut current, &mut combinations);

        let mut model_clone = self.model.clone();
        let mut scenarios = Vec::with_capacity(combinations.len());
        for combination in &combinations {
            let mut parameter_values = IndexMap::new();

            for (param_idx, &perturbation) in combination.iter().enumerate() {
                let param = &config.parameters[param_idx];
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

            for (param_idx, _) in combination.iter().enumerate().rev() {
                let param = &config.parameters[param_idx];
                self.restore_parameter(&mut model_clone, &param.node_id, param.period_id)?;
            }

            scenarios.push(SensitivityScenario {
                parameter_values,
                results,
            });
        }

        Ok(SensitivityResult {
            config: config.clone(),
            scenarios,
        })
    }

    /// Run tornado sensitivity.
    fn run_tornado(&self, config: &SensitivityConfig) -> Result<SensitivityResult> {
        let mut result = self.run_diagonal(config)?;
        if config.target_metrics.is_empty() {
            return Ok(result);
        }

        let mut baseline_evaluator = Evaluator::new();
        let baseline = baseline_evaluator.evaluate(self.model)?;
        result.scenarios.sort_by(|lhs, rhs| {
            let rhs_impact = max_target_impact(&baseline, &rhs.results, &config.target_metrics);
            let lhs_impact = max_target_impact(&baseline, &lhs.results, &config.target_metrics);
            descending_f64(lhs_impact, rhs_impact)
        });
        Ok(result)
    }

    fn apply_parameter_override(
        &self,
        model: &mut FinancialModelSpec,
        node_id: &str,
        period_id: finstack_core::dates::PeriodId,
        value: f64,
    ) -> Result<()> {
        // Refuse to bump historical actuals: "what if revenue was 5%
        // higher" is a forecast statement, not a rewrite of recorded
        // history. This mirrors `scenario_set::apply_overrides`, which
        // filters to `!p.is_actual` forecast periods, and prevents
        // sensitivity analyses from silently corrupting reported data.
        let is_actual = self
            .model
            .periods
            .iter()
            .find(|p| p.id == period_id)
            .map(|p| p.is_actual)
            .unwrap_or(false);
        if is_actual {
            return Err(Error::invalid_input(format!(
                "Cannot override parameter '{}' for actual (historical) period '{}'; \
                 sensitivities may only bump forecast periods",
                node_id, period_id
            )));
        }

        if let Some(node) = model.nodes.get_mut(node_id) {
            let mut values = node.values.clone().unwrap_or_default();
            values.insert(period_id, AmountOrScalar::scalar(value));
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
        period_id: finstack_core::dates::PeriodId,
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

fn scenario_parameter_key(node_id: &str, period_id: finstack_core::dates::PeriodId) -> String {
    format!("{}@{}", node_id, period_id)
}

fn build_parameter_grid(
    parameters: &[super::types::ParameterSpec],
    idx: usize,
    current: &mut Vec<f64>,
    combinations: &mut Vec<Vec<f64>>,
) {
    if idx == parameters.len() {
        combinations.push(current.clone());
        return;
    }

    for perturbation in &parameters[idx].perturbations {
        current.push(*perturbation);
        build_parameter_grid(parameters, idx + 1, current, combinations);
        current.pop();
    }
}

fn max_target_impact(
    baseline: &finstack_statements::evaluator::StatementResult,
    scenario: &finstack_statements::evaluator::StatementResult,
    target_metrics: &[String],
) -> f64 {
    target_metrics
        .iter()
        .flat_map(|metric| {
            baseline
                .nodes
                .get(metric)
                .into_iter()
                .flat_map(move |baseline_periods| {
                    baseline_periods
                        .iter()
                        .filter_map(move |(period_id, baseline_value)| {
                            scenario
                                .get(metric, period_id)
                                .map(|scenario_value| (scenario_value - baseline_value).abs())
                                .filter(|delta| !delta.is_nan())
                        })
                })
        })
        .fold(0.0, f64::max)
}

fn descending_f64(lhs: f64, rhs: f64) -> std::cmp::Ordering {
    let lhs = if lhs.is_nan() { f64::NEG_INFINITY } else { lhs };
    let rhs = if rhs.is_nan() { f64::NEG_INFINITY } else { rhs };
    rhs.total_cmp(&lhs)
}

// ── Tornado chart generation ──

/// Generate tornado chart entries for a specific metric from sensitivity results.
///
/// Each entry represents one parameter's downside and upside impact on the
/// target metric relative to its baseline value. Entries are sorted by
/// descending absolute swing magnitude.
///
/// # Arguments
///
/// * `result`      - Completed sensitivity analysis result.
/// * `metric_node` - Node identifier for the metric to inspect.
/// * `period_hint` - Optional period to look up; if `None`, uses the first
///   available period for the node.
pub fn generate_tornado_entries(
    result: &SensitivityResult,
    metric_node: &str,
    period_hint: Option<PeriodId>,
) -> Vec<TornadoEntry> {
    let mut entries = Vec::new();

    for param in &result.config.parameters {
        if let Some(entry) = build_tornado_entry(result, param, metric_node, period_hint) {
            entries.push(entry);
        }
    }

    entries.sort_by(|a, b| descending_f64(a.swing().abs(), b.swing().abs()));

    entries
}

fn build_tornado_entry(
    result: &SensitivityResult,
    param: &ParameterSpec,
    metric_node: &str,
    period_hint: Option<PeriodId>,
) -> Option<TornadoEntry> {
    let parameter_key = scenario_parameter_key(&param.node_id, param.period_id);
    let mut min_record: Option<(f64, f64)> = None;
    let mut max_record: Option<(f64, f64)> = None;
    let mut baseline_metric = None;

    for scenario in &result.scenarios {
        let Some(param_value) = scenario.parameter_values.get(&parameter_key) else {
            continue;
        };
        let metric_value = extract_metric_value(&scenario.results, metric_node, period_hint)?;

        if approx_equal(*param_value, param.base_value) {
            baseline_metric = Some(metric_value);
        }

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

    let base = baseline_metric
        .or_else(|| min_record.map(|(_, value)| value))
        .or_else(|| max_record.map(|(_, value)| value))?;

    let downside = min_record.map(|(_, value)| value - base).unwrap_or(0.0);
    let upside = max_record.map(|(_, value)| value - base).unwrap_or(0.0);

    Some(TornadoEntry {
        parameter_id: param.node_id.clone(),
        downside,
        upside,
    })
}

fn extract_metric_value(
    results: &StatementResult,
    node_id: &str,
    period_hint: Option<PeriodId>,
) -> Option<f64> {
    if let Some(period) = period_hint {
        results.get(node_id, &period)
    } else {
        results
            .nodes
            .get(node_id)
            .and_then(|periods| periods.values().next().copied())
    }
}

fn approx_equal(lhs: f64, rhs: f64) -> bool {
    let scale = lhs.abs().max(rhs.abs()).max(1.0);
    (lhs - rhs).abs() <= 1e-9 * scale
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::scenarios::types::ParameterSpec;
    use finstack_core::dates::PeriodId;
    use finstack_statements::builder::ModelBuilder;

    #[test]
    fn test_diagonal_sensitivity() {
        let period = PeriodId::quarter(2025, 1);
        let period2 = PeriodId::quarter(2025, 2);
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
        let period = PeriodId::quarter(2025, 1);
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
        let period = PeriodId::quarter(2025, 1);
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
    fn test_full_grid_scenario_metadata_distinguishes_period_overrides() {
        let period1 = PeriodId::quarter(2025, 1);
        let period2 = PeriodId::quarter(2025, 2);
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
    fn test_generate_tornado_entries_sorts_nan_swings_last() {
        let period = PeriodId::quarter(2025, 1);
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

        let result = SensitivityResult { config, scenarios };
        let entries = generate_tornado_entries(&result, &metric, Some(period));

        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].parameter_id, "revenue");
        assert_eq!(entries[1].parameter_id, "cogs");
    }

    #[test]
    fn test_max_target_impact_preserves_infinite_deltas() {
        let period = PeriodId::quarter(2025, 1);
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

        let impact = max_target_impact(&baseline, &scenario, &["gross_profit".to_string()]);

        assert!(impact.is_infinite());
        assert!(impact.is_sign_positive());
    }

    #[test]
    fn test_descending_f64_orders_infinite_before_finite_and_nan_last() {
        let mut values = [10.0, f64::INFINITY, f64::NAN];

        values.sort_by(|lhs, rhs| descending_f64(*lhs, *rhs));

        assert!(values[0].is_infinite());
        assert_eq!(values[1], 10.0);
        assert!(values[2].is_nan());
    }
}
