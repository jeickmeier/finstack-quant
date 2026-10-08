use super::super::helpers::*;
use super::super::types::*;
use super::context::AttributionInputs;
use finstack_quant_valuations::metrics::MetricId;

pub(super) fn apply(
    inputs: &AttributionInputs<'_>,
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
            push_first_order_step(
                attribution,
                "Fx",
                FactorWorking::Scalar {
                    unit: MoveUnit::Percent,
                    sensitivity: *fx01,
                    market_move: fx_shift,
                },
                fx_amount,
            );
            attribution.fx_pnl = factor_money_or_invalid(
                fx_amount,
                inputs.val_t1.value.currency(),
                "FX P&L",
                &mut attribution.meta.notes,
                non_finite_detected,
            );
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
