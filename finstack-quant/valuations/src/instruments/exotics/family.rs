//! The exotics family: path-dependent and structured options, plus the
//! commodity instruments.

use crate::metrics::{MetricRegistry, MetricRegistryError};
use crate::pricer::{
    register_generic, Family, InstrumentType, ModelKey, PricerRegistry, PricingError,
};

/// Registration unit for the exotic and commodity instruments.
pub static EXOTICS: Family = Family::new("exotics", register_pricers, register_metrics);

fn register_pricers(registry: &mut PricerRegistry) -> Result<(), PricingError> {
    // Asian options.
    registry.register(
        crate::instruments::exotics::asian_option::pricer::AsianOptionMcPricer::default(),
    )?;
    registry.register(
        crate::instruments::exotics::asian_option::pricer::AsianOptionAnalyticalGeometricPricer,
    )?;
    registry.register(
        crate::instruments::exotics::asian_option::pricer::AsianOptionSemiAnalyticalTwPricer,
    )?;
    registry.register(
        crate::instruments::exotics::asian_option::heston_mc_pricer::AsianOptionHestonMcPricer::default(),
    )?;

    // Barrier options.
    registry.register(
        crate::instruments::exotics::barrier_option::pricer::BarrierOptionMcPricer::default(),
    )?;
    registry.register(
        crate::instruments::exotics::barrier_option::pricer::BarrierOptionAnalyticalPricer,
    )?;
    registry.register(
        crate::instruments::exotics::barrier_option::pde_pricer::BarrierOptionPdePricer::default(),
    )?;
    registry.register(
        crate::instruments::exotics::barrier_option::heston_mc_pricer::BarrierOptionHestonMcPricer::default(),
    )?;

    // Lookback options.
    registry.register(
        crate::instruments::exotics::lookback_option::pricer::LookbackOptionMcPricer::default(),
    )?;
    registry.register(
        crate::instruments::exotics::lookback_option::pricer::LookbackOptionAnalyticalPricer,
    )?;

    // Rate-linked structures.
    registry.register(
        crate::instruments::exotics::range_accrual::pricer::RangeAccrualStaticReplicationPricer,
    )?;
    registry.register(
        crate::instruments::exotics::range_accrual::pricer::RangeAccrualMcPricer::default(),
    )?;
    registry.register(crate::instruments::exotics::tarn::TarnPricer::default())?;
    registry.register(crate::instruments::exotics::snowball::SnowballHw1fMcPricer::default())?;
    registry.register(crate::instruments::exotics::snowball::SnowballDiscountingPricer)?;
    registry.register(
        crate::instruments::exotics::callable_range_accrual::CallableRangeAccrualPricer::default(),
    )?;

    // Commodities.
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
    register_generic!(
        registry,
        InstrumentType::CommodityForward,
        crate::instruments::CommodityForward
    );
    register_generic!(
        registry,
        InstrumentType::CommoditySwap,
        crate::instruments::CommoditySwap
    );
    register_generic!(
        registry,
        InstrumentType::CommoditySpotOption,
        crate::instruments::CommoditySpotOption,
        ModelKey::Black76
    );
    registry.register(
        crate::instruments::commodity::commodity_asian_option::pricer::CommodityAsianOptionAnalyticalPricer,
    )?;
    register_generic!(
        registry,
        InstrumentType::CommoditySwaption,
        crate::instruments::CommoditySwaption,
        ModelKey::Black76
    );
    // Kirk's approximation.
    register_generic!(
        registry,
        InstrumentType::CommoditySpreadOption,
        crate::instruments::CommoditySpreadOption,
        ModelKey::Black76
    );
    // Monte Carlo Schwartz-Smith spot option.
    registry.register(
        crate::instruments::commodity::commodity_spot_option::pricer::CommoditySpotOptionMcPricer::new(
            crate::instruments::commodity::commodity_spot_option::CommodityMcParams {
                params: finstack_quant_models::monte_carlo::process::schwartz_smith::SchwartzSmithParams {
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

fn register_metrics(registry: &mut MetricRegistry) -> Result<(), MetricRegistryError> {
    crate::instruments::exotics::lookback_option::metrics::register_lookback_option_metrics(
        registry,
    )?;
    crate::instruments::exotics::asian_option::metrics::register_asian_option_metrics(registry)?;
    crate::instruments::exotics::barrier_option::metrics::register_barrier_option_metrics(
        registry,
    )?;
    crate::instruments::exotics::range_accrual::metrics::register_range_accrual_metrics(registry)?;
    crate::instruments::exotics::tarn::metrics::register_tarn_metrics(registry)?;
    crate::instruments::exotics::snowball::metrics::register_snowball_metrics(registry)?;
    crate::instruments::exotics::callable_range_accrual::metrics::register_callable_range_accrual_metrics(registry)?;
    crate::instruments::commodity::commodity_forward::metrics::register_commodity_forward_metrics(
        registry,
    )?;
    crate::instruments::commodity::commodity_future::metrics::register_commodity_future_metrics(
        registry,
    )?;
    crate::instruments::commodity::commodity_future_option::metrics::register_commodity_future_option_metrics(registry)?;
    crate::instruments::commodity::commodity_swap::metrics::register_commodity_swap_metrics(
        registry,
    )?;
    crate::instruments::commodity::commodity_spot_option::metrics::register_commodity_spot_option_metrics(registry)?;
    crate::instruments::commodity::commodity_asian_option::metrics::register_commodity_asian_option_metrics(registry)?;
    crate::instruments::commodity::commodity_swaption::metrics::register_commodity_swaption_metrics(registry)?;
    crate::instruments::commodity::commodity_spread_option::metrics::register_commodity_spread_option_metrics(registry)?;
    Ok(())
}
