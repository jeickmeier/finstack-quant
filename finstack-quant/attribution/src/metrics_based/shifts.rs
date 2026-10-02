use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::diff::{
    measure_discount_curve_shift, measure_inflation_index_shift, TenorSamplingMethod,
};
use finstack_quant_core::types::CurveId;
use finstack_quant_core::HashMap;
use finstack_quant_core::{Error, Result};
use finstack_quant_valuations::metrics::{parse_key_rate_label, MetricId};
use finstack_quant_valuations::recalibration::RecalibrationProvider;

/// Extract every supplied key-rate coordinate for the declared curves.
///
/// # Arguments
///
/// * `measures` - Flattened sensitivity metrics from the opening valuation.
/// * `curve_ids` - Complete declared curve set for this factor family.
/// * `metric_prefix` - Canonical per-tenor sensitivity metric identifier.
///
/// A nonempty bucketed family must cover every declared curve. Invalid labels,
/// duplicate coordinates and missing curves are errors, never partial success.
pub(crate) fn extract_keyrate_per_curve(
    measures: &indexmap::IndexMap<MetricId, f64>,
    curve_ids: &[CurveId],
    metric_prefix: &str,
) -> Result<HashMap<CurveId, Vec<(f64, f64)>>> {
    let mut result = HashMap::default();
    for curve_id in curve_ids {
        let prefix = format!("{metric_prefix}::{curve_id}::");
        let mut buckets = measures
            .iter()
            .filter_map(|(key, sensitivity)| {
                key.as_str()
                    .strip_prefix(&prefix)
                    .map(|label| parse_key_rate_label(label).map(|tenor| (tenor, *sensitivity)))
            })
            .collect::<Result<Vec<_>>>()?;
        buckets.sort_by(|left, right| left.0.total_cmp(&right.0));
        if buckets.windows(2).any(|pair| pair[0].0 >= pair[1].0) {
            return Err(Error::Validation(format!(
                "{metric_prefix} for '{curve_id}' contains duplicate year coordinates"
            )));
        }
        if !buckets.is_empty() {
            result.insert(curve_id.clone(), buckets);
        }
    }
    validate_curve_coverage(&result, curve_ids, metric_prefix)?;
    Ok(result)
}

fn validate_curve_coverage<T>(
    buckets: &HashMap<CurveId, T>,
    curve_ids: &[CurveId],
    metric_prefix: &str,
) -> Result<()> {
    if !buckets.is_empty() {
        for curve_id in curve_ids {
            if !buckets.contains_key(curve_id) {
                return Err(Error::Validation(format!(
                    "{metric_prefix} is incomplete: declared curve '{curve_id}' has no buckets"
                )));
            }
        }
    }
    Ok(())
}

/// One credit sensitivity paired with its observed, unit-consistent move.
pub(crate) struct CreditKeyRateBucket {
    pub(crate) tenor_years: f64,
    pub(crate) sensitivity: f64,
    pub(crate) move_bp: f64,
    /// Exact opening replay index; absent for caller-supplied curve-coordinate risk.
    pub(crate) quote_index: Option<usize>,
}

/// Pair all credit buckets with their original replay quote identities.
///
/// # Arguments
///
/// * `measures` - Opening quote-space CS01 metrics, in currency per basis point.
/// * `curve_ids` - Complete declared credit curve set.
/// * `market_t0` - Opening curves and exact replay quote bindings.
/// * `market_t1` - Closing curves and matching quote bindings.
/// * `provider` - Canonical calibration provider resolving exact opening pillars.
pub(crate) fn extract_credit_keyrates(
    measures: &indexmap::IndexMap<MetricId, f64>,
    curve_ids: &[CurveId],
    market_t0: &MarketContext,
    market_t1: &MarketContext,
    provider: &dyn RecalibrationProvider,
) -> Result<HashMap<CurveId, Vec<CreditKeyRateBucket>>> {
    use finstack_quant_calibration::quotes::cds::CdsQuote;
    let mut result = HashMap::default();
    for curve_id in curve_ids {
        let prefix = format!("bucketed_cs01::{curve_id}::");
        let supplied: indexmap::IndexMap<_, _> = measures
            .iter()
            .filter_map(|(key, value)| {
                key.as_str()
                    .strip_prefix(&prefix)
                    .map(|label| (label, *value))
            })
            .collect();
        if supplied.is_empty() {
            continue;
        }
        let hazard = market_t0.get_hazard(curve_id.as_str()).ok();
        let buckets = if let Some(recipe_t0) =
            hazard.as_ref().and_then(|curve| curve.hazard_calibration())
        {
            let hazard = hazard
                .as_ref()
                .ok_or_else(|| Error::Internal("missing hazard curve".into()))?;
            let closing_hazard = market_t1.get_hazard(curve_id.as_str())?;
            let recipe_t1 = closing_hazard.hazard_calibration().ok_or_else(|| {
                Error::Validation(format!(
                    "bucketed_cs01 for '{curve_id}' requires closing replay quote bindings"
                ))
            })?;
            let closing_buckets = provider.hazard_spread_risk_buckets(&closing_hazard)?;
            let replay_buckets = provider.hazard_spread_risk_buckets(hazard)?;
            if supplied.len() != replay_buckets.len() {
                return Err(Error::Validation(format!(
                    "bucketed_cs01 for '{curve_id}' has {} buckets but the replay contract requires {}",
                    supplied.len(), replay_buckets.len()
                )));
            }
            let closing_quotes = closing_buckets
                .iter()
                .map(|bucket| {
                    let input = recipe_t1
                        .spread_risk_inputs
                        .get(bucket.quote_index)
                        .ok_or_else(|| {
                            Error::Validation(format!(
                                "invalid closing replay quote index {} for '{curve_id}'",
                                bucket.quote_index
                            ))
                        })?;
                    let quote: CdsQuote =
                        serde_json::from_value(input.quote.clone()).map_err(|error| {
                            Error::Validation(format!("invalid closing CDS replay quote: {error}"))
                        })?;
                    Ok((bucket.pillar_date, quote))
                })
                .collect::<Result<Vec<_>>>()?;
            replay_buckets
                .iter()
                .map(|bucket| {
                    let label = bucket.get_metric_label(&replay_buckets);
                    let sensitivity = supplied.get(label.as_str()).copied().ok_or_else(|| {
                        Error::Validation(format!(
                    "bucketed_cs01 for '{curve_id}' is missing exact replay bucket '{label}'"
                ))
                    })?;
                    let input = recipe_t0
                        .spread_risk_inputs
                        .get(bucket.quote_index)
                        .ok_or_else(|| {
                            Error::Validation(format!(
                                "invalid replay quote index {} for '{curve_id}'",
                                bucket.quote_index
                            ))
                        })?;
                    let opening_quote: CdsQuote = serde_json::from_value(input.quote.clone())
                        .map_err(|error| {
                            Error::Validation(format!("invalid opening CDS replay quote: {error}"))
                        })?;
                    let mut matching = closing_quotes.iter().filter(|(date, quote)| {
                        *date == bucket.pillar_date && quote.id().as_str() == bucket.quote_id
                    });
                    let (_, closing_quote) = matching.next().ok_or_else(|| {
                        Error::Validation(format!(
                            "bucketed_cs01 for '{curve_id}' has no closing quote '{}' at {}",
                            bucket.quote_id, bucket.pillar_date
                        ))
                    })?;
                    if matching.next().is_some() {
                        return Err(Error::Validation(format!(
                            "ambiguous closing CDS replay quote '{}'",
                            bucket.quote_id
                        )));
                    }
                    Ok(CreditKeyRateBucket {
                        tenor_years: bucket.pillar_time,
                        sensitivity,
                        move_bp: closing_quote.coupon_bp() - opening_quote.coupon_bp(),
                        quote_index: Some(bucket.quote_index),
                    })
                })
                .collect::<Result<Vec<_>>>()?
        } else {
            // Caller-supplied curve-coordinate CS01 uses the same per-tenor
            // par-spread/zero-rate measurement as its declared curve family.
            let numeric = extract_keyrate_per_curve(
                measures,
                std::slice::from_ref(curve_id),
                "bucketed_cs01",
            )?;
            let coordinates = numeric
                .get(curve_id)
                .ok_or_else(|| Error::Internal("missing credit coordinates".into()))?;
            let tenors: Vec<_> = coordinates.iter().map(|(tenor, _)| *tenor).collect();
            let moves =
                finstack_quant_core::market_data::diff::measure_per_tenor_credit_curve_shift(
                    curve_id.as_str(),
                    market_t0,
                    market_t1,
                    &tenors,
                )?;
            coordinates
                .iter()
                .zip(moves)
                .map(|((tenor, sensitivity), move_bp)| CreditKeyRateBucket {
                    tenor_years: *tenor,
                    sensitivity: *sensitivity,
                    move_bp,
                    quote_index: None,
                })
                .collect()
        };
        result.insert(curve_id.clone(), buckets);
    }
    validate_curve_coverage(&result, curve_ids, "bucketed_cs01")?;
    Ok(result)
}

/// Measure the per-tenor discount-curve zero-rate shift (in basis points) at
/// the supplied tenors.
///
/// Unlike [`measure_discount_curve_shift`], which averages the shift over a
/// fixed tenor grid (and so mis-attributes a non-parallel move), this returns
/// the shift at each requested tenor so the caller can pair it with the
/// per-tenor (key-rate) DV01.
fn measure_per_tenor_discount_shift(
    curve_id: &str,
    market_t0: &MarketContext,
    market_t1: &MarketContext,
    tenors: &[f64],
) -> Option<Vec<f64>> {
    let curve_t0 = market_t0.get_discount(curve_id).ok()?;
    let curve_t1 = market_t1.get_discount(curve_id).ok()?;
    Some(
        tenors
            .iter()
            .map(|&t| (curve_t1.zero(t) - curve_t0.zero(t)) * 10_000.0)
            .collect(),
    )
}

/// Per-tenor rate shift (bp) for a rates curve that may be a discount curve
/// (zero rates) **or** a forward/projection curve (forward rates).
///
/// the rates ladder must consume forward-curve DV01 too —
/// `BucketedDv01` emits per-tenor series for projection curves, and a basis
/// move (discount and forward moving differently) is mis-attributed when only
/// discount curves are measured.
pub(super) fn measure_per_tenor_rate_shift(
    curve_id: &str,
    market_t0: &MarketContext,
    market_t1: &MarketContext,
    tenors: &[f64],
) -> Option<Vec<f64>> {
    if let Some(shifts) = measure_per_tenor_discount_shift(curve_id, market_t0, market_t1, tenors) {
        return Some(shifts);
    }
    let curve_t0 = market_t0.get_forward(curve_id).ok()?;
    let curve_t1 = market_t1.get_forward(curve_id).ok()?;
    Some(
        tenors
            .iter()
            .map(|&t| (curve_t1.rate(t) - curve_t0.rate(t)) * 10_000.0)
            .collect(),
    )
}

/// Mean over the standard tenor grid (`t > 0`) of the per-tenor move
/// `r1 − r0` in basis points, taking `|Δ|` when `absolute`. Tenors where
/// either side is non-finite are skipped; `None` when no tenor contributed.
///
/// # Arguments
///
/// * `sample` - Returns `(r0, r1)` — the T₀ and T₁ rate at a tenor in years.
/// * `absolute` - `true` for the L1 mean used by the twist guards, `false`
///   for the signed mean.
fn mean_tenor_shift_bp(sample: impl Fn(f64) -> (f64, f64), absolute: bool) -> Option<f64> {
    use finstack_quant_core::market_data::diff::STANDARD_TENORS;
    let mut total = 0.0;
    let mut count = 0usize;
    for &t in STANDARD_TENORS {
        if t <= 0.0 {
            continue;
        }
        let (r0, r1) = sample(t);
        if r0.is_finite() && r1.is_finite() {
            let delta = r1 - r0;
            total += if absolute {
                delta.abs() * 10_000.0
            } else {
                delta * 10_000.0
            };
            count += 1;
        }
    }
    if count == 0 {
        None
    } else {
        Some(total / count as f64)
    }
}

/// Arithmetic mean of `shift(item)` over the items where it is `Some`.
///
/// # Arguments
///
/// * `items` - Curves, ids or other keys to sample.
/// * `shift` - Per-item shift; `None` excludes the item from the mean.
///
/// Returns `(mean, count)`; the mean is `None` when nothing contributed.
pub(super) fn average_over<T>(
    items: impl IntoIterator<Item = T>,
    shift: impl Fn(T) -> Option<f64>,
) -> (Option<f64>, usize) {
    let mut total = 0.0;
    let mut count = 0usize;
    for item in items {
        if let Some(value) = shift(item) {
            total += value;
            count += 1;
        }
    }
    (
        if count > 0 {
            Some(total / count as f64)
        } else {
            None
        },
        count,
    )
}

/// Signed mean forward-rate shift (bp) over the standard tenor grid —
/// forward-curve counterpart of `measure_discount_curve_shift`.
fn measure_forward_curve_shift_bp(
    curve_id: &str,
    market_t0: &MarketContext,
    market_t1: &MarketContext,
) -> Option<f64> {
    let curve_t0 = market_t0.get_forward(curve_id).ok()?;
    let curve_t1 = market_t1.get_forward(curve_id).ok()?;
    mean_tenor_shift_bp(|t| (curve_t0.rate(t), curve_t1.rate(t)), false)
}

/// Signed mean rate shift (bp) for a curve that may be a discount or a
/// forward/projection curve.
pub(super) fn measure_rate_curve_shift_bp(
    curve_id: &str,
    market_t0: &MarketContext,
    market_t1: &MarketContext,
) -> Option<f64> {
    measure_discount_curve_shift(
        curve_id,
        market_t0,
        market_t1,
        TenorSamplingMethod::Standard,
    )
    .ok()
    .or_else(|| measure_forward_curve_shift_bp(curve_id, market_t0, market_t1))
}

/// Mean of the per-tenor *absolute* discount-curve zero-rate shift (bp) on
/// the standard tenor grid.
///
/// Where [`measure_discount_curve_shift`] returns the signed mean (which
/// collapses toward zero for a twist), this returns the L1 mean so a
/// non-parallel move still registers a large magnitude. Used by the
/// rates-convexity block to detect "the average is small but the curve
/// genuinely moved".
///
/// Returns `0.0` if either side's curve is missing.
fn discount_curve_abs_shift_bp(
    curve_id: &str,
    market_t0: &MarketContext,
    market_t1: &MarketContext,
) -> f64 {
    let (Ok(c0), Ok(c1)) = (
        market_t0.get_discount(curve_id),
        market_t1.get_discount(curve_id),
    ) else {
        return 0.0;
    };
    mean_tenor_shift_bp(|t| (c0.zero(t), c1.zero(t)), true).unwrap_or(0.0)
}

/// L1-mean rate shift (bp) for a curve that may be a discount or a
/// forward/projection curve. Forward-aware counterpart of
/// [`discount_curve_abs_shift_bp`] for the twist-guard block.
pub(super) fn rate_curve_abs_shift_bp(
    curve_id: &str,
    market_t0: &MarketContext,
    market_t1: &MarketContext,
) -> f64 {
    let v = discount_curve_abs_shift_bp(curve_id, market_t0, market_t1);
    if v > 0.0 {
        return v;
    }
    let (Ok(c0), Ok(c1)) = (
        market_t0.get_forward(curve_id),
        market_t1.get_forward(curve_id),
    ) else {
        return 0.0;
    };
    mean_tenor_shift_bp(|t| (c0.rate(t), c1.rate(t)), true).unwrap_or(0.0)
}

/// Threshold below which a signed mean shift is considered twist-dominated
/// relative to its L1 magnitude. Below this level, signed-average convexity
/// understates the true quadratic contribution, so downstream consumers should
/// fall back to per-tenor convexity.
const TWIST_FRACTION_THRESHOLD: f64 = 1e-2;

/// Mean of the per-tenor *absolute* credit-curve shift (bp) on the standard
/// tenor grid. Counterpart of [`discount_curve_abs_shift_bp`] for credit.
///
/// For a hazard curve this is the L1 mean of the par CDS spread move; for a
/// discount-style credit curve it is the L1 mean of the zero-rate move. Either way it pairs with the signed
/// mean that the per-method credit attribution consumes.
///
/// Returns `0.0` if either side's curve is missing.
pub(super) fn credit_curve_abs_shift_bp(
    curve_id: &str,
    market_t0: &MarketContext,
    market_t1: &MarketContext,
) -> f64 {
    use finstack_quant_core::market_data::diff::STANDARD_TENORS;
    let tenors: Vec<f64> = STANDARD_TENORS
        .iter()
        .copied()
        .filter(|t| *t > 0.0)
        .collect();
    let Ok(shifts) = finstack_quant_core::market_data::diff::measure_per_tenor_credit_curve_shift(
        curve_id, market_t0, market_t1, &tenors,
    ) else {
        return 0.0;
    };
    let (total_abs, count) = shifts
        .iter()
        .filter(|v| v.is_finite())
        .fold((0.0, 0usize), |(acc, n), v| (acc + v.abs(), n + 1));
    if count == 0 {
        0.0
    } else {
        total_abs / count as f64
    }
}

/// Mean of the per-tenor *absolute* inflation-curve shift (bp) on the standard
/// tenor grid. Counterpart of [`discount_curve_abs_shift_bp`] for inflation.
///
/// Returns `0.0` if either side's curve is missing.
pub(super) fn inflation_source_abs_shift_bp(
    curve_id: &str,
    market_t0: &MarketContext,
    market_t1: &MarketContext,
) -> f64 {
    let index_abs = measure_inflation_index_shift(curve_id, market_t0, market_t1)
        .map(f64::abs)
        .unwrap_or(0.0);
    let (Ok(c0), Ok(c1)) = (
        market_t0.get_inflation_curve(curve_id),
        market_t1.get_inflation_curve(curve_id),
    ) else {
        return index_abs;
    };
    // Inflation rate at tenor t from the cpi ratio (mirrors the
    // measure_inflation_curve_shift formula in core::market_data::diff).
    let rate =
        |c: &finstack_quant_core::market_data::term_structures::InflationCurve, t: f64| -> f64 {
            let ratio = c.cpi(t) / c.base_cpi();
            ratio.powf(1.0 / t) - 1.0
        };
    mean_tenor_shift_bp(|t| (rate(&c0, t), rate(&c1, t)), true).unwrap_or(0.0)
}

/// Format a diagnostic note when a signed average shift is twist-dominated
/// — i.e. `|signed_avg| < TWIST_FRACTION_THRESHOLD × l1_avg`. In that regime,
/// scalar second-order terms `½·γ·avg²` collapse toward 0 even though the
/// true `½·Δxᵀ·H·Δx` contribution is non-trivial.
///
/// Returns `None` when not twist-dominated (signed average is the dominant
/// component) or when there is no L1 magnitude to compare against.
pub(super) fn twist_diagnostic_note(
    factor_label: &str,
    signed_avg: f64,
    l1_avg: f64,
) -> Option<String> {
    if l1_avg <= 0.0 {
        return None;
    }
    if signed_avg.abs() >= TWIST_FRACTION_THRESHOLD * l1_avg {
        return None;
    }
    Some(format!(
        "{factor_label} second-order may be understated: curves twisted \
         (signed mean shift {signed_avg:.3}bp vs L1 mean shift {l1_avg:.3}bp); \
         the scalar `½·γ·avg²` term collapses for twist-dominated moves. \
         Consider per-tenor second-order or parallel/waterfall attribution \
         for an accurate second-order contribution."
    ))
}
