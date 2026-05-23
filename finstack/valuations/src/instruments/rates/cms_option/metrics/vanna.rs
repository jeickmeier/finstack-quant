use crate::instruments::common_impl::pricing::time::relative_df_discount_curve;
use crate::instruments::rates::cms_option::pricer::{convexity_adjustment, CmsOptionPricer};
use crate::instruments::rates::cms_option::types::CmsOption;
use crate::metrics::{MetricCalculator, MetricContext};
use crate::models::d1_d2_black76;
use finstack_core::dates::{DateExt, DayCountContext};
use finstack_core::math::norm_pdf;
use finstack_core::Result;

/// Vanna calculator for CMS options.
///
/// # Note
///
/// This metric requires the CMS pricer to compute forward swap rates.
/// Uses an analytical approximation:
/// Vanna = d(Vega)/d(SwapRate)
/// Accounts for convexity adjustment sensitivity.
///
/// # Errors
///
/// Returns an error if vol_surface_id is not provided.
pub(crate) struct VannaCalculator;

impl MetricCalculator for VannaCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let inst = context.instrument_as::<CmsOption>()?;
        let pricer = CmsOptionPricer::new();
        let curves = &context.curves;
        let as_of = context.as_of;
        inst.validate()?;
        let strike = inst.strike_f64()?;

        let mut total_vanna = 0.0;
        let discount_curve = curves.get_discount(inst.discount_curve_id.as_ref())?;

        let vol_surface = curves.get_surface(inst.vol_surface_id.as_str())?;

        for (i, &fixing_date) in inst.fixing_dates.iter().enumerate() {
            let payment_date = inst.payment_dates[i];
            let accrual_fraction = inst.accrual_fractions[i];

            if payment_date <= as_of {
                continue;
            }

            // 1. Calculate Forward Swap Rate
            let swap_start = fixing_date;
            let swap_tenor_months = (inst.cms_tenor * 12.0).round() as i32;
            let swap_end = swap_start.add_months(swap_tenor_months);

            let (forward_swap_rate, _) =
                pricer.calculate_forward_swap_rate(inst, curves, as_of, swap_start, swap_end)?;

            // 2. Volatility and Time
            let time_to_fixing =
                inst.day_count
                    .year_fraction(as_of, fixing_date, DayCountContext::default())?;

            if time_to_fixing <= 1e-6 {
                continue;
            }

            let vol = vol_surface.value_clamped(time_to_fixing, strike);

            // 3. Convexity Adjustment Derivative
            // Convexity = 0.5 * vol^2 * T * G(S)
            // where G(S) = swap_tenor / (1 + S * swap_tenor)^2
            // d(Convexity)/d(Vol) = vol * T * G(S) = 2 * Convexity / Vol
            let conv_adj =
                convexity_adjustment(vol, time_to_fixing, inst.cms_tenor, forward_swap_rate);
            let d_conv_d_vol = if vol.abs() > 1e-10 {
                2.0 * conv_adj / vol
            } else {
                0.0
            };

            let adjusted_rate = forward_swap_rate + conv_adj;

            // 4. Black-76 Vanna and Gamma
            // Vanna_Black = - N'(d1) * d2 / sigma
            // Gamma_Black = N'(d1) / (F * sigma * sqrt(T))
            // Discount factor uses curve-consistent relative DF
            let df_pay = relative_df_discount_curve(discount_curve.as_ref(), as_of, payment_date)?;

            // Use combined d1_d2 for efficiency
            let (d1, d2) = d1_d2_black76(adjusted_rate, strike, vol, time_to_fixing);
            let nd1_prime = norm_pdf(d1);

            let sqrt_t = time_to_fixing.sqrt();

            // Vanna_Black (un-discounted relative to payment date)
            let vanna_black = -nd1_prime * d2 / vol;

            // Gamma_Black (un-discounted relative to payment date)
            let gamma_black = if adjusted_rate > 1e-10 {
                nd1_prime / (adjusted_rate * vol * sqrt_t)
            } else {
                0.0
            };

            // Total Vanna for this period
            // Vanna_Total = Discount * [ Gamma_Black * d(Convexity)/d(Vol) + Vanna_Black ]
            let period_vanna =
                df_pay * accrual_fraction * (gamma_black * d_conv_d_vol + vanna_black);

            total_vanna += period_vanna;
        }

        Ok(total_vanna * inst.notional.amount())
    }
}
