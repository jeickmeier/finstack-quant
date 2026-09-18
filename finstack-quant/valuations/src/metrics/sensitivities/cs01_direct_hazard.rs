//! Direct hazard-rate bump CS01 for hazard curves without a replayable
//! calibration recipe.
//!
//! The quote-space calculators in [`super::cs01`] re-bootstrap the hazard
//! curve from bumped par spreads and therefore need the curve's calibration
//! recipe. An analyst-built curve (flat hazard from an assumed spread, or
//! knots typed in from a rating transition) carries no recipe, so those
//! calculators refuse it. These calculators shift the hazard rates directly:
//! a parallel bump on every knot, or one knot at a time for the bucketed
//! series, with the same central difference, sign convention (long credit
//! risk loses when hazard rises, so CS01 is negative) and metric keys as the
//! quote-space calculators. The bump is scaled to a **spread-equivalent**
//! shock through the credit triangle `s ≈ (1 − R) · λ` using the curve's own
//! recovery rate, so one basis point of par spread moves the intensity by
//! `1 / (1 − R)` basis points and the result is comparable, to first order,
//! with the quote-space number under the same `cs01::<curve>` key.

use std::marker::PhantomData;
use std::sync::Arc;

use finstack_quant_core::market_data::term_structures::HazardCurve;

use super::config as sens_config;
use super::cs01::{cs01_reval, reprice_with_hazard, sensitivity_central_diff};
use crate::instruments::common_impl::traits::Instrument;
use crate::metrics::{MetricCalculator, MetricContext, MetricId};

/// Parallel hazard-rate bump CS01.
pub(crate) struct DirectHazardParallelCs01<I> {
    _phantom: PhantomData<I>,
}

impl<I> Default for DirectHazardParallelCs01<I> {
    fn default() -> Self {
        Self {
            _phantom: PhantomData,
        }
    }
}

/// Bucketed hazard-rate bump CS01: one bucket per hazard-curve knot.
pub(crate) struct DirectHazardBucketedCs01<I> {
    _phantom: PhantomData<I>,
}

impl<I> Default for DirectHazardBucketedCs01<I> {
    fn default() -> Self {
        Self {
            _phantom: PhantomData,
        }
    }
}

/// Hazard-intensity bump (basis points) equivalent to a one-basis-point par
/// spread shock under the credit triangle, from the curve's recovery rate.
fn spread_equivalent_hazard_bump_bp(hazard: &HazardCurve, spread_bump_bp: f64) -> f64 {
    let loss_given_default = 1.0 - hazard.recovery_rate();
    if loss_given_default > 1e-6 && loss_given_default <= 1.0 {
        spread_bump_bp / loss_given_default
    } else {
        spread_bump_bp
    }
}

/// Symmetric central difference of the instrument value under `bump(±bp)`,
/// falling back to a forward difference when the down bump would push a
/// hazard rate below zero.
fn hazard_bump_sensitivity(
    context: &mut MetricContext,
    hazard: &Arc<HazardCurve>,
    bump_bp: f64,
    bump: impl Fn(&HazardCurve, f64) -> finstack_quant_core::Result<HazardCurve>,
) -> finstack_quant_core::Result<f64> {
    let mut reval = cs01_reval(context);
    context.with_market_scratch(|_, scratch| {
        let up = bump(hazard.as_ref(), bump_bp)?;
        let pv_up = reprice_with_hazard(scratch, up, hazard, &mut reval)?;
        match bump(hazard.as_ref(), -bump_bp) {
            Ok(down) => {
                let pv_down = reprice_with_hazard(scratch, down, hazard, &mut reval)?;
                Ok(sensitivity_central_diff(pv_up, pv_down, bump_bp))
            }
            Err(_) => {
                let pv_base = reval(scratch)?;
                Ok((pv_up - pv_base) / bump_bp)
            }
        }
    })
}

impl<I> MetricCalculator for DirectHazardParallelCs01<I>
where
    I: Instrument + 'static,
{
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let instrument: &I = context.instrument_as()?;
        let Some((hazard_id, _)) =
            super::cs01::resolve_optional_cs01_curves(instrument, false, "CS01")?
        else {
            return Ok(0.0);
        };
        let bump_bp = sens_config::from_context_or_default(
            context.get_config(),
            context.get_metric_overrides(),
        )?
        .credit_spread_bump_bp;
        let hazard = context.curves.get_hazard(hazard_id.as_str())?;
        let hazard_bp = spread_equivalent_hazard_bump_bp(hazard.as_ref(), bump_bp);
        let cs01 = hazard_bump_sensitivity(context, &hazard, hazard_bp, |curve, bp| {
            curve.with_parallel_hazard_rate_bump_bp(bp)
        })? * (hazard_bp / bump_bp);
        context.computed.insert(
            MetricId::composite(&MetricId::Cs01, &[hazard_id.as_str()]),
            cs01,
        );
        Ok(cs01)
    }
}

impl<I> MetricCalculator for DirectHazardBucketedCs01<I>
where
    I: Instrument + 'static,
{
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let instrument: &I = context.instrument_as()?;
        let Some((hazard_id, _)) =
            super::cs01::resolve_optional_cs01_curves(instrument, false, "CS01")?
        else {
            return Ok(0.0);
        };
        let bump_bp = sens_config::from_context_or_default(
            context.get_config(),
            context.get_metric_overrides(),
        )?
        .credit_spread_bump_bp;
        let hazard = context.curves.get_hazard(hazard_id.as_str())?;
        let hazard_bp = spread_equivalent_hazard_bump_bp(hazard.as_ref(), bump_bp);
        let tenors: Vec<f64> = hazard.knot_points().map(|(tenor, _)| tenor).collect();

        let mut series = Vec::with_capacity(tenors.len());
        let mut total = 0.0;
        for tenor in tenors {
            let cs01 = hazard_bump_sensitivity(context, &hazard, hazard_bp, |curve, bp| {
                curve.with_tenor_hazard_rate_bumps_bp(&[(tenor, bp)])
            })? * (hazard_bp / bump_bp);
            series.push((sens_config::format_bucket_label_cow(tenor), cs01));
            total += cs01;
        }
        context.store_bucketed_series(
            MetricId::composite(&MetricId::BucketedCs01, &[hazard_id.as_str()]),
            series,
        );
        Ok(total)
    }
}
