//! FX option delta convention metrics.

use crate::instruments::fx::fx_option::FxOption;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_quant_core::Result;

/// Forward delta calculator for FX options.
pub(crate) struct DeltaForwardCalculator;

impl MetricCalculator for DeltaForwardCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let option: &FxOption = context.instrument_as()?;
        let greeks = crate::instruments::fx::fx_option::pricer::compute_greeks(
            option,
            &context.curves,
            context.as_of,
        )?;
        Ok(greeks.delta_forward)
    }
}

/// Premium-adjusted delta calculator for FX options.
pub(crate) struct DeltaPremiumAdjustedCalculator;

impl MetricCalculator for DeltaPremiumAdjustedCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let option: &FxOption = context.instrument_as()?;
        let greeks = crate::instruments::fx::fx_option::pricer::compute_greeks(
            option,
            &context.curves,
            context.as_of,
        )?;
        Ok(greeks.delta_premium_adjusted)
    }
}
