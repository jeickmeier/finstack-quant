//! Composite rich/cheap scoring across multiple valuation dimensions.
//!
//! Combines percentile rank, z-score, and regression residual signals
//! into a weighted composite relative value score.

use super::multiples::compute_multiple;
use super::peer_set::PeerSet;
use super::stats::{percentile_rank, regression_fair_value, z_score};
use super::types::{CompanyId, CompanyMetrics, Multiple};
use finstack_quant_core::math::stats::OnlineStats;
use finstack_quant_core::{Error, Result};
use serde::{Deserialize, Serialize};

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
#[serde(rename_all = "snake_case")]
pub enum ScoreDirection {
    /// Higher Y than peers means the subject is cheap (spread-like metrics).
    #[default]
    HigherIsCheap,
    /// Higher Y than peers means the subject is rich (multiple-like metrics).
    HigherIsRich,
}

/// Configuration for a single rich/cheap scoring dimension.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoringDimension {
    /// Human-readable label (e.g., "Spread vs Leverage").
    pub label: String,
    /// How to extract the Y variable (dependent) from CompanyMetrics.
    pub y_extractor: MetricExtractor,
    /// How to extract the X variable(s) (explanatory) from CompanyMetrics.
    pub x_extractors: Vec<MetricExtractor>,
    /// Weight of this dimension in the composite score (0.0 to 1.0).
    pub weight: f64,
    /// Rich/cheap sign convention for the Y metric (default:
    /// [`ScoreDirection::HigherIsCheap`], i.e. spread-like).
    #[serde(default)]
    pub direction: ScoreDirection,
}

/// Identifies which metric to extract from CompanyMetrics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MetricExtractor {
    /// A named field (e.g., "leverage", "oas_bps", "ebitda_margin").
    Named(String),
    /// A valuation multiple.
    Multiple(Multiple),
    /// A custom metric key from the `custom` map.
    Custom(String),
}

/// Decomposed score for a single dimension.
#[derive(Debug, Clone, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Serialize, Deserialize)]
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
pub fn score_relative_value(
    peer_set: &PeerSet,
    dimensions: &[ScoringDimension],
) -> Result<RelativeValueResult> {
    if dimensions.is_empty() {
        return Err(Error::Validation(
            "at least one scoring dimension is required".into(),
        ));
    }

    let mut dim_scores = Vec::with_capacity(dimensions.len());
    let mut weighted_sum = 0.0;
    let mut total_weight = 0.0;
    let mut confidence_num = 0.0;
    let mut confidence_den = 0.0;

    for dim in dimensions {
        // Extract Y values from peers and subject
        let peer_y = extract_values(peer_set, &dim.y_extractor);
        let subject_y = extract_subject_value(peer_set, &dim.y_extractor);
        let (peer_vals, subject_val) = match (peer_y.as_slice(), subject_y) {
            (vals, Some(sv)) if !vals.is_empty() => (vals, sv),
            _ => continue, // Skip dimension if data is insufficient
        };

        let pctile = percentile_rank(peer_vals, subject_val).unwrap_or(0.5);
        let zs = z_score(peer_vals, subject_val).unwrap_or(0.0);

        let (reg_residual, r_sq, std_residual) = if !dim.x_extractors.is_empty() {
            // Run single-factor regression using the first X extractor.
            // Extract (x, y) pairwise per peer so a peer missing one metric
            // drops the *pair* instead of misaligning the two vectors.
            let (peer_x, peer_y_aligned) =
                extract_pairs(peer_set, &dim.x_extractors[0], &dim.y_extractor);
            let subject_x = extract_subject_value(peer_set, &dim.x_extractors[0]);
            match subject_x {
                Some(sx) if peer_x.len() >= 3 => {
                    match regression_fair_value(&peer_x, &peer_y_aligned, sx, subject_val) {
                        Some(reg) => {
                            let std_res = standardized_residual(
                                &peer_x,
                                &peer_y_aligned,
                                reg.intercept,
                                reg.slope,
                                reg.residual,
                            );
                            (Some(reg.residual), Some(reg.r_squared), std_res)
                        }
                        None => (None, None, None),
                    }
                }
                _ => (None, None, None),
            }
        } else {
            (None, None, None)
        };

        // Raw signal in "higher Y = cheap" orientation: the standardized
        // regression residual (positive = actual above fitted fair value),
        // or the raw z-score for univariate dimensions. The dimension's
        // direction flag then maps it to the composite convention
        // (positive = cheap).
        let raw_signal = std_residual.unwrap_or(zs);
        let score = match dim.direction {
            ScoreDirection::HigherIsCheap => raw_signal,
            ScoreDirection::HigherIsRich => -raw_signal,
        };
        weighted_sum += dim.weight * score;
        total_weight += dim.weight;
        if let Some(rsq) = r_sq {
            confidence_num += dim.weight * rsq;
            confidence_den += dim.weight;
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

    let composite = if total_weight > 0.0 {
        weighted_sum / total_weight
    } else {
        0.0
    };
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

/// Extract aligned `(x, y)` pairs from peers, keeping a peer only when
/// both metrics are present and finite.
///
/// This guarantees positional alignment for regression: a peer missing
/// one of the two metrics is dropped entirely instead of shifting every
/// subsequent pairing.
fn extract_pairs(
    peer_set: &PeerSet,
    x_extractor: &MetricExtractor,
    y_extractor: &MetricExtractor,
) -> (Vec<f64>, Vec<f64>) {
    let mut xs = Vec::with_capacity(peer_set.peers.len());
    let mut ys = Vec::with_capacity(peer_set.peers.len());
    for peer in &peer_set.peers {
        if let (Some(x), Some(y)) = (
            extract_single(peer, x_extractor),
            extract_single(peer, y_extractor),
        ) {
            if x.is_finite() && y.is_finite() {
                xs.push(x);
                ys.push(y);
            }
        }
    }
    (xs, ys)
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
    match extractor {
        MetricExtractor::Named(name) => match name.as_str() {
            "enterprise_value" => metrics.enterprise_value,
            "market_cap" => metrics.market_cap,
            "share_price" => metrics.share_price,
            "oas_bps" => metrics.oas_bps,
            "yield_pct" => metrics.yield_pct,
            "ebitda" => metrics.ebitda,
            "revenue" => metrics.revenue,
            "ebit" => metrics.ebit,
            "ufcf" => metrics.ufcf,
            "lfcf" => metrics.lfcf,
            "net_income" => metrics.net_income,
            "book_value" => metrics.book_value,
            "tangible_book_value" => metrics.tangible_book_value,
            "dividends_per_share" => metrics.dividends_per_share,
            "leverage" => metrics.leverage,
            "interest_coverage" => metrics.interest_coverage,
            "revenue_growth" => metrics.revenue_growth,
            "ebitda_margin" => metrics.ebitda_margin,
            _ => None,
        },
        MetricExtractor::Multiple(multiple) => compute_multiple(metrics, *multiple),
        MetricExtractor::Custom(key) => metrics.custom.get(key).copied(),
    }
}
