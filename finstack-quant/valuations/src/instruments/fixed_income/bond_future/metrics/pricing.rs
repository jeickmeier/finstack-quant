//! Pricing diagnostics for bond futures.

use crate::instruments::fixed_income::bond_future::BondFuture;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_quant_core::Result;

/// Lifecycle-aware current or final futures price for bond futures.
pub(crate) struct FuturesPriceCalculator;

impl MetricCalculator for FuturesPriceCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let future: &BondFuture = context.instrument_as()?;
        future.mark_price(&context.curves, context.as_of)
    }
}

/// CTD conversion factor calculator for bond futures.
pub(crate) struct ConversionFactorCalculator;

impl MetricCalculator for ConversionFactorCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let future: &BondFuture = context.instrument_as()?;
        future.ctd_conversion_factor()
    }
}
