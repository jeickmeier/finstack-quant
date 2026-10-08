use super::super::helpers::*;
use super::super::types::*;
use super::context::MetricsContext;
use finstack_quant_valuations::metrics::MetricId;

pub(super) fn apply(
    inputs: &MetricsContext<'_>,
    attribution: &mut PnlAttribution,
    non_finite_detected: &mut bool,
) {
    // 4. FX attribution (FX01 or FX Delta)
    //
    // METRIC DEFINITION:
    // - FX01: Dollar value of 1% FX rate change ($ / %)
    // - Formula: FX01 × Δfx (where Δfx is FX rate change in %)
    if let Some(fx01) = inputs.val_t0.measures.get(MetricId::Fx01.as_str()) {
        // FX01 × spot change (FX01 is typically per 1% move)
        if let Some(fx_shift) = inputs.shifts.fx_shift_pct {
            let fx_amount = fx01 * fx_shift;
            attribution.fx_pnl =
                inputs.money(fx_amount, "FX P&L", attribution, non_finite_detected);
            // Fx01 is the JOINT sensitivity to a simultaneous move of all the
            // instrument's FX pairs, but the shift above is measured on the
            // single `fx_exposure()` pair — approximate when the instrument
            // declares more than one pair.
            if inputs.market_deps.fx_pairs.len() > 1 {
                attribution.meta.notes.push(format!(
                    "FX attribution pairs the joint Fx01 sensitivity with the primary \
                         FX pair's move only; the instrument declares {} FX pairs, so \
                         differential moves across pairs are approximated",
                    inputs.market_deps.fx_pairs.len()
                ));
            }
        } else {
            note_warning(
                attribution,
                "FX attribution has FX01 but no measurable FX shift; FX P&L set to zero",
                inputs.instrument.id(),
                "fx",
            );
        }
    }
}
