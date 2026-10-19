//! CMS option metrics module.
//!
//! Provides greek coverage for CMS options using finite difference methods.
//! Note: Some metrics (delta, convexity adjustment risk) require the CMS pricer
//! to be fully implemented to compute forward swap rates and convexity adjustments.

use crate::cms_common::ConvexityAdjustmentRiskCalculator;
use crate::cms_option::pricer::convexity_adjustment_with_frequency;
use crate::cms_option::types::CmsOption;
use finstack_quant_core::dates::{DateExt, DayCountContext};
use finstack_quant_core::market_data::bumps::{BumpSpec, MarketBump};
use finstack_quant_core::math::norm_pdf;
use finstack_quant_core::Result;
use finstack_quant_models::d1_d2_black76;
use finstack_quant_valuations_core::metrics::bump_discount_curve_parallel;
use finstack_quant_valuations_core::metrics::bump_surface_vol_absolute;
use finstack_quant_valuations_core::metrics::sensitivities::config as sens_config;
use finstack_quant_valuations_core::metrics::VOL_POINTS_PER_ABSOLUTE_VOL;
use finstack_quant_valuations_core::metrics::{MetricCalculator, MetricContext, MetricRegistry};

/// Register CMS option metrics with the registry.
pub(crate) fn register_cms_option_metrics(
    registry: &mut MetricRegistry,
) -> std::result::Result<(), finstack_quant_valuations_core::metrics::MetricRegistryError> {
    use finstack_quant_valuations_core::pricer::InstrumentType;
    finstack_quant_valuations_core::register_metrics! {
        registry: registry,
        instrument: InstrumentType::CmsOption,
        metrics: [
            (Delta, DeltaCalculator),
            (Vega, VegaCalculator),
            (Rho, RhoCalculator),
            (Dv01, finstack_quant_valuations_core::metrics::UnifiedDv01Calculator::new(finstack_quant_valuations_core::metrics::Dv01CalculatorConfig::parallel_combined())),
            (Vanna, VannaCalculator),
            (Volga, VolgaCalculator),
            (BucketedDv01, finstack_quant_valuations_core::metrics::UnifiedDv01Calculator::new(finstack_quant_valuations_core::metrics::Dv01CalculatorConfig::triangular_key_rate())),
            // Convexity adjustment risk (custom metric)
            (ConvexityAdjustmentRisk, ConvexityAdjustmentRiskCalculator::<CmsOption>(std::marker::PhantomData)),
        ]
    }

    Ok(())
}

// Delta Calculator

/// Delta calculator for CMS options.
pub(crate) struct DeltaCalculator;

impl MetricCalculator for DeltaCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let bump_bp = sens_config::resolve(context)?.rate_bump_bp;
        let option: &CmsOption = context.instrument_as()?;
        let base_pv = context.base_value.amount();

        // Parallel shift of the curve that drives the forward swap rate.
        let curves_bumped = context.curves.bump([MarketBump::Curve {
            id: option.forward_curve_id.clone(),
            spec: BumpSpec::parallel_bp(bump_bp),
        }])?;

        let pv_bumped = context.reprice_raw(&curves_bumped, context.as_of)?;

        // Delta = PV change per 1bp of the forward curve.
        Ok((pv_bumped - base_pv) / bump_bp)
    }
}

// Vega Calculator

/// Vega calculator for CMS options.
pub(crate) struct VegaCalculator;

impl MetricCalculator for VegaCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let vol_bump = sens_config::resolve(context)?.vol_bump_decimal;
        let option: &CmsOption = context.instrument_as()?;
        let as_of = context.as_of;
        let base_pv = context.base_value.amount();

        let final_date = option.fixing_dates.last().copied().unwrap_or(as_of);
        let t = option.day_count.year_fraction(
            as_of,
            final_date,
            finstack_quant_core::dates::DayCountContext::default(),
        )?;
        if t <= 0.0 {
            return Ok(0.0);
        }

        // Bump volatility surface by an absolute vol amount (vol points).
        let curves_bumped =
            bump_surface_vol_absolute(&context.curves, option.vol_surface_id.as_str(), vol_bump)?;

        // Reprice with bumped vol
        let pv_bumped = context.reprice_raw(&curves_bumped, as_of)?;

        // Vega per **vol point** (consistent with `MetricId::Vega` and the
        // FD/analytic vega used elsewhere): normalize by the bump expressed in
        // vol points (`bump * VOL_POINTS_PER_ABSOLUTE_VOL`), not the raw
        // absolute-vol bump, which would overstate vega by 100×.
        let vega = (pv_bumped - base_pv) / (vol_bump * VOL_POINTS_PER_ABSOLUTE_VOL);

        Ok(vega)
    }
}

// Rho Calculator

/// Rho calculator for CMS options.
pub(crate) struct RhoCalculator;

impl MetricCalculator for RhoCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let bump_bp = sens_config::resolve(context)?.rate_bump_bp;
        let option: &CmsOption = context.instrument_as()?;
        let as_of = context.as_of;
        let base_pv = context.base_value.amount();

        let final_date = option.fixing_dates.last().copied().unwrap_or(as_of);
        let t = option.day_count.year_fraction(
            as_of,
            final_date,
            finstack_quant_core::dates::DayCountContext::default(),
        )?;
        if t <= 0.0 {
            return Ok(0.0);
        }

        // The helper's argument is in basis points (1.0 = 1bp).
        let curves_bumped =
            bump_discount_curve_parallel(&context.curves, &option.discount_curve_id, bump_bp)?;

        // Reprice with bumped curve
        let pv_bumped = context.reprice_raw(&curves_bumped, as_of)?;

        // Rho per 1bp = (PV(rate + bump) − PV(base)) / bump_bp
        Ok((pv_bumped - base_pv) / bump_bp)
    }
}

// Vanna Calculator

/// Vanna calculator for CMS options.
pub(crate) struct VannaCalculator;

impl MetricCalculator for VannaCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let inst = context.instrument_as::<CmsOption>()?;
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
            // Seasoned period: the pricer values it as intrinsic on the
            // recorded fixing, which has no vol dependence.
            if fixing_date < as_of {
                continue;
            }

            // 1. Calculate Forward Swap Rate
            let reference_swap = inst.reference_swap();
            let swap_start = reference_swap.reference_swap_start(fixing_date)?;
            let swap_tenor_months =
                crate::cms_common::cms_tenor_months(inst.cms_tenor, "CmsOption cms_tenor")?;
            let swap_end = swap_start.add_months(swap_tenor_months)?;

            let (forward_swap_rate, _) =
                reference_swap.forward_rate_and_annuity(curves, as_of, swap_start, swap_end)?;

            // 2. Volatility and Time
            // Calendar time for the vol axis: ACT/365F, not the accrual day count.
            let time_to_fixing = finstack_quant_core::dates::DayCount::Act365F.year_fraction(
                as_of,
                fixing_date,
                DayCountContext::default(),
            )?;

            if time_to_fixing <= 1e-6 {
                continue;
            }

            // As in the pricer: σ(K) prices the Black payoff, σ(F) drives the
            // convexity adjustment (a property of the swap-rate distribution).
            let vol = finstack_quant_models::volatility::get_surface_vol_clamped(
                &vol_surface,
                time_to_fixing,
                strike,
            );
            let atm_vol = finstack_quant_models::volatility::get_surface_vol_clamped(
                &vol_surface,
                time_to_fixing,
                forward_swap_rate,
            );

            // 3. Convexity Adjustment Derivative
            // Convexity = 0.5 * σ(F)^2 * T * G(S)
            // where G(S) = swap_tenor / (1 + S * swap_tenor)^2
            // d(Convexity)/dσ(F) = 2 * Convexity / σ(F); a parallel vol shift
            // moves σ(F) and σ(K) equally, so this is also d/dσ.
            let conv_adj = convexity_adjustment_with_frequency(
                atm_vol,
                time_to_fixing,
                inst.cms_tenor.to_years(),
                forward_swap_rate,
                reference_swap.payments_per_year()?,
                crate::cms_common::signed_act365f_year_fraction(swap_start, payment_date)?,
            );
            let d_conv_d_vol = if atm_vol.abs() > 1e-10 {
                2.0 * conv_adj / atm_vol
            } else {
                0.0
            };

            let adjusted_rate = forward_swap_rate + conv_adj;

            // 4. Black-76 Vanna and Gamma
            // Vanna_Black = - N'(d1) * d2 / sigma
            // Gamma_Black = N'(d1) / (F * sigma * sqrt(T))
            // Discount factor uses curve-consistent relative DF
            let df_pay = discount_curve
                .as_ref()
                .df_between_dates(as_of, payment_date)?;

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

        // The Black-76 vanna above is per unit (decimal) vol; report per
        // **vol point** on the σ axis (consistent with vega and
        // `MetricId::Vanna`).
        Ok(total_vanna * inst.notional.amount() / VOL_POINTS_PER_ABSOLUTE_VOL)
    }
}

// Volga Calculator

/// Volga calculator for CMS options.
pub(crate) struct VolgaCalculator;

impl MetricCalculator for VolgaCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let option: &CmsOption = context.instrument_as()?;
        let as_of = context.as_of;
        let base_pv = context.base_value.amount();

        let final_date = option.fixing_dates.last().copied().unwrap_or(as_of);
        let t = option.day_count.year_fraction(
            as_of,
            final_date,
            finstack_quant_core::dates::DayCountContext::default(),
        )?;
        if t <= 0.0 {
            return Ok(0.0);
        }

        let vol_bump = sens_config::resolve(context)?.vol_bump_decimal;

        let curves_vol_up =
            bump_surface_vol_absolute(&context.curves, option.vol_surface_id.as_str(), vol_bump)?;
        let pv_vol_up = context.reprice_raw(&curves_vol_up, as_of)?;

        let curves_vol_down =
            bump_surface_vol_absolute(&context.curves, option.vol_surface_id.as_str(), -vol_bump)?;
        let pv_vol_down = context.reprice_raw(&curves_vol_down, as_of)?;

        // Volga per **vol point squared** (consistent with `MetricId::Volga`):
        // normalize by the bump in vol points, squared. Dividing by the raw
        // `vol_bump²` would overstate volga by 100² = 10,000×.
        let width = vol_bump * VOL_POINTS_PER_ABSOLUTE_VOL;
        let volga = (pv_vol_up - 2.0 * base_pv + pv_vol_down) / (width * width);
        Ok(volga)
    }
}
