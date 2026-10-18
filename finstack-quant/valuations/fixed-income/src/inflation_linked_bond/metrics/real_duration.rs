//! ILB real duration metric calculator.

use crate::inflation_linked_bond::InflationLinkedBond;
use finstack_quant_valuations_core::metrics::sensitivities::config as sens_config;
use finstack_quant_valuations_core::metrics::{MetricCalculator, MetricContext};

/// Default real-yield shock for the central difference: 1bp.
const DEFAULT_YTM_BUMP_BP: f64 = 1.0;

/// Real duration calculator for ILB.
///
/// Shocks the real yield by `metric_pricing_overrides.bump_config.ytm_bump_bp`
/// when set, otherwise by 1bp.
pub(crate) struct RealDurationCalculator;

impl MetricCalculator for RealDurationCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let bump_bp = sens_config::resolve(context)?
            .ytm_bump_bp
            .unwrap_or(DEFAULT_YTM_BUMP_BP);
        let ilb: &InflationLinkedBond = context.instrument_as()?;
        ilb.real_duration(context.as_of, bump_bp)
    }
}
