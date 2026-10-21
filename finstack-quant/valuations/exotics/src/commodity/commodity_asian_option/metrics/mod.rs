//! Commodity Asian option metrics module.
//!
//! Provides risk sensitivities for commodity Asian options:
//! - **Delta**: Forward curve sensitivity (bump-and-reprice on PriceCurve)
//! - **Vega**: Volatility sensitivity (bump-and-reprice on vol surface)
//! - **DV01**: Interest rate sensitivity (discount curve bump)
//! - **BucketedDv01**: Key-rate DV01

mod greeks;

use finstack_quant_valuations_core::metrics::MetricRegistry;
use finstack_quant_valuations_core::pricer::InstrumentType;

/// Register commodity Asian option metrics with the registry.
pub(crate) fn register_commodity_asian_option_metrics(
    registry: &mut MetricRegistry,
) -> std::result::Result<(), finstack_quant_valuations_core::metrics::MetricRegistryError> {
    finstack_quant_valuations_core::register_metrics! {
        registry: registry,
        instrument: InstrumentType::CommodityAsianOption,
        metrics: [
            (Delta, greeks::AsianDeltaCalculator),
            (Vega, greeks::AsianVegaCalculator),
            (Dv01, finstack_quant_valuations_core::metrics::sensitivities::dv01::UnifiedDv01Calculator::new(finstack_quant_valuations_core::metrics::sensitivities::dv01::Dv01CalculatorConfig::parallel_combined())),
            (BucketedDv01, finstack_quant_valuations_core::metrics::sensitivities::dv01::UnifiedDv01Calculator::new(finstack_quant_valuations_core::metrics::sensitivities::dv01::Dv01CalculatorConfig::triangular_key_rate())),
        ]
    }
    Ok(())
}
