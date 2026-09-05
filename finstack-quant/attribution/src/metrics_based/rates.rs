use super::super::helpers::*;
use super::super::types::*;
use super::context::AttributionInputs;
use super::shifts::{
    average_over, extract_bucketed_dv01_per_curve, extract_keyrate_per_curve,
    measure_per_tenor_rate_shift, measure_rate_curve_shift_bp, rate_curve_abs_shift_bp,
    twist_diagnostic_note,
};
use finstack_quant_core::config::{RoundingContext, ZeroKind};
use finstack_quant_core::math::NeumaierAccumulator;
use finstack_quant_valuations::metrics::MetricId;

// Large Move Warning Thresholds
//
// These thresholds define when market moves are large enough that second-order
// Taylor expansion may produce significant approximation errors (>5% relative).
//
// Beyond these thresholds, consider using parallel or waterfall attribution
// for more accurate results.

/// Maximum rate shift (in basis points) before warning about approximation accuracy.
/// Beyond ~100bp, third-order and higher terms become significant.
const LARGE_RATE_MOVE_THRESHOLD_BP: f64 = 100.0;

/// Threshold below which the total |DV01| weight is treated as zero when
/// forming the DV01-weighted convexity shift (falls back to the unweighted
/// mean instead of dividing by ~0).
const KEYRATE_WEIGHT_EPS: f64 = 1e-12;

pub(super) fn apply(
    inputs: &AttributionInputs<'_>,
    attribution: &mut PnlAttribution,
    non_finite_detected: &mut bool,
) {
    // 2. Rates curves attribution (DV01)
    //
    // METRIC DEFINITION:
    // - DV01: Dollar value of 1 basis point ($ / bp)
    // - BucketedDv01: Per-curve / per-tenor DV01 sensitivities
    // - Formula: PnL = Σ(DV01_i × Shift_i) for each curve/tenor i
    //
    // Accuracy ladder (best first):
    //   (a) key-rate aware: Σ_curve Σ_tenor DV01_{curve,tenor} × Δr_{curve,tenor}.
    //       Correct for non-parallel (steepener / twist) curve moves.
    //   (b) per-curve bucketed: Σ_curve DV01_curve × avg(Δr_curve). Correct for
    //       cross-curve basis but assumes each curve moved in parallel.
    //   (c) aggregate: DV01_total × avg(Δr). Coarsest.

    let curve_ids = &inputs.rates_curve_ids;
    // (a) per-tenor (key-rate) DV01 — the most accurate input.
    let keyrate_dv01 =
        extract_keyrate_per_curve(&inputs.val_t0.measures, curve_ids, "bucketed_dv01");
    // (b) per-curve total DV01 — fallback when no per-tenor series exist.
    let bucketed_dv01 = extract_bucketed_dv01_per_curve(&inputs.val_t0.measures, curve_ids);

    let has_keyrate = !keyrate_dv01.is_empty();
    let has_bucketed = !bucketed_dv01.is_empty();
    let mut rates_pnl = 0.0;
    // Average rate shift used for the rates convexity / large-move blocks.
    // - Key-rate / bucketed branches: average only over curves with data.
    // - Fallback branch: preamble average over all rates curves with a
    //   measurable shift.
    let mut convexity_avg_shift_bp: Option<f64> = None;

    if has_keyrate {
        // KEY-RATE AWARE: pair per-tenor DV01 with the per-tenor curve shift
        // so a steepener is not collapsed to average-shift × parallel-DV01.
        //
        // Curves without per-tenor data fall through to per-curve bucketed DV01
        // when present, rather than being dropped into residual with no note.
        let mut rates_acc = NeumaierAccumulator::new();
        let mut shift_acc = NeumaierAccumulator::new();
        // DV01-weighted shift for the convexity block: Σ|DV01_i|·Δr_i and
        // Σ|DV01_i|. An unweighted mean over DV01 cells collapses to ~0 for a
        // steepener even when the position's risk sits at one end of the
        // curve, killing the convexity term.
        let mut weighted_shift_acc = NeumaierAccumulator::new();
        let mut weight_acc = NeumaierAccumulator::new();
        let mut shift_terms = 0usize;
        let mut curves_with_data = 0usize;
        let mut curves_via_fallback: Vec<String> = Vec::new();
        for curve_id in curve_ids {
            let Some(buckets) = keyrate_dv01.get(curve_id) else {
                // Per-curve fallback for mixed coverage.
                if let Some(&dv01_for_curve) = bucketed_dv01.get(curve_id) {
                    if let Some(shift) = measure_rate_curve_shift_bp(
                        curve_id.as_str(),
                        inputs.market_t0,
                        inputs.market_t1,
                    ) {
                        rates_acc.add(dv01_for_curve * shift);
                        shift_acc.add(shift);
                        weighted_shift_acc.add(dv01_for_curve.abs() * shift);
                        weight_acc.add(dv01_for_curve.abs());
                        shift_terms += 1;
                        curves_via_fallback.push(curve_id.as_str().to_string());
                    }
                }
                continue;
            };
            let tenors: Vec<f64> = buckets.iter().map(|(t, _)| *t).collect();
            let Some(shifts) = measure_per_tenor_rate_shift(
                curve_id.as_str(),
                inputs.market_t0,
                inputs.market_t1,
                &tenors,
            ) else {
                continue;
            };
            for ((_, dv01), shift) in buckets.iter().zip(shifts.iter()) {
                rates_acc.add(dv01 * shift);
                shift_acc.add(*shift);
                weighted_shift_acc.add(dv01.abs() * shift);
                weight_acc.add(dv01.abs());
                shift_terms += 1;
            }
            curves_with_data += 1;
        }
        rates_pnl = rates_acc.total();
        attribution.rates_curves_pnl = factor_money_or_invalid(
            rates_pnl,
            inputs.val_t1.value.currency(),
            "rates curves P&L (key-rate)",
            &mut attribution.meta.notes,
            non_finite_detected,
        );

        if shift_terms > 0 {
            // DV01-weighted mean shift across all (curve, tenor) cells with
            // data — the scalar input to the coarse convexity block:
            //   Σ|DV01_i|·Δr_i / Σ|DV01_i|.
            // Guard: when Σ|DV01| ≈ 0 (all-zero DV01 cells) fall back to the
            // unweighted mean rather than dividing by ~0.
            let total_weight = weight_acc.total();
            convexity_avg_shift_bp = Some(if total_weight > KEYRATE_WEIGHT_EPS {
                weighted_shift_acc.total() / total_weight
            } else {
                shift_acc.total() / shift_terms as f64
            });
        }
        if curves_with_data > 0 {
            attribution.meta.notes.push(format!(
                "Rates attribution computed using key-rate (per-tenor) DV01 across {} curve(s); \
                     non-parallel curve moves are attributed per tenor",
                curves_with_data
            ));
        }
        if !curves_via_fallback.is_empty() {
            attribution.meta.notes.push(format!(
                "Rates curves without per-tenor DV01 attributed via per-curve bucketed DV01 \
                     (parallel-move assumption): {}",
                curves_via_fallback.join(", ")
            ));
        }
    } else if has_bucketed {
        // PER-CURVE BUCKETED: sum per-curve contributions. Each curve is still
        // assumed to move in parallel (no per-tenor series available).
        let mut total_shift = 0.0;
        let mut curves_with_data = 0usize;
        for curve_id in curve_ids {
            if let Some(&dv01_for_curve) = bucketed_dv01.get(curve_id) {
                if let Some(shift) = measure_rate_curve_shift_bp(
                    curve_id.as_str(),
                    inputs.market_t0,
                    inputs.market_t1,
                ) {
                    rates_pnl += dv01_for_curve * shift;
                    total_shift += shift;
                    curves_with_data += 1;
                }
            }
        }

        attribution.rates_curves_pnl = factor_money_or_invalid(
            rates_pnl,
            inputs.val_t1.value.currency(),
            "rates curves P&L (bucketed)",
            &mut attribution.meta.notes,
            non_finite_detected,
        );

        if curves_with_data > 0 {
            convexity_avg_shift_bp = Some(total_shift / curves_with_data as f64);
            attribution.meta.notes.push(format!(
                "Rates attribution computed using per-curve bucketed DV01 across {} curves \
                     (each curve assumed to move in parallel); provide per-tenor BucketedDv01 \
                     series for key-rate-aware attribution of non-parallel moves",
                curves_with_data
            ));
        }
    } else if let Some(dv01) = inputs.val_t0.measures.get(MetricId::Dv01.as_str()) {
        // Fallback: use aggregate DV01 with the preamble's average shift.
        let avg_shift = if let Some(avg_shift) = inputs.shifts.avg_rate_shift_bp {
            avg_shift
        } else {
            note_warning(
                    attribution,
                    "Rates attribution has DV01 but no measurable discount-curve shift; rates P&L set to zero",
                    inputs.instrument.id(),
                    "rates_curves",
                );
            0.0
        };
        rates_pnl = dv01 * avg_shift;
        convexity_avg_shift_bp = inputs.shifts.avg_rate_shift_bp;

        attribution.rates_curves_pnl = factor_money_or_invalid(
            rates_pnl,
            inputs.val_t1.value.currency(),
            "rates curves P&L (aggregate dv01)",
            &mut attribution.meta.notes,
            non_finite_detected,
        );

        // Add note about averaging limitation
        if inputs.shifts.rate_curves_measured > 1 {
            attribution.meta.notes.push(format!(
                "Rates attribution uses aggregate DV01 with average shift across {} curves; \
                     provide BucketedDv01 metric for more accurate per-curve attribution",
                inputs.shifts.rate_curves_measured
            ));
        }
    } else if inputs
        .shifts
        .avg_rate_shift_bp
        .is_some_and(|s| s.abs() > 0.0)
    {
        // No DV01 metric at all while the curves measurably moved: the rates
        // P&L stays zero and the move lands in the residual. Note it for
        // symmetric diagnosability with the carry block.
        note_warning(
            attribution,
            "Rates attribution skipped: no Dv01/BucketedDv01 metric in the T0 valuation \
                 while the rates curves moved; rates P&L set to zero (move flows to residual)",
            inputs.instrument.id(),
            "rates_curves",
        );
    }

    // 2b. Rates curves convexity (second-order)
    //
    // UNIT CONTRACT:
    // - `measure_discount_curve_shift` returns a shift in BASIS POINTS.
    // - `Convexity` / `IrConvexity` are percentage second-derivative metrics (dimensionless).
    // - P&L formula: ½ × P₀ × Convexity × (Δr_decimal)², where Δr_decimal = shift_bp / 10_000.
    //
    // LIMITATION: Assumes parallel/average shifts and small moves; for large or non-parallel
    // moves, use bump-and-reprice curve gamma when available.
    //
    // TWIST GUARD: if the signed average is much smaller than
    // the L1 (absolute) average, the curves were twisted (e.g. short-end +50bp,
    // long-end −50bp averages to ~0). In that regime the scalar convexity term
    // `½·γ·avg²` collapses to ≈0 even though the true second-order
    // contribution `½·Δrᵀ·H·Δr` is non-trivial. Emit a note so the consumer
    // knows the convexity number is *not* a real upper bound.
    let avg_rate_abs_shift_bp: Option<f64> = average_over(&inputs.rates_curve_ids, |curve_id| {
        let v = rate_curve_abs_shift_bp(curve_id.as_str(), inputs.market_t0, inputs.market_t1);
        (v > 0.0).then_some(v)
    })
    .0;
    if let Some(avg_shift) = convexity_avg_shift_bp {
        let rc = RoundingContext::default();
        // The two convexity MetricIds have DIFFERENT producer units and must
        // not be merged :
        //
        // - `Convexity` (bond producer) is *street convexity*:
        //   `(1/P)·d²P/dy² / 100` (Bloomberg YAS convention, golden-verified
        //   in valuations). P&L term: ½ × P₀ × Convexity × 100 × (Δy)².
        // - `IrConvexity` (IRS producer) is the *raw dollar second
        //   derivative* `d²PV/dr²` (no P normalization — a near-par swap has
        //   PV ≈ 0 but real gamma). P&L term: ½ × IrConvexity × (Δy)².
        let street_convexity = inputs
            .val_t0
            .measures
            .get(MetricId::Convexity.as_str())
            .filter(|&&v| !rc.is_effectively_zero(v, ZeroKind::Generic));
        let dollar_convexity = inputs
            .val_t0
            .measures
            .get(MetricId::IrConvexity.as_str())
            .filter(|&&v| !rc.is_effectively_zero(v, ZeroKind::Generic));

        let shift_decimal = avg_shift / 10_000.0;
        let convexity_pnl_opt = match (street_convexity, dollar_convexity) {
            (Some(&convexity), _) => {
                // Street convexity: ½ × P₀ × C × 100 × (Δy)².
                debug_assert!(
                    convexity.is_finite(),
                    "Convexity metric must be finite for P&L attribution, got {convexity}"
                );
                let p0 = inputs.val_t0.value.amount();
                Some(0.5 * p0 * convexity * 100.0 * shift_decimal * shift_decimal)
            }
            (None, Some(&ir_convexity)) => {
                // Dollar convexity: ½ × d²PV/dr² × (Δy)² — no P₀ factor.
                debug_assert!(
                    ir_convexity.is_finite(),
                    "IrConvexity metric must be finite for P&L attribution, got {ir_convexity}"
                );
                Some(0.5 * ir_convexity * shift_decimal * shift_decimal)
            }
            (None, None) => None,
        };

        if let Some(convexity_pnl) = convexity_pnl_opt {
            attribution.rates_curves_pnl = factor_money_or_invalid(
                attribution.rates_curves_pnl.amount() + convexity_pnl,
                inputs.val_t1.value.currency(),
                "rates convexity P&L",
                &mut attribution.meta.notes,
                non_finite_detected,
            );
        }

        // Check for large rate moves that may exceed approximation accuracy
        if avg_shift.abs() > LARGE_RATE_MOVE_THRESHOLD_BP {
            attribution.meta.notes.push(format!(
                "Warning: Large rate move ({:.0}bp) exceeds {}bp threshold; \
                     third-order+ effects ignored, consider parallel/waterfall attribution \
                     for more accurate results",
                avg_shift.abs(),
                LARGE_RATE_MOVE_THRESHOLD_BP
            ));
        }

        // Twist-domination warning.
        if let Some(abs_shift) = avg_rate_abs_shift_bp {
            if let Some(note) = twist_diagnostic_note("Rates convexity", avg_shift, abs_shift) {
                attribution.meta.notes.push(note);
                attribution
                    .meta
                    .notes
                    .push("Rates convexity: unreliable / bounds-exceeded".to_string());
            }
        }
    }
}
