//! Variance analysis for financial statement results.
//!
//! This module provides tools to compare two evaluated statement results
//! (e.g., management case vs bank base case, forecast vs actuals) and
//! produce structured variance outputs.
//!
//! Key features:
//! - **Absolute and percentage variance** between a baseline and comparison
//! - **Per-metric, per-period rows** suitable for tabular export
//! - **Bridge decomposition** helpers for driver-level attribution
//! - Optional serializable table export for bindings and reports
//!
//! # Examples
//!
//! Basic usage with two `StatementResult`:
//!
//! ```rust
//! use finstack_statements_analytics::analysis::{VarianceAnalyzer, VarianceConfig};
//! use finstack_statements::prelude::*;
//!
//! # fn main() -> finstack_statements::Result<()> {
//! // Build and evaluate a simple model twice (e.g., management vs bank case)
//! let model = ModelBuilder::new("demo")
//!     .periods("2025Q1..Q2", None)?
//!     .compute("revenue", "100000")?
//!     .compute("ebitda", "60000")?
//!     .build()?;
//!
//! let mut evaluator = Evaluator::new();
//! let mgmt_results = evaluator.evaluate(&model)?;
//! let bank_results = evaluator.evaluate(&model)?;
//!
//! let analyzer = VarianceAnalyzer::new(&mgmt_results, &bank_results);
//! let config = VarianceConfig::new(
//!     "management_case",
//!     "bank_case",
//!     vec!["revenue", "ebitda"],
//!     vec![PeriodId::quarter(2025, 1)],
//! );
//!
//! let report = analyzer.compute(&config)?;
//! assert_eq!(report.rows.len(), 2);
//! # Ok(())
//! # }
//! ```

use finstack_core::dates::PeriodId;
use finstack_core::math::ZERO_TOLERANCE;
use finstack_core::table::{TableColumn, TableColumnData, TableColumnRole, TableEnvelope};
use finstack_statements::error::{Error, Result};
use finstack_statements::evaluator::StatementResult;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::json;

/// Configuration for variance analysis between two `StatementResult`.
///
/// This is a lightweight configuration that mirrors the JSON example in the
/// design docs:
///
/// ```json
/// {
///   "variance_config": {
///     "baseline": "management_case",
///     "comparisons": ["bank_case", "actuals"],
///     "metrics": ["revenue", "ebitda", "free_cash_flow"],
///     "periods": ["2025Q1", "2025Q2"]
///   }
/// }
/// ```
///
/// The Rust implementation focuses on a single baseline / comparison pair.
/// Multi-comparison workflows can be built by running multiple analyzers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VarianceConfig {
    /// Human-readable name for the baseline (e.g. `"management_case"`).
    pub baseline_label: String,

    /// Human-readable name for the comparison (e.g. `"bank_case"`).
    pub comparison_label: String,

    /// Node identifiers to compare (e.g. `["revenue", "ebitda"]`).
    pub metrics: Vec<String>,

    /// Periods to include in the variance report.
    pub periods: Vec<PeriodId>,
}

impl VarianceConfig {
    /// Create a new variance configuration.
    pub fn new(
        baseline_label: impl Into<String>,
        comparison_label: impl Into<String>,
        metrics: Vec<impl Into<String>>,
        periods: Vec<PeriodId>,
    ) -> Self {
        Self {
            baseline_label: baseline_label.into(),
            comparison_label: comparison_label.into(),
            metrics: metrics.into_iter().map(|m| m.into()).collect(),
            periods,
        }
    }
}

/// One row of variance output.
///
/// Represents variance for a single `(metric, period)` pair.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VarianceRow {
    /// Period identifier (e.g. `"2025Q1"`).
    pub period: PeriodId,

    /// Metric / node identifier (e.g. `"revenue"`).
    pub metric: String,

    /// Baseline value.
    pub baseline: f64,

    /// Comparison value.
    pub comparison: f64,

    /// Absolute variance: `comparison - baseline`.
    pub abs_var: f64,

    /// Percentage variance (fraction): `abs_var / baseline`.
    ///
    /// When the baseline is effectively zero this is `None` (Options
    /// serialize as JSON `null`) rather than a spurious `0.0`, which
    /// previously hid meaningful absolute deltas whenever the
    /// denominator was small. Reporting layers should interpret a
    /// `None` as "undefined / not meaningful" and fall back to the
    /// absolute variance.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pct_var: Option<f64>,

    /// Optional driver breakdown: `driver → contribution` in the same units
    /// as the underlying metric.
    ///
    /// This is populated by bridge decomposition helpers and can be rendered
    /// as a "bridge" column in reports.
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    pub driver_contribution: IndexMap<String, f64>,
}

/// Full variance report between a baseline and comparison.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VarianceReport {
    /// Label for the baseline scenario (e.g. `"management_case"`).
    pub baseline_label: String,

    /// Label for the comparison scenario (e.g. `"bank_case"`).
    pub comparison_label: String,

    /// Per-metric, per-period variance rows.
    pub rows: Vec<VarianceRow>,
}

/// Single driver contribution entry in a bridge chart.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeStep {
    /// Driver node identifier (e.g. `"revenue"`).
    pub driver: String,

    /// Contribution of this driver to the total variance, in the same units
    /// as the target metric.
    pub contribution: f64,
}

/// Bridge chart for a single target metric and period.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeChart {
    /// Target metric identifier (e.g. `"ebitda"`).
    pub target_metric: String,

    /// Period identifier.
    pub period: PeriodId,

    /// Baseline scenario label.
    pub baseline_label: String,

    /// Comparison scenario label.
    pub comparison_label: String,

    /// Baseline value of the target metric.
    pub baseline_value: f64,

    /// Comparison value of the target metric.
    pub comparison_value: f64,

    /// Ordered list of driver contributions.
    pub steps: Vec<BridgeStep>,

    /// Residual not explained by the driver deltas:
    /// `(comparison_value - baseline_value) - Σ steps.contribution`.
    ///
    /// Driver contributions are raw deltas in *driver* units, not
    /// sensitivities of the target metric, so they generally do not sum
    /// to the target variance; this term makes the gap explicit instead
    /// of leaving the bridge silently unbalanced.
    #[serde(default)]
    pub unexplained: f64,
}

/// Variance analyzer between two evaluated statement results.
pub struct VarianceAnalyzer<'a> {
    baseline: &'a StatementResult,
    comparison: &'a StatementResult,
}

impl<'a> VarianceAnalyzer<'a> {
    /// Create a new variance analyzer.
    pub fn new(baseline: &'a StatementResult, comparison: &'a StatementResult) -> Self {
        Self {
            baseline,
            comparison,
        }
    }

    /// Compute absolute and percentage variance for the configured metrics
    /// and periods.
    ///
    /// - `abs_var = comparison - baseline`
    /// - `pct_var = abs_var / baseline` (`None` when baseline is ~0 so
    ///   callers can distinguish a truly 0% change from an undefined
    ///   ratio with a non-trivial absolute move)
    pub fn compute(&self, config: &VarianceConfig) -> Result<VarianceReport> {
        if config.metrics.is_empty() {
            return Err(Error::invalid_input(
                "VarianceConfig.metrics cannot be empty",
            ));
        }

        if config.periods.is_empty() {
            return Err(Error::invalid_input(
                "VarianceConfig.periods cannot be empty",
            ));
        }

        let mut rows = Vec::new();

        for metric in &config.metrics {
            for period in &config.periods {
                let baseline = self.baseline.get(metric, period).ok_or_else(|| {
                    Error::missing_data(format!(
                        "Missing baseline value for '{}' @ {}",
                        metric, period
                    ))
                })?;

                let comparison = self.comparison.get(metric, period).ok_or_else(|| {
                    Error::missing_data(format!(
                        "Missing comparison value for '{}' @ {}",
                        metric, period
                    ))
                })?;

                let abs_var = comparison - baseline;
                let pct_var = if baseline.abs() < ZERO_TOLERANCE {
                    None
                } else {
                    Some(abs_var / baseline)
                };

                rows.push(VarianceRow {
                    period: *period,
                    metric: metric.clone(),
                    baseline,
                    comparison,
                    abs_var,
                    pct_var,
                    driver_contribution: IndexMap::new(),
                });
            }
        }

        Ok(VarianceReport {
            baseline_label: config.baseline_label.clone(),
            comparison_label: config.comparison_label.clone(),
            rows,
        })
    }

    /// Compute a simple additive bridge decomposition for a target metric and period.
    ///
    /// The current implementation uses a straightforward "delta driver" approach:
    ///
    /// ```text
    /// contribution(driver) = comparison(driver) - baseline(driver)
    /// ```
    ///
    /// This is intentionally conservative and does **not** attempt to enforce
    /// that the sum of contributions must equal the total variance of the
    /// target metric: contributions are deltas in *driver* units, not
    /// target-metric sensitivities. The gap is reported explicitly in
    /// [`BridgeChart::unexplained`]. More sophisticated attribution schemes
    /// (including multiplicative drivers such as volume × price) can be
    /// layered on top.
    pub fn bridge_decomposition(
        &self,
        target_metric: &str,
        period: PeriodId,
        drivers: &[&str],
        baseline_label: &str,
        comparison_label: &str,
    ) -> Result<BridgeChart> {
        let baseline_value = self.baseline.get(target_metric, &period).ok_or_else(|| {
            Error::missing_data(format!(
                "Missing baseline value for '{}' @ {}",
                target_metric, period
            ))
        })?;

        let comparison_value = self.comparison.get(target_metric, &period).ok_or_else(|| {
            Error::missing_data(format!(
                "Missing comparison value for '{}' @ {}",
                target_metric, period
            ))
        })?;

        let mut steps = Vec::new();

        for driver in drivers {
            let base_drv = self.baseline.get(driver, &period).ok_or_else(|| {
                Error::missing_data(format!("Missing baseline driver '{}' @ {}", driver, period))
            })?;

            let cmp_drv = self.comparison.get(driver, &period).ok_or_else(|| {
                Error::missing_data(format!(
                    "Missing comparison driver '{}' @ {}",
                    driver, period
                ))
            })?;

            steps.push(BridgeStep {
                driver: (*driver).to_string(),
                contribution: cmp_drv - base_drv,
            });
        }

        let explained: f64 = steps.iter().map(|step| step.contribution).sum();
        let unexplained = (comparison_value - baseline_value) - explained;

        Ok(BridgeChart {
            target_metric: target_metric.to_string(),
            period,
            baseline_label: baseline_label.to_string(),
            comparison_label: comparison_label.to_string(),
            baseline_value,
            comparison_value,
            steps,
            unexplained,
        })
    }
}

impl VarianceReport {
    /// Check if the report is empty.
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }
}

impl VarianceReport {
    /// Export variance rows to a table envelope.
    ///
    /// Schema:
    /// - `period` (String)
    /// - `metric` (String)
    /// - `baseline` (Float64)
    /// - `comparison` (Float64)
    /// - `abs_var` (Float64)
    /// - `pct_var` (Float64, nullable)
    /// - `driver_contribution` (String, nullable)
    pub fn to_table(&self) -> Result<TableEnvelope> {
        let mut periods = Vec::with_capacity(self.rows.len());
        let mut metrics = Vec::with_capacity(self.rows.len());
        let mut baselines = Vec::with_capacity(self.rows.len());
        let mut comparisons = Vec::with_capacity(self.rows.len());
        let mut abs_vars = Vec::with_capacity(self.rows.len());
        let mut pct_vars = Vec::with_capacity(self.rows.len());
        let mut driver_contributions = Vec::with_capacity(self.rows.len());

        for row in &self.rows {
            periods.push(row.period.to_string());
            metrics.push(row.metric.clone());
            baselines.push(row.baseline);
            comparisons.push(row.comparison);
            abs_vars.push(row.abs_var);
            pct_vars.push(row.pct_var);

            if row.driver_contribution.is_empty() {
                driver_contributions.push(None::<String>);
            } else {
                let joined = row
                    .driver_contribution
                    .iter()
                    .map(|(driver, value)| format!("{driver}: {value}"))
                    .collect::<Vec<_>>()
                    .join(", ");
                driver_contributions.push(Some(joined));
            }
        }

        let mut metadata = IndexMap::new();
        metadata.insert("layout".to_string(), json!("long"));
        metadata.insert("source".to_string(), json!("variance_report"));

        TableEnvelope::new_with_metadata(
            vec![
                TableColumn::new("period", TableColumnData::String(periods))
                    .with_role(TableColumnRole::Index),
                TableColumn::new("metric", TableColumnData::String(metrics))
                    .with_role(TableColumnRole::Dimension),
                TableColumn::new("baseline", TableColumnData::Float64(baselines))
                    .with_role(TableColumnRole::Measure),
                TableColumn::new("comparison", TableColumnData::Float64(comparisons))
                    .with_role(TableColumnRole::Measure),
                TableColumn::new("abs_var", TableColumnData::Float64(abs_vars))
                    .with_role(TableColumnRole::Measure),
                TableColumn::new("pct_var", TableColumnData::NullableFloat64(pct_vars))
                    .with_role(TableColumnRole::Measure),
                TableColumn::new(
                    "driver_contribution",
                    TableColumnData::NullableString(driver_contributions),
                )
                .with_role(TableColumnRole::Attribute),
            ],
            metadata,
        )
        .map_err(Into::into)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_core::dates::PeriodId;

    fn make_results(values: &[(&str, PeriodId, f64)]) -> StatementResult {
        let mut results = StatementResult::new();

        for (node_id, period, value) in values {
            results
                .nodes
                .entry((*node_id).to_string())
                .or_default()
                .insert(*period, *value);
        }

        results
    }

    #[test]
    fn baseline_vs_baseline_yields_zero_variance() {
        let period = PeriodId::quarter(2025, 1);

        let baseline =
            make_results(&[("revenue", period, 100_000.0), ("ebitda", period, 60_000.0)]);
        let comparison = baseline.clone();

        let analyzer = VarianceAnalyzer::new(&baseline, &comparison);
        let config = VarianceConfig::new(
            "baseline",
            "baseline",
            vec!["revenue", "ebitda"],
            vec![period],
        );

        let report = analyzer.compute(&config).expect("variance should succeed");
        assert_eq!(report.rows.len(), 2);

        for row in report.rows {
            assert_eq!(row.abs_var, 0.0);
            // Non-zero baseline with zero delta → Some(0.0).
            assert_eq!(row.pct_var, Some(0.0));
        }
    }

    #[test]
    fn handles_negative_variance_and_pct() {
        let period = PeriodId::quarter(2025, 1);

        let baseline = make_results(&[("revenue", period, 100.0)]);
        let comparison = make_results(&[("revenue", period, 95.0)]);

        let analyzer = VarianceAnalyzer::new(&baseline, &comparison);
        let config = VarianceConfig::new("baseline", "comparison", vec!["revenue"], vec![period]);

        let report = analyzer.compute(&config).expect("variance should succeed");
        assert_eq!(report.rows.len(), 1);

        let row = &report.rows[0];
        assert_eq!(row.abs_var, -5.0);
        let pct = row
            .pct_var
            .expect("pct_var should be defined for non-zero baseline");
        assert!((pct - (-0.05)).abs() < 1e-12);
    }

    #[test]
    fn pct_variance_is_none_when_baseline_is_zero() {
        let period = PeriodId::quarter(2025, 1);

        let baseline = make_results(&[("revenue", period, 0.0)]);
        let comparison = make_results(&[("revenue", period, 10.0)]);

        let analyzer = VarianceAnalyzer::new(&baseline, &comparison);
        let config = VarianceConfig::new("baseline", "comparison", vec!["revenue"], vec![period]);

        let report = analyzer.compute(&config).expect("variance should succeed");
        let row = &report.rows[0];

        // The absolute variance is still meaningful; the ratio is not.
        assert_eq!(row.abs_var, 10.0);
        assert!(row.pct_var.is_none());
    }

    #[test]
    fn bridge_decomposition_reports_unexplained_residual() {
        let period = PeriodId::quarter(2025, 1);

        let baseline = make_results(&[
            ("ebitda", period, 100.0),
            ("revenue", period, 200.0),
            ("cogs", period, 80.0),
        ]);
        let comparison = make_results(&[
            ("ebitda", period, 110.0),
            ("revenue", period, 210.0),
            ("cogs", period, 75.0),
        ]);

        let analyzer = VarianceAnalyzer::new(&baseline, &comparison);
        let chart = analyzer
            .bridge_decomposition("ebitda", period, &["revenue", "cogs"], "base", "cmp")
            .expect("bridge");

        // Target delta = 10; driver deltas = +10 (revenue) and -5 (cogs).
        let explained: f64 = chart.steps.iter().map(|s| s.contribution).sum();
        assert!((explained - 5.0).abs() < 1e-12);
        assert!(
            (chart.unexplained - 5.0).abs() < 1e-12,
            "unexplained must close the bridge: got {}",
            chart.unexplained
        );
        assert!(
            ((chart.comparison_value - chart.baseline_value) - (explained + chart.unexplained))
                .abs()
                < 1e-12
        );
    }

    #[test]
    fn bridge_decomposition_produces_expected_contributions() {
        let period = PeriodId::quarter(2025, 1);

        let baseline = make_results(&[
            ("ebitda", period, 100.0),
            ("revenue", period, 200.0),
            ("cogs", period, 80.0),
        ]);

        let comparison = make_results(&[
            ("ebitda", period, 110.0),
            ("revenue", period, 210.0),
            ("cogs", period, 75.0),
        ]);

        let analyzer = VarianceAnalyzer::new(&baseline, &comparison);
        let chart = analyzer
            .bridge_decomposition(
                "ebitda",
                period,
                &["revenue", "cogs"],
                "baseline",
                "comparison",
            )
            .expect("bridge should succeed");

        assert_eq!(chart.target_metric, "ebitda");
        assert_eq!(chart.baseline_value, 100.0);
        assert_eq!(chart.comparison_value, 110.0);
        assert_eq!(chart.steps.len(), 2);

        assert_eq!(chart.steps[0].driver, "revenue");
        assert!((chart.steps[0].contribution - 10.0).abs() < 1e-12);

        assert_eq!(chart.steps[1].driver, "cogs");
        assert!((chart.steps[1].contribution - (-5.0)).abs() < 1e-12);
    }
}
