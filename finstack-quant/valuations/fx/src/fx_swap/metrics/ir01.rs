//! Domestic and foreign IR01 for FX swaps.
//!
//! Computes sensitivity to a parallel bump in one leg's discount curve
//! via scratch-market repricing and central finite difference.

use crate::fx_swap::FxSwap;
use finstack_quant_core::Result;
use finstack_quant_core::market_data::bumps::BumpSpec;
use finstack_quant_valuations_core::metrics::central_diff_by_width;
use finstack_quant_valuations_core::metrics::sensitivities::bump_config;
use finstack_quant_valuations_core::metrics::{MetricCalculator, MetricContext};

/// Discount curve an [`FxSwapIr01`] bumps.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Ir01Curve {
    /// Quote-currency (domestic) discount curve.
    Domestic,
    /// Base-currency (foreign) discount curve.
    Foreign,
}

/// IR01 (sensitivity to a 1bp parallel shift in one discount curve).
pub(crate) struct FxSwapIr01(pub(crate) Ir01Curve);

impl MetricCalculator for FxSwapIr01 {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let fx_swap: &FxSwap = context.instrument_as()?;
        if crate::market_inputs::event_has_occurred(fx_swap.far_date, context.as_of) {
            return Ok(0.0);
        }
        let curve_id = match self.0 {
            Ir01Curve::Domestic => fx_swap.domestic_discount_curve_id.clone(),
            Ir01Curve::Foreign => fx_swap.foreign_discount_curve_id.clone(),
        };
        let bump_bp = bump_config::resolve(context)?.rate_bump_bp;
        let as_of = context.as_of;
        let (pv_up, pv_down) = context.with_market_scratch(|context, scratch| {
            let token_up =
                scratch.apply_curve_bump_in_place(&curve_id, BumpSpec::parallel_bp(bump_bp))?;
            let pv_up = context.reprice_raw(scratch, as_of)?;
            scratch.revert_scratch_bump(token_up)?;

            let token_down =
                scratch.apply_curve_bump_in_place(&curve_id, BumpSpec::parallel_bp(-bump_bp))?;
            let pv_down = context.reprice_raw(scratch, as_of)?;
            scratch.revert_scratch_bump(token_down)?;

            Ok((pv_up, pv_down))
        })?;
        central_diff_by_width(pv_up, pv_down, 2.0 * bump_bp)
    }
}
