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
//! 3. **`spot_id`**: Only as a fallback, bump spot to propagate via cost-of-carry
//!
//! This ensures greeks are consistent with the Black-76 pricing model, regardless of
//! how the forward price is specified in the market data.

use crate::metrics::{MetricId, MetricRegistry};
use crate::pricer::InstrumentType;
use std::sync::Arc;

/// Register commodity option metrics with the registry.
pub(crate) fn register_commodity_option_metrics(registry: &mut MetricRegistry) {
    crate::register_metrics! {
        registry: registry,
        instrument: InstrumentType::CommodityOption,
        metrics: [
            // Forward-based greeks implemented via provider traits on CommodityOption.
            (Delta, crate::metrics::OptionGreekCalculator::<crate::instruments::CommodityOption>::delta()),
            (Gamma, crate::metrics::OptionGreekCalculator::<crate::instruments::CommodityOption>::gamma()),
            (Vega, crate::metrics::OptionGreekCalculator::<crate::instruments::CommodityOption>::vega()),
            (Vanna, crate::metrics::OptionGreekCalculator::<crate::instruments::CommodityOption>::vanna()),
            (Volga, crate::metrics::OptionGreekCalculator::<crate::instruments::CommodityOption>::volga()),
        ]
    }

    registry.register_metric(
        MetricId::Dv01,
        Arc::new(crate::metrics::UnifiedDv01Calculator::<
            crate::instruments::CommodityOption,
        >::new(
            crate::metrics::Dv01CalculatorConfig::parallel_combined()
        )),
        &[InstrumentType::CommodityOption],
    );
    registry.register_metric(
        MetricId::BucketedDv01,
        Arc::new(crate::metrics::UnifiedDv01Calculator::<
            crate::instruments::CommodityOption,
        >::new(
            crate::metrics::Dv01CalculatorConfig::triangular_key_rate(),
        )),
        &[InstrumentType::CommodityOption],
    );
}
