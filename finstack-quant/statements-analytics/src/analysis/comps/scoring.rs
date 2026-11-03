//! Composite rich/cheap scoring across multiple valuation dimensions.
//!
//! Combines percentile rank, z-score, and regression residual signals
//! into a weighted composite relative value score.

use super::multiples::compute_multiple;
use super::peer_set::PeerSet;
use super::stats::{RegressionResult, percentile_rank, regression_fair_value, z_score};
use super::types::{CompanyId, CompanyMetrics, Multiple};
use finstack_quant_core::math::stats::OnlineStats;
use finstack_quant_core::{Error, Result};
use serde::{Deserialize, Serialize};
use std::str::FromStr;

/// Rich/cheap sign convention for a scoring dimension's Y metric.
///
/// Determines how a higher-than-peers Y value maps onto the composite
/// score (positive = cheap). Spread- and yield-like metrics are cheap
/// when high ([`HigherIsCheap`](Self::HigherIsCheap)); valuation
/// multiples (P/E, EV/EBITDA) are rich when high
/// ([`HigherIsRich`](Self::HigherIsRich)).
///
/// The direction is applied consistently to both the regression-residual
/// path and the univariate z-score path. The default,
/// `HigherIsCheap`, matches the historical regression-path convention
/// (positive residual = actual spread above fitted fair spread = cheap).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum ScoreDirection {
    /// Higher Y than peers means the subject is cheap (spread-like metrics).
    #[default]
    HigherIsCheap,
    /// Higher Y than peers means the subject is rich (multiple-like metrics).
    HigherIsRich,
}

impl FromStr for ScoreDirection {
    type Err = String;

    fn from_str(value: &str) -> std::result::Result<Self, Self::Err> {
        match value {
            "higher_is_cheap" => Ok(Self::HigherIsCheap),
            "higher_is_rich" => Ok(Self::HigherIsRich),
            _ => Err(format!(
                "unknown direction '{value}' (expected 'higher_is_cheap' or 'higher_is_rich')"
            )),
        }
    }
}

/// Configuration for a single rich/cheap scoring dimension.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ScoringDimension {
    /// Human-readable label (e.g., "Spread vs Leverage").
    pub label: String,
    /// How to extract the Y variable (dependent) from CompanyMetrics.
    pub y_extractor: MetricExtractor,
    /// Optional explanatory metric for single-factor regression.
    /// Omit it to score the dependent metric against its peer distribution.
    pub x_extractor: Option<MetricExtractor>,
    /// Finite, non-negative relative weight in the composite score; zero disables it.
    pub weight: f64,
    /// Rich/cheap sign convention for the Y metric (default:
    /// [`ScoreDirection::HigherIsCheap`], i.e. spread-like).
    #[serde(default)]
    pub direction: ScoreDirection,
}

/// Identifies which metric to extract from CompanyMetrics.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum MetricExtractor {
    /// A named field (e.g., "leverage", "oas_bp", "ebitda_margin").
    Named(String),
    /// A valuation multiple.
    Multiple(Multiple),
    /// A custom metric key from the `custom` map.
    Custom(String),
}

impl FromStr for MetricExtractor {
    type Err = String;

    fn from_str(name: &str) -> std::result::Result<Self, Self::Err> {
        if let Some(multiple) = name.strip_prefix("multiple:") {
            multiple.parse().map(Self::Multiple)
        } else if CompanyMetrics::is_named_metric(name) {
            Ok(Self::Named(name.to_string()))
        } else {
            Ok(Self::Custom(name.to_string()))
        }
    }
}

/// Decomposed score for a single dimension.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub struct DimensionScore {
    /// Label of the dimension.
    pub label: String,
    /// Percentile rank of the subject's Y within peers (0-1). Whether a
    /// high percentile means rich or cheap depends on the dimension's
    /// [`ScoringDimension::direction`].
    pub percentile: f64,
    /// Z-score of the subject's Y relative to the peer distribution
    /// (raw, before the direction convention is applied).
    pub z_score: f64,
    /// Raw regression residual in Y units (actual − fitted). Positive
    /// means the subject's Y is above the peer fair-value line; the
    /// composite uses the standardized, direction-adjusted form.
    pub regression_residual: Option<f64>,
    /// R-squared of the regression (confidence measure).
    pub r_squared: Option<f64>,
    /// Dimension weight in composite.
    pub weight: f64,
}

/// Composite relative value result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub struct RelativeValueResult {
    /// Company being scored.
    pub company_id: CompanyId,
    /// Composite rich/cheap score.
    ///
    /// Positive = cheap (trading below fair value across dimensions).
    /// Negative = rich (trading above fair value).
    /// Magnitude indicates conviction (bounded by number of dimensions
    /// and their weights).
    pub composite_score: f64,
    /// Per-dimension decomposition.
    pub dimensions: Vec<DimensionScore>,
    /// Confidence in the composite score (average R-squared across
    /// regression-based dimensions, weighted by dimension weight).
    pub confidence: f64,
    /// Number of peers used in the analysis.
    pub peer_count: usize,
}

/// Why a configured dimension contributes no signal to the composite score.
///
/// The conditions are tested in declaration order and the first that holds is
/// reported.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum DimensionExclusion {
    /// The configured weight is zero, which disables the dimension.
    ZeroWeight,
    /// The subject has no finite value of the Y metric.
    NoSubjectValue,
    /// No peer has a finite value of the Y metric.
    NoPeerValues,
    /// Fewer than two peers have the Y metric, or their values do not differ,
    /// so the z-score is undefined.
    NoDispersion,
    /// The subject has no finite value of the X metric.
    NoSubjectX,
    /// The regression cannot be fitted: fewer than three peers have both
    /// metrics, X does not vary, or a fitted figure is non-finite.
    RegressionUnusable,
    /// The regression fits but the peers sit on the line, so the subject's
    /// residual cannot be standardized.
    NoResidualDispersion,
}

/// Every figure computed for one dimension, shared by [`score_relative_value`]
/// and the relative value report so the two cannot disagree.
pub(super) struct DimensionEvaluation<'a> {
    /// Subject's Y metric.
    pub(super) subject_y: Option<f64>,
    /// Subject's X metric; `None` without an X extractor.
    pub(super) subject_x: Option<f64>,
    /// Y metric of every peer that has it, in peer order.
    pub(super) peer_y: Vec<f64>,
    /// `(company, x, y)` of every peer that has both metrics, in peer order.
    pub(super) peer_pairs: Vec<(&'a CompanyId, f64, f64)>,
    /// Percentile rank of the subject's Y within `peer_y`.
    pub(super) percentile: Option<f64>,
    /// Z-score of the subject's Y within `peer_y`.
    pub(super) z_score: Option<f64>,
    /// Fit over `peer_pairs` evaluated at the subject.
    pub(super) regression: Option<RegressionResult>,
    /// Direction-adjusted signal (positive = cheap) entering the composite;
    /// `None` exactly when `excluded_reason` is set.
    pub(super) signal: Option<f64>,
    /// Why the dimension contributes no signal; `None` when it is used.
    pub(super) excluded_reason: Option<DimensionExclusion>,
}

/// Evaluate one dimension against the peer set.
///
/// Figures are computed whether or not the dimension is used, so an excluded
/// dimension still reports what it has. A regression dimension never falls back
/// to its z-score.
pub(super) fn evaluate_dimension<'a>(
    peer_set: &'a PeerSet,
    dim: &ScoringDimension,
) -> DimensionEvaluation<'a> {
    let subject_y = extract_subject_value(peer_set, &dim.y_extractor);
    let peer_y = extract_values(peer_set, &dim.y_extractor);
    let percentile = subject_y.and_then(|y| percentile_rank(&peer_y, y));
    let z = subject_y.and_then(|y| z_score(&peer_y, y));

    let subject_x = dim
        .x_extractor
        .as_ref()
        .and_then(|x| extract_subject_value(peer_set, x));
    // Pairwise per peer, so a peer missing one metric drops the *pair* instead
    // of misaligning the two vectors.
    let peer_pairs = dim
        .x_extractor
        .as_ref()
        .map_or_else(Vec::new, |x| extract_pairs(peer_set, x, &dim.y_extractor));
    let (peer_x, peer_y_aligned): (Vec<f64>, Vec<f64>) =
        peer_pairs.iter().map(|&(_, x, y)| (x, y)).unzip();
    let regression = subject_x
        .zip(subject_y)
        .and_then(|(x, y)| regression_fair_value(&peer_x, &peer_y_aligned, x, y));
    let std_residual = regression.as_ref().and_then(|reg| {
        standardized_residual(
            &peer_x,
            &peer_y_aligned,
            reg.intercept,
            reg.slope,
            reg.residual,
        )
    });

    let excluded_reason = if dim.weight == 0.0 {
        Some(DimensionExclusion::ZeroWeight)
    } else if subject_y.is_none() {
        Some(DimensionExclusion::NoSubjectValue)
    } else if peer_y.is_empty() {
        Some(DimensionExclusion::NoPeerValues)
    } else if z.is_none() {
        Some(DimensionExclusion::NoDispersion)
    } else if dim.x_extractor.is_none() {
        None
    } else if subject_x.is_none() {
        Some(DimensionExclusion::NoSubjectX)
    } else if regression.is_none() {
        Some(DimensionExclusion::RegressionUnusable)
    } else if std_residual.is_none() {
        Some(DimensionExclusion::NoResidualDispersion)
    } else {
        None
    };

    // Raw signal in "higher Y = cheap" orientation: the standardized regression
    // residual (positive = actual above fitted fair value), or the raw z-score
    // for univariate dimensions. The dimension's direction flag then maps it to
    // the composite convention (positive = cheap).
    let raw_signal = if dim.x_extractor.is_some() {
        std_residual
    } else {
        z
    };
    let signal = raw_signal
        .filter(|_| excluded_reason.is_none())
        .map(|raw| match dim.direction {
            ScoreDirection::HigherIsCheap => raw,
            ScoreDirection::HigherIsRich => -raw,
        });

    DimensionEvaluation {
        subject_y,
        subject_x,
        peer_y,
        peer_pairs,
        percentile,
        z_score: z,
        regression,
        signal,
        excluded_reason,
    }
}

/// Reject negative or non-finite dimension weights.
pub(super) fn validate_dimension_weights(dimensions: &[ScoringDimension]) -> Result<()> {
    if dimensions
        .iter()
        .any(|dim| !dim.weight.is_finite() || dim.weight < 0.0)
    {
        return Err(Error::Validation(
            "scoring dimension weights must be finite and non-negative".into(),
        ));
    }
    Ok(())
}

/// Score a subject company against its peer set across multiple dimensions.
///
/// For each `ScoringDimension`:
/// 1. Extract Y and X metrics from peers and subject.
/// 2. Compute percentile rank of subject's Y among peers' Y values.
/// 3. Compute z-score of subject's Y in the peer distribution.
/// 4. If X extractors are provided, run regression(s) and compute residual.
/// 5. Combine into a dimension score.
///
/// The composite score is the weighted average of per-dimension
/// standardized residuals (regression-based dimensions: the subject's
/// residual divided by the sample standard deviation of peer residuals
/// around the fitted line) or raw z-scores (univariate dimensions). Both
/// signals are unitless, so configured weights compare like with like.
/// Each dimension's [`ScoringDimension::direction`] maps its signal onto
/// the composite sign convention: positive = cheap.
/// Unusable dimensions are excluded and the remaining weights are normalized.
/// A regression dimension requires a finite fitted residual and positive peer
/// residual dispersion; it never falls back to a univariate score.
///
/// # Arguments
///
/// * `peer_set` - Peer universe containing the subject company and the
///   comparison companies from which metric distributions are built.
/// * `dimensions` - Non-empty scoring dimensions defining metric extractors,
///   direction conventions, and relative weights.
///
/// # Errors
///
/// Returns validation errors for an empty dimension set, negative/non-finite
/// weights, no usable positive-weight dimension, or a non-finite composite.
pub fn score_relative_value(
    peer_set: &PeerSet,
    dimensions: &[ScoringDimension],
) -> Result<RelativeValueResult> {
    let result = score_relative_value_unchecked(peer_set, dimensions)?;
    if !is_finite_score(&result) {
        return Err(Error::Validation(
            "relative value score must be finite".into(),
        ));
    }
    Ok(result)
}

/// Whether a score's composite and confidence are both finite, the one
/// condition [`score_relative_value`] adds to [`score_relative_value_unchecked`].
pub(super) fn is_finite_score(result: &RelativeValueResult) -> bool {
    result.composite_score.is_finite() && result.confidence.is_finite()
}

/// [`score_relative_value`] without its final finiteness check: the composite
/// and confidence may be non-finite. Every other error is the same.
///
/// # Arguments
///
/// * `peer_set` - Peer universe containing the subject and its peers.
/// * `dimensions` - Non-empty scoring dimensions with finite, non-negative
///   weights.
pub(super) fn score_relative_value_unchecked(
    peer_set: &PeerSet,
    dimensions: &[ScoringDimension],
) -> Result<RelativeValueResult> {
    if dimensions.is_empty() {
        return Err(Error::Validation(
            "at least one scoring dimension is required".into(),
        ));
    }
    validate_dimension_weights(dimensions)?;
    if !dimensions.iter().any(|dim| dim.weight > 0.0) {
        return Err(Error::Validation(
            "at least one usable positive-weight scoring dimension is required".into(),
        ));
    }

    let mut dim_scores = Vec::with_capacity(dimensions.len());
    let mut weighted_sum = 0.0;
    let mut total_weight = 0.0;
    let mut confidence_num = 0.0;
    let mut confidence_den = 0.0;
    let mut weight_scale = 0.0;
    let mut confidence_scale = 0.0;

    for dim in dimensions {
        let evaluation = evaluate_dimension(peer_set, dim);
        let (Some(score), Some(pctile), Some(zs)) =
            (evaluation.signal, evaluation.percentile, evaluation.z_score)
        else {
            continue;
        };
        let reg_residual = evaluation.regression.as_ref().map(|reg| reg.residual);
        let r_sq = evaluation.regression.as_ref().map(|reg| reg.r_squared);

        // Scaling all weights by the same factor preserves their relative
        // influence while avoiding overflow from large finite input weights.
        if dim.weight > weight_scale {
            let factor = weight_scale / dim.weight;
            weighted_sum *= factor;
            total_weight *= factor;
            weight_scale = dim.weight;
        }
        let weight = dim.weight / weight_scale;
        weighted_sum += weight * score;
        total_weight += weight;
        if let Some(rsq) = r_sq {
            // Confidence averages only regression dimensions. Scale that
            // subset independently so a large univariate weight cannot erase
            // otherwise usable regression confidence by underflow.
            if dim.weight > confidence_scale {
                let factor = confidence_scale / dim.weight;
                confidence_num *= factor;
                confidence_den *= factor;
                confidence_scale = dim.weight;
            }
            let confidence_weight = dim.weight / confidence_scale;
            confidence_num += confidence_weight * rsq;
            confidence_den += confidence_weight;
        }

        dim_scores.push(DimensionScore {
            label: dim.label.clone(),
            percentile: pctile,
            z_score: zs,
            regression_residual: reg_residual,
            r_squared: r_sq,
            weight: dim.weight,
        });
    }

    if total_weight <= 0.0 {
        return Err(Error::Validation(
            "no usable positive-weight scoring dimension".into(),
        ));
    }
    let composite = weighted_sum / total_weight;
    let confidence = if confidence_den > 0.0 {
        confidence_num / confidence_den
    } else {
        0.0
    };
    Ok(RelativeValueResult {
        company_id: peer_set.subject.id.clone(),
        composite_score: composite,
        dimensions: dim_scores,
        confidence,
        peer_count: peer_set.peer_count(),
    })
}

/// Extract metric values from all peers in the set.
fn extract_values(peer_set: &PeerSet, extractor: &MetricExtractor) -> Vec<f64> {
    peer_set
        .peers
        .iter()
        .filter_map(|c| extract_single(c, extractor))
        .collect()
}

/// Extract aligned `(company, x, y)` observations from peers, keeping a peer
/// only when both metrics are present and finite.
///
/// This guarantees positional alignment for regression: a peer missing
/// one of the two metrics is dropped entirely instead of shifting every
/// subsequent pairing.
fn extract_pairs<'a>(
    peer_set: &'a PeerSet,
    x_extractor: &MetricExtractor,
    y_extractor: &MetricExtractor,
) -> Vec<(&'a CompanyId, f64, f64)> {
    peer_set
        .peers
        .iter()
        .filter_map(|peer| {
            Some((
                &peer.id,
                extract_single(peer, x_extractor)?,
                extract_single(peer, y_extractor)?,
            ))
        })
        .collect()
}

/// Standardize the subject's regression residual against the peer
/// residual distribution.
///
/// Computes each peer's residual from the fitted line
/// `y - (intercept + slope * x)` and divides the subject residual by the
/// sample standard deviation of those residuals, yielding a unitless
/// signal comparable to a z-score.
///
/// Returns `None` when the peer residual dispersion is zero or
/// non-finite (e.g., a perfectly collinear peer set), in which case the
/// caller cannot meaningfully standardize.
fn standardized_residual(
    xs: &[f64],
    ys: &[f64],
    intercept: f64,
    slope: f64,
    subject_residual: f64,
) -> Option<f64> {
    let mut os = OnlineStats::new();
    for (x, y) in xs.iter().zip(ys.iter()) {
        os.update(y - (intercept + slope * x));
    }
    let sd = os.std_dev();
    if !sd.is_finite() || sd < 1e-15 {
        return None;
    }
    let std_res = subject_residual / sd;
    std_res.is_finite().then_some(std_res)
}

/// Extract the subject's metric value.
fn extract_subject_value(peer_set: &PeerSet, extractor: &MetricExtractor) -> Option<f64> {
    extract_single(&peer_set.subject, extractor)
}

/// Extract a single metric value from a `CompanyMetrics`.
fn extract_single(metrics: &CompanyMetrics, extractor: &MetricExtractor) -> Option<f64> {
    let value = match extractor {
        MetricExtractor::Named(name) => metrics.named_metric(name),
        MetricExtractor::Multiple(multiple) => compute_multiple(metrics, *multiple),
        MetricExtractor::Custom(key) => metrics.custom.get(key).copied(),
    };
    value.filter(|value| value.is_finite())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::comps::PeriodBasis;

    fn company(id: &str, leverage: Option<f64>, spread: Option<f64>) -> CompanyMetrics {
        let mut company = CompanyMetrics::new(id);
        company.leverage = leverage;
        company.oas_bp = spread;
        company
    }

    fn dimension(regression: bool, weight: f64) -> ScoringDimension {
        ScoringDimension {
            label: if regression {
                "regression"
            } else {
                "univariate"
            }
            .into(),
            y_extractor: MetricExtractor::Named("oas_bp".into()),
            x_extractor: regression.then(|| MetricExtractor::Named("leverage".into())),
            weight,
            direction: ScoreDirection::HigherIsCheap,
        }
    }

    #[test]
    fn unusable_regression_does_not_fall_back_to_an_univariate_signal() {
        let peers = vec![
            company("a", Some(1.0), Some(10.0)),
            company("b", Some(2.0), Some(20.0)),
            company("c", Some(3.0), Some(30.0)),
        ];
        // Subject is exactly on a perfect peer fair-value line but has a high
        // raw spread; substituting its z-score falsely labels it cheap.
        let set = PeerSet::new(
            company("subject", Some(4.0), Some(40.0)),
            peers,
            PeriodBasis::Ltm,
        );
        assert!(score_relative_value(&set, &[dimension(true, 1.0)]).is_err());
        let result =
            score_relative_value(&set, &[dimension(true, f64::MAX), dimension(false, 1.0)])
                .expect("usable univariate evidence");
        assert_eq!(result.dimensions.len(), 1);
        assert_eq!(result.dimensions[0].label, "univariate");
        assert_eq!(result.composite_score, 2.0);
        assert_eq!(result.confidence, 0.0);
    }

    #[test]
    fn no_data_or_zero_dispersion_is_unavailable_instead_of_neutral() {
        let subject = company("subject", Some(1.0), Some(10.0));
        for peers in [
            vec![],
            vec![company("a", None, None)],
            vec![
                company("a", None, Some(10.0)),
                company("b", None, Some(10.0)),
            ],
        ] {
            let set = PeerSet::new(subject.clone(), peers, PeriodBasis::Ltm);
            assert!(score_relative_value(&set, &[dimension(false, 1.0)]).is_err());
        }
    }

    #[test]
    fn missing_subject_predictor_excludes_regression() {
        let peers = vec![
            company("a", Some(1.0), Some(10.0)),
            company("b", Some(2.0), Some(23.0)),
            company("c", Some(3.0), Some(30.0)),
        ];
        let set = PeerSet::new(
            company("subject", None, Some(40.0)),
            peers,
            PeriodBasis::Ltm,
        );
        assert!(score_relative_value(&set, &[dimension(true, 1.0)]).is_err());
    }

    #[test]
    fn rejects_invalid_weights_and_requires_positive_usable_weight() {
        let peers = vec![
            company("a", None, Some(10.0)),
            company("b", None, Some(20.0)),
        ];
        let set = PeerSet::new(
            company("subject", None, Some(30.0)),
            peers,
            PeriodBasis::Ltm,
        );
        for weight in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            assert!(score_relative_value(&set, &[dimension(false, weight)]).is_err());
        }
        let result = score_relative_value(
            &set,
            &[dimension(false, f64::MAX), dimension(false, f64::MAX)],
        )
        .expect("large finite weights normalize");
        assert!(result.composite_score.is_finite());
        assert_eq!(
            result.composite_score,
            z_score(&[10.0, 20.0], 30.0).expect("z score")
        );
    }

    #[test]
    fn univariate_weight_does_not_underflow_regression_confidence() {
        let peers = vec![
            company("a", Some(1.0), Some(10.0)),
            company("b", Some(2.0), Some(23.0)),
            company("c", Some(3.0), Some(30.0)),
        ];
        let set = PeerSet::new(
            company("subject", Some(4.0), Some(40.0)),
            peers,
            PeriodBasis::Ltm,
        );
        let result = score_relative_value(
            &set,
            &[
                dimension(true, f64::MIN_POSITIVE),
                dimension(false, f64::MAX),
            ],
        )
        .expect("usable evidence");
        assert_eq!(result.dimensions.len(), 2);
        let regression_r_squared = result.dimensions[0]
            .r_squared
            .expect("regression confidence");
        assert!(regression_r_squared > 0.9);
        assert_eq!(result.confidence, regression_r_squared);
    }

    #[test]
    fn unchecked_score_differs_from_the_checked_one_only_on_a_non_finite_composite() {
        // Each z-score is about 1.4e308; their weighted sum overflows.
        let set = PeerSet::new(
            company("subject", None, Some(1e308)),
            vec![company("a", None, Some(1.0)), company("b", None, Some(2.0))],
            PeriodBasis::Ltm,
        );
        let dimensions = [dimension(false, 1.0), dimension(false, 1.0)];
        let unchecked =
            score_relative_value_unchecked(&set, &dimensions).expect("only the composite fails");
        assert!(!is_finite_score(&unchecked));
        assert!(score_relative_value(&set, &dimensions).is_err());

        // Every other rejection is an error of both, never a non-finite score.
        for dimensions in [
            vec![],
            vec![dimension(false, 0.0)],
            vec![dimension(true, 1.0)],
        ] {
            assert!(score_relative_value_unchecked(&set, &dimensions).is_err());
            assert!(score_relative_value(&set, &dimensions).is_err());
        }

        let finite = PeerSet::new(
            company("subject", None, Some(3.0)),
            vec![company("a", None, Some(1.0)), company("b", None, Some(2.0))],
            PeriodBasis::Ltm,
        );
        let dimensions = [dimension(false, 1.0)];
        let unchecked = score_relative_value_unchecked(&finite, &dimensions).expect("finite");
        assert!(is_finite_score(&unchecked));
        assert_eq!(
            Some(unchecked),
            score_relative_value(&finite, &dimensions).ok()
        );
    }
}
