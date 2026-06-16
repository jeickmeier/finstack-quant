//! Monte Carlo evaluation types and aggregation utilities.

use crate::error::{Error, Result};
use crate::evaluator::results::EvalWarning;
use crate::types::FinancialModelSpec;
use finstack_quant_core::dates::PeriodId;
use finstack_quant_core::table::{TableColumn, TableColumnData, TableColumnRole, TableEnvelope};
use indexmap::IndexMap;
use serde_json::json;

/// Results for a single Monte Carlo path: per-node per-period values and any warnings emitted.
pub(crate) type PathResult = (IndexMap<String, IndexMap<PeriodId, f64>>, Vec<EvalWarning>);
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Configuration for Monte Carlo evaluation of a statement model.
///
/// # Choosing the number of paths
///
/// The required path count depends on the quantities of interest:
///
/// | Use case | Recommended `n_paths` |
/// |---|---|
/// | Mean / median estimates | 1 000 – 2 000 |
/// | 5th / 95th percentiles | 5 000 – 10 000 |
/// | 1st / 99th percentiles or CVaR | 10 000 – 50 000 |
/// | Breach-probability estimates | 10 000+ |
///
/// Standard-error of a percentile estimate scales as
/// $O\bigl(1/\sqrt{n}\bigr)$, so tails require proportionally more
/// paths to converge. When in doubt, run two simulations with
/// different seeds and compare results; if the metric of interest
/// moves by more than the desired precision, increase `n_paths`.
///
/// The default constructor ([`MonteCarloConfig::new`]) does **not** impose a
/// minimum—callers choose the path count explicitly so the trade-off between
/// accuracy and runtime is visible.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MonteCarloConfig {
    /// Number of Monte Carlo paths to simulate.
    pub n_paths: usize,
    /// Base random seed used to derive per-path seeds.
    pub seed: u64,
    /// Percentiles to compute in the closed interval [0, 1].
    #[serde(default)]
    pub percentiles: Vec<f64>,
    /// Whether to include the long-format path table in serialized results.
    #[serde(default)]
    pub include_path_data: bool,
}

impl MonteCarloConfig {
    /// Create a new Monte Carlo configuration with default percentiles.
    pub fn new(n_paths: usize, seed: u64) -> Self {
        Self {
            n_paths,
            seed,
            percentiles: vec![0.05, 0.5, 0.95],
            include_path_data: false,
        }
    }

    /// Override the percentiles to compute.
    #[must_use]
    pub fn with_percentiles(mut self, percentiles: Vec<f64>) -> Self {
        self.percentiles = percentiles;
        self
    }

    /// Include the long-format path table in results.
    #[must_use]
    pub fn with_path_data(mut self, include_path_data: bool) -> Self {
        self.include_path_data = include_path_data;
        self
    }
}

/// Per-metric percentile time series.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PercentileSeries {
    /// Metric / node identifier.
    pub metric: String,
    /// Period → ordered list of `(percentile, value)` pairs.
    pub values: IndexMap<PeriodId, Vec<(f64, f64)>>,
}

/// Monte Carlo results for a statement model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonteCarloResults {
    /// Aggregated percentile results: `metric → PercentileSeries`.
    pub percentile_results: IndexMap<String, PercentileSeries>,
    /// Number of Monte Carlo paths simulated.
    pub n_paths: usize,
    /// Percentiles computed for each metric/period.
    pub percentiles: Vec<f64>,
    /// Forecast (non-actual) periods included in the simulation.
    pub forecast_periods: Vec<PeriodId>,
    /// Warnings encountered while evaluating Monte Carlo paths.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<EvalWarning>,
    /// Internal storage of path-level values keyed by `(path_id, value)` so
    /// per-path aggregations (e.g. breach probability) remain correct
    /// regardless of parallel merge order.
    #[serde(skip)]
    pub(crate) path_values: IndexMap<String, IndexMap<PeriodId, Vec<(u32, f64)>>>,
    /// Optional full path data in long-format table form.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path_data: Option<TableEnvelope>,
}

impl MonteCarloResults {
    /// Get a time series of a specific percentile for a metric.
    pub fn get_percentile_series(
        &self,
        metric: &str,
        percentile: f64,
    ) -> Option<IndexMap<PeriodId, f64>> {
        let series = self.percentile_results.get(metric)?;
        let mut out = IndexMap::new();

        for (period, pairs) in &series.values {
            if let Some((_, value)) = pairs.iter().find(|(q, _)| (*q - percentile).abs() < 1e-12) {
                out.insert(*period, *value);
            }
        }

        if out.is_empty() {
            None
        } else {
            Some(out)
        }
    }

    /// Estimate the probability that a metric exceeds a threshold in any forecast period.
    ///
    /// This checks **upside** breaches only (`value > threshold`). For downside
    /// breaches (e.g. DSCR falling below a floor), negate both the metric values
    /// and the threshold, or use a derived metric that flips the sign.
    ///
    /// Returns `None` if the metric has no data, no forecast periods, or if any
    /// period's path vector is shorter than `n_paths` (incomplete simulation).
    pub fn breach_probability(&self, metric: &str, threshold: f64) -> Option<f64> {
        let metric_map = self.path_values.get(metric)?;
        if metric_map.is_empty() || self.n_paths == 0 {
            return None;
        }

        let forecast_set: HashSet<PeriodId> = self.forecast_periods.iter().copied().collect();
        let period_vectors: Vec<&Vec<(u32, f64)>> = metric_map
            .iter()
            .filter(|(p, _)| forecast_set.contains(p))
            .map(|(_, v)| v)
            .collect();

        if period_vectors.is_empty() {
            return None;
        }

        if period_vectors.iter().any(|v| v.len() < self.n_paths) {
            return None;
        }

        // Group by path_id (robust to rayon-reduce ordering) and mark a path
        // as breached if any forecast period exceeds the threshold.
        let mut breached: std::collections::HashMap<u32, bool> =
            std::collections::HashMap::with_capacity(self.n_paths);
        for period_values in &period_vectors {
            for (path_id, value) in period_values.iter() {
                let entry = breached.entry(*path_id).or_insert(false);
                if *value > threshold {
                    *entry = true;
                }
            }
        }

        if breached.len() < self.n_paths {
            return None;
        }

        let breached_paths = breached.values().filter(|b| **b).count();
        Some(breached_paths as f64 / self.n_paths as f64)
    }
}

fn normalize_percentiles(raw: &[f64]) -> Result<Vec<f64>> {
    if raw.is_empty() {
        return Ok(vec![0.05, 0.5, 0.95]);
    }

    // Reject out-of-range values instead of silently clamping: a user writing
    // `5` for the 5th percentile would otherwise get the maximum.
    for &q in raw {
        if !q.is_finite() || !(0.0..=1.0).contains(&q) {
            return Err(Error::eval(format!(
                "Monte Carlo percentiles must be in [0, 1] (e.g. 0.05 for the \
                 5th percentile), got {q}"
            )));
        }
    }

    let mut v: Vec<f64> = raw.to_vec();
    v.sort_by(|a, b| a.total_cmp(b));
    v.dedup();
    Ok(v)
}

pub(crate) struct MonteCarloAccumulator {
    expected_paths: usize,
    observed_paths: usize,
    percentiles: Vec<f64>,
    forecast_periods: Vec<PeriodId>,
    forecast_set: HashSet<PeriodId>,
    path_values: IndexMap<String, IndexMap<PeriodId, Vec<(u32, f64)>>>,
    include_path_data: bool,
    warnings: Vec<EvalWarning>,
    path_ids: Vec<u32>,
    periods: Vec<String>,
    metrics: Vec<String>,
    values: Vec<f64>,
}

impl MonteCarloAccumulator {
    pub(crate) fn new(model: &FinancialModelSpec, config: &MonteCarloConfig) -> Result<Self> {
        let forecast_periods: Vec<PeriodId> = model
            .periods
            .iter()
            .filter(|p| !p.is_actual)
            .map(|p| p.id)
            .collect();

        if forecast_periods.is_empty() {
            return Err(Error::eval(
                "Monte Carlo evaluation requires at least one forecast period. \
                 Use .periods(range, Some(actuals_cutoff)) to define forecast periods.",
            ));
        }

        Ok(Self {
            expected_paths: config.n_paths,
            observed_paths: 0,
            percentiles: normalize_percentiles(&config.percentiles)?,
            forecast_set: forecast_periods.iter().copied().collect(),
            forecast_periods,
            path_values: IndexMap::new(),
            include_path_data: config.include_path_data,
            warnings: Vec::new(),
            path_ids: Vec::new(),
            periods: Vec::new(),
            metrics: Vec::new(),
            values: Vec::new(),
        })
    }

    pub(crate) fn push_path(
        &mut self,
        path_idx: usize,
        path_results: IndexMap<String, IndexMap<PeriodId, f64>>,
        warnings: Vec<EvalWarning>,
    ) -> Result<()> {
        self.observed_paths += 1;
        self.warnings.extend(warnings);

        for (metric, period_map) in path_results {
            let metric_entry = self.path_values.entry(metric.clone()).or_default();
            for (period_id, value) in period_map {
                if !self.forecast_set.contains(&period_id) {
                    continue;
                }
                if !value.is_finite() {
                    return Err(Error::eval(format!(
                        "Monte Carlo aggregation encountered non-finite value for metric '{}' in period {}",
                        metric, period_id
                    )));
                }
                metric_entry
                    .entry(period_id)
                    .or_default()
                    .push((path_idx as u32, value));
                if self.include_path_data {
                    self.path_ids.push(path_idx as u32);
                    self.periods.push(period_id.to_string());
                    self.metrics.push(metric.clone());
                    self.values.push(value);
                }
            }
        }

        Ok(())
    }

    pub(crate) fn finish(mut self) -> Result<MonteCarloResults> {
        if self.observed_paths == 0 {
            return Err(Error::eval(
                "Monte Carlo aggregation requires at least one path",
            ));
        }
        if self.observed_paths != self.expected_paths {
            return Err(Error::eval(format!(
                "Monte Carlo aggregation mismatch: expected {} paths, got {}",
                self.expected_paths, self.observed_paths
            )));
        }

        let mut percentile_results: IndexMap<String, PercentileSeries> = IndexMap::new();
        for (metric, period_map) in &self.path_values {
            let mut series = PercentileSeries {
                metric: metric.clone(),
                values: IndexMap::new(),
            };

            for (period_id, values) in period_map {
                if values.is_empty() {
                    continue;
                }

                let mut sorted: Vec<f64> = values.iter().map(|(_, v)| *v).collect();
                sorted.sort_by(|a, b| a.total_cmp(b));
                let len = sorted.len();
                let mut pairs: Vec<(f64, f64)> = Vec::with_capacity(self.percentiles.len());
                for &q in &self.percentiles {
                    let index = q * (len.saturating_sub(1) as f64);
                    let lower = index.floor() as usize;
                    let upper = index.ceil() as usize;
                    let value = if lower == upper {
                        sorted[lower]
                    } else {
                        let weight = index - lower as f64;
                        sorted[lower] * (1.0 - weight) + sorted[upper] * weight
                    };
                    pairs.push((q, value));
                }

                series.values.insert(*period_id, pairs);
            }

            percentile_results.insert(metric.clone(), series);
        }

        // Rayon's reduce tree makes the merge order of partitions
        // nondeterministic, so sort serialized outputs (path-table rows and
        // warnings) into a canonical order: serial ≡ parallel must hold for
        // everything that reaches the wire.
        self.warnings
            .sort_by_cached_key(|warning| format!("{warning:?}"));

        let path_data = if !self.include_path_data || self.path_ids.is_empty() {
            None
        } else {
            let mut order: Vec<usize> = (0..self.path_ids.len()).collect();
            order.sort_by(|&a, &b| {
                (self.path_ids[a], &self.metrics[a], &self.periods[a]).cmp(&(
                    self.path_ids[b],
                    &self.metrics[b],
                    &self.periods[b],
                ))
            });
            self.path_ids = order.iter().map(|&i| self.path_ids[i]).collect();
            self.periods = order.iter().map(|&i| self.periods[i].clone()).collect();
            self.metrics = order.iter().map(|&i| self.metrics[i].clone()).collect();
            self.values = order.iter().map(|&i| self.values[i]).collect();
            let mut metadata = IndexMap::new();
            metadata.insert("layout".to_string(), json!("long"));
            metadata.insert("source".to_string(), json!("statement_monte_carlo_paths"));
            Some(TableEnvelope::new_with_metadata(
                vec![
                    TableColumn::new("path_id", TableColumnData::UInt32(self.path_ids))
                        .with_role(TableColumnRole::Dimension),
                    TableColumn::new("period", TableColumnData::String(self.periods))
                        .with_role(TableColumnRole::Index),
                    TableColumn::new("metric", TableColumnData::String(self.metrics))
                        .with_role(TableColumnRole::Dimension),
                    TableColumn::new("value", TableColumnData::Float64(self.values))
                        .with_role(TableColumnRole::Measure),
                ],
                metadata,
            )?)
        };

        Ok(MonteCarloResults {
            percentile_results,
            n_paths: self.observed_paths,
            percentiles: self.percentiles,
            forecast_periods: self.forecast_periods,
            warnings: self.warnings,
            path_values: self.path_values,
            path_data,
        })
    }

    // Only the rayon `try_reduce` path calls `merge`; the wasm32 serial
    // accumulator folds in place, so this is dead code there.
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    pub(crate) fn merge(mut self, other: Self) -> Result<Self> {
        if self.expected_paths != other.expected_paths
            || self.percentiles != other.percentiles
            || self.forecast_periods != other.forecast_periods
            || self.include_path_data != other.include_path_data
        {
            return Err(Error::eval(
                "Monte Carlo accumulator merge mismatch across parallel partitions",
            ));
        }

        self.observed_paths += other.observed_paths;
        self.warnings.extend(other.warnings);

        for (metric, period_map) in other.path_values {
            let target_metric = self.path_values.entry(metric).or_default();
            for (period_id, values) in period_map {
                target_metric.entry(period_id).or_default().extend(values);
            }
        }

        self.path_ids.extend(other.path_ids);
        self.periods.extend(other.periods);
        self.metrics.extend(other.metrics);
        self.values.extend(other.values);

        Ok(self)
    }

    pub(crate) fn empty_like(&self) -> Self {
        Self {
            expected_paths: self.expected_paths,
            observed_paths: 0,
            percentiles: self.percentiles.clone(),
            forecast_periods: self.forecast_periods.clone(),
            forecast_set: self.forecast_set.clone(),
            path_values: IndexMap::new(),
            include_path_data: self.include_path_data,
            warnings: Vec::new(),
            path_ids: Vec::new(),
            periods: Vec::new(),
            metrics: Vec::new(),
            values: Vec::new(),
        }
    }
}

/// Aggregate path-level results into [`MonteCarloResults`].
#[cfg(test)]
pub(crate) fn aggregate_monte_carlo_paths(
    model: &FinancialModelSpec,
    config: &MonteCarloConfig,
    all_paths: &[PathResult],
) -> Result<MonteCarloResults> {
    let mut accumulator = MonteCarloAccumulator::new(model, config)?;
    for (path_idx, (path_results, warnings)) in all_paths.iter().cloned().enumerate() {
        accumulator.push_path(path_idx, path_results, warnings)?;
    }
    accumulator.finish()
}

#[cfg(test)]
mod tests {
    use super::{
        aggregate_monte_carlo_paths, normalize_percentiles, MonteCarloAccumulator, MonteCarloConfig,
    };
    use crate::builder::ModelBuilder;
    use crate::evaluator::EvalWarning;
    use crate::types::AmountOrScalar;
    use finstack_quant_core::dates::PeriodId;
    use indexmap::IndexMap;

    #[test]
    fn normalize_percentiles_sorts_and_dedupes() {
        let raw = vec![0.95, 0.05, 0.5, 0.5];
        let norm = normalize_percentiles(&raw).expect("valid percentiles");
        assert_eq!(norm, vec![0.05, 0.5, 0.95]);
    }

    #[test]
    fn normalize_percentiles_rejects_out_of_range() {
        // A user writing `5` for the 5th percentile must get an error, not a
        // silent clamp to the maximum.
        assert!(normalize_percentiles(&[5.0]).is_err());
        assert!(normalize_percentiles(&[-0.1]).is_err());
        assert!(normalize_percentiles(&[f64::NAN]).is_err());
    }

    #[test]
    fn aggregate_rejects_non_finite_path_values() {
        let period = PeriodId::quarter(2025, 1);
        let model = ModelBuilder::new("mc-agg")
            .periods("2025Q1..Q1", None)
            .expect("valid periods")
            .value("revenue", &[(period, AmountOrScalar::scalar(100.0))])
            .build()
            .expect("valid model");
        let config = MonteCarloConfig::new(1, 7);

        let mut path = IndexMap::new();
        path.insert(
            "revenue".to_string(),
            [(period, f64::NAN)].into_iter().collect(),
        );

        let err = aggregate_monte_carlo_paths(&model, &config, &[(path, Vec::new())])
            .expect_err("NaN must fail");
        assert!(err.to_string().contains("non-finite"));
    }

    #[test]
    fn breach_probability_is_order_independent() {
        // Two paths; path 0 breaches in period 1, path 1 never breaches.
        // Expected probability: exactly 0.5 regardless of rayon-reduce order.
        let period = PeriodId::quarter(2025, 1);
        let model = ModelBuilder::new("mc-breach")
            .periods("2025Q1..Q1", None)
            .expect("valid periods")
            .value("revenue", &[(period, AmountOrScalar::scalar(100.0))])
            .build()
            .expect("valid model");
        let config = MonteCarloConfig::new(2, 7);

        for insertion_order in [[0usize, 1], [1, 0]] {
            let mut acc = MonteCarloAccumulator::new(&model, &config).expect("acc");
            let values = [200.0, 50.0];
            for &path_idx in &insertion_order {
                let mut path = IndexMap::new();
                path.insert(
                    "revenue".to_string(),
                    [(period, values[path_idx])].into_iter().collect(),
                );
                acc.push_path(path_idx, path, Vec::new())
                    .expect("push_path");
            }
            let results = acc.finish().expect("finish");
            let p = results
                .breach_probability("revenue", 100.0)
                .expect("breach");
            assert!(
                (p - 0.5).abs() < 1e-12,
                "breach probability must be 0.5 regardless of insertion order, got {p}"
            );
        }
    }

    #[test]
    fn accumulator_preserves_warnings_for_valid_paths() {
        let period = PeriodId::quarter(2025, 1);
        let model = ModelBuilder::new("mc-agg")
            .periods("2025Q1..Q1", None)
            .expect("valid periods")
            .value("revenue", &[(period, AmountOrScalar::scalar(100.0))])
            .build()
            .expect("valid model");
        let config = MonteCarloConfig::new(1, 7);
        let mut accumulator = MonteCarloAccumulator::new(&model, &config).expect("accumulator");

        let mut path = IndexMap::new();
        path.insert(
            "revenue".to_string(),
            [(period, 100.0)].into_iter().collect(),
        );
        accumulator
            .push_path(
                0,
                path,
                vec![EvalWarning::DivisionByZero {
                    node_id: "revenue".into(),
                    period,
                }],
            )
            .expect("path should be accepted");
        let results = accumulator.finish().expect("results should finish");
        assert_eq!(results.warnings.len(), 1);
    }

    #[test]
    fn monte_carlo_config_rejects_unknown_fields() {
        let json = r#"{"n_paths": 10, "seed": 7, "bogus_field": true}"#;
        assert!(serde_json::from_str::<MonteCarloConfig>(json).is_err());
    }

    #[test]
    fn path_data_rows_sorted_regardless_of_push_order() {
        // Rayon merge order is nondeterministic; finish() must emit a
        // canonical (path_id, metric, period) row order either way.
        let period = PeriodId::quarter(2025, 1);
        let model = ModelBuilder::new("mc-sort")
            .periods("2025Q1..Q1", None)
            .expect("valid periods")
            .value("revenue", &[(period, AmountOrScalar::scalar(100.0))])
            .build()
            .expect("valid model");
        let config = MonteCarloConfig::new(2, 7).with_path_data(true);

        let mut acc = MonteCarloAccumulator::new(&model, &config).expect("acc");
        for path_idx in [1usize, 0] {
            let mut path = IndexMap::new();
            path.insert(
                "revenue".to_string(),
                [(period, 100.0 + path_idx as f64)].into_iter().collect(),
            );
            acc.push_path(path_idx, path, Vec::new()).expect("push");
        }
        let results = acc.finish().expect("finish");
        let table = results.path_data.expect("path data");
        let path_id_column = table
            .columns
            .iter()
            .find(|c| c.name == "path_id")
            .expect("path_id column");
        match &path_id_column.data {
            finstack_quant_core::table::TableColumnData::UInt32(ids) => {
                assert_eq!(ids, &vec![0, 1], "rows must be sorted by path_id");
            }
            other => panic!("unexpected column type: {other:?}"),
        }
    }

    #[test]
    fn path_data_is_opt_in() {
        let period = PeriodId::quarter(2025, 1);
        let model = ModelBuilder::new("mc-path-data")
            .periods("2025Q1..Q1", None)
            .expect("valid periods")
            .value("revenue", &[(period, AmountOrScalar::scalar(100.0))])
            .build()
            .expect("valid model");

        let mut path = IndexMap::new();
        path.insert(
            "revenue".to_string(),
            [(period, 100.0)].into_iter().collect(),
        );

        let default_config = MonteCarloConfig::new(1, 7);
        let default_results =
            aggregate_monte_carlo_paths(&model, &default_config, &[(path.clone(), Vec::new())])
                .expect("default aggregation should finish");
        assert!(default_results.path_data.is_none());
        assert_eq!(
            default_results.breach_probability("revenue", 50.0),
            Some(1.0)
        );

        let opt_in_config = MonteCarloConfig::new(1, 7).with_path_data(true);
        let opt_in_results =
            aggregate_monte_carlo_paths(&model, &opt_in_config, &[(path, Vec::new())])
                .expect("opt-in aggregation should finish");
        assert!(opt_in_results.path_data.is_some());
    }
}
