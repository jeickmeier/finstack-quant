//! Variance swap DV01 using forward bump-and-reprice.

use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::equity::variance_swap::VarianceSwap;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_quant_core::market_data::bumps::{BumpSpec, MarketBump};

/// Variance swap DV01 expressed as PV change per 1bp parallel discount-curve bump.
pub(crate) struct Dv01Calculator;

impl MetricCalculator for Dv01Calculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let swap: &VarianceSwap = context.instrument_as()?;
        let bump_bp = crate::metrics::resolve_sensitivities_config(context.config())?.rate_bump_bp;

        let base_pv = context.base_value.amount();
        let bumped_ctx = context.curves.as_ref().bump([MarketBump::Curve {
            id: swap.discount_curve_id.clone(),
            spec: BumpSpec::parallel_bp(bump_bp),
        }])?;
        let bumped_pv = swap.value(&bumped_ctx, context.as_of)?.amount();

        Ok((bumped_pv - base_pv) / bump_bp)
    }
}
