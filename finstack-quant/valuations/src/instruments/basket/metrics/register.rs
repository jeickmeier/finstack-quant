//! Metric registration and calculators.

use super::WeightRiskCalculator;
use crate::metrics::MetricRegistry;

use super::basket_mark_shock::BasketMarkShockCalculator;
use crate::instruments::basket::types::Basket;
use crate::metrics::MetricCalculator;
use crate::metrics::MetricContext;
use finstack_quant_core::Result;

/// Register all Basket metrics with the registry
pub(crate) fn register_basket_metrics(
    registry: &mut MetricRegistry,
) -> std::result::Result<(), crate::metrics::MetricRegistryError> {
    use crate::pricer::InstrumentType;

    crate::register_metrics! {
        registry: registry,
        instrument: InstrumentType::Basket,
        metrics: [
            (ConstituentCount, ConstituentCountCalculator),
            // Custom metrics for basket-specific risks
            (BasketMarkShock, BasketMarkShockCalculator),
            (WeightRisk, WeightRiskCalculator),
        ]
    };
    Ok(())
}

/// Calculate number of constituents in the basket
///
/// Returns the number of constituents in the basket.
pub(super) struct ConstituentCountCalculator;

impl MetricCalculator for ConstituentCountCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let basket = context.instrument_as::<Basket>()?;
        Ok(basket.constituent_count() as f64)
    }
}
