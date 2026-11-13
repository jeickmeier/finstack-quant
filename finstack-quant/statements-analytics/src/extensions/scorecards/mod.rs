//! Credit scorecard analysis extension.
//!
//! Credit rating assignment from financial metrics and configurable
//! thresholds on a caller-supplied scorecard scale, with weighted scoring and
//! optional minimum-rating compliance checks. The scale is resolved once from
//! the supplied [`RatingScaleRegistry`] and kept as an immutable snapshot;
//! there is no embedded or default scale (record CORE-CONVENTIONS-019).
//!
//! # Configuration Schema
//!
//! ```json
//! {
//!   "rating_scale": "internal",
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
//! ```no_run
//! use finstack_quant_core::rating_scales::RatingScaleRegistry;
//! use finstack_quant_statements_analytics::extensions::{
//!     CreditScorecardExtension, ScorecardConfig, ScorecardMetric,
//! };
//! use finstack_quant_statements::evaluator::{Evaluator, StatementResult};
//! use finstack_quant_statements::types::FinancialModelSpec;
//!
//! # fn main() -> finstack_quant_statements::Result<()> {
//! # let model: FinancialModelSpec = unimplemented!("build a model");
//! # let registry_json: &str = unimplemented!("a finstack_quant.rating_scales/2 document");
//! let registry = RatingScaleRegistry::from_json(registry_json)?;
//! let mut evaluator = Evaluator::new();
//! let results = evaluator.evaluate(&model)?;
//!
//! let config = ScorecardConfig {
//!     rating_scale: "internal".into(),
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
//! let extension = CreditScorecardExtension::new(config, &registry)?;
//! let report = extension.execute(&model, &results)?;
//! # let _ = report;
//! # Ok(())
//! # }
//! ```

use finstack_quant_core::rating_scales::{RatingScaleEntry, RatingScaleRegistry};
use finstack_quant_statements::Result;
use finstack_quant_statements::evaluator::StatementResult;
use finstack_quant_statements::types::FinancialModelSpec;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

/// Credit scorecard analysis extension for rating and stress testing.
///
/// **Features:**
/// - Credit rating assignment using weighted metric scores
/// - One caller-supplied scale, resolved once and snapshotted at construction
/// - Configurable thresholds per rating level, validated against that scale
/// - Minimum rating compliance checks on positions in that scale
pub struct CreditScorecardExtension {
    /// Extension configuration
    config: ScorecardConfig,
    /// Immutable snapshot of the selected registry entry.
    scale: RatingScaleEntry,
    /// Per metric (config order): threshold bounds keyed by band position.
    thresholds: Vec<IndexMap<usize, (f64, f64)>>,
    /// Band position of `config.min_rating`, if configured.
    min_position: Option<usize>,
}

/// Configuration for credit scorecard analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ScorecardConfig {
    /// Id of the scale in the supplied registry; lookup ignores outer
    /// whitespace and ASCII case. Required: there is no default scale.
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
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
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

fn default_weight() -> f64 {
    1.0
}

/// Status of a scorecard run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum ScorecardStatus {
    /// Scorecard executed successfully
    Success,
    /// Scorecard execution failed
    Failed,
}

/// Report produced by [`CreditScorecardExtension::execute`].
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub struct ScorecardReport {
    /// Overall execution status
    pub status: ScorecardStatus,

    /// Human-readable summary
    pub message: String,

    /// Structured output (rating, total_score, metric_scores, rating_scale).
    /// With no usable positive-weight evidence, rating and total_score are
    /// null, partial is true, and status is Failed.
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
    /// Create a credit scorecard extension on a scale of the supplied registry.
    ///
    /// The selected entry (scale, id and source) is resolved once and kept
    /// as an immutable snapshot; threshold labels and the minimum rating are
    /// resolved against it here, before any evaluation.
    ///
    /// # Arguments
    /// * `config` - Pre-built [`ScorecardConfig`] driving the analysis
    /// * `registry` - Caller-supplied rating-scale registry holding
    ///   `config.rating_scale`
    ///
    /// # Errors
    ///
    /// Returns an invalid-input error if [`Self::validate_config`] rejects the
    /// configuration.
    pub fn new(config: ScorecardConfig, registry: &RatingScaleRegistry) -> Result<Self> {
        let Resolved {
            scale,
            thresholds,
            min_position,
        } = Self::resolve(&config, registry)?;
        Ok(Self {
            config,
            scale,
            thresholds,
            min_position,
        })
    }

    /// The snapshot of the selected registry entry.
    pub fn rating_scale(&self) -> &RatingScaleEntry {
        &self.scale
    }

    /// Get the current configuration.
    pub fn config(&self) -> &ScorecardConfig {
        &self.config
    }

    /// Validate a configuration against a registry without executing.
    ///
    /// # Arguments
    /// * `config` - Configuration to check
    /// * `registry` - Caller-supplied rating-scale registry
    ///
    /// # Errors
    ///
    /// Returns an invalid-input error for a scale id the registry does not
    /// hold, a threshold or minimum-rating label the selected scale does not
    /// hold or that repeats after outer trim and ASCII case folding,
    /// negative/non-finite metric weights, implausible nonzero total weights,
    /// or an invalid explicit period ID.
    pub fn validate_config(config: &ScorecardConfig, registry: &RatingScaleRegistry) -> Result<()> {
        Self::resolve(config, registry).map(|_| ())
    }

    /// Validate `config` and resolve its scale snapshot, per-metric threshold
    /// positions and minimum-rating position.
    fn resolve(config: &ScorecardConfig, registry: &RatingScaleRegistry) -> Result<Resolved> {
        let invalid = |message: String| finstack_quant_statements::Error::invalid_input(message);
        let scale = registry
            .entry(&config.rating_scale)
            .map_err(|e| invalid(format!("Unsupported rating_scale: {e}")))?
            .clone();
        let mut thresholds = Vec::with_capacity(config.metrics.len());
        for metric in &config.metrics {
            let mut positions = IndexMap::new();
            for (label, bounds) in &metric.thresholds {
                let position = scale.get_scale().level_position(label).map_err(|e| {
                    invalid(format!("Scorecard metric '{}' threshold: {e}", metric.name))
                })?;
                if positions.insert(position, *bounds).is_some() {
                    return Err(invalid(format!(
                        "Scorecard metric '{}' repeats threshold label '{}'",
                        metric.name,
                        label.trim()
                    )));
                }
            }
            thresholds.push(positions);
        }
        let min_position = config
            .min_rating
            .as_deref()
            .map(|label| {
                scale
                    .get_scale()
                    .level_position(label)
                    .map_err(|e| invalid(format!("Scorecard min_rating: {e}")))
            })
            .transpose()?;
        Self::validate_settings(config)?;
        Ok(Resolved {
            scale,
            thresholds,
            min_position,
        })
    }

    fn validate_settings(config: &ScorecardConfig) -> Result<()> {
        for metric in &config.metrics {
            if !metric.weight.is_finite() || metric.weight < 0.0 {
                return Err(finstack_quant_statements::Error::invalid_input(format!(
                    "Scorecard metric '{}' weight must be finite and non-negative, got {}",
                    metric.name, metric.weight
                )));
            }
        }

        let total_weight: f64 = config.metrics.iter().map(|m| m.weight).sum();
        if total_weight > 0.0 && !(0.01..=100.0).contains(&total_weight) {
            return Err(finstack_quant_statements::Error::invalid_input(format!(
                "Total metric weights ({}) should be between 0.01 and 100.0",
                total_weight
            )));
        }

        if let Some(period) = &config.period {
            period
                .parse::<finstack_quant_core::dates::PeriodId>()
                .map_err(|e| {
                    finstack_quant_statements::Error::invalid_input(format!(
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
    ) -> Result<&'m finstack_quant_core::dates::Period> {
        if let Some(period_str) = &config.period {
            let pid: finstack_quant_core::dates::PeriodId = period_str.parse().map_err(|e| {
                finstack_quant_statements::Error::invalid_input(format!(
                    "Invalid scorecard period '{period_str}': {e}"
                ))
            })?;
            return model.periods.iter().find(|p| p.id == pid).ok_or_else(|| {
                finstack_quant_statements::Error::invalid_input(format!(
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
            .ok_or_else(|| finstack_quant_statements::Error::invalid_input("No periods in model"))
    }

    /// Run scorecard analysis against the provided model and evaluation results.
    ///
    /// # Arguments
    /// * `model` - The evaluated financial model
    /// * `results` - Evaluation output to inspect
    ///
    /// Individual metric failures are collected in the returned report, rather
    /// than aborting the entire scorecard. A report with such failures has
    /// `ScorecardStatus::Failed`.
    ///
    /// # Errors
    ///
    /// Returns an error for missing/invalid extension configuration, an invalid
    /// or missing target period, or an invalid rating-scale/minimum-rating
    /// comparison. Per-metric evaluation errors are reported in `errors`.
    pub fn execute(
        &self,
        model: &FinancialModelSpec,
        results: &StatementResult,
    ) -> Result<ScorecardReport> {
        let _span = tracing::info_span!("statements_analytics.credit_scorecard.execute").entered();

        let config = &self.config;
        let target_period = Self::resolve_target_period(config, model)?;

        let mut scores = Vec::new();
        let mut errors = Vec::new();
        let mut warnings = Vec::new();
        let mut excluded = 0usize;

        let mut base_context = None;
        for (metric_config, thresholds) in config.metrics.iter().zip(&self.thresholds) {
            match self.evaluate_metric(
                metric_config,
                thresholds,
                model,
                results,
                target_period,
                &mut base_context,
            ) {
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

        // Excluded NM factors renormalize the remaining weights.
        let total_score = self.calculate_weighted_score(&scores);
        if total_score.is_none() {
            errors.push("Credit rating unavailable: no usable positive-weight evidence".into());
        }

        let scale = self.scale.get_scale();
        let rated = total_score
            .map(|score| {
                let level = scale.classify(score)?;
                Ok::<_, finstack_quant_core::Error>((
                    level.get_name().to_string(),
                    scale.level_position(level.get_name())?,
                ))
            })
            .transpose()?;
        let rating = rated.as_ref().map(|(name, _)| name.clone());

        if let (Some(min_rating), Some(min_position), Some((rating, position))) =
            (&config.min_rating, self.min_position, &rated)
            && *position > min_position
        {
            warnings.push(format!(
                "Credit rating {} is below minimum required {}",
                rating,
                min_rating.trim()
            ));
        }

        let (status, message) = if errors.is_empty() {
            (
                ScorecardStatus::Success,
                format!(
                    "Credit scorecard complete. Rating: {} (Score: {:.2})",
                    rating.as_deref().unwrap_or("unavailable"),
                    total_score.unwrap_or_default()
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
            serde_json::json!(
                scores
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
                    .collect::<Vec<_>>()
            ),
        );
        data.insert(
            "rating_scale".into(),
            serde_json::json!(self.scale.get_id()),
        );
        data.insert(
            "rating_scale_source".into(),
            serde_json::json!(self.scale.get_source()),
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

    /// Prepare the immutable history and target-period values once per scorecard.
    fn prepare_context(
        model: &FinancialModelSpec,
        results: &StatementResult,
        target_period: &finstack_quant_core::dates::Period,
    ) -> Result<finstack_quant_statements::evaluator::EvaluationContext> {
        let node_to_column: indexmap::IndexMap<finstack_quant_statements::types::NodeId, usize> =
            model
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

        let mut eval_context = finstack_quant_statements::evaluator::EvaluationContext::new(
            target_period.id,
            &model.periods,
            std::sync::Arc::new(node_to_column),
            std::sync::Arc::new(historical_results),
        );

        if let Some(ref cs) = results.cs_cashflows {
            eval_context.capital_structure_cashflows = Some(cs.clone());
        }

        for (node_id, node_values) in &results.nodes {
            if let Some(value) = node_values.get(&target_period.id)
                && eval_context.node_to_column.contains_key(node_id.as_str())
            {
                eval_context.set_value(node_id, *value)?;
            }
        }

        Ok(eval_context)
    }

    /// Evaluate a single metric at the target period.
    ///
    /// Returns `score: None` (factor excluded from the weighted average, à la
    /// rating-agency "NM" treatment) when the metric value is non-finite or
    /// no threshold bucket matches; the remaining factor weights renormalize.
    fn evaluate_metric(
        &self,
        metric: &ScorecardMetric,
        thresholds: &IndexMap<usize, (f64, f64)>,
        model: &FinancialModelSpec,
        results: &StatementResult,
        target_period: &finstack_quant_core::dates::Period,
        base_context: &mut Option<finstack_quant_statements::evaluator::EvaluationContext>,
    ) -> Result<MetricEvaluation> {
        let expr = finstack_quant_statements::dsl::parse_and_compile(&metric.formula)?;

        let context = match base_context {
            Some(context) => context,
            None => base_context.insert(Self::prepare_context(model, results, target_period)?),
        };
        let mut eval_context = context.clone();

        let value = finstack_quant_statements::evaluator::evaluate_formula(
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

        // An unmatched value is excluded rather than silently scoring
        // the registry default.
        match self.matching_threshold_score(value, thresholds) {
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
                    metric.name,
                    value,
                    self.scale.get_id()
                )),
            }),
        }
    }

    /// Representative score of the best band whose threshold contains `value`.
    fn matching_threshold_score(
        &self,
        value: f64,
        thresholds: &IndexMap<usize, (f64, f64)>,
    ) -> Option<f64> {
        // Boundary convention: buckets are matched in scale order (best
        // band first) and every bucket is closed on both ends `[min, max]`.
        // A value on the shared boundary between two adjacent buckets
        // therefore matches both, and best-first iteration resolves it to
        // the **better** band — regardless of whether the metric is
        // higher-is-better (coverage-style) or lower-is-better
        // (leverage-style).
        self.scale
            .get_scale()
            .get_ratings()
            .iter()
            .enumerate()
            .find_map(|(position, level)| {
                thresholds.get(&position).and_then(|(min, max)| {
                    (value >= *min && value <= *max).then_some(level.get_score())
                })
            })
    }

    /// Calculate weighted average score.
    fn calculate_weighted_score(&self, scores: &[MetricScore]) -> Option<f64> {
        if scores.is_empty() {
            return None;
        }

        let total_weight: f64 = scores.iter().map(|s| s.weight).sum();
        if total_weight.abs() < f64::EPSILON {
            return None;
        }

        let score = scores.iter().map(|s| s.score * s.weight).sum::<f64>() / total_weight;
        score.is_finite().then_some(score)
    }
}

/// Scale snapshot and label positions resolved from a configuration.
struct Resolved {
    scale: RatingScaleEntry,
    thresholds: Vec<IndexMap<usize, (f64, f64)>>,
    min_position: Option<usize>,
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

#[cfg(test)]
mod tests {
    use super::*;

    fn registry() -> RatingScaleRegistry {
        RatingScaleRegistry::from_json(
            r#"{"schema":"finstack_quant.rating_scales/2","rating_scales":[{
                "id":"internal","source":"test policy",
                "scale":{"scale_name":"Internal","ratings":[
                    {"name":"AAA","score":100.0,"min_score":96.0},
                    {"name":"AA+","score":95.0,"min_score":91.0},
                    {"name":"AA","score":90.0,"min_score":0.0}
                ]}}]}"#,
        )
        .expect("valid registry")
    }

    fn extension_with(thresholds: &[(&str, (f64, f64))]) -> CreditScorecardExtension {
        CreditScorecardExtension::new(
            ScorecardConfig {
                rating_scale: "internal".into(),
                metrics: vec![ScorecardMetric {
                    name: "m".into(),
                    formula: "1".into(),
                    weight: 1.0,
                    thresholds: thresholds
                        .iter()
                        .map(|(label, bounds)| ((*label).to_string(), *bounds))
                        .collect(),
                    description: None,
                }],
                min_rating: None,
                period: None,
            },
            &registry(),
        )
        .expect("valid extension")
    }

    fn score(extension: &CreditScorecardExtension, value: f64) -> Option<f64> {
        extension.matching_threshold_score(value, &extension.thresholds[0])
    }

    #[test]
    fn unmatched_value_returns_no_score() {
        let extension = extension_with(&[("AAA", (0.0, 1.0))]);
        assert_eq!(
            score(&extension, 2.5),
            None,
            "unmatched value must not score a fallback"
        );
    }

    #[test]
    fn unknown_scale_threshold_and_min_rating_labels_are_rejected() {
        let config = |scale: &str, label: &str, min: Option<&str>| ScorecardConfig {
            rating_scale: scale.into(),
            metrics: vec![ScorecardMetric {
                name: "m".into(),
                formula: "1".into(),
                weight: 1.0,
                thresholds: [(label.to_string(), (0.0, 1.0))].into_iter().collect(),
                description: None,
            }],
            min_rating: min.map(str::to_string),
            period: None,
        };
        let registry = registry();
        assert!(CreditScorecardExtension::new(config("sp", "AAA", None), &registry).is_err());
        assert!(CreditScorecardExtension::new(config("internal", "BBB", None), &registry).is_err());
        assert!(
            CreditScorecardExtension::new(config("internal", "AAA", Some("NR")), &registry)
                .is_err()
        );
        assert!(
            CreditScorecardExtension::new(config(" INTERNAL ", " aa+ ", Some("aa")), &registry)
                .is_ok()
        );
        let mut repeated = config("internal", "AAA", None);
        repeated.metrics[0]
            .thresholds
            .insert(" aaa ".into(), (1.0, 2.0));
        assert!(CreditScorecardExtension::new(repeated, &registry).is_err());
    }

    #[test]
    fn calculate_weighted_score_treats_sub_epsilon_weights_as_zero() {
        let extension = extension_with(&[]);
        let scores = vec![MetricScore {
            metric_name: "leverage".to_string(),
            value: 2.5,
            score: 80.0,
            weight: f64::EPSILON / 4.0,
        }];

        assert_eq!(extension.calculate_weighted_score(&scores), None);
    }

    // Scorecard boundary convention
    //
    // Buckets are matched in scale order (best band first) and every bucket
    // is closed `[min, max]`, so a value on a shared boundary always resolves
    // to the *better* of the two adjacent bands — for both higher-is-better
    // (coverage) and lower-is-better (leverage) metrics.

    #[test]
    fn scorecard_top_boundary_value_lands_in_best_bucket() {
        let extension = extension_with(&[("AAA", (95.0, 100.0)), ("AA+", (90.0, 95.0))]);
        assert_eq!(
            score(&extension, 95.0),
            Some(100.0),
            "top-bucket upper bound is inclusive"
        );
        assert_eq!(score(&extension, 100.0), Some(100.0));
    }

    #[test]
    fn scorecard_non_top_shared_boundary_goes_to_better_rating() {
        let extension = extension_with(&[
            ("AAA", (95.0, 100.0)),
            ("AA+", (90.0, 95.0)),
            ("AA", (85.0, 90.0)),
        ]);
        assert_eq!(
            score(&extension, 90.0),
            Some(95.0),
            "shared boundary 90.0 between AA+ and AA must resolve to AA+ (better)"
        );
        assert_eq!(
            score(&extension, 85.0),
            Some(90.0),
            "lower-bound value must land in the bucket it bounds"
        );
    }

    #[test]
    fn scorecard_leverage_boundary_goes_to_better_rating() {
        let extension =
            extension_with(&[("AAA", (0.0, 1.0)), ("AA+", (1.0, 2.0)), ("AA", (2.0, 3.0))]);
        assert_eq!(
            score(&extension, 2.0),
            Some(95.0),
            "leverage 2.0x on the AA+/AA boundary must resolve to AA+ (better)"
        );
        assert_eq!(score(&extension, 1.0), Some(100.0));
    }
}
