//! Direct hazard-rate intensity sensitivity (`Hazard01` / `BucketedHazard01`).
//!
//! Shifts canonical hazard intensities `λᵢ` directly in basis points (`1 bp =
//! 1e-4` annual decimal intensity) without `1 / (1 - R)` par-spread conversion,
//! publishing results under `hazard01::<curve>` and
//! `bucketed_hazard01::<curve>`.
//!
//! Uses a central difference when all affected knots satisfy `λ >= bump_bp * 1e-4`
//! and explicitly switches to a forward difference when a downward intensity
//! shock would leave the non-negative domain `λ >= 0`.

use std::sync::Arc;

use finstack_quant_core::market_data::term_structures::HazardCurve;
use finstack_quant_core::math::summation::NeumaierAccumulator;
use finstack_quant_core::math::units::ONE_BASIS_POINT;
use finstack_quant_valuations_core::metrics::sensitivities::config as sens_config;
use finstack_quant_valuations_core::metrics::sensitivities::cs01::{
    cs01_reval, reprice_with_hazard, resolve_optional_cs01_curves,
};
use finstack_quant_valuations_core::metrics::{
    MetricCalculator, MetricContext, MetricId, format_key_rate_label, sensitivity_central_diff,
};

/// Parallel direct hazard-rate intensity sensitivity (`hazard01::<curve>`).
#[derive(Default)]
pub struct DirectHazardParallelHazard01;

/// Bucketed direct hazard-rate intensity sensitivity (`bucketed_hazard01::<curve>`):
/// one bucket per exact positive hazard-curve knot.
#[derive(Default)]
pub struct DirectHazardBucketedHazard01;

/// Finite-difference sensitivity under direct `bump(±bp)`, selecting forward
/// difference upfront only when a downward bump would push an affected
/// intensity below zero (`min_affected_lambda < bump_bp * 1e-4`).
fn hazard_bump_sensitivity(
    context: &mut MetricContext,
    hazard: &Arc<HazardCurve>,
    bump_bp: f64,
    min_affected_lambda: f64,
    bump: impl Fn(&HazardCurve, f64) -> finstack_quant_core::Result<HazardCurve>,
) -> finstack_quant_core::Result<f64> {
    let step_decimal = bump_bp * ONE_BASIS_POINT;
    let mut reval = cs01_reval(context);
    context.with_market_scratch(|_, scratch| {
        let up = bump(hazard.as_ref(), bump_bp)?;
        let pv_up = reprice_with_hazard(scratch, up, hazard, &mut reval)?;
        if min_affected_lambda < step_decimal {
            let pv_base = reval(scratch)?;
            Ok((pv_up - pv_base) / bump_bp)
        } else {
            let down = bump(hazard.as_ref(), -bump_bp)?;
            let pv_down = reprice_with_hazard(scratch, down, hazard, &mut reval)?;
            sensitivity_central_diff(pv_up, pv_down, bump_bp)
        }
    })
}

impl MetricCalculator for DirectHazardParallelHazard01 {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let Some((hazard_id, _)) =
            resolve_optional_cs01_curves(context.instrument.as_ref(), false, "Hazard01")?
        else {
            return Ok(0.0);
        };
        let bump_bp = sens_config::resolve(context)?.credit_spread_bump_bp;
        let hazard = context.curves.get_hazard(hazard_id.as_str())?;
        let min_lambda = hazard
            .iter_knots()
            .map(|(_, lambda)| lambda)
            .fold(f64::INFINITY, f64::min);
        let hazard01 =
            hazard_bump_sensitivity(context, &hazard, bump_bp, min_lambda, |curve, bp| {
                curve.with_parallel_hazard_rate_bump_bp(bp)
            })?;
        context.computed.insert(
            MetricId::composite(&MetricId::Hazard01, &[hazard_id.as_str()]),
            hazard01,
        );
        Ok(hazard01)
    }
}

impl MetricCalculator for DirectHazardBucketedHazard01 {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let Some((hazard_id, _)) =
            resolve_optional_cs01_curves(context.instrument.as_ref(), false, "BucketedHazard01")?
        else {
            return Ok(0.0);
        };
        let bump_bp = sens_config::resolve(context)?.credit_spread_bump_bp;
        let hazard = context.curves.get_hazard(hazard_id.as_str())?;
        let knots: Vec<(f64, f64)> = hazard.iter_knots().collect();

        let mut series = Vec::with_capacity(knots.len());
        let mut total = NeumaierAccumulator::new();
        for (tenor, base_lambda) in knots {
            let hazard01 =
                hazard_bump_sensitivity(context, &hazard, bump_bp, base_lambda, |curve, bp| {
                    curve.with_tenor_hazard_rate_bumps_bp(&[(tenor, bp)])
                })?;
            series.push((
                std::borrow::Cow::Owned(format_key_rate_label(tenor)),
                hazard01,
            ));
            total.add(hazard01)?;
        }
        context.store_bucketed_series(
            MetricId::composite(&MetricId::BucketedHazard01, &[hazard_id.as_str()]),
            series,
        );
        total.get_total()
    }
}
