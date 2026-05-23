//! Pricer registrations for commodity instruments.
//!
//! Covers: CommodityForward, CommoditySwap, CommodityOption,
//! CommodityAsianOption, CommoditySwaption, CommoditySpreadOption.

use super::{register_generic, InstrumentType, ModelKey, PricerRegistry};

/// Register pricers for commodity instruments.
pub(crate) fn register_commodity_pricers(registry: &mut PricerRegistry) {
    // Commodity Forward
    register_generic!(
        registry,
        InstrumentType::CommodityForward,
        crate::instruments::CommodityForward
    );

    // Commodity Swap
    register_generic!(
        registry,
        InstrumentType::CommoditySwap,
        crate::instruments::CommoditySwap
    );

    // Commodity Option
    register_generic!(
        registry,
        InstrumentType::CommodityOption,
        crate::instruments::CommodityOption,
        ModelKey::Black76
    );
    register_generic!(
        registry,
        InstrumentType::CommodityOption,
        crate::instruments::CommodityOption,
        ModelKey::Discounting
    );

    // Commodity Asian Option
    registry.register(
        InstrumentType::CommodityAsianOption,
        ModelKey::AsianTurnbullWakeman,
        crate::instruments::commodity::commodity_asian_option::pricer::CommodityAsianOptionAnalyticalPricer,
    );

    // Commodity Swaption
    register_generic!(
        registry,
        InstrumentType::CommoditySwaption,
        crate::instruments::CommoditySwaption,
        ModelKey::Black76
    );

    // Commodity Spread Option (Kirk's approximation)
    register_generic!(
        registry,
        InstrumentType::CommoditySpreadOption,
        crate::instruments::CommoditySpreadOption,
        ModelKey::Black76
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
