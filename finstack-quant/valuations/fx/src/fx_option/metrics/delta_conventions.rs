//! FX option delta convention metrics.

use crate::fx_option::FxOption;
use finstack_quant_core::Result;
use finstack_quant_valuations_core::metrics::{MetricCalculator, MetricContext};

/// Forward delta calculator for FX options.
pub(crate) struct DeltaForwardCalculator;

impl MetricCalculator for DeltaForwardCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let option: &FxOption = context.instrument_as()?;
        let greeks =
            crate::fx_option::pricer::compute_greeks(option, &context.curves, context.as_of)?;
        Ok(greeks.delta_forward)
    }
}

/// Premium-adjusted spot delta calculator for FX options.
pub(crate) struct DeltaPremiumAdjustedSpotCalculator;

impl MetricCalculator for DeltaPremiumAdjustedSpotCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let option: &FxOption = context.instrument_as()?;
        let greeks =
            crate::fx_option::pricer::compute_greeks(option, &context.curves, context.as_of)?;
        Ok(greeks.delta_premium_adjusted_spot)
    }
}

/// Premium-adjusted forward delta calculator for FX options.
pub(crate) struct DeltaPremiumAdjustedForwardCalculator;

impl MetricCalculator for DeltaPremiumAdjustedForwardCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let option: &FxOption = context.instrument_as()?;
        let greeks =
            crate::fx_option::pricer::compute_greeks(option, &context.curves, context.as_of)?;
        Ok(greeks.delta_premium_adjusted_forward)
    }
}
