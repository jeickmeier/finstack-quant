//! Commodity Asian option metrics module.
//!
//! Provides risk sensitivities for commodity Asian options:
//! - **Delta**: Forward curve sensitivity (bump-and-reprice on PriceCurve)
//! - **Vega**: Volatility sensitivity (bump-and-reprice on vol surface)
//! - **DV01**: Interest rate sensitivity (discount curve bump)
//! - **BucketedDv01**: Key-rate DV01

mod greeks;

use crate::metrics::{MetricId, MetricRegistry};
use crate::pricer::InstrumentType;
use std::sync::Arc;

/// Register commodity Asian option metrics with the registry.
pub(crate) fn register_commodity_asian_option_metrics(registry: &mut MetricRegistry) {
    registry.register_metric(
        MetricId::Delta,
        Arc::new(greeks::AsianDeltaCalculator),
        &[InstrumentType::CommodityAsianOption],
    );
    registry.register_metric(
        MetricId::Vega,
        Arc::new(greeks::AsianVegaCalculator),
        &[InstrumentType::CommodityAsianOption],
    );
    registry.register_metric(
        MetricId::Dv01,
        Arc::new(crate::metrics::UnifiedDv01Calculator::<
            crate::instruments::commodity::commodity_asian_option::CommodityAsianOption,
        >::new(
            crate::metrics::Dv01CalculatorConfig::parallel_combined()
        )),
        &[InstrumentType::CommodityAsianOption],
    );
    registry.register_metric(
        MetricId::BucketedDv01,
        Arc::new(crate::metrics::UnifiedDv01Calculator::<
            crate::instruments::commodity::commodity_asian_option::CommodityAsianOption,
        >::new(
            crate::metrics::Dv01CalculatorConfig::triangular_key_rate(),
        )),
        &[InstrumentType::CommodityAsianOption],
    );
}
