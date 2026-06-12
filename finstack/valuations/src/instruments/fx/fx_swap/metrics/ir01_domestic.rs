//! Domestic IR01 for FX Swaps.
//!
//! Computes sensitivity to a 1bp parallel bump in the domestic (quote) discount curve
//! using central finite difference for O(h²) accuracy.

use crate::instruments::fx::fx_swap::pricing_helper::FxSwapPricingContext;
use crate::instruments::fx::fx_swap::FxSwap;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_core::Result;

/// Domestic IR01 (sensitivity to 1bp parallel shift in domestic curve).
///
/// Uses central finite difference:
/// IR01 = (PV(dom_rates + 1bp) - PV(dom_rates - 1bp)) / 2
pub(crate) struct DomesticIR01;

/// Standard 1bp bump for IR01 calculation.
const IR01_BUMP: f64 = 0.0001;

impl MetricCalculator for DomesticIR01 {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let fx_swap: &FxSwap = context.instrument_as()?;
        let curves = std::sync::Arc::clone(&context.curves);
        let as_of = context.as_of;

        // Use shared pricing context for consistent calculations
        let ctx = FxSwapPricingContext::build(fx_swap, &curves, as_of)?;

        let domestic_disc = curves.get_discount(fx_swap.domestic_discount_curve_id.as_str())?;

        // Calculate year fractions for bump application
        let t_near = domestic_disc.day_count().year_fraction(
            as_of,
            fx_swap.near_date,
            finstack_core::dates::DayCountContext::default(),
        )?;
        let t_far = domestic_disc.day_count().year_fraction(
            as_of,
            fx_swap.far_date,
            finstack_core::dates::DayCountContext::default(),
        )?;

        // Helper to calculate PV with bumped domestic DFs
        let calculate_pv = |bump: f64| -> Result<f64> {
            // Apply parallel bump: df_bumped = df * exp(-bump * t)
            let df_dom_near_b = ctx.df_dom_near * (-bump * t_near).exp();
            let df_dom_far_b = ctx.df_dom_far * (-bump * t_far).exp();

            // Far rate uses bumped domestic DF in parity if not fixed
            let far_rate = match fx_swap.far_rate {
                Some(rate) => rate,
                None => FxSwapPricingContext::calculate_cip_forward(
                    ctx.contract_near_rate,
                    df_dom_near_b,
                    df_dom_far_b,
                    ctx.df_for_near,
                    ctx.df_for_far,
                )?,
            };

            // Foreign leg PV (unchanged DFs)
            let pv_for_leg = ctx.pv_foreign_leg_base();

            // Domestic leg PV with bumped DFs
            let pv_dom_leg = ctx.pv_domestic_leg_with_params(
                ctx.contract_near_rate,
                far_rate,
                df_dom_near_b,
                df_dom_far_b,
            );

            Ok(pv_for_leg * ctx.model_spot + pv_dom_leg)
        };

        // Central finite difference
        let pv_up = calculate_pv(IR01_BUMP)?;
        let pv_down = calculate_pv(-IR01_BUMP)?;

        Ok((pv_up - pv_down) / 2.0)
    }
}
