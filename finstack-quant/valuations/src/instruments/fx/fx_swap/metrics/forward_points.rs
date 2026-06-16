//! Forward points metric for FX Swaps.
//!
//! Computes forward points as `far_rate - near_rate`, where the near rate is
//! either provided on the instrument or sourced from the FX matrix, and the far
//! rate is either provided or derived from covered interest parity using the
//! discount curves.

use crate::instruments::fx::fx_swap::pricing_helper::FxSwapPricingContext;
use crate::instruments::fx::fx_swap::FxSwap;
use crate::metrics::{MetricCalculator, MetricContext};

/// Forward points (far rate - near rate).
///
/// Forward points represent the interest rate differential between the two
/// currencies expressed in FX terms. When domestic rates exceed foreign rates,
/// forward points are positive (forward at premium).
pub(crate) struct ForwardPoints;

impl MetricCalculator for ForwardPoints {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let fx_swap: &FxSwap = context.instrument_as()?;
        let curves = std::sync::Arc::clone(&context.curves);
        let as_of = context.as_of;

        // Use shared pricing context for consistent calculations
        let ctx = FxSwapPricingContext::build(fx_swap, &curves, as_of)?;

        Ok(ctx.forward_points())
    }
}
