//! Par spread calculator for fixed income index TRS.

use crate::fi_trs::FiIndexTotalReturnSwap;
use finstack_quant_core::Result;
use finstack_quant_valuations_core::instruments::TrsEngine;
use finstack_quant_valuations_core::metrics::{MetricCalculator, MetricContext, MetricId};

/// Calculates the par spread for a fixed income index TRS under the carry-only return model.
///
/// The par spread solves `NPV_receiver = 0`:
///
/// ```text
/// NPV = PV(total_return) - PV(financing_float) - s * Annuity = 0
/// s_par = (PV(total_return) - PV(financing_float)) / Annuity
/// ```
///
/// where `PV(financing_float)` is the financing leg PV excluding the spread
/// component. This is a market-level quote (analogous to a swap rate) computed
/// from the total-return-receiver's perspective regardless of trade side.
pub(crate) struct ParSpreadCalculator;

impl MetricCalculator for ParSpreadCalculator {
    fn dependencies(&self) -> &[MetricId] {
        &[MetricId::FinancingAnnuity]
    }

    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        // Warn once per process, not on every metric call (matches the pricer).
        static CARRY_MODEL_WARNING: std::sync::Once = std::sync::Once::new();
        CARRY_MODEL_WARNING.call_once(|| {
            tracing::warn!(
                "FiIndexTotalReturnSwap par spread is computed from a carry-only analytic \
                 model, not a full fixed-income index mark-to-market model"
            );
        });
        let annuity = context.computed_metric(&MetricId::FinancingAnnuity)?;
        let trs: &FiIndexTotalReturnSwap = context.instrument_as()?;
        let curves = context.curves.as_ref();
        let tr_pv = trs.pv_total_return_leg(curves, context.as_of)?;
        TrsEngine::par_spread_bp(
            tr_pv,
            &trs.financing_leg,
            &trs.schedule,
            trs.notional,
            curves,
            context.as_of,
            annuity,
        )
    }
}
