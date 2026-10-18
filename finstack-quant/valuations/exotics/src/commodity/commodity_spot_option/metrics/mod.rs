//! Commodity option metrics module.
//!
//! Registers option greeks and rate sensitivities for commodity options.
//!
//! # Forward-Based Greeks
//!
//! Commodity options are priced using Black-76, which is forward/futures-based.
//! The gamma and vanna calculators use **forward-based** sensitivities, bumping
//! the forward price driver in priority order:
//!
//! 1. **`quoted_forward`**: If the instrument has a quoted forward override, bump that
//! 2. **`PriceCurve`**: If a PriceCurve exists for `forward_curve_id`, bump it (parallel percent)
//! Observed spot is required separately for early exercise and simulation;
//! it does not substitute for the contractual delivery forward.
//!
//! This ensures greeks are consistent with the Black-76 pricing model, regardless of
//! how the forward price is specified in the market data.

pub(crate) mod selected_model;

use finstack_quant_valuations_core::metrics::{MetricId, MetricRegistry};
use finstack_quant_valuations_core::pricer::InstrumentType;
use std::sync::Arc;

/// Register commodity option metrics with the registry.
pub(crate) fn register_commodity_spot_option_metrics(
    registry: &mut MetricRegistry,
) -> std::result::Result<(), finstack_quant_valuations_core::metrics::MetricRegistryError> {
    finstack_quant_valuations_core::register_metrics! {
        registry: registry,
        instrument: InstrumentType::CommoditySpotOption,
        metrics: [
            // Forward-based greeks implemented via provider traits on CommoditySpotOption.
            (Delta, finstack_quant_valuations_core::metrics::OptionGreekCalculator::<crate::CommoditySpotOption>::delta()),
            (Gamma, finstack_quant_valuations_core::metrics::OptionGreekCalculator::<crate::CommoditySpotOption>::gamma()),
            (Vega, finstack_quant_valuations_core::metrics::OptionGreekCalculator::<crate::CommoditySpotOption>::vega()),
            (Vanna, finstack_quant_valuations_core::metrics::OptionGreekCalculator::<crate::CommoditySpotOption>::vanna()),
            (Volga, finstack_quant_valuations_core::metrics::OptionGreekCalculator::<crate::CommoditySpotOption>::volga()),
        ]
    }

    registry.register_metric(
        MetricId::Dv01,
        Arc::new(
            finstack_quant_valuations_core::metrics::UnifiedDv01Calculator::new(
                finstack_quant_valuations_core::metrics::Dv01CalculatorConfig::parallel_combined(),
            ),
        ),
        &[InstrumentType::CommoditySpotOption],
    )?;
    registry.register_metric(
        MetricId::BucketedDv01,
        Arc::new(
            finstack_quant_valuations_core::metrics::UnifiedDv01Calculator::new(
                finstack_quant_valuations_core::metrics::Dv01CalculatorConfig::triangular_key_rate(
                ),
            ),
        ),
        &[InstrumentType::CommoditySpotOption],
    )?;
    Ok(())
}
