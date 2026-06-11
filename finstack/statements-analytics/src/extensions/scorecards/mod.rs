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
//!     period: None,
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

    /// Period to rate, as a parseable `PeriodId` string (e.g. `"2025Q4"`).
    ///
    /// When `None`, the scorecard rates the last *actual* period in the model
    /// if any exists, otherwise the last model period. Metric formulas see
    /// only periods up to (and including) the rated period as history.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub period: Option<String>,
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

        if let Some(period) = &config.period {
            period
                .parse::<finstack_core::dates::PeriodId>()
                .map_err(|e| {
                    finstack_statements::error::Error::invalid_input(format!(
                        "Invalid scorecard period '{period}': {e}"
                    ))
                })?;
        }

        Ok(())
    }

    /// Resolve the period to rate: explicit config period (must exist in the
    /// model), else the last actual period, else the last model period.
    fn resolve_target_period<'m>(
        config: &ScorecardConfig,
        model: &'m FinancialModelSpec,
    ) -> Result<&'m finstack_core::dates::Period> {
        if let Some(period_str) = &config.period {
            let pid: finstack_core::dates::PeriodId = period_str.parse().map_err(|e| {
                finstack_statements::error::Error::invalid_input(format!(
                    "Invalid scorecard period '{period_str}': {e}"
                ))
            })?;
            return model.periods.iter().find(|p| p.id == pid).ok_or_else(|| {
                finstack_statements::error::Error::invalid_input(format!(
                    "Scorecard period '{period_str}' not found in model periods"
                ))
            });
        }

        model
            .periods
            .iter()
            .rev()
            .find(|p| p.is_actual)
            .or_else(|| model.periods.last())
            .ok_or_else(|| finstack_statements::error::Error::registry("No periods in model"))
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
        Self::validate_config(&config)?;

        let target_period = Self::resolve_target_period(&config, model)?.clone();

        let mut scores = Vec::new();
        let mut errors = Vec::new();
        let mut warnings = Vec::new();
        let mut excluded = 0usize;

        // Evaluate each metric
        for metric_config in &config.metrics {
            match self.evaluate_metric(metric_config, model, results, &config, &target_period) {
                Ok(evaluation) => {
                    if let Some(warning) = evaluation.warning {
                        warnings.push(warning);
                    }
                    match evaluation.score {
                        Some(score) => scores.push(score),
                        None => excluded += 1,
                    }
                }
                Err(e) => errors.push(format!("Metric '{}': {}", metric_config.name, e)),
            }
        }

        // Calculate weighted average score over the included factors only
        // (excluded NM factors renormalize the remaining weights).
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
        data.insert(
            "period".into(),
            serde_json::json!(target_period.id.to_string()),
        );

        // Stamp partial-ness: when any configured factor failed or was
        // excluded (NM), make the weight degradation visible instead of
        // silently renormalizing.
        let configured_weight: f64 = config.metrics.iter().map(|m| m.weight).sum();
        let included_weight: f64 = scores.iter().map(|s| s.weight).sum();
        let partial = excluded > 0 || !errors.is_empty();
        let weight_coverage = if configured_weight > 0.0 {
            included_weight / configured_weight
        } else {
            0.0
        };
        data.insert("partial".into(), serde_json::json!(partial));
        data.insert("weight_coverage".into(), serde_json::json!(weight_coverage));

        Ok(ScorecardReport {
            status,
            message,
            data,
            warnings,
            errors,
        })
    }

    /// Evaluate a single metric at the target period.
    ///
    /// Returns `score: None` (factor excluded from the weighted average, à la
    /// rating-agency "NM" treatment) when the metric value is non-finite or
    /// no threshold bucket matches; the remaining factor weights renormalize.
    fn evaluate_metric(
        &self,
        metric: &ScorecardMetric,
        model: &FinancialModelSpec,
        results: &StatementResult,
        config: &ScorecardConfig,
        target_period: &finstack_core::dates::Period,
    ) -> Result<MetricEvaluation> {
        // Parse and evaluate the formula
        let expr = finstack_statements::dsl::parse_and_compile(&metric.formula)?;

        let node_to_column: indexmap::IndexMap<finstack_statements::types::NodeId, usize> = model
            .nodes
            .keys()
            .enumerate()
            .map(|(i, k)| (k.clone(), i))
            .collect();

        // History visible to time-series helpers: periods strictly before
        // the target period (no look-ahead past the rated period).
        let mut historical_results = indexmap::IndexMap::new();
        for period in &model.periods {
            if period.id >= target_period.id {
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
            target_period.id,
            std::sync::Arc::new(node_to_column),
            std::sync::Arc::new(historical_results),
        );

        if let Some(ref cs) = results.cs_cashflows {
            eval_context.capital_structure_cashflows = Some(cs.clone());
        }

        for (node_id, node_values) in &results.nodes {
            if let Some(value) = node_values.get(&target_period.id) {
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

        // Non-finite metric value → factor is not meaningful; exclude it.
        if !value.is_finite() {
            return Ok(MetricEvaluation {
                score: None,
                warning: Some(format!(
                    "Credit scorecard metric '{}' is not finite ({value}); excluding factor and renormalizing weights",
                    metric.name
                )),
            });
        }

        // Calculate score based on thresholds; an unmatched value is also
        // excluded rather than silently scoring the registry default.
        match self.matching_threshold_score(value, &metric.thresholds, &config.rating_scale)? {
            Some(score) => Ok(MetricEvaluation {
                score: Some(MetricScore {
                    metric_name: metric.name.clone(),
                    value,
                    score,
                    weight: metric.weight,
                }),
                warning: None,
            }),
            None => Ok(MetricEvaluation {
                score: None,
                warning: Some(format!(
                    "Credit scorecard metric '{}' thresholds did not match value {} for {}; excluding factor and renormalizing weights",
                    metric.name, value, config.rating_scale
                )),
            }),
        }
    }

    fn matching_threshold_score(
        &self,
        value: f64,
        thresholds: &indexmap::IndexMap<String, (f64, f64)>,
        rating_scale: &str,
    ) -> Result<Option<f64>> {
        let scale = get_rating_scale(rating_scale)?;

        // Boundary convention: buckets are matched in registry order
        // (best rating first) and every bucket is closed on both ends
        // `[min, max]`. A value on the shared boundary between two adjacent
        // buckets therefore matches both, and best-first iteration resolves
        // it to the **better** rating — regardless of whether the metric is
        // higher-is-better (coverage-style, shared boundary is the better
        // bucket's lower bound) or lower-is-better (leverage-style, shared
        // boundary is the better bucket's upper bound).
        Ok(scale.ratings.iter().find_map(|level| {
            thresholds.get(&level.name).and_then(|(min, max)| {
                if value >= *min && value <= *max {
                    Some(level.score)
                } else {
                    None
                }
            })
        }))
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
    /// `None` when the factor was excluded (non-finite value or no matching
    /// threshold bucket); excluded factors renormalize the remaining weights.
    score: Option<MetricScore>,
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
    fn unmatched_value_returns_no_score() {
        let extension = CreditScorecardExtension::new();
        let mut thresholds = indexmap::IndexMap::new();
        thresholds.insert("AAA".to_string(), (0.0, 1.0));

        let score = extension
            .matching_threshold_score(2.5, &thresholds, "S&P")
            .expect("registry score");

        assert_eq!(score, None, "unmatched value must not score a fallback");
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
    //
    // Buckets are matched in registry order (best rating first) and every
    // bucket is closed `[min, max]`, so a value on a shared boundary always
    // resolves to the *better* of the two adjacent ratings — for both
    // higher-is-better (coverage) and lower-is-better (leverage) metrics.
    // =====================================================================

    /// With adjacent buckets `AAA: [95, 100]` and `AA+: [90, 95]`, a value
    /// of exactly 95 sits on the shared boundary and must land in AAA
    /// (the better bucket, matched first in registry order).
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

    /// At a shared boundary between two non-top ratings the better rating
    /// wins because it is matched first in registry order and all buckets
    /// are closed.
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
        // 85.0 — lower bound of AA: lands in AA.
        assert_eq!(
            extension
                .matching_threshold_score(85.0, &thresholds, "S&P")
                .expect("registry score"),
            Some(90.0),
            "lower-bound value must land in the bucket it bounds"
        );
    }

    /// Direction consistency: for a lower-is-better (leverage-type) metric
    /// the better bucket's *upper* bound is the shared boundary — a value
    /// exactly on it must land in the better bucket, not the worse one.
    #[test]
    fn scorecard_leverage_boundary_goes_to_better_rating() {
        let extension = CreditScorecardExtension::new();
        let mut thresholds = indexmap::IndexMap::new();
        thresholds.insert("AAA".to_string(), (0.0, 1.0));
        thresholds.insert("AA+".to_string(), (1.0, 2.0));
        thresholds.insert("AA".to_string(), (2.0, 3.0));

        // Leverage exactly 2.0 — boundary between AA+ and AA → AA+ (better).
        assert_eq!(
            extension
                .matching_threshold_score(2.0, &thresholds, "S&P")
                .expect("registry score"),
            Some(95.0),
            "leverage 2.0x on the AA+/AA boundary must resolve to AA+ (better)"
        );
        // Leverage 1.0 — boundary between AAA and AA+ → AAA.
        assert_eq!(
            extension
                .matching_threshold_score(1.0, &thresholds, "S&P")
                .expect("registry score"),
            Some(100.0)
        );
    }
}
