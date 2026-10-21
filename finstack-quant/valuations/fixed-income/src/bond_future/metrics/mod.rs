//! Bond-future pricing and interest-rate metrics.
//!
//! The registered metrics are:
//! - `FuturesPrice`: the lifecycle mark — `terms.quoted_price` when supplied,
//!   otherwise the carry-adjusted forward clean CTD price per 100 face divided
//!   by the resolved basket conversion factor (`BondFuture::fair_price`); the
//!   official `terms.settlement_price` after the last trading date.
//! - `ConversionFactor`: the resolved CTD's unitless factor from the
//!   deliverable basket.
//! - `Dv01`: NPV sensitivity to the configured parallel combined rate-curve
//!   bump.
//! - `BucketedDv01`: NPV sensitivity under configured triangular key-rate
//!   bumps.
//!
//! Conversion-factor scaling follows the valuation formulas:
//! `Model_Price = Forward_Clean_CTD_Percent / Conversion_Factor` and
//! `NPV = Position_Sign × Contracts × Multiplier × (Mark - Entry_Price)`, where
//! the mark is the model price while the contract is live.

use finstack_quant_valuations_core::metrics::MetricRegistry;
use finstack_quant_valuations_core::pricer::InstrumentType;

mod pricing;

/// Register `FuturesPrice`, `ConversionFactor`, `Dv01`, and `BucketedDv01` for
/// bond futures.
///
/// # Arguments
///
/// - `registry`: Metric registry to update for the `BondFuture` instrument type.
pub(crate) fn register_bond_future_metrics(
    registry: &mut MetricRegistry,
) -> std::result::Result<(), finstack_quant_valuations_core::metrics::MetricRegistryError> {
    finstack_quant_valuations_core::register_metrics! {
        registry: registry,
        instrument: InstrumentType::BondFuture,
        metrics: [
            (FuturesPrice, pricing::FuturesPriceCalculator),
            (ConversionFactor, pricing::ConversionFactorCalculator),
            (Dv01, finstack_quant_valuations_core::metrics::sensitivities::dv01::UnifiedDv01Calculator::new(finstack_quant_valuations_core::metrics::sensitivities::dv01::Dv01CalculatorConfig::parallel_combined())),
            (BucketedDv01, finstack_quant_valuations_core::metrics::sensitivities::dv01::UnifiedDv01Calculator::new(finstack_quant_valuations_core::metrics::sensitivities::dv01::Dv01CalculatorConfig::triangular_key_rate())),
        ]
    };
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_valuations_core::metrics::{MetricId, MetricRegistry};

    #[test]
    fn test_metrics_registration() {
        let mut registry = MetricRegistry::new();
        register_bond_future_metrics(&mut registry).expect("bond future metric registration");

        // Verify metrics are registered for BondFuture
        let metrics = registry.metrics_for_instrument(InstrumentType::BondFuture);

        // Verify DV01 is registered
        assert!(
            metrics.contains(&MetricId::Dv01),
            "DV01 metric should be registered for BondFuture"
        );

        // Verify Bucketed DV01 is registered
        assert!(
            metrics.contains(&MetricId::BucketedDv01),
            "BucketedDv01 metric should be registered for BondFuture"
        );
        assert!(
            metrics.contains(&MetricId::FuturesPrice),
            "FuturesPrice metric should be registered for BondFuture"
        );
        assert!(
            metrics.contains(&MetricId::ConversionFactor),
            "ConversionFactor metric should be registered for BondFuture"
        );
    }
}
