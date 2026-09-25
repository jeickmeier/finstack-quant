use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::diff::{
    measure_discount_curve_shift, measure_inflation_index_shift, TenorSamplingMethod,
};
use finstack_quant_core::types::CurveId;
use finstack_quant_core::HashMap;
use finstack_quant_valuations::metrics::MetricId;

/// Extract per-curve bucketed DV01 sensitivities from ValuationResult measures.
///
/// Bucketed DV01 metrics are stored with composite keys like:
/// - `"bucketed_dv01::USD-OIS::5y"` per-tenor keys — the shape the
///   `BucketedDv01` producer actually emits (the per-curve total goes only to
///   `computed_series`, never to `measures`)
/// - `"bucketed_dv01::USD-OIS"` for a per-curve total DV01 (accepted for
///   backward compatibility and preferred when present)
/// - `"bucketed_dv01"` for the primary curve (if single curve instrument)
///
/// When the direct per-curve key is absent, the per-curve total is derived by
/// summing the instrument's per-tenor keys over the standard bucket grid.
///
/// # Arguments
///
/// * `measures` - Measures from ValuationResult containing flattened bucketed metrics
/// * `curve_ids` - List of discount curves required by the instrument
///
/// # Returns
///
/// HashMap mapping each curve ID to its total DV01 sensitivity.
pub(super) fn extract_bucketed_dv01_per_curve(
    measures: &indexmap::IndexMap<MetricId, f64>,
    curve_ids: &[CurveId],
) -> HashMap<CurveId, f64> {
    use finstack_quant_valuations::metrics::STANDARD_BUCKET_LABELS;

    let mut result = HashMap::default();

    // Pattern 1: Explicit per-curve keys "bucketed_dv01::{curve_id}".
    // Reuse a single key buffer instead of a per-curve `format!` allocation.
    let mut key = String::new();
    for curve_id in curve_ids {
        key.clear();
        key.push_str("bucketed_dv01::");
        key.push_str(curve_id.as_str());
        if let Some(&dv01) = measures.get(key.as_str()) {
            result.insert(curve_id.clone(), dv01);
            continue;
        }
        // Pattern 1b: the producer never emits the direct per-curve key — it
        // flattens per-tenor keys "bucketed_dv01::{curve}::{label}". Derive
        // the per-curve total by summing those.
        key.push_str("::");
        let prefix_len = key.len();
        let mut total = 0.0;
        let mut found = false;
        for label in STANDARD_BUCKET_LABELS {
            key.truncate(prefix_len);
            key.push_str(label);
            if let Some(&dv01) = measures.get(key.as_str()) {
                total += dv01;
                found = true;
            }
        }
        if found {
            result.insert(curve_id.clone(), total);
        }
    }

    // Pattern 2: For single-curve instruments, check the base key
    if result.is_empty() && curve_ids.len() == 1 {
        if let Some(&dv01) = measures.get("bucketed_dv01") {
            result.insert(curve_ids[0].clone(), dv01);
        }
    }

    // Diagnostic: warn when bucketed DV01 is unavailable for curves the caller
    // requested. Downstream attribution then falls back to coarser parallel
    // DV01 — silent without this warning.
    for curve_id in curve_ids {
        if !result.contains_key(curve_id) {
            tracing::warn!(
                curve_id = %curve_id.as_str(),
                "bucketed_dv01 unavailable for curve; attribution will fall back to aggregate \
                 parallel DV01 — results will be coarser",
            );
        }
    }

    result
}

/// Extract per-curve **key-rate** (per-tenor) sensitivities flattened under
/// composite keys `{metric_prefix}::{curve}::{tenor_label}` (for example
/// `bucketed_dv01::USD-OIS::5y` or `bucketed_cs01::ACME-HAZ::5y`).
///
/// Walks the standard bucket grid and collects, per curve, the
/// `(tenor_years, sensitivity)` pairs that are present.
///
/// # Arguments
///
/// * `measures` - Metric map from a priced valuation result.
/// * `curve_ids` - Curves to look up.
/// * `metric_prefix` - Key prefix of the bucketed metric family
///   (`"bucketed_dv01"` or `"bucketed_cs01"`).
///
/// Returns a map `curve → Vec<(tenor_years, sensitivity)>`; a curve is absent
/// when none of its per-tenor keys were found (caller then falls back to the
/// coarser per-curve-total or aggregate path).
pub(crate) fn extract_keyrate_per_curve(
    measures: &indexmap::IndexMap<MetricId, f64>,
    curve_ids: &[CurveId],
    metric_prefix: &str,
) -> HashMap<CurveId, Vec<(f64, f64)>> {
    use finstack_quant_valuations::metrics::{STANDARD_BUCKETS_YEARS, STANDARD_BUCKET_LABELS};

    let mut result: HashMap<CurveId, Vec<(f64, f64)>> = HashMap::default();
    // Reuse one key buffer across all curves/tenors: build the
    // `{prefix}::{curve}::` prefix once per curve, then swap only the
    // trailing tenor label — no per-tenor `format!` allocation.
    let mut key = String::new();
    for curve_id in curve_ids {
        let mut buckets: Vec<(f64, f64)> = Vec::new();
        key.clear();
        key.push_str(metric_prefix);
        key.push_str("::");
        key.push_str(curve_id.as_str());
        key.push_str("::");
        let prefix_len = key.len();
        for (&tenor_years, label) in STANDARD_BUCKETS_YEARS
            .iter()
            .zip(STANDARD_BUCKET_LABELS.iter())
        {
            key.truncate(prefix_len);
            key.push_str(label);
            if let Some(&value) = measures.get(key.as_str()) {
                buckets.push((tenor_years, value));
            }
        }
        if !buckets.is_empty() {
            result.insert(curve_id.clone(), buckets);
        }
    }
    result
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
