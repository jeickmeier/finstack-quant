//! ILB real duration metric calculator.

use crate::instruments::fixed_income::inflation_linked_bond::InflationLinkedBond;
use crate::metrics::sensitivities::config as sens_config;
use crate::metrics::{MetricCalculator, MetricContext};

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
