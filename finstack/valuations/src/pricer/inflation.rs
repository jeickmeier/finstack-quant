//! Pricer registrations for inflation instruments.
//!
//! Covers: InflationSwap, YoYInflationSwap, InflationCapFloor.

use super::{register_generic, InstrumentType, ModelKey, PricerRegistry};

/// Register pricers for inflation instruments (swaps, caps/floors).
pub fn register_inflation_pricers(registry: &mut PricerRegistry) {
    // Inflation Swap
    register_generic!(
        registry,
        InstrumentType::InflationSwap,
        crate::instruments::InflationSwap
    );

    // YoY Inflation Swap
    register_generic!(
        registry,
        InstrumentType::YoYInflationSwap,
        crate::instruments::rates::inflation_swap::YoYInflationSwap
    );

    // Inflation Cap/Floor
    registry.register(
        InstrumentType::InflationCapFloor,
        ModelKey::Black76,
        crate::instruments::rates::inflation_cap_floor::pricer::SimpleInflationCapFloorPricer::default(),
    );
    registry.register(
        InstrumentType::InflationCapFloor,
        ModelKey::Normal,
        crate::instruments::rates::inflation_cap_floor::pricer::SimpleInflationCapFloorPricer::with_model(
            ModelKey::Normal,
        ),
    );
}
