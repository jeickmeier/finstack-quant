//! Pricer registrations for commodity instruments.
//!
//! Covers: CommodityForward, CommodityFuture, CommodityFutureOption,
//! CommoditySwap, CommodityOption, CommodityAsianOption, CommoditySwaption,
//! CommoditySpreadOption.

use super::{register_generic, InstrumentType, ModelKey, PricerRegistry};

/// Register pricers for commodity instruments.
pub(crate) fn register_commodity_pricers(
    registry: &mut PricerRegistry,
) -> std::result::Result<(), crate::pricer::PricingError> {
    register_generic!(
        registry,
        InstrumentType::CommodityFuture,
        crate::instruments::CommodityFuture
    );
    register_generic!(
        registry,
        InstrumentType::CommodityFutureOption,
        crate::instruments::CommodityFutureOption
    );

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

    // Commodity Asian Option
    registry.register(
        crate::instruments::commodity::commodity_asian_option::pricer::CommodityAsianOptionAnalyticalPricer,
    )?;

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
                num_paths: 100_000,
                num_steps: 252,
                seed: None,
            },
        ),
    )?;
    Ok(())
}
