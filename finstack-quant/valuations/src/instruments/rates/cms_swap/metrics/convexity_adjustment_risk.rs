//! Convexity adjustment risk calculator for CMS swaps.
//!
//! Computes the dollar value of the convexity adjustment by comparing the
//! full PV (with convexity) against the linear PV (without convexity).
//!
//! Risk = PV(full) - PV(linear)

use crate::instruments::rates::cms_swap::pricer::CmsSwapPricer;
use crate::instruments::rates::cms_swap::types::CmsSwap;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_quant_core::Result;

pub(crate) struct ConvexityAdjustmentRiskCalculator;

impl MetricCalculator for ConvexityAdjustmentRiskCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let swap: &CmsSwap = context.instrument_as()?;
        let as_of = context.as_of;
        let base_pv = context.base_value.amount();

        let pricer = CmsSwapPricer::new();
        let linear_pv = pricer
            .price_internal_with_convexity(swap, &context.curves, as_of, 0.0)?
            .amount();

        Ok(base_pv - linear_pv)
    }
}
