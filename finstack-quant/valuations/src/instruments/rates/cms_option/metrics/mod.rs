//! CMS option metrics module.
//!
//! Provides greek coverage for CMS options using finite difference methods.
//! Note: Some metrics (delta, convexity adjustment risk) require the CMS pricer
//! to be fully implemented to compute forward swap rates and convexity adjustments.

use crate::instruments::common_impl::pricing::time::relative_df_discount_curve;
use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::rates::cms_option::pricer::{
    convexity_adjustment_with_frequency, CmsOptionPricer,
};
use crate::instruments::rates::cms_option::types::CmsOption;
use crate::metrics::bump_discount_curve_parallel;
use crate::metrics::bump_sizes;
use crate::metrics::bump_surface_vol_absolute;
use crate::metrics::VOL_POINTS_PER_ABSOLUTE_VOL;
use crate::metrics::{MetricCalculator, MetricContext, MetricId, MetricRegistry};
use crate::models::d1_d2_black76;
use finstack_quant_core::dates::{DateExt, DayCountContext};
use finstack_quant_core::market_data::bumps::{BumpSpec, MarketBump};
use finstack_quant_core::math::norm_pdf;
use finstack_quant_core::Result;
use std::sync::Arc;

/// Register CMS option metrics with the registry.
pub(crate) fn register_cms_option_metrics(registry: &mut MetricRegistry) {
    use crate::pricer::InstrumentType;
    crate::register_metrics! {
        registry: registry,
        instrument: InstrumentType::CmsOption,
        metrics: [
            (Delta, DeltaCalculator),
            (Vega, VegaCalculator),
            (Rho, RhoCalculator),
            (Dv01, crate::metrics::UnifiedDv01Calculator::<
                crate::instruments::CmsOption,
            >::new(crate::metrics::Dv01CalculatorConfig::parallel_combined())),
            (Vanna, VannaCalculator),
            (Volga, VolgaCalculator),
            (BucketedDv01, crate::metrics::UnifiedDv01Calculator::<
                crate::instruments::CmsOption,
            >::new(crate::metrics::Dv01CalculatorConfig::triangular_key_rate())),
        ]
    }

    // Convexity adjustment risk (custom metric)
    registry.register_metric(
        MetricId::ConvexityAdjustmentRisk,
        Arc::new(ConvexityAdjustmentRiskCalculator),
        &[InstrumentType::CmsOption],
    );
}

// ---------------------------------------------------------------------------
// Delta Calculator
// ---------------------------------------------------------------------------

/// Delta calculator for CMS options.
pub(crate) struct DeltaCalculator;

impl MetricCalculator for DeltaCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let option: &CmsOption = context.instrument_as()?;
        let base_pv = context.base_value.amount();

        // Determine which curve drives the forward rate
        let curve_to_bump = &option.forward_curve_id;

        // Bump the relevant curve by 1bp (parallel shift)
        let bump_bp = 1.0;
        let curves_bumped = context.curves.bump([MarketBump::Curve {
            id: curve_to_bump.clone(),
            spec: BumpSpec::parallel_bp(bump_bp),
        }])?;

        // Reprice
        let pv_bumped = option.value(&curves_bumped, context.as_of)?.amount();

        // Delta = Change in PV
        Ok(pv_bumped - base_pv)
    }
}

// ---------------------------------------------------------------------------
// Vega Calculator
// ---------------------------------------------------------------------------

/// Vega calculator for CMS options.
pub(crate) struct VegaCalculator;

impl MetricCalculator for VegaCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let option: &CmsOption = context.instrument_as()?;
        let as_of = context.as_of;
        let base_pv = context.base_value.amount();

        // Check if expired
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
        let curves_bumped = bump_surface_vol_absolute(
            &context.curves,
            option.vol_surface_id.as_str(),
            bump_sizes::VOLATILITY,
        )?;

        // Reprice with bumped vol
        let pv_bumped = option.value(&curves_bumped, as_of)?.amount();

        // Vega per **vol point** (consistent with `MetricId::Vega` and the
        // FD/analytic vega used elsewhere): normalize by the bump expressed in
        // vol points (`bump * VOL_POINTS_PER_ABSOLUTE_VOL`), not the raw
        // absolute-vol bump, which would overstate vega by 100×.
        let vega = (pv_bumped - base_pv) / (bump_sizes::VOLATILITY * VOL_POINTS_PER_ABSOLUTE_VOL);

        Ok(vega)
    }
}

// ---------------------------------------------------------------------------
// Rho Calculator
// ---------------------------------------------------------------------------

/// Rho calculator for CMS options.
pub(crate) struct RhoCalculator;

impl MetricCalculator for RhoCalculator {
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

        // Bump discount curve by 1bp (0.0001)
        let bump_bp = 0.0001;
        let curves_bumped =
            bump_discount_curve_parallel(&context.curves, &option.discount_curve_id, bump_bp)?;

        // Reprice with bumped curve
        let pv_bumped = option.value(&curves_bumped, as_of)?.amount();

        // Rho = PV(rate + 1bp) − PV(base)
        let rho = pv_bumped - base_pv;

        Ok(rho)
    }
}

// ---------------------------------------------------------------------------
// Vanna Calculator
// ---------------------------------------------------------------------------

/// Vanna calculator for CMS options.
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
            let swap_start = inst.reference_swap_start(fixing_date)?;
            let swap_tenor_months = (inst.cms_tenor * 12.0).round() as i32;
            let swap_end = swap_start.add_months(swap_tenor_months);

            let (forward_swap_rate, _) =
                pricer.calculate_forward_swap_rate(inst, curves, as_of, swap_start, swap_end)?;

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

            let vol = vol_surface.value_clamped(time_to_fixing, strike);

            // 3. Convexity Adjustment Derivative
            // Convexity = 0.5 * vol^2 * T * G(S)
            // where G(S) = swap_tenor / (1 + S * swap_tenor)^2
            // d(Convexity)/d(Vol) = vol * T * G(S) = 2 * Convexity / Vol
            let conv_adj = convexity_adjustment_with_frequency(
                vol,
                time_to_fixing,
                inst.cms_tenor,
                forward_swap_rate,
                1.0 / inst.resolved_swap_fixed_freq().to_years_simple(),
            );
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

// ---------------------------------------------------------------------------
// Volga Calculator
// ---------------------------------------------------------------------------

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

        let vol_bump = bump_sizes::VOLATILITY;

        let curves_vol_up =
            bump_surface_vol_absolute(&context.curves, option.vol_surface_id.as_str(), vol_bump)?;
        let pv_vol_up = option.value(&curves_vol_up, as_of)?.amount();

        let curves_vol_down =
            bump_surface_vol_absolute(&context.curves, option.vol_surface_id.as_str(), -vol_bump)?;
        let pv_vol_down = option.value(&curves_vol_down, as_of)?.amount();

        // Volga per **vol point squared** (consistent with `MetricId::Volga`):
        // normalize by the bump in vol points, squared. Dividing by the raw
        // `vol_bump²` would overstate volga by 100² = 10,000×.
        let width = vol_bump * VOL_POINTS_PER_ABSOLUTE_VOL;
        let volga = (pv_vol_up - 2.0 * base_pv + pv_vol_down) / (width * width);
        Ok(volga)
    }
}

// ---------------------------------------------------------------------------
// Convexity Adjustment Risk Calculator
// ---------------------------------------------------------------------------

/// Convexity adjustment risk calculator for CMS options.
pub(crate) struct ConvexityAdjustmentRiskCalculator;

impl MetricCalculator for ConvexityAdjustmentRiskCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let option: &CmsOption = context.instrument_as()?;
        let as_of = context.as_of;
        let base_pv = context.base_value.amount();

        // Reprice with zero convexity
        let pricer = CmsOptionPricer::new();
        let linear_pv = pricer
            .price_internal_with_convexity(
                option,
                &context.curves,
                as_of,
                0.0, // No convexity
            )?
            .amount();

        // Risk is the difference
        Ok(base_pv - linear_pv)
    }
}
