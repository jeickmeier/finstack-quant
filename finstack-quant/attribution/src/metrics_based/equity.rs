use super::context::MetricsContext;
use super::{push_first_order_step, push_second_order_step};
use crate::helpers::FactorWorking;
use crate::{MoveUnit, PnlAttribution};
use finstack_quant_core::market_data::diff::measure_scalar_absolute_shift;
use finstack_quant_valuations::metrics::MetricId;

pub(super) fn apply_spot(
    inputs: &MetricsContext<'_>,
    attribution: &mut PnlAttribution,
    non_finite_detected: &mut bool,
) {
    // 6. Market scalars: spot price Delta/Gamma attribution
    //
    // METRIC DEFINITION (see MetricId::Delta / MetricId::Gamma):
    // - Delta: dPV/dS — currency per UNIT of underlying move
    // - Gamma: d²PV/dS² — currency per (unit underlying)²
    // - Formula: PnL = Delta × ΔS + ½ × Gamma × (ΔS)², with ΔS the ABSOLUTE
    //   spot move. Multiplying by a percentage shift would mis-scale the P&L
    //   by 100/S₀ (resp. (100/S₀)²), exact only when S₀ = 100.
    //
    // Uses market_scalar_ids from MarketDependencies to identify underlying spot prices.
    let market_scalar_ids = &inputs.market_deps.market_scalar_ids;
    let delta_opt = inputs.val_t0.measures.get(MetricId::Delta.as_str());
    let gamma_opt = inputs.val_t0.measures.get(MetricId::Gamma.as_str());

    if let Some(&delta) = delta_opt {
        debug_assert!(
            delta.is_finite(),
            "Delta metric must be finite for P&L attribution, got {delta}"
        );

        let primary_shift = market_scalar_ids.first().and_then(|spot_id| {
            measure_scalar_absolute_shift(spot_id, inputs.market_t0, inputs.market_t1).ok()
        });

        if let Some(spot_shift) = primary_shift {
            let mut total_spot_pnl = delta * spot_shift;
            push_first_order_step(
                attribution,
                "Spot",
                FactorWorking::Scalar {
                    unit: MoveUnit::PriceUnit,
                    sensitivity: delta,
                    market_move: spot_shift,
                },
                total_spot_pnl,
            );

            // Second-order: Gamma applied to the same primary spot move.
            if let Some(&gamma) = gamma_opt {
                debug_assert!(
                    gamma.is_finite(),
                    "Gamma metric must be finite for P&L attribution, got {gamma}"
                );
                let gamma_pnl = 0.5 * gamma * spot_shift * spot_shift;
                total_spot_pnl += gamma_pnl;
                push_second_order_step(
                    attribution,
                    "SpotGamma",
                    MoveUnit::PriceUnit,
                    spot_shift,
                    gamma_pnl,
                );
            }

            attribution.market_scalars_pnl = inputs.money(
                total_spot_pnl,
                "market scalars (delta/gamma) P&L",
                attribution,
                non_finite_detected,
            );
        } else {
            *non_finite_detected = true;
            attribution.meta.notes.push(
                "Spot Delta/Gamma has no measurable declared primary spot; attribution flagged invalid".into()
            );
        }
    }
}
