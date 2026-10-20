use super::context::MetricsContext;
use super::shifts::{inflation_source_abs_shift_bp, note_twist};
use super::{push_first_order_step, push_second_order_step};
use crate::helpers::FactorWorking;
use crate::{MoveUnit, PnlAttribution};
use finstack_quant_core::market_data::diff::measure_inflation_source_shift;
use finstack_quant_valuations::metrics::MetricId;

pub(super) fn apply(
    inputs: &MetricsContext<'_>,
    attribution: &mut PnlAttribution,
    non_finite_detected: &mut bool,
) {
    // 9. Inflation sensitivity
    if let Some(inflation01) = inputs.val_t0.measures.get(MetricId::Inflation01.as_str()) {
        // Restrict the shift to the instrument's declared inflation sources.
        let curve_ids = &inputs.market_deps.curves.inflation_curves;

        let mut total_shift = 0.0;
        let mut curve_count = 0;
        let mut measurement_failed = false;

        for curve_id in curve_ids {
            match measure_inflation_source_shift(
                curve_id.as_str(),
                inputs.market_t0,
                inputs.market_t1,
            ) {
                Ok(shift_bp) => {
                    total_shift += shift_bp;
                    curve_count += 1;
                }
                Err(err) => {
                    measurement_failed = true;
                    attribution.meta.notes.push(format!(
                        "Inflation attribution could not measure declared source '{}': {err}",
                        curve_id.as_str()
                    ));
                }
            }
        }

        if measurement_failed || curve_ids.is_empty() {
            *non_finite_detected = true;
            if curve_ids.is_empty() {
                attribution.meta.notes.push(
                    "Inflation01 was supplied but the instrument declared no inflation source"
                        .to_string(),
                );
            }
        }

        let avg_shift = if curve_count > 0 {
            total_shift / curve_count as f64
        } else {
            0.0
        };
        let inflation_amount = inflation01 * avg_shift;
        push_first_order_step(
            attribution,
            "Inflation",
            FactorWorking::Scalar {
                unit: MoveUnit::BasisPoint,
                sensitivity: *inflation01,
                market_move: avg_shift,
            },
            inflation_amount,
        );
        attribution.inflation_curves_pnl = inputs.money(
            inflation_amount,
            "inflation P&L",
            attribution,
            non_finite_detected,
        );

        if let Some(inflation_convexity) = inputs
            .val_t0
            .measures
            .get(MetricId::InflationConvexity.as_str())
        {
            debug_assert!(
                inflation_convexity.is_finite(),
                "InflationConvexity metric must be finite for P&L attribution, got {inflation_convexity}"
            );
            let shift_decimal = avg_shift / 10_000.0;
            let convexity_pnl = 0.5 * inflation_convexity * shift_decimal * shift_decimal;
            push_second_order_step(
                attribution,
                "InflationConvexity",
                MoveUnit::BasisPoint,
                avg_shift,
                convexity_pnl,
            );
            attribution.inflation_curves_pnl = inputs.money(
                attribution.inflation_curves_pnl.amount() + convexity_pnl,
                "inflation convexity P&L",
                attribution,
                non_finite_detected,
            );

            note_twist(
                &mut attribution.meta.notes,
                "Inflation convexity",
                avg_shift,
                curve_ids,
                |id| inflation_source_abs_shift_bp(id, inputs.market_t0, inputs.market_t1),
            );
        }
    }
}
