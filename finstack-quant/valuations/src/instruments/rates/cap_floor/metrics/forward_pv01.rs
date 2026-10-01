//! Forward curve PV01 for interest rate options (per 1bp parallel bump of forward curve).

use crate::instruments::rates::cap_floor::CapFloor;
use crate::metrics::sensitivities::config as sens_config;
use crate::metrics::sensitivities::cs01::sensitivity_central_diff;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_quant_core::market_data::bumps::{BumpSpec, MarketBump};
use finstack_quant_core::Result;

/// Forward PV01 calculator (per 1bp parallel forward curve bump).
///
/// The projection curve alone is shifted, keeping its interpolation,
/// extrapolation and calibration metadata, and the option is repriced with
/// the pricing model the metric request selected. Discounting is unchanged.
pub(crate) struct ForwardPv01Calculator;

impl MetricCalculator for ForwardPv01Calculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let option: &CapFloor = context.instrument_as()?;
        // A projection curve distinct from discounting must exist to be bumped.
        context.curves.get_forward(&option.forward_curve_id)?;

        let bump_bp = sens_config::from_context_or_default(
            context.get_config(),
            context.get_metric_pricing_overrides(),
        )?
        .rate_bump_bp;
        let reprice = |bp: f64| -> Result<f64> {
            let bumped = context.curves.bump([MarketBump::Curve {
                id: option.forward_curve_id.clone(),
                spec: BumpSpec::parallel_bp(bp),
            }])?;
            context.reprice_raw(&bumped, context.as_of)
        };
        Ok(sensitivity_central_diff(
            reprice(bump_bp)?,
            reprice(-bump_bp)?,
            bump_bp,
        ))
    }
}
