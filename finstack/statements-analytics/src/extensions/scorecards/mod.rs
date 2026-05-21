//! Credit scorecard analysis extension.
//!
//! This extension provides credit rating assignment based on financial metrics
//! and configurable thresholds.
//!
//! # Features
//!
//! - Credit rating assignment based on financial metrics
//! - Configurable rating scales and thresholds
//! - Weighted scoring across multiple metrics
//! - Support for multiple rating agencies (S&P, Moody's, Fitch)
//! - Minimum rating compliance checks
//! - Detailed metric evaluation with scores and weights
//!
//! # Configuration Schema
//!
//! ```json
//! {
//!   "rating_scale": "S&P",
//!   "metrics": [
//!     {
//!       "name": "debt_to_ebitda",
//!       "formula": "total_debt / ttm(ebitda)",
//!       "weight": 0.3,
//!       "thresholds": {
//!         "AAA": [0.0, 1.0],
//!         "AA": [1.0, 2.0],
//!         "A": [2.0, 3.0],
//!         "BBB": [3.0, 4.0],
//!         "BB": [4.0, 5.0],
//!         "B": [5.0, 6.0],
//!         "CCC": [6.0, 999.0]
//!       }
//!     },
//!     {
//!       "name": "interest_coverage",
//!       "formula": "ebitda / interest_expense",
//!       "weight": 0.25,
//!       "thresholds": {
//!         "AAA": [8.0, 999.0],
//!         "AA": [6.0, 8.0],
//!         "A": [4.5, 6.0],
//!         "BBB": [3.0, 4.5],
//!         "BB": [2.0, 3.0],
//!         "B": [1.0, 2.0],
//!         "CCC": [0.0, 1.0]
//!       }
//!     }
//!   ]
//! }
//! ```
//!
//! # Example Usage
//!
//! ```ignore
//! use finstack_statements_analytics::extensions::{
//!     CreditScorecardExtension, ScorecardConfig, ScorecardMetric,
//! };
//! use finstack_statements::evaluator::{Evaluator, StatementResult};
//! use finstack_statements::types::FinancialModelSpec;
//!
//! # fn main() -> finstack_statements::Result<()> {
//! # let model: FinancialModelSpec = unimplemented!("build a model");
//! let mut evaluator = Evaluator::new();
//! let results = evaluator.evaluate(&model)?;
//!
//! let config = ScorecardConfig {
//!     rating_scale: "S&P".into(),
//!     metrics: vec![ScorecardMetric {
//!         name: "debt_to_ebitda".into(),
//!         formula: "total_debt / ttm(ebitda)".into(),
//!         weight: 1.0,
//!         thresholds: indexmap::IndexMap::new(),
//!         description: None,
//!     }],
//!     min_rating: None,
//! };
//!
//! let mut extension = CreditScorecardExtension::with_config(config);
//! let report = extension.execute(&model, &results)?;
//! # let _ = report;
//! # Ok(())
//! # }
//! ```

pub use finstack_core::rating_scales::{RatingLevel, ScorecardScale};
use finstack_statements::evaluator::StatementResult;
use finstack_statements::types::FinancialModelSpec;
use finstack_statements::Result;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

/// Get the appropriate rating scale based on name.
fn get_rating_scale(scale_name: &str) -> Result<&'static ScorecardScale> {
    Ok(finstack_core::rating_scales::embedded_registry()?.rating_scale(scale_name)?)
}

fn is_supported_rating_scale(scale_name: &str) -> bool {
    finstack_core::rating_scales::embedded_registry()
        .map(|registry| registry.is_known_rating_scale(scale_name))
        .unwrap_or(false)
}

fn default_scorecard_score() -> Result<f64> {
    Ok(finstack_core::rating_scales::embedded_registry()?.default_scorecard_score())
}

/// Credit scorecard analysis extension for rating and stress testing.
///
/// **Features:**
/// - Credit rating assignment using weighted metric scores
/// - Support for multiple rating scales (S&P, Moody's, Fitch)
/// - Configurable thresholds per rating level
/// - Minimum rating compliance checks
pub struct CreditScorecardExtension {
    /// Extension configuration
    config: Option<ScorecardConfig>,
}

/// Configuration for credit scorecard analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScorecardConfig {
    /// Rating scale to use (e.g., "S&P", "Moody's", "Fitch")
    #[serde(default = "default_rating_scale")]
    pub rating_scale: String,

    /// List of metrics to evaluate
    #[serde(default)]
    pub metrics: Vec<ScorecardMetric>,

    /// Minimum acceptable rating (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_rating: Option<String>,
}

/// Definition of a scorecard metric.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScorecardMetric {
    /// Metric name
    pub name: String,

    /// Formula to calculate the metric (DSL syntax)
    pub formula: String,

    /// Weight in overall score (0.0 to 1.0)
    #[serde(default = "default_weight")]
    pub weight: f64,

    /// Rating thresholds: rating → [min, max]
    #[serde(default)]
    pub thresholds: indexmap::IndexMap<String, (f64, f64)>,

    /// Description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

fn default_rating_scale() -> String {
    "S&P".into()
}

fn default_weight() -> f64 {
    1.0
}

/// Status of a scorecard run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScorecardStatus {
    /// Scorecard executed successfully
    Success,
    /// Scorecard execution failed
    Failed,
}

/// Report produced by [`CreditScorecardExtension::execute`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScorecardReport {
    /// Overall execution status
    pub status: ScorecardStatus,

    /// Human-readable summary
    pub message: String,

    /// Structured output (rating, total_score, metric_scores, rating_scale)
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    pub data: IndexMap<String, serde_json::Value>,

    /// Warnings (non-fatal)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,

    /// Errors (per-metric failures)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub errors: Vec<String>,
}

impl CreditScorecardExtension {
    /// Create a new credit scorecard extension with default configuration.
    ///
    /// # Example
    /// ```rust
    /// # use finstack_statements_analytics::extensions::CreditScorecardExtension;
    /// let extension = CreditScorecardExtension::new();
    /// assert!(extension.config().is_none());
    /// ```
    pub fn new() -> Self {
        Self { config: None }
    }

    /// Create a new credit scorecard extension with the given configuration.
    ///
    /// # Arguments
    /// * `config` - Pre-built [`ScorecardConfig`] to use
    pub fn with_config(config: ScorecardConfig) -> Self {
        Self {
            config: Some(config),
        }
    }

    /// Get the current configuration.
    pub fn config(&self) -> Option<&ScorecardConfig> {
        self.config.as_ref()
    }

    /// Set the configuration.
    ///
    /// # Arguments
    /// * `config` - New configuration to assign
    pub fn set_config(&mut self, config: ScorecardConfig) {
        self.config = Some(config);
    }

    /// Validate a configuration without executing.
    ///
    /// Useful for schema-style checks before constructing the extension.
    pub fn validate_config(config: &ScorecardConfig) -> Result<()> {
        if !is_supported_rating_scale(&config.rating_scale) {
            return Err(finstack_statements::error::Error::invalid_input(format!(
                "Unsupported rating_scale '{}'. Expected one of: S&P, Moody's, Fitch",
                config.rating_scale
            )));
        }

        let total_weight: f64 = config.metrics.iter().map(|m| m.weight).sum();
        if total_weight > 0.0 && !(0.01..=100.0).contains(&total_weight) {
            return Err(finstack_statements::error::Error::invalid_input(format!(
                "Total metric weights ({}) should be between 0.01 and 100.0",
                total_weight
            )));
        }

        Ok(())
    }

    /// Run scorecard analysis against the provided model and evaluation results.
    ///
    /// Requires that [`CreditScorecardExtension::with_config`] or
    /// [`CreditScorecardExtension::set_config`] has supplied a configuration;
    /// otherwise returns an error.
    ///
    /// # Arguments
    /// * `model` - The evaluated financial model
    /// * `results` - Evaluation output to inspect
    pub fn execute(
        &mut self,
        model: &FinancialModelSpec,
        results: &StatementResult,
    ) -> Result<ScorecardReport> {
        let _span = tracing::info_span!("statements_analytics.credit_scorecard.execute").entered();

        let config = self.config.clone().ok_or_else(|| {
            finstack_statements::error::Error::registry(
                "Credit scorecard extension requires configuration",
            )
        })?;

        let mut scores = Vec::new();
        let mut errors = Vec::new();
        let mut warnings = Vec::new();

        // Evaluate each metric
        for metric_config in &config.metrics {
            match self.evaluate_metric(metric_config, model, results, &config) {
                Ok(evaluation) => {
                    if let Some(warning) = evaluation.warning {
                        warnings.push(warning);
                    }
                    scores.push(evaluation.score);
                }
                Err(e) => errors.push(format!("Metric '{}': {}", metric_config.name, e)),
            }
        }

        // Calculate weighted average score
        let total_score = self.calculate_weighted_score(&scores);

        // Determine rating based on scale
        let rating = self.determine_rating(total_score, &config.rating_scale)?;

        // Check minimum rating requirement
        if let Some(min_rating) = &config.min_rating {
            if !self.meets_minimum_rating(&rating, min_rating, &config.rating_scale)? {
                warnings.push(format!(
                    "Credit rating {} is below minimum required {}",
                    rating, min_rating
                ));
            }
        }

        // Build report
        let (status, message) = if errors.is_empty() {
            (
                ScorecardStatus::Success,
                format!(
                    "Credit scorecard complete. Rating: {} (Score: {:.2})",
                    rating, total_score
                ),
            )
        } else {
            (
                ScorecardStatus::Failed,
                format!("Credit scorecard failed with {} errors", errors.len()),
            )
        };

        let mut data = IndexMap::new();
        data.insert("rating".into(), serde_json::json!(rating));
        data.insert("total_score".into(), serde_json::json!(total_score));
        data.insert(
            "metric_scores".into(),
            serde_json::json!(scores
                .iter()
                .map(|s| {
                    serde_json::json!({
                        "metric": s.metric_name,
                        "value": s.value,
                        "score": s.score,
                        "weight": s.weight,
                        "weighted_score": s.score * s.weight,
                    })
                })
                .collect::<Vec<_>>()),
        );
        data.insert(
            "rating_scale".into(),
            serde_json::json!(config.rating_scale),
        );

        Ok(ScorecardReport {
            status,
            message,
            data,
            warnings,
            errors,
        })
    }

    /// Evaluate a single metric.
    fn evaluate_metric(
        &self,
        metric: &ScorecardMetric,
        model: &FinancialModelSpec,
        results: &StatementResult,
        config: &ScorecardConfig,
    ) -> Result<MetricEvaluation> {
        // Parse and evaluate the formula
        let expr = finstack_statements::dsl::parse_and_compile(&metric.formula)?;

        // Create evaluation context for the last period (or average across all)
        let last_period = model
            .periods
            .last()
            .ok_or_else(|| finstack_statements::error::Error::registry("No periods in model"))?;

        let node_to_column: indexmap::IndexMap<finstack_statements::types::NodeId, usize> = model
            .nodes
            .keys()
            .enumerate()
            .map(|(i, k)| (k.clone(), i))
            .collect();

        let mut historical_results = indexmap::IndexMap::new();
        for period in &model.periods {
            if period.id == last_period.id {
                continue;
            }
            let mut period_values = indexmap::IndexMap::new();
            for (node_id, node_periods) in &results.nodes {
                if let Some(value) = node_periods.get(&period.id) {
                    period_values.insert(node_id.clone(), *value);
                }
            }
            if !period_values.is_empty() {
                historical_results.insert(period.id, period_values);
            }
        }

        let mut eval_context = finstack_statements::evaluator::EvaluationContext::new(
            last_period.id,
            std::sync::Arc::new(node_to_column),
            std::sync::Arc::new(historical_results),
        );

        if let Some(ref cs) = results.cs_cashflows {
            eval_context.capital_structure_cashflows = Some(cs.clone());
        }

        for (node_id, node_values) in &results.nodes {
            if let Some(value) = node_values.get(&last_period.id) {
                if eval_context.node_to_column.contains_key(node_id.as_str()) {
                    eval_context.set_value(node_id, *value)?;
                }
            }
        }

        // Evaluate the formula
        let value = finstack_statements::evaluator::formula::evaluate_formula(
            &expr,
            &mut eval_context,
            Some(metric.name.as_str()),
        )?;

        // Calculate score based on thresholds
        let score = self.calculate_metric_score(value, &metric.thresholds, &config.rating_scale)?;
        let warning = if self
            .matching_threshold_score(value, &metric.thresholds, &config.rating_scale)?
            .is_none()
        {
            Some(format!(
                "Credit scorecard metric '{}' thresholds did not match value {} for {}; using fallback score {}",
                metric.name, value, config.rating_scale, default_scorecard_score()?
            ))
        } else {
            None
        };

        Ok(MetricEvaluation {
            score: MetricScore {
                metric_name: metric.name.clone(),
                value,
                score,
                weight: metric.weight,
            },
            warning,
        })
    }

    fn matching_threshold_score(
        &self,
        value: f64,
        thresholds: &indexmap::IndexMap<String, (f64, f64)>,
        rating_scale: &str,
    ) -> Result<Option<f64>> {
        let scale = get_rating_scale(rating_scale)?;

        // Boundary convention: the best (first-iterated) rating uses
        // closed-on-both-ends `[min, max]` so a value at the scale
        // maximum still lands there; every other bucket uses `[min,
        // max)` so a value on the shared boundary between two adjacent
        // buckets lands in the **better** of the two — matching the
        // published S&P / Moody's conventions which define strict
        // upper bounds on non-top buckets.
        Ok(scale.ratings.iter().enumerate().find_map(|(idx, level)| {
            thresholds.get(&level.name).and_then(|(min, max)| {
                let is_best_rating = idx == 0;
                let lower_ok = value >= *min;
                let upper_ok = if is_best_rating {
                    value <= *max
                } else {
                    value < *max
                };
                if lower_ok && upper_ok {
                    Some(level.score)
                } else {
                    None
                }
            })
        }))
    }

    /// Calculate score based on thresholds.
    ///
    /// Uses the configured rating scale (S&P by default) to map metric values
    /// to numeric scores based on user-provided thresholds.
    fn calculate_metric_score(
        &self,
        value: f64,
        thresholds: &indexmap::IndexMap<String, (f64, f64)>,
        rating_scale: &str,
    ) -> Result<f64> {
        if let Some(score) = self.matching_threshold_score(value, thresholds, rating_scale)? {
            return Ok(score);
        }

        // Default score if no threshold matches
        let default_score = default_scorecard_score()?;
        tracing::warn!(
            rating_scale,
            value,
            default_score,
            "credit scorecard thresholds did not match metric value; using fallback score"
        );
        Ok(default_score)
    }

    /// Calculate weighted average score.
    fn calculate_weighted_score(&self, scores: &[MetricScore]) -> f64 {
        if scores.is_empty() {
            return 0.0;
        }

        let total_weight: f64 = scores.iter().map(|s| s.weight).sum();
        if total_weight.abs() < f64::EPSILON {
            return 0.0;
        }

        scores.iter().map(|s| s.score * s.weight).sum::<f64>() / total_weight
    }

    /// Determine rating based on total score.
    ///
    /// Uses the configured rating scale to map a numeric score to a credit rating.
    /// Supports S&P, Moody's, and Fitch scales.
    fn determine_rating(&self, score: f64, rating_scale: &str) -> Result<String> {
        let scale = get_rating_scale(rating_scale)?;

        // Find the rating by checking score thresholds
        for level in &scale.ratings {
            if score >= level.min_score {
                return Ok(level.name.clone());
            }
        }

        // Fallback to lowest rating
        Ok(scale
            .ratings
            .last()
            .map(|l| l.name.clone())
            .unwrap_or_default())
    }

    /// Check if rating meets minimum requirement.
    ///
    /// Compares ratings using the configured rating scale with exact matching.
    /// Returns true if the rating is equal to or better than the minimum.
    fn meets_minimum_rating(
        &self,
        rating: &str,
        min_rating: &str,
        rating_scale: &str,
    ) -> Result<bool> {
        let scale = get_rating_scale(rating_scale)?;

        // Find positions in the rating scale (lower index = better rating).
        // Use exact string matching to avoid false matches (e.g., "AA" matching "A").
        let rating_pos = scale.ratings.iter().position(|l| l.name == rating);
        let min_pos = scale.ratings.iter().position(|l| l.name == min_rating);

        match (rating_pos, min_pos) {
            (Some(r), Some(m)) => Ok(r <= m), // Lower index = better rating
            _ => Ok(false),
        }
    }
}

/// Score for a single metric.
struct MetricScore {
    metric_name: String,
    value: f64,
    score: f64,
    weight: f64,
}

struct MetricEvaluation {
    score: MetricScore,
    warning: Option<String>,
}

impl Default for CreditScorecardExtension {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calculate_metric_score_falls_back_when_thresholds_miss() {
        let extension = CreditScorecardExtension::new();
        let mut thresholds = indexmap::IndexMap::new();
        thresholds.insert("AAA".to_string(), (0.0, 1.0));

        let score = extension
            .calculate_metric_score(2.5, &thresholds, "S&P")
            .expect("registry score");

        assert_eq!(score, default_scorecard_score().expect("registry score"));
    }

    #[test]
    fn calculate_weighted_score_treats_sub_epsilon_weights_as_zero() {
        let extension = CreditScorecardExtension::new();
        let scores = vec![MetricScore {
            metric_name: "leverage".to_string(),
            value: 2.5,
            score: 80.0,
            weight: f64::EPSILON / 4.0,
        }];

        let weighted = extension.calculate_weighted_score(&scores);

        assert_eq!(weighted, 0.0);
    }

    // =====================================================================
    // Scorecard boundary convention
    // =====================================================================

    /// With adjacent buckets `AAA: [95, 100]` and `AA+: [90, 95]`, a value
    /// of exactly 95 sits on the shared boundary. Per the S&P /
    /// Moody's convention, the **better** (AAA) rating is the one with
    /// the closed upper bound, so value 95 should land in AAA, not
    /// AA+.
    ///
    /// Pre-fix the code used `[min, max]` for every bucket; since the
    /// iterator is best-first, AAA was returned first either way, so
    /// 95 went to AAA. That accidentally agreed with the convention
    /// *at the top boundary only*. The bug manifests below, at the
    /// AA+/AA boundary.
    #[test]
    fn scorecard_top_boundary_value_lands_in_best_bucket() {
        let extension = CreditScorecardExtension::new();
        let mut thresholds = indexmap::IndexMap::new();
        thresholds.insert("AAA".to_string(), (95.0, 100.0));
        thresholds.insert("AA+".to_string(), (90.0, 95.0));

        // Value 95 is the boundary between AAA and AA+. Top bucket
        // gets [min, max] — so 95 is AAA.
        assert_eq!(
            extension
                .matching_threshold_score(95.0, &thresholds, "S&P")
                .expect("registry score"),
            Some(100.0),
            "top-bucket upper bound is inclusive"
        );
        // Value 100 is the absolute max → still AAA (top-bucket-closed).
        assert_eq!(
            extension
                .matching_threshold_score(100.0, &thresholds, "S&P")
                .expect("registry score"),
            Some(100.0)
        );
    }

    /// The substantive fix: at a shared boundary BETWEEN two non-top
    /// ratings, the better rating should win (because the worse
    /// rating's upper bound is strict).
    ///
    /// Pre-fix: value 90 on `AA+: [90, 95]` + `AA: [85, 90]` returned
    /// 95 (AA+) because `90 <= 90` matched AA+ first... but also
    /// `90 <= 90` for AA would have matched — the user saw AA+
    /// correctly by virtue of iteration order, not by design.
    ///
    /// Post-fix: value 90 on `AA+: [90, 95)` (non-top half-open) +
    /// `AA: [85, 90)` returns AA+ cleanly — 90 is the strict lower
    /// bound of AA+, the strict upper bound of AA. No ambiguity.
    #[test]
    fn scorecard_non_top_shared_boundary_goes_to_better_rating() {
        let extension = CreditScorecardExtension::new();
        let mut thresholds = indexmap::IndexMap::new();
        thresholds.insert("AAA".to_string(), (95.0, 100.0));
        thresholds.insert("AA+".to_string(), (90.0, 95.0));
        thresholds.insert("AA".to_string(), (85.0, 90.0));

        // 90.0 — boundary between AA+ and AA → AA+ (better).
        assert_eq!(
            extension
                .matching_threshold_score(90.0, &thresholds, "S&P")
                .expect("registry score"),
            Some(95.0),
            "shared boundary 90.0 between AA+ and AA must resolve to AA+ (better)"
        );
        // 85.0 — lower bound of AA, upper bound of (nothing defined below).
        // Since AA is not top, AA's range is [85, 90), which INCLUDES 85.
        assert_eq!(
            extension
                .matching_threshold_score(85.0, &thresholds, "S&P")
                .expect("registry score"),
            Some(90.0),
            "lower-bound value must land in the bucket it bounds"
        );
    }
}
