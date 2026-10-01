use super::super::helpers::*;
use super::super::types::*;
use super::context::AttributionInputs;
use super::shifts::{average_over, credit_curve_abs_shift_bp, twist_diagnostic_note};
use finstack_quant_core::math::NeumaierAccumulator;
use finstack_quant_valuations::metrics::MetricId;

// Large Move Warning Thresholds
//
// These thresholds define when market moves are large enough that second-order
// Taylor expansion may produce significant approximation errors (>5% relative).
//
// Beyond these thresholds, consider using parallel or waterfall attribution
// for more accurate results.

/// Maximum credit spread shift (in basis points) before warning.
/// Credit spread convexity is typically larger than rate convexity.
const LARGE_SPREAD_MOVE_THRESHOLD_BP: f64 = 50.0;

pub(super) fn apply(
    inputs: &AttributionInputs<'_>,
    attribution: &mut PnlAttribution,
    non_finite_detected: &mut bool,
) {
    // 3. Credit curves attribution (CS01)
    //
    // METRIC DEFINITION:
    // - Cs01 / BucketedCs01: $ per bp of credit-curve move ($ / bp).
    // - Formula: PnL = Σ_curve Σ_tenor BucketedCs01_{curve,tenor} × Δs_{curve,tenor}
    //   where Δs is the credit-curve move from `measure_credit_curve_shift` /
    //   `measure_per_tenor_credit_curve_shift`. Those measure the move in
    //   whichever basis the instrument's CS01 is defined on: a par CDS spread
    //   move for a hazard curve, or a zero-rate move for a discount-style
    //   credit curve. Pairing a par-spread CS01 with a hazard-rate move would
    //   overstate credit P&L by 1/(1−R), so the move always matches the CS01.
    //
    // Accuracy ladder (best first):
    //   (a) key-rate: per-tenor BucketedCs01 × per-tenor credit-curve move —
    //       correct for non-parallel (steepener / twist) credit-curve moves.
    //   (b) aggregate: Cs01 × avg(credit-curve move). Coarser; assumes parallel.
    let credit_curve_ids = &inputs.market_deps.curves.credit_curves;
    let keyrate_cs01 = &inputs.credit_keyrates;
    let mut credit_has_data = false;
    // Mean par-spread shift fed to the credit-convexity (second-order) block.
    let mut credit_convexity_avg_shift_bp: Option<f64> = None;

    if !keyrate_cs01.is_empty() {
        // KEY-RATE AWARE: pair per-tenor BucketedCs01 with the per-tenor
        // par-spread move. A credit-curve steepener is attributed per tenor
        // instead of collapsing to an average-shift × parallel-CS01 product —
        // so no twist guard / omit-on-twist workaround is needed.
        // Exact quote coordinates and complete coverage were validated
        // before this stage; every supplied bucket contributes once.
        let mut credit_acc = NeumaierAccumulator::new();
        let mut shift_acc = NeumaierAccumulator::new();
        let mut shift_terms = 0usize;
        let mut curves_with_data = 0usize;
        for curve_id in credit_curve_ids {
            let Some(buckets) = keyrate_cs01.get(curve_id) else {
                continue;
            };
            for bucket in buckets {
                credit_acc.add(bucket.sensitivity * bucket.move_bp);
                shift_acc.add(bucket.move_bp);
                shift_terms += 1;
            }
            curves_with_data += 1;
        }
        if curves_with_data > 0 {
            attribution.credit_curves_pnl = factor_money_or_invalid(
                credit_acc.total(),
                inputs.val_t1.value.currency(),
                "credit curves P&L (key-rate)",
                &mut attribution.meta.notes,
                non_finite_detected,
            );
            credit_has_data = true;
            if shift_terms > 0 {
                credit_convexity_avg_shift_bp = Some(shift_acc.total() / shift_terms as f64);
            }
            attribution.meta.notes.push(format!(
                "Credit attribution computed using key-rate (per-tenor) BucketedCs01 across \
                     {} curve(s); non-parallel credit-curve moves are attributed per tenor",
                curves_with_data
            ));
        }
    }
    if !credit_has_data {
        if let Some(cs01) = inputs.val_t0.measures.get(MetricId::Cs01.as_str()) {
            // Aggregate fallback: parallel CS01 × average credit-curve move.
            let avg_shift = if let Some(avg_shift) = inputs.shifts.avg_credit_shift_bp {
                avg_shift
            } else {
                note_warning(
                    attribution,
                    "Credit attribution has Cs01 but no measurable credit-curve shift; credit P&L set to zero",
                    inputs.instrument.id(),
                    "credit_curves",
                );
                0.0
            };
            attribution.credit_curves_pnl = factor_money_or_invalid(
                cs01 * avg_shift,
                inputs.val_t1.value.currency(),
                "credit curves P&L",
                &mut attribution.meta.notes,
                non_finite_detected,
            );
            credit_has_data = true;
            credit_convexity_avg_shift_bp = inputs.shifts.avg_credit_shift_bp;
            if inputs.shifts.credit_curves_measured > 1 {
                attribution.meta.notes.push(format!(
                "Credit attribution uses aggregate Cs01 with average credit-curve shift across \
                     {} curves; provide BucketedCs01 for key-rate-aware attribution of \
                     non-parallel moves",
                inputs.shifts.credit_curves_measured
            ));
            }
        } else if inputs
            .shifts
            .avg_credit_shift_bp
            .is_some_and(|s| s.abs() > 0.0)
        {
            // No CS01 metric at all while the credit curves measurably moved —
            // note the silent zero (see the rates-ladder counterpart above).
            note_warning(
                attribution,
                "Credit attribution skipped: no Cs01/BucketedCs01 metric in the T0 valuation \
                 while the credit curves moved; credit P&L set to zero (move flows to residual)",
                inputs.instrument.id(),
                "credit_curves",
            );
        }
    }

    // 3b. Credit curves gamma (second-order).
    //
    // UNIT CONTRACT: CsGamma is ∂²V/∂s² in $ per decimal² of *par spread*.
    //   ΔP_gamma = ½ × CsGamma × (Δs_decimal)², Δs = mean par-spread move.
    //
    // TWIST GUARD: like rates convexity, the scalar `½·γ·avg²` term collapses
    // when the credit curve is twisted (signed mean ≈ 0). Emit a note so the
    // consumer knows the gamma number is not a real upper bound. Average over
    // the same credit curves the metrics-based attribution consumed.
    let avg_credit_abs_shift_bp: Option<f64> =
        average_over(&inputs.market_deps.curves.credit_curves, |curve_id| {
            let v =
                credit_curve_abs_shift_bp(curve_id.as_str(), inputs.market_t0, inputs.market_t1);
            (v > 0.0).then_some(v)
        })
        .0;
    if credit_has_data {
        if let Some(avg_shift) = credit_convexity_avg_shift_bp {
            if let Some(cs_gamma) = inputs.val_t0.measures.get(MetricId::CsGamma.as_str()) {
                let shift_decimal = avg_shift / 10_000.0;
                let gamma_pnl = 0.5 * cs_gamma * shift_decimal * shift_decimal;
                attribution.credit_curves_pnl = factor_money_or_invalid(
                    attribution.credit_curves_pnl.amount() + gamma_pnl,
                    inputs.val_t1.value.currency(),
                    "credit gamma P&L",
                    &mut attribution.meta.notes,
                    non_finite_detected,
                );
            }

            if avg_shift.abs() > LARGE_SPREAD_MOVE_THRESHOLD_BP {
                attribution.meta.notes.push(format!(
                    "Warning: Large credit spread move ({:.0}bp) exceeds {}bp threshold; \
                         consider parallel/waterfall attribution for more accurate results",
                    avg_shift.abs(),
                    LARGE_SPREAD_MOVE_THRESHOLD_BP
                ));
            }

            if let Some(abs_shift) = avg_credit_abs_shift_bp {
                if let Some(note) = twist_diagnostic_note("Credit gamma", avg_shift, abs_shift) {
                    attribution.meta.notes.push(note);
                    attribution
                        .meta
                        .notes
                        .push("Credit gamma: unreliable / bounds-exceeded".to_string());
                }
            }
        }
    }
}
