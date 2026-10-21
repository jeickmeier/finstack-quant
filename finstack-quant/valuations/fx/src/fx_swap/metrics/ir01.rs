//! Domestic and foreign IR01 for FX swaps.
//!
//! Computes sensitivity to a 1bp parallel bump in one leg's discount curve
//! using central finite difference for O(h²) accuracy.

use crate::fx_swap::pricing_helper::FxSwapPricingContext;
use crate::fx_swap::FxSwap;
use finstack_quant_core::Result;
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
///
/// Uses central finite difference on the selected curve's discount factors:
/// IR01 = (PV(rates + 1bp) - PV(rates - 1bp)) / 2
pub(crate) struct FxSwapIr01(pub(crate) Ir01Curve);

/// Standard 1bp bump for IR01 calculation.
const IR01_BUMP: f64 = 0.0001;

impl MetricCalculator for FxSwapIr01 {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let fx_swap: &FxSwap = context.instrument_as()?;
        let curves = std::sync::Arc::clone(&context.curves);
        let as_of = context.as_of;

        // Use shared pricing context for consistent calculations
        let ctx = FxSwapPricingContext::build(fx_swap, &curves, as_of)?;

        let curve_id = match self.0 {
            Ir01Curve::Domestic => &fx_swap.domestic_discount_curve_id,
            Ir01Curve::Foreign => &fx_swap.foreign_discount_curve_id,
        };
        let disc = curves.get_discount(curve_id.as_str())?;

        // Year fractions for bump application, on the bumped curve's basis.
        let t_near = disc.day_count().year_fraction(
            as_of,
            fx_swap.near_date,
            finstack_quant_core::dates::DayCountContext::default(),
        )?;
        let t_far = disc.day_count().year_fraction(
            as_of,
            fx_swap.far_date,
            finstack_quant_core::dates::DayCountContext::default(),
        )?;

        let calculate_pv = |bump: f64| -> Result<f64> {
            // Apply parallel bump to the selected curve: df_bumped = df * exp(-bump * t)
            let (df_dom_near, df_dom_far, df_for_near, df_for_far) = match self.0 {
                Ir01Curve::Domestic => (
                    ctx.df_dom_near * (-bump * t_near).exp(),
                    ctx.df_dom_far * (-bump * t_far).exp(),
                    ctx.df_for_near,
                    ctx.df_for_far,
                ),
                Ir01Curve::Foreign => (
                    ctx.df_dom_near,
                    ctx.df_dom_far,
                    ctx.df_for_near * (-bump * t_near).exp(),
                    ctx.df_for_far * (-bump * t_far).exp(),
                ),
            };

            let near_rate = match fx_swap.near_rate {
                Some(rate) => rate,
                None => FxSwapPricingContext::calculate_cip_forward(
                    ctx.model_spot,
                    df_dom_near,
                    df_for_near,
                )?,
            };
            let far_rate = match fx_swap.far_rate {
                Some(rate) => rate,
                None => FxSwapPricingContext::calculate_cip_forward(
                    ctx.model_spot,
                    df_dom_far,
                    df_for_far,
                )?,
            };

            let pv_for_leg = ctx.pv_foreign_leg_base_with_dfs(df_for_near, df_for_far);
            let pv_dom_leg =
                ctx.pv_domestic_leg_with_params(near_rate, far_rate, df_dom_near, df_dom_far);

            Ok(pv_for_leg * ctx.model_spot + pv_dom_leg)
        };

        // Central finite difference
        let pv_up = calculate_pv(IR01_BUMP)?;
        let pv_down = calculate_pv(-IR01_BUMP)?;

        Ok((pv_up - pv_down) / 2.0)
    }
}
