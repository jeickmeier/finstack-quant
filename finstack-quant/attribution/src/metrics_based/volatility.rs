use super::super::helpers::*;
use super::super::types::*;
use super::context::MetricsContext;
use finstack_quant_valuations::metrics::MetricId;

/// Maximum volatility shift (in percentage points) before warning.
/// Vol-of-vol effects become significant beyond ~5% absolute vol change.
const LARGE_VOL_MOVE_THRESHOLD_PCT: f64 = 5.0;

pub(super) fn apply(
    inputs: &MetricsContext<'_>,
    attribution: &mut PnlAttribution,
    non_finite_detected: &mut bool,
) {
    // 5. Volatility attribution (Vega)
    //
    // METRIC DEFINITION:
    // - Vega: Dollar value of 1 percentage point volatility change ($ / vol point)
    // - Formula: Vega × Δσ (where Δσ is in percentage points, e.g., 1.0 for 1% vol change)
    if let Some(vega) = inputs.val_t0.measures.get(MetricId::Vega.as_str()) {
        // Vega × vol change (in percentage points). Preserves prior behavior:
        // vol PnL is only recorded when the instrument has a vol surface AND
        // the surface shift measurement succeeded (both conditions captured
        // by `avg_vol_shift_abs` being `Some`).
        if let Some(vol_shift) = inputs.shifts.avg_vol_shift_abs {
            // vol_shift is already in percentage points
            let vol_amount = vega * vol_shift;
            attribution.vol_pnl =
                inputs.money(vol_amount, "vol P&L", attribution, non_finite_detected);

            // 5b. Volatility convexity (Volga - second-order)
            if let Some(volga) = inputs.val_t0.measures.get(MetricId::Volga.as_str()) {
                // Volga term: ½ × Volga × (Δσ)²
                let volga_pnl = 0.5 * volga * vol_shift * vol_shift;

                attribution.vol_pnl = inputs.money(
                    attribution.vol_pnl.amount() + volga_pnl,
                    "volga P&L",
                    attribution,
                    non_finite_detected,
                );
            }

            // Check for large vol moves that may exceed approximation accuracy
            if vol_shift.abs() > LARGE_VOL_MOVE_THRESHOLD_PCT {
                attribution.meta.notes.push(format!(
                    "Warning: Large volatility move ({:.1}%) exceeds {:.1}% threshold; \
                         vol-of-vol effects ignored, consider parallel/waterfall attribution",
                    vol_shift.abs(),
                    LARGE_VOL_MOVE_THRESHOLD_PCT
                ));
            }
        } else {
            note_warning(
                    attribution,
                    "Volatility attribution has Vega but no measurable volatility-surface shift; vol P&L set to zero",
                    inputs.instrument.id(),
                    "vol",
                );
        }
    }
}
