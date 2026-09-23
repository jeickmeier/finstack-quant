//! Pricing diagnostics for bond futures.

use crate::instruments::fixed_income::bond_future::pricer::BondFuturePricer;
use crate::instruments::fixed_income::bond_future::BondFuture;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_quant_core::Result;

/// Quoted/model futures price calculator for bond futures.
pub(crate) struct FuturesPriceCalculator;

impl MetricCalculator for FuturesPriceCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let future: &BondFuture = context.instrument_as()?;
        let ctd = future.ctd_bond.as_ref().ok_or_else(|| {
            finstack_quant_core::Error::Validation(format!(
                "BondFuture '{}' requires embedded ctd_bond for futures_price metric",
                future.id.as_str()
            ))
        })?;
        let conversion_factor = future.ctd_conversion_factor()?;
        BondFuturePricer::calculate_model_price_for_future(
            future,
            ctd,
            conversion_factor,
            &context.curves,
            context.as_of,
        )
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
