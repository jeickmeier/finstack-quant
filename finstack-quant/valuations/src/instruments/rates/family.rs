//! The rates family: linear rates, inflation, caps/floors, swaptions and CMS.

use crate::metrics::{MetricRegistry, MetricRegistryError};
use crate::pricer::{
    register_generic, Family, InstrumentType, ModelKey, PricerRegistry, PricingError,
};

/// Registration unit for the rates instruments.
pub static RATES: Family = Family::new("rates", register_pricers, register_metrics);

fn register_pricers(registry: &mut PricerRegistry) -> Result<(), PricingError> {
    register_generic!(
        registry,
        InstrumentType::Irs,
        crate::instruments::InterestRateSwap
    );
    register_generic!(
        registry,
        InstrumentType::Fra,
        crate::instruments::ForwardRateAgreement
    );
    register_generic!(
        registry,
        InstrumentType::BasisSwap,
        crate::instruments::rates::basis_swap::BasisSwap
    );
    register_generic!(
        registry,
        InstrumentType::Deposit,
        crate::instruments::Deposit
    );
    register_generic!(
        registry,
        InstrumentType::XccySwap,
        crate::instruments::XccySwap
    );
    register_generic!(
        registry,
        InstrumentType::Repo,
        crate::instruments::rates::repo::Repo
    );
    register_generic!(
        registry,
        InstrumentType::InterestRateFuture,
        crate::instruments::rates::ir_future::InterestRateFuture
    );
    register_generic!(
        registry,
        InstrumentType::InterestRateFutureOption,
        crate::instruments::InterestRateFutureOption
    );

    // Caps/floors and European swaptions.
    registry.register(
        crate::instruments::rates::cap_floor::pricing::pricer::SimpleCapFloorBlackPricer,
    )?;
    registry.register(crate::instruments::rates::cap_floor::hw_pricer::CapFloorHullWhitePricer)?;
    registry.register(crate::instruments::rates::swaption::pricer::SimpleSwaptionBlackPricer)?;
    registry.register(crate::instruments::rates::swaption::pricer::SimpleSwaptionNormalPricer)?;
    registry.register(
        crate::instruments::rates::swaption::hw_pricer::SwaptionHullWhitePricer::default(),
    )?;

    // Bermudan swaptions.
    registry.register(crate::instruments::rates::swaption::BermudanSwaptionPricer::lsmc())?;
    registry.register(crate::instruments::rates::swaption::BermudanSwaptionPricer::tree())?;
    registry.register(
        crate::instruments::rates::swaption::lmm_pricer::BermudanSwaptionLmmPricer::default(),
    )?;

    // CMS products.
    registry.register(crate::instruments::rates::cms_option::pricer::CmsOptionPricer::new())?;
    registry.register(
        crate::instruments::rates::cms_option::replication_pricer::CmsReplicationPricer::new(),
    )?;
    registry.register(crate::instruments::rates::cms_swap::pricer::CmsSwapPricer::new())?;
    registry
        .register(crate::instruments::rates::cms_swap::pricer::CmsSwapReplicationPricer::new())?;
    registry
        .register(crate::instruments::rates::cms_spread_option::CmsSpreadOptionPricer::new())?;

    // Inflation.
    register_generic!(
        registry,
        InstrumentType::InflationSwap,
        crate::instruments::InflationSwap
    );
    register_generic!(
        registry,
        InstrumentType::YoYInflationSwap,
        crate::instruments::rates::inflation_swap::YoYInflationSwap
    );
    registry.register(
        crate::instruments::rates::inflation_cap_floor::pricer::SimpleInflationCapFloorPricer::default(),
    )?;
    registry.register(
        crate::instruments::rates::inflation_cap_floor::pricer::SimpleInflationCapFloorPricer::with_model(
            ModelKey::Normal,
        ),
    )?;
    Ok(())
}

fn register_metrics(registry: &mut MetricRegistry) -> Result<(), MetricRegistryError> {
    crate::instruments::rates::irs::metrics::register_irs_metrics(registry)?;
    crate::instruments::rates::deposit::metrics::register_deposit_metrics(registry)?;
    crate::instruments::rates::fra::metrics::register_fra_metrics(registry)?;
    crate::instruments::rates::ir_future::metrics::register_ir_future_metrics(registry)?;
    crate::instruments::rates::ir_future_option::metrics::register_interest_rate_future_option_metrics(registry)?;
    crate::instruments::rates::inflation_swap::metrics::register_inflation_swap_metrics(registry)?;
    crate::instruments::rates::inflation_cap_floor::metrics::register_inflation_cap_floor_metrics(
        registry,
    )?;
    crate::instruments::rates::cap_floor::metrics::register_cap_floor_metrics(registry)?;
    crate::instruments::rates::swaption::metrics::register_swaption_metrics(registry)?;
    crate::instruments::rates::swaption::metrics::register_bermudan_swaption_metrics(registry)?;
    crate::instruments::rates::xccy_swap::metrics::register_xccy_swap_metrics(registry)?;
    crate::instruments::rates::repo::metrics::register_repo_metrics(registry)?;
    crate::instruments::rates::basis_swap::metrics::register_basis_swap_metrics(registry)?;
    crate::instruments::rates::cms_option::metrics::register_cms_option_metrics(registry)?;
    crate::instruments::rates::cms_swap::metrics::register_cms_swap_metrics(registry)?;
    crate::instruments::rates::cms_spread_option::metrics::register_cms_spread_option_metrics(
        registry,
    )?;
    Ok(())
}
