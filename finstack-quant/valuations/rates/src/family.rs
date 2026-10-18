//! The rates family: linear rates, inflation, caps/floors, swaptions and CMS.

use finstack_quant_valuations_core::metrics::{MetricRegistry, MetricRegistryError};
use finstack_quant_valuations_core::pricer::{
    register_generic, Family, InstrumentType, ModelKey, PricerRegistry, PricingError,
};

/// Registration unit for the rates instruments.
pub static RATES: Family = Family::new("rates", register_pricers, register_metrics);

fn register_pricers(registry: &mut PricerRegistry) -> Result<(), PricingError> {
    register_generic!(registry, InstrumentType::Irs, crate::InterestRateSwap);
    register_generic!(registry, InstrumentType::Fra, crate::ForwardRateAgreement);
    register_generic!(
        registry,
        InstrumentType::BasisSwap,
        crate::basis_swap::BasisSwap
    );
    register_generic!(registry, InstrumentType::Deposit, crate::Deposit);
    register_generic!(registry, InstrumentType::XccySwap, crate::XccySwap);
    register_generic!(registry, InstrumentType::Repo, crate::repo::Repo);
    register_generic!(
        registry,
        InstrumentType::InterestRateFuture,
        crate::ir_future::InterestRateFuture
    );
    register_generic!(
        registry,
        InstrumentType::InterestRateFutureOption,
        crate::InterestRateFutureOption
    );

    // Caps/floors and European swaptions.
    registry.register(crate::cap_floor::pricing::pricer::SimpleCapFloorBlackPricer)?;
    registry.register(crate::cap_floor::hw_pricer::CapFloorHullWhitePricer)?;
    registry.register(crate::swaption::pricer::SimpleSwaptionBlackPricer)?;
    registry.register(crate::swaption::pricer::SimpleSwaptionNormalPricer)?;
    registry.register(crate::swaption::hw_pricer::SwaptionHullWhitePricer::default())?;

    // Bermudan swaptions.
    registry.register(crate::swaption::BermudanSwaptionPricer::lsmc())?;
    registry.register(crate::swaption::BermudanSwaptionPricer::tree())?;
    registry.register(crate::swaption::lmm_pricer::BermudanSwaptionLmmPricer::default())?;

    // CMS products.
    registry.register(crate::cms_option::pricer::CmsOptionPricer::new())?;
    registry.register(crate::cms_option::replication_pricer::CmsReplicationPricer::new())?;
    registry.register(crate::cms_swap::pricer::CmsSwapPricer::new())?;
    registry.register(crate::cms_swap::pricer::CmsSwapReplicationPricer::new())?;
    registry.register(crate::cms_spread_option::CmsSpreadOptionPricer::new())?;

    // Inflation.
    register_generic!(
        registry,
        InstrumentType::InflationSwap,
        crate::InflationSwap
    );
    register_generic!(
        registry,
        InstrumentType::YoYInflationSwap,
        crate::inflation_swap::YoYInflationSwap
    );
    registry
        .register(crate::inflation_cap_floor::pricer::SimpleInflationCapFloorPricer::default())?;
    registry.register(
        crate::inflation_cap_floor::pricer::SimpleInflationCapFloorPricer::with_model(
            ModelKey::Normal,
        ),
    )?;
    Ok(())
}

fn register_metrics(registry: &mut MetricRegistry) -> Result<(), MetricRegistryError> {
    crate::irs::metrics::register_irs_metrics(registry)?;
    crate::deposit::metrics::register_deposit_metrics(registry)?;
    crate::fra::metrics::register_fra_metrics(registry)?;
    crate::ir_future::metrics::register_ir_future_metrics(registry)?;
    crate::ir_future_option::metrics::register_interest_rate_future_option_metrics(registry)?;
    crate::inflation_swap::metrics::register_inflation_swap_metrics(registry)?;
    crate::inflation_cap_floor::metrics::register_inflation_cap_floor_metrics(registry)?;
    crate::cap_floor::metrics::register_cap_floor_metrics(registry)?;
    crate::swaption::metrics::register_swaption_metrics(registry)?;
    crate::swaption::metrics::register_bermudan_swaption_metrics(registry)?;
    crate::xccy_swap::metrics::register_xccy_swap_metrics(registry)?;
    crate::repo::metrics::register_repo_metrics(registry)?;
    crate::basis_swap::metrics::register_basis_swap_metrics(registry)?;
    crate::cms_option::metrics::register_cms_option_metrics(registry)?;
    crate::cms_swap::metrics::register_cms_swap_metrics(registry)?;
    crate::cms_spread_option::metrics::register_cms_spread_option_metrics(registry)?;
    Ok(())
}
