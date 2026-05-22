//! Pricer registrations for commodity instruments.
//!
//! Covers: CommodityForward, CommoditySwap, CommodityOption,
//! CommodityAsianOption, CommoditySwaption, CommoditySpreadOption.

use super::{InstrumentType, ModelKey, PricerRegistry};

/// Register pricers for commodity instruments.
pub fn register_commodity_pricers(registry: &mut PricerRegistry) {
    // Commodity Forward
    registry.register(
        InstrumentType::CommodityForward,
        ModelKey::Discounting,
        crate::instruments::common_impl::GenericInstrumentPricer::<
            crate::instruments::CommodityForward,
        >::discounting(InstrumentType::CommodityForward),
    );

    // Commodity Swap
    registry.register(
        InstrumentType::CommoditySwap,
        ModelKey::Discounting,
        crate::instruments::common_impl::GenericInstrumentPricer::<
            crate::instruments::CommoditySwap,
        >::discounting(InstrumentType::CommoditySwap),
    );

    // Commodity Option
    registry.register(
        InstrumentType::CommodityOption,
        ModelKey::Black76,
        crate::instruments::common_impl::GenericInstrumentPricer::<
            crate::instruments::CommodityOption,
        >::new(InstrumentType::CommodityOption, ModelKey::Black76),
    );
    registry.register(
        InstrumentType::CommodityOption,
        ModelKey::Discounting,
        crate::instruments::common_impl::GenericInstrumentPricer::<
            crate::instruments::CommodityOption,
        >::new(InstrumentType::CommodityOption, ModelKey::Discounting),
    );

    // Commodity Asian Option
    registry.register(
        InstrumentType::CommodityAsianOption,
        ModelKey::AsianTurnbullWakeman,
        crate::instruments::commodity::commodity_asian_option::pricer::CommodityAsianOptionAnalyticalPricer,
    );

    // Commodity Swaption
    registry.register(
        InstrumentType::CommoditySwaption,
        ModelKey::Black76,
        crate::instruments::common_impl::GenericInstrumentPricer::<
            crate::instruments::CommoditySwaption,
        >::new(InstrumentType::CommoditySwaption, ModelKey::Black76),
    );

    // Commodity Spread Option (Kirk's approximation)
    registry.register(
        InstrumentType::CommoditySpreadOption,
        ModelKey::Black76,
        crate::instruments::common_impl::GenericInstrumentPricer::<
            crate::instruments::CommoditySpreadOption,
        >::new(InstrumentType::CommoditySpreadOption, ModelKey::Black76),
    );

    // Commodity Option - Monte Carlo Schwartz-Smith

    registry.register(
        InstrumentType::CommodityOption,
        ModelKey::MonteCarloSchwartzSmith,
        crate::instruments::commodity::commodity_option::pricer::CommodityOptionMcPricer::new(
            crate::instruments::commodity::commodity_option::CommodityMcParams {
                model: crate::instruments::commodity::commodity_option::CommodityPricingModel::SchwartzSmith {
                    kappa: 1.0,
                    sigma_x: 0.3,
                    sigma_y: 0.15,
                    rho_xy: 0.3,
                    mu_y: 0.0,
                    lambda_x: 0.0,
                },
                n_paths: 100_000,
                n_steps: 252,
                seed: None,
            },
        ),
    );
}
