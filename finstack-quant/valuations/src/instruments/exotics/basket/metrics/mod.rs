//! Basket metrics module.
//!
//! Provides metric calculators specific to `Basket`, split into focused files.
//! The calculators compose with the shared metrics framework and are registered
//! via `register_basket_metrics`.
//!
//! Exposed metrics:
//! - Constituent count
//! - One-percent relative mark shock
//!
//! Note: Present value is handled by the instrument's built-in value() method.

mod basket_mark_shock;
mod weight_risk;

use crate::metrics::MetricRegistry;

use crate::instruments::exotics::basket::types::Basket;
use crate::metrics::MetricCalculator;
use crate::metrics::MetricContext;
use basket_mark_shock::BasketMarkShockCalculator;
use finstack_quant_core::Result;
pub(crate) use weight_risk::WeightRiskCalculator;

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
