//! Market-convention DV01 for interest rate swaps.
//!
//! Bloomberg SWPM reports IRS DV01 under a constant par-rate bump convention.
//! This differs from generic curve DV01, which bumps zero/forward curves directly.

use crate::instruments::common_impl::numeric::decimal_to_f64;
use crate::instruments::rates::irs::{InterestRateSwap, PayReceive};
use crate::metrics::{MetricCalculator, MetricContext, MetricId};
use finstack_quant_core::market_data::bumps::BumpSpec;
use std::sync::Arc;

const ONE_BP_DECIMAL: f64 = crate::constants::ONE_BASIS_POINT;

/// IRS DV01 calculator using par-rate bump convention.
pub(crate) struct IrsDv01Calculator;

impl MetricCalculator for IrsDv01Calculator {
    fn dependencies(&self) -> &[MetricId] {
        &[MetricId::Annuity, MetricId::ParRate]
    }

    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let irs: &InterestRateSwap = context.instrument_as()?;
        let annuity = *context.computed.get(&MetricId::Annuity).ok_or_else(|| {
            finstack_quant_core::Error::Validation("IRS DV01 requires annuity".to_string())
        })?;
        let par_rate = *context.computed.get(&MetricId::ParRate).ok_or_else(|| {
            finstack_quant_core::Error::Validation("IRS DV01 requires par_rate".to_string())
        })?;
        let fixed_rate = decimal_to_f64(irs.fixed.rate, "fixed leg rate")?;
        let bump_bp = crate::metrics::sensitivities::config::from_context_or_default(
            context.config(),
            context.get_metric_overrides(),
        )?
        .rate_bump_bp;

        let d_annuity_dbp = annuity_derivative_per_bp(context, irs, bump_bp)?;
        let receive_fixed_dv01 = irs.notional.amount()
            * ((fixed_rate - par_rate) * d_annuity_dbp - annuity * ONE_BP_DECIMAL);

        Ok(match irs.side {
            PayReceive::Receive => receive_fixed_dv01,
            PayReceive::Pay => -receive_fixed_dv01,
        })
    }
}

fn annuity_derivative_per_bp(
    context: &MetricContext,
    irs: &InterestRateSwap,
    bump_bp: f64,
) -> finstack_quant_core::Result<f64> {
    if bump_bp.abs() <= f64::EPSILON {
        return Ok(0.0);
    }

    // Bump up and down on independent clones that are moved straight into the
    // annuity computation (which needs an owned `MarketContext` to Arc-wrap). This
    // drops one full `MarketContext` clone per DV01 versus bumping a shared scratch
    // in place and cloning it for each side, and removes the revert bookkeeping.
    let mut curves_up = context.curves.as_ref().clone();
    let _bump_up = curves_up
        .apply_curve_bump_in_place(&irs.fixed.discount_curve_id, BumpSpec::parallel_bp(bump_bp))?;
    let annuity_up = annuity_with_curves(context, curves_up)?;

    let mut curves_down = context.curves.as_ref().clone();
    let _bump_down = curves_down.apply_curve_bump_in_place(
        &irs.fixed.discount_curve_id,
        BumpSpec::parallel_bp(-bump_bp),
    )?;
    let annuity_down = annuity_with_curves(context, curves_down)?;

    Ok((annuity_up - annuity_down) / (2.0 * bump_bp))
}

fn annuity_with_curves(
    context: &MetricContext,
    curves: finstack_quant_core::market_data::context::MarketContext,
) -> finstack_quant_core::Result<f64> {
    let mut bumped_context = MetricContext::new(
        Arc::clone(&context.instrument),
        Arc::new(curves),
        context.as_of,
        context.base_value,
        context.config_arc(),
    );
    super::annuity::AnnuityCalculator.calculate(&mut bumped_context)
}
