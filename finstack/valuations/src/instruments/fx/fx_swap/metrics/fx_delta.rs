//! FX Delta calculator for FX Swaps.
//!
//! Computes FX delta (FX spot sensitivity) using central finite differences.
//!
//! # FX Delta vs FX01
//!
//! Both `FxDelta` and `Fx01` now report PV change per **1% relative** spot
//! move. The two metrics are numerically equivalent for `FxSwap`; they are
//! kept separate only for API surface stability. `Fx01` is registered via
//! the shared `metrics::sensitivities::fx01::GenericFx01Calculator`, which
//! uses `MarketBump::FxPct` on the FX matrix and reprices through the
//! canonical pricer. `FxDelta` uses the in-line `FxSwapPricingContext`
//! shortcut — slightly faster but functionally identical.
//!
//! Historical note: `Fx01` previously meant "1bp absolute" (Δspot = 0.0001)
//! across all FX instruments. The convention was changed to "1% relative"
//! so attribution consumers — which multiply Fx01 by a percentage-points
//! shift — produce correct P&L magnitudes.

use crate::instruments::fx::fx_swap::pricing_helper::FxSwapPricingContext;
use crate::instruments::fx::fx_swap::FxSwap;
use crate::metrics::bump_sizes;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_core::Result;

/// FX Delta calculator for FX Swaps.
///
/// Computes the PV change for a 1% relative move in spot rate using
/// central finite difference for O(h²) accuracy.
pub(crate) struct FxDeltaCalculator;

impl MetricCalculator for FxDeltaCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let fx_swap: &FxSwap = context.instrument_as()?;
        let curves = std::sync::Arc::clone(&context.curves);
        let as_of = context.as_of;

        // Use shared pricing context for consistent calculations
        let ctx = FxSwapPricingContext::build(fx_swap, &curves, as_of)?;

        // Helper to calculate PV for a given spot rate
        let calculate_pv = |spot: f64| -> Result<f64> {
            // Recompute near/far rates with bumped spot when not fixed
            let near_rate = fx_swap.near_rate.unwrap_or(spot);

            // Recompute forward with bumped spot
            let model_fwd = FxSwapPricingContext::calculate_cip_forward(
                spot,
                ctx.df_dom_near,
                ctx.df_dom_far,
                ctx.df_for_near,
                ctx.df_for_far,
            )?;

            let far_rate = fx_swap.far_rate.unwrap_or(model_fwd);

            Ok(ctx.total_pv_with_spot(spot, near_rate, far_rate))
        };

        // Central finite difference with 1% relative bump
        let rate_bump_amt = ctx.model_spot * bump_sizes::SPOT;
        let pv_up = calculate_pv(ctx.model_spot + rate_bump_amt)?;
        let pv_down = calculate_pv(ctx.model_spot - rate_bump_amt)?;

        // FX Delta = (PV_up - PV_down) / (2 × bump_pct)
        // This gives the PV change per 1% move in spot
        let fx_delta = (pv_up - pv_down) / (2.0 * bump_sizes::SPOT);

        Ok(fx_delta)
    }
}
