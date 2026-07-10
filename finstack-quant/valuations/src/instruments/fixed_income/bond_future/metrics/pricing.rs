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
        let conversion_factor = ctd_conversion_factor(future)?;
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
        ctd_conversion_factor(future)
    }
}

fn ctd_conversion_factor(future: &BondFuture) -> Result<f64> {
    let ctd_id = if let Some(ctd_id) = &future.ctd_bond_id {
        ctd_id
    } else if let Some(ctd_bond) = &future.ctd_bond {
        &ctd_bond.id
    } else if future.deliverable_basket.len() == 1 {
        &future.deliverable_basket[0].bond_id
    } else {
        return Err(finstack_quant_core::Error::Validation(format!(
            "BondFuture '{}' requires ctd_bond_id for conversion_factor metric",
            future.id.as_str()
        )));
    };

    future
        .deliverable_basket
        .iter()
        .find(|deliverable| deliverable.bond_id == *ctd_id)
        .map(|deliverable| deliverable.conversion_factor)
        .ok_or_else(|| {
            finstack_quant_core::Error::Validation(format!(
                "BondFuture '{}' CTD '{}' is not in deliverable_basket",
                future.id.as_str(),
                ctd_id.as_str()
            ))
        })
}
