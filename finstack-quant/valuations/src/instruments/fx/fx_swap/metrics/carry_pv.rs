//! Carry PV metric for FX swaps.
//!
//! Computes the present value contribution from holding the foreign leg
//! (converted to the domestic currency) using model-implied spot/forward
//! rates. This approximates the "carry" earned from the interest differential
//! between currencies when maintaining the swap position.

use crate::instruments::fx::fx_swap::pricing_helper::FxSwapPricingContext;
use crate::instruments::fx::fx_swap::FxSwap;
use crate::metrics::{MetricCalculator, MetricContext};

/// Carry PV calculator for FX swaps.
///
/// Carry PV represents the PV contribution from the interest rate differential
/// between the two currencies. It is computed as the difference between:
/// - Near leg value at model spot (receive base, convert to quote)
/// - Far leg value at model forward (pay base, convert to quote)
pub(crate) struct CarryPv;

impl MetricCalculator for CarryPv {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let fx_swap: &FxSwap = context.instrument_as()?;
        let curves = std::sync::Arc::clone(&context.curves);
        let as_of = context.as_of;

        // Use shared pricing context for consistent calculations
        let ctx = FxSwapPricingContext::build(fx_swap, &curves, as_of)?;

        // Carry PV: model-implied near value minus model-implied far value
        // This captures the benefit/cost of the interest rate differential
        let term1 = if ctx.include_near {
            ctx.base_notional * ctx.model_spot * ctx.df_dom_near
        } else {
            0.0
        };
        let term2 = if ctx.include_far {
            ctx.base_notional * ctx.model_forward * ctx.df_dom_far
        } else {
            0.0
        };

        let carry_pv = term1 - term2;
        Ok(carry_pv)
    }
}
