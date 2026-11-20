use finstack_quant_calibration::quotes::CdsQuote;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::diff::{
    HazardDiffQuery, HazardDiffScale, RateDiffScale, ZeroRateDiffParams,
    measure_hazard_intensity_diff, measure_price_scalar_diff, measure_unitless_scalar_diff,
    zero_rate_difference,
};
use finstack_quant_core::market_data::term_structures::HazardCurve;
use finstack_quant_core::math::stats;
use finstack_quant_core::types::CurveId;

#[inline]
fn exact_f64_eq(a: f64, b: f64) -> bool {
    a.to_bits() == b.to_bits() || ((a.to_bits() << 1) == 0 && (b.to_bits() << 1) == 0)
}
use finstack_quant_core::{Error, FxHashMap, Result};
use finstack_quant_valuations::metrics::{MetricId, parse_key_rate_label};
use finstack_quant_valuations::recalibration::RecalibrationProvider;

/// Diagnostic tenor grid (in years) shared by aggregate rolling-horizon attribution shift helpers.
pub(crate) const DIAGNOSTIC_RATE_TENORS_YEARS: [f64; 9] =
    [0.25, 0.5, 1.0, 2.0, 3.0, 5.0, 7.0, 10.0, 30.0];

/// Explicit rate-curve family preventing silent discount/forward registry fallback.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RateCurveFamily {
    /// Discount-factor curve measured via continuously compounded zero rates.
    Discount,
    /// Projection-factor curve measured via continuously compounded zero rates.
    Forward,
}

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
) -> Result<FxHashMap<CurveId, Vec<(f64, f64)>>> {
    let mut result = FxHashMap::default();
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
    buckets: &FxHashMap<CurveId, T>,
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
) -> Result<FxHashMap<CurveId, Vec<CreditKeyRateBucket>>> {
    let mut result = FxHashMap::default();
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
                    supplied.len(),
                    replay_buckets.len()
                )));
            }
            let closing_quotes = closing_buckets
                .iter()
                .map(|bucket| {
                    let input = recipe_t1
                        .spread_risk_inputs()
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
                        .spread_risk_inputs()
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
            let moves = measure_per_tenor_credit_curve_shift_bp(
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

/// Pair direct-intensity hazard sensitivity buckets (`bucketed_hazard01`) with native
/// [`measure_hazard_intensity_diff`] shifts in intensity basis points.
///
/// # Arguments
///
/// * `measures` - Opening sensitivity map containing optional `bucketed_hazard01::<curve>::<tenor>` entries.
/// * `curve_ids` - Complete declared credit curve set.
/// * `market_t0` - Earlier market snapshot.
/// * `market_t1` - Later market snapshot.
pub(crate) fn extract_hazard_intensity_keyrates(
    measures: &indexmap::IndexMap<MetricId, f64>,
    curve_ids: &[CurveId],
    market_t0: &MarketContext,
    market_t1: &MarketContext,
) -> Result<FxHashMap<CurveId, Vec<CreditKeyRateBucket>>> {
    let per_curve = extract_keyrate_per_curve(measures, curve_ids, "bucketed_hazard01")?;
    let mut result = FxHashMap::default();
    for (curve_id, coordinates) in per_curve {
        let tenors: Vec<f64> = coordinates.iter().map(|(tenor, _)| *tenor).collect();
        let diff = measure_hazard_intensity_diff(
            &HazardDiffQuery::new(curve_id.as_str(), &tenors, HazardDiffScale::IntensityBp),
            market_t0,
            market_t1,
        )?;
        let buckets = coordinates
            .into_iter()
            .zip(diff.into_rows())
            .map(|((tenor_years, sensitivity), row)| CreditKeyRateBucket {
                tenor_years,
                sensitivity,
                move_bp: row.diff,
                quote_index: None,
            })
            .collect();
        result.insert(curve_id, buckets);
    }
    Ok(result)
}

/// Measure aggregate native hazard intensity shift (intensity bp) strictly over [`MarketContext::get_hazard`].
///
/// # Arguments
///
/// * `curve_id` - Hazard curve identifier in both snapshots.
/// * `market_t0` - Earlier market snapshot.
/// * `market_t1` - Later market snapshot.
pub(crate) fn measure_hazard_intensity_shift_bp(
    curve_id: &str,
    market_t0: &MarketContext,
    market_t1: &MarketContext,
) -> Result<f64> {
    let h0 = market_t0.get_hazard(curve_id)?;
    let times: Vec<f64> = h0.knots().to_vec();
    let diff = measure_hazard_intensity_diff(
        &HazardDiffQuery::new(curve_id, &times, HazardDiffScale::IntensityBp),
        market_t0,
        market_t1,
    )?;
    let shifts: Vec<f64> = diff.into_rows().into_iter().map(|row| row.diff).collect();
    stats::mean(&shifts)?.ok_or_else(|| {
        Error::Validation("hazard intensity shift produced an empty mean".to_string())
    })
}

/// Measure the per-tenor discount-curve zero-rate shift (in basis points) at
/// the supplied tenors strictly from [`MarketContext::get_discount`].
///
/// # Arguments
///
/// * `curve_id` - Discount curve identifier in both market snapshots.
/// * `market_t0` - Earlier market snapshot.
/// * `market_t1` - Later market snapshot.
/// * `tenors` - Non-empty slice of strictly positive tenor coordinates in years.
pub(crate) fn measure_discount_curve_tenor_shifts_bp(
    curve_id: &str,
    market_t0: &MarketContext,
    market_t1: &MarketContext,
    tenors: &[f64],
) -> Result<Vec<f64>> {
    if tenors.is_empty() {
        return Err(Error::Validation(
            "discount curve tenor shift measurement requires at least one tenor".to_string(),
        ));
    }
    let curve_t0 = market_t0.get_discount(curve_id)?;
    let curve_t1 = market_t1.get_discount(curve_id)?;
    factor_zero_rate_shifts_bp(tenors, |t| curve_t0.df(t), |t| curve_t1.df(t))
}

/// Continuously compounded zero-rate shift (bp) between two factor curves at each tenor.
fn factor_zero_rate_shifts_bp(
    tenors: &[f64],
    df_t0: impl Fn(f64) -> Result<f64>,
    df_t1: impl Fn(f64) -> Result<f64>,
) -> Result<Vec<f64>> {
    tenors
        .iter()
        .map(|&t| {
            zero_rate_difference(ZeroRateDiffParams {
                d0_anchor: 1.0,
                d0_maturity: df_t0(t)?,
                d1_anchor: 1.0,
                d1_maturity: df_t1(t)?,
                alpha: t,
                scale: RateDiffScale::BasisPoints,
            })
        })
        .collect()
}

/// Measure the signed mean discount-curve zero-rate shift (in bp) over [`DIAGNOSTIC_RATE_TENORS_YEARS`].
///
/// # Arguments
///
/// * `curve_id` - Discount curve identifier in both market snapshots.
/// * `market_t0` - Earlier market snapshot.
/// * `market_t1` - Later market snapshot.
pub(crate) fn measure_discount_curve_shift_bp(
    curve_id: &str,
    market_t0: &MarketContext,
    market_t1: &MarketContext,
) -> Result<f64> {
    let shifts = measure_discount_curve_tenor_shifts_bp(
        curve_id,
        market_t0,
        market_t1,
        &DIAGNOSTIC_RATE_TENORS_YEARS,
    )?;
    stats::mean(&shifts)?
        .ok_or_else(|| Error::Validation("discount curve shift produced an empty mean".to_string()))
}

/// Measure the per-tenor projection-factor zero-rate shift (in basis points)
/// strictly from [`MarketContext::get_forward`].
///
/// Uses the continuously compounded zero rate of the projection factor, the
/// same coordinate that forward-curve key-rate bumps shock.
///
/// # Arguments
///
/// * `curve_id` - Forward curve identifier in both market snapshots.
/// * `market_t0` - Earlier market snapshot.
/// * `market_t1` - Later market snapshot.
/// * `tenors` - Non-empty slice of strictly positive tenor coordinates in years.
pub(crate) fn measure_forward_curve_tenor_shifts_bp(
    curve_id: &str,
    market_t0: &MarketContext,
    market_t1: &MarketContext,
    tenors: &[f64],
) -> Result<Vec<f64>> {
    if tenors.is_empty() {
        return Err(Error::Validation(
            "forward curve tenor shift measurement requires at least one tenor".to_string(),
        ));
    }
    let curve_t0 = market_t0.get_forward(curve_id)?;
    let curve_t1 = market_t1.get_forward(curve_id)?;
    factor_zero_rate_shifts_bp(tenors, |t| curve_t0.df(t), |t| curve_t1.df(t))
}

/// Signed mean projection-factor zero-rate shift (bp) over [`DIAGNOSTIC_RATE_TENORS_YEARS`].
fn measure_forward_curve_shift_bp(
    curve_id: &str,
    market_t0: &MarketContext,
    market_t1: &MarketContext,
) -> Result<f64> {
    let shifts = measure_forward_curve_tenor_shifts_bp(
        curve_id,
        market_t0,
        market_t1,
        &DIAGNOSTIC_RATE_TENORS_YEARS,
    )?;
    stats::mean(&shifts)?
        .ok_or_else(|| Error::Validation("forward curve shift produced an empty mean".to_string()))
}

/// Per-tenor rate shift (bp) dispatched strictly on the declared [`RateCurveFamily`].
pub(super) fn measure_per_tenor_rate_shift(
    curve_id: &str,
    family: RateCurveFamily,
    market_t0: &MarketContext,
    market_t1: &MarketContext,
    tenors: &[f64],
) -> Result<Vec<f64>> {
    match family {
        RateCurveFamily::Discount => {
            measure_discount_curve_tenor_shifts_bp(curve_id, market_t0, market_t1, tenors)
        }
        RateCurveFamily::Forward => {
            measure_forward_curve_tenor_shifts_bp(curve_id, market_t0, market_t1, tenors)
        }
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
    mut shift: impl FnMut(T) -> Option<f64>,
) -> (Option<f64>, usize) {
    let values: Vec<f64> = items.into_iter().filter_map(&mut shift).collect();
    let count = values.len();
    let mean = stats::mean(&values).ok().flatten();
    (mean, count)
}

fn l1_mean(shifts: &[f64]) -> f64 {
    let abs_vals: Vec<f64> = shifts
        .iter()
        .copied()
        .filter(|v| v.is_finite())
        .map(f64::abs)
        .collect();
    stats::mean(&abs_vals).ok().flatten().unwrap_or(0.0)
}

/// Signed mean rate shift (bp) dispatched strictly on the declared [`RateCurveFamily`].
pub(super) fn measure_rate_curve_shift_bp(
    curve_id: &str,
    family: RateCurveFamily,
    market_t0: &MarketContext,
    market_t1: &MarketContext,
) -> Result<f64> {
    match family {
        RateCurveFamily::Discount => {
            measure_discount_curve_shift_bp(curve_id, market_t0, market_t1)
        }
        RateCurveFamily::Forward => measure_forward_curve_shift_bp(curve_id, market_t0, market_t1),
    }
}

/// L1-mean rate shift (bp) dispatched strictly on [`RateCurveFamily`] for twist diagnostics.
///
/// Returns `0.0` if either side's curve is missing or invalid.
pub(super) fn rate_curve_abs_shift_bp(
    curve_id: &str,
    family: RateCurveFamily,
    market_t0: &MarketContext,
    market_t1: &MarketContext,
) -> f64 {
    match family {
        RateCurveFamily::Discount => measure_discount_curve_tenor_shifts_bp(
            curve_id,
            market_t0,
            market_t1,
            &DIAGNOSTIC_RATE_TENORS_YEARS,
        ),
        RateCurveFamily::Forward => measure_forward_curve_tenor_shifts_bp(
            curve_id,
            market_t0,
            market_t1,
            &DIAGNOSTIC_RATE_TENORS_YEARS,
        ),
    }
    .map(|shifts| l1_mean(&shifts))
    .unwrap_or(0.0)
}

/// Extract matched quote-space delta tuples `(pillar_time, delta_bp)` from two calibrated hazard recipes.
fn extract_matched_quote_deltas(
    curve_id: &str,
    h0: &HazardCurve,
    h1: &HazardCurve,
) -> Result<Vec<(f64, f64)>> {
    let (Some(recipe0), Some(recipe1)) = (h0.hazard_calibration(), h1.hazard_calibration()) else {
        return Err(Error::Validation(format!(
            "credit par-spread diff for '{curve_id}' requires matched hazard calibration recipes on both snapshots"
        )));
    };
    let inputs0 = recipe0.spread_risk_inputs();
    let inputs1 = recipe1.spread_risk_inputs();
    if inputs0.is_empty() || inputs0.len() != inputs1.len() {
        return Err(Error::Validation(format!(
            "credit par-spread diff for '{curve_id}' requires aligned non-empty spread_risk_inputs across both snapshots"
        )));
    }
    let parsed1 = inputs1
        .iter()
        .map(|item| {
            serde_json::from_value::<CdsQuote>(item.quote.clone())
                .map(|q| (item.pillar_date, q))
                .map_err(|error| {
                    Error::Validation(format!(
                        "invalid closing spread-risk quote on '{curve_id}': {error}"
                    ))
                })
        })
        .collect::<Result<Vec<_>>>()?;

    let mut deltas = Vec::with_capacity(inputs0.len());
    for item0 in inputs0 {
        let q0: CdsQuote = serde_json::from_value(item0.quote.clone()).map_err(|error| {
            Error::Validation(format!(
                "invalid opening spread-risk quote on '{curve_id}': {error}"
            ))
        })?;
        let mut matches = parsed1.iter().filter(|(date1, q1)| {
            *date1 == item0.pillar_date && q1.id().as_str() == q0.id().as_str()
        });
        let Some((_, q1)) = matches.next() else {
            return Err(Error::Validation(format!(
                "credit par-spread diff for '{curve_id}' missing matching closing quote '{}' at {}",
                q0.id(),
                item0.pillar_date
            )));
        };
        if matches.next().is_some() {
            return Err(Error::Validation(format!(
                "credit par-spread diff for '{curve_id}' found ambiguous closing quote '{}'",
                q0.id()
            )));
        }
        let delta_bp = q1.coupon_bp() - q0.coupon_bp();
        if !delta_bp.is_finite() {
            return Err(Error::Validation(format!(
                "calibrated CDS spread delta overflowed for '{curve_id}'"
            )));
        }
        deltas.push((item0.pillar_time, delta_bp));
    }
    Ok(deltas)
}

/// Evaluate calibrated CDS par-spread shifts in bp at exact recipe `tenors`.
fn measure_matched_quote_tenor_shifts_bp(
    h0: &HazardCurve,
    h1: &HazardCurve,
    curve_id: &str,
    tenors: &[f64],
) -> Result<Vec<f64>> {
    if tenors.is_empty() {
        return Err(Error::Validation(
            "hazard curve tenor shift measurement requires at least one tenor".to_string(),
        ));
    }
    let deltas = extract_matched_quote_deltas(curve_id, h0, h1)?;
    let mut shifts = Vec::with_capacity(tenors.len());
    for &t in tenors {
        if !t.is_finite() || t <= 0.0 {
            return Err(Error::Validation(format!(
                "credit curve tenor must be finite and strictly positive, got {t}"
            )));
        }
        let mut matching = deltas
            .iter()
            .filter(|&&(pillar_t, _)| exact_f64_eq(pillar_t, t));
        let Some(&(_, delta_bp)) = matching.next() else {
            return Err(Error::Validation(format!(
                "credit par-spread diff for '{curve_id}' has no calibrated quote at tenor {t}"
            )));
        };
        if matching.next().is_some() {
            return Err(Error::Validation(format!(
                "credit par-spread diff for '{curve_id}' found multiple quotes at tenor {t}; use typed replay bucket labels"
            )));
        }
        shifts.push(delta_bp);
    }
    Ok(shifts)
}

/// Measure aggregate hazard par-spread move (bp) from matched calibration spread-risk inputs.
fn measure_hazard_par_spread_mean_bp(
    curve_id: &str,
    h0: &HazardCurve,
    h1: &HazardCurve,
) -> Result<f64> {
    let quote_deltas: Vec<f64> = extract_matched_quote_deltas(curve_id, h0, h1)?
        .into_iter()
        .map(|(_, delta_bp)| delta_bp)
        .collect();
    stats::mean(&quote_deltas)?
        .ok_or_else(|| Error::Validation("credit spread delta mean was empty".to_string()))
}

fn dispatch_credit_family<T>(
    curve_id: &str,
    market_t0: &MarketContext,
    market_t1: &MarketContext,
    on_hazard: impl FnOnce(&HazardCurve, &HazardCurve) -> Result<T>,
    on_discount: impl FnOnce() -> Result<T>,
) -> Result<T> {
    match market_t0.get_hazard(curve_id) {
        Ok(h0) => {
            let h1 = market_t1.get_hazard(curve_id)?;
            on_hazard(&h0, &h1)
        }
        Err(err) => {
            if market_t0.get_discount(curve_id).is_ok() {
                on_discount()
            } else {
                Err(err)
            }
        }
    }
}

/// Measure per-tenor credit-curve shifts (bp) enforcing matching curve families across snapshots.
///
/// # Arguments
///
/// * `curve_id` - Credit curve identifier present as the same family (`HazardCurve` or risky `DiscountCurve`) in both snapshots.
/// * `market_t0` - Earlier market snapshot.
/// * `market_t1` - Later market snapshot.
/// * `tenors` - Non-empty slice of positive tenor coordinates in years.
pub(crate) fn measure_per_tenor_credit_curve_shift_bp(
    curve_id: &str,
    market_t0: &MarketContext,
    market_t1: &MarketContext,
    tenors: &[f64],
) -> Result<Vec<f64>> {
    dispatch_credit_family(
        curve_id,
        market_t0,
        market_t1,
        |h0, h1| measure_matched_quote_tenor_shifts_bp(h0, h1, curve_id, tenors),
        || measure_discount_curve_tenor_shifts_bp(curve_id, market_t0, market_t1, tenors),
    )
}

/// Measure aggregate credit-curve shift (bp) enforcing matching curve families across snapshots.
///
/// # Arguments
///
/// * `curve_id` - Credit curve identifier present as the same family (`HazardCurve` or risky `DiscountCurve`) in both snapshots.
/// * `market_t0` - Earlier market snapshot.
/// * `market_t1` - Later market snapshot.
pub(crate) fn measure_credit_curve_shift_bp(
    curve_id: &str,
    market_t0: &MarketContext,
    market_t1: &MarketContext,
) -> Result<f64> {
    dispatch_credit_family(
        curve_id,
        market_t0,
        market_t1,
        |h0, h1| measure_hazard_par_spread_mean_bp(curve_id, h0, h1),
        || measure_discount_curve_shift_bp(curve_id, market_t0, market_t1),
    )
}

/// Measure aggregate par CDS spread shift (bp) strictly over [`MarketContext::get_hazard`].
///
/// # Arguments
///
/// * `curve_id` - Hazard curve identifier in both snapshots.
/// * `market_t0` - Earlier market snapshot.
/// * `market_t1` - Later market snapshot.
pub(crate) fn measure_par_spread_shift_bp(
    curve_id: &str,
    market_t0: &MarketContext,
    market_t1: &MarketContext,
) -> Result<f64> {
    let h0 = market_t0.get_hazard(curve_id)?;
    let h1 = market_t1.get_hazard(curve_id)?;
    measure_hazard_par_spread_mean_bp(curve_id, &h0, &h1)
}

/// Threshold below which a signed mean shift is considered twist-dominated
/// relative to its L1 magnitude. Below this level, signed-average convexity
/// understates the true quadratic contribution, so downstream consumers should
/// fall back to per-tenor convexity.
const TWIST_FRACTION_THRESHOLD: f64 = 1e-2;

/// Mean of the per-tenor *absolute* credit-curve shift (bp) on [`DIAGNOSTIC_RATE_TENORS_YEARS`].
///
/// Returns `0.0` if either side's curve is missing or incomparable.
pub(super) fn credit_curve_abs_shift_bp(
    curve_id: &str,
    market_t0: &MarketContext,
    market_t1: &MarketContext,
) -> f64 {
    measure_per_tenor_credit_curve_shift_bp(
        curve_id,
        market_t0,
        market_t1,
        &DIAGNOSTIC_RATE_TENORS_YEARS,
    )
    .map(|shifts| l1_mean(&shifts))
    .unwrap_or(0.0)
}

/// Measure per-tenor annual inflation-curve rate shifts (in bp) strictly from [`MarketContext::get_inflation_curve`].
///
/// Never falls back to [`InflationIndex`](finstack_quant_core::market_data::scalars::InflationIndex).
fn measure_inflation_curve_tenor_shifts_bp(
    curve_id: &str,
    market_t0: &MarketContext,
    market_t1: &MarketContext,
    tenors: &[f64],
) -> Result<Vec<f64>> {
    if tenors.is_empty() {
        return Err(Error::Validation(
            "inflation curve shift measurement requires at least one tenor".to_string(),
        ));
    }
    let c0 = market_t0.get_inflation_curve(curve_id)?;
    let c1 = market_t1.get_inflation_curve(curve_id)?;
    let mut shifts = Vec::with_capacity(tenors.len());
    for &t in tenors {
        let i0_a = c0.base_cpi();
        let i0_m = c0.cpi(t)?;
        let i1_a = c1.base_cpi();
        let i1_m = c1.cpi(t)?;
        let delta_r = -zero_rate_difference(ZeroRateDiffParams {
            d0_anchor: i0_a,
            d0_maturity: i0_m,
            d1_anchor: i1_a,
            d1_maturity: i1_m,
            alpha: t,
            scale: RateDiffScale::Decimal,
        })?;
        let r0 = (i0_m / i0_a).ln() / t;
        if !r0.is_finite() {
            return Err(Error::Validation(format!(
                "baseline annual CPI log-growth overflowed at tenor {t} for '{curve_id}'"
            )));
        }
        let diff_bp = if delta_r <= 0.0 && 0.0 <= delta_r {
            0.0
        } else {
            r0.exp() * delta_r.exp_m1() * 10_000.0
        };
        if !diff_bp.is_finite() {
            return Err(Error::Validation(format!(
                "inflation curve shift overflowed at tenor {t} for '{curve_id}'"
            )));
        }
        shifts.push(diff_bp);
    }
    Ok(shifts)
}

/// Measure signed mean annual inflation-curve rate shift (bp) strictly over [`MarketContext::get_inflation_curve`].
///
/// # Arguments
///
/// * `curve_id` - Inflation curve identifier in both market snapshots.
/// * `market_t0` - Earlier market snapshot.
/// * `market_t1` - Later market snapshot.
pub(crate) fn measure_inflation_curve_shift_bp(
    curve_id: &str,
    market_t0: &MarketContext,
    market_t1: &MarketContext,
) -> Result<f64> {
    let shifts = measure_inflation_curve_tenor_shifts_bp(
        curve_id,
        market_t0,
        market_t1,
        &DIAGNOSTIC_RATE_TENORS_YEARS,
    )?;
    stats::mean(&shifts)?.ok_or_else(|| {
        Error::Validation("inflation curve shift produced an empty mean".to_string())
    })
}

/// Mean of the per-tenor *absolute* inflation-curve shift (bp) on [`DIAGNOSTIC_RATE_TENORS_YEARS`].
///
/// Returns `0.0` if either side's curve is missing or invalid.
pub(super) fn inflation_curve_abs_shift_bp(
    curve_id: &str,
    market_t0: &MarketContext,
    market_t1: &MarketContext,
) -> f64 {
    measure_inflation_curve_tenor_shifts_bp(
        curve_id,
        market_t0,
        market_t1,
        &DIAGNOSTIC_RATE_TENORS_YEARS,
    )
    .map(|shifts| l1_mean(&shifts))
    .unwrap_or(0.0)
}

/// Measure exact native scalar difference `x1 - x0` in currency/unitless units.
///
/// Dispatches strictly within matching scalar kinds: `Price` entries subtract via
/// [`measure_price_scalar_diff`] (exact `Decimal` subtraction before `.amount()`) and
/// `Unitless` entries subtract via [`measure_unitless_scalar_diff`].
///
/// # Arguments
///
/// * `id` - Market scalar identifier in both market snapshots.
/// * `market_t0` - Earlier market snapshot.
/// * `market_t1` - Later market snapshot.
pub(crate) fn measure_scalar_native_amount_diff(
    id: &str,
    market_t0: &MarketContext,
    market_t1: &MarketContext,
) -> Result<f64> {
    let s0 = market_t0.get_price(id)?;
    if s0.get_price().is_some() {
        Ok(measure_price_scalar_diff(id, id, market_t0, market_t1)?.amount())
    } else {
        measure_unitless_scalar_diff(id, id, market_t0, market_t1)
    }
}

/// Push the twist diagnostic for `factor_label` when `signed_avg_bp` is small
/// against the L1 mean of `abs_shift_bp` over `items` (items with no
/// measurable move are skipped).
pub(super) fn note_twist<I>(
    notes: &mut Vec<String>,
    factor_label: &str,
    signed_avg_bp: f64,
    items: impl IntoIterator<Item = I>,
    abs_shift_bp: impl Fn(I) -> f64,
) {
    let (Some(l1_avg), _) = average_over(items, |item| {
        let shift = abs_shift_bp(item);
        (shift > 0.0).then_some(shift)
    }) else {
        return;
    };
    if let Some(note) = twist_diagnostic_note(factor_label, signed_avg_bp, l1_avg) {
        notes.push(note);
        notes.push(format!("{factor_label}: unreliable / bounds-exceeded"));
    }
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
