//! Pricer registrations for exotic instruments.
//!
//! Covers: Basket, AsianOption, BarrierOption, LookbackOption, QuantoOption,
//! Autocallable, CmsOption, CmsSwap, CliquetOption, RangeAccrual, BermudanSwaption.

use super::{register_generic, InstrumentType, ModelKey, PricerRegistry};

/// Register pricers for exotic instruments (barriers, lookbacks, Asians,
/// autocallables, quantos, cliquets, range accruals, Bermudan swaptions).
pub fn register_exotic_pricers(registry: &mut PricerRegistry) {
    // Basket
    register_generic!(
        registry,
        InstrumentType::Basket,
        crate::instruments::exotics::basket::Basket
    );

    // Asian Option

    registry.register(
        InstrumentType::AsianOption,
        ModelKey::MonteCarloGBM,
        crate::instruments::exotics::asian_option::pricer::AsianOptionMcPricer::default(),
    );
    registry.register(
        InstrumentType::AsianOption,
        ModelKey::AsianGeometricBS,
        crate::instruments::exotics::asian_option::pricer::AsianOptionAnalyticalGeometricPricer,
    );
    registry.register(
        InstrumentType::AsianOption,
        ModelKey::AsianTurnbullWakeman,
        crate::instruments::exotics::asian_option::pricer::AsianOptionSemiAnalyticalTwPricer,
    );

    // Barrier Option

    registry.register(
        InstrumentType::BarrierOption,
        ModelKey::MonteCarloGBM,
        crate::instruments::exotics::barrier_option::pricer::BarrierOptionMcPricer::default(),
    );
    registry.register(
        InstrumentType::BarrierOption,
        ModelKey::BarrierBSContinuous,
        crate::instruments::exotics::barrier_option::pricer::BarrierOptionAnalyticalPricer,
    );

    // Lookback Option

    registry.register(
        InstrumentType::LookbackOption,
        ModelKey::MonteCarloGBM,
        crate::instruments::exotics::lookback_option::pricer::LookbackOptionMcPricer::default(),
    );
    registry.register(
        InstrumentType::LookbackOption,
        ModelKey::LookbackBSContinuous,
        crate::instruments::exotics::lookback_option::pricer::LookbackOptionAnalyticalPricer,
    );

    // Quanto Option
    registry.register(
        InstrumentType::QuantoOption,
        ModelKey::QuantoBS,
        crate::instruments::fx::quanto_option::pricer::QuantoOptionAnalyticalPricer,
    );

    // Autocallable

    registry.register(
        InstrumentType::Autocallable,
        ModelKey::MonteCarloGBM,
        crate::instruments::equity::autocallable::pricer::AutocallableMcPricer::default(),
    );

    // CMS Option
    registry.register(
        InstrumentType::CmsOption,
        ModelKey::Black76,
        crate::instruments::rates::cms_option::pricer::CmsOptionPricer::new(),
    );
    registry.register(
        InstrumentType::CmsOption,
        ModelKey::Discounting,
        crate::instruments::rates::cms_option::pricer::CmsOptionPricer::with_model(
            ModelKey::Discounting,
        ),
    );

    // CMS Option - Static Replication (Andersen-Piterbarg)
    registry.register(
        InstrumentType::CmsOption,
        ModelKey::StaticReplication,
        crate::instruments::rates::cms_option::replication_pricer::CmsReplicationPricer::new(),
    );

    // CMS Swap
    registry.register(
        InstrumentType::CmsSwap,
        ModelKey::Black76,
        crate::instruments::rates::cms_swap::pricer::CmsSwapPricer::new(),
    );

    // CMS Spread Option - Gaussian copula with SABR marginals
    registry.register(
        InstrumentType::CmsSpreadOption,
        ModelKey::StaticReplication,
        crate::instruments::rates::cms_spread_option::CmsSpreadOptionPricer::new(),
    );

    // Cliquet Option

    registry.register(
        InstrumentType::CliquetOption,
        ModelKey::MonteCarloGBM,
        crate::instruments::equity::cliquet_option::pricer::CliquetOptionMcPricer::default(),
    );

    // Range Accrual

    registry.register(
        InstrumentType::RangeAccrual,
        ModelKey::StaticReplication,
        crate::instruments::rates::range_accrual::pricer::RangeAccrualStaticReplicationPricer,
    );
    registry.register(
        InstrumentType::RangeAccrual,
        ModelKey::MonteCarloGBM,
        crate::instruments::rates::range_accrual::pricer::RangeAccrualMcPricer::default(),
    );

    // TARN - Hull-White 1F Monte Carlo
    registry.register(
        InstrumentType::Tarn,
        ModelKey::MonteCarloHullWhite1F,
        crate::instruments::rates::tarn::TarnPricer::default(),
    );

    // Snowball / Inverse Floater
    registry.register(
        InstrumentType::Snowball,
        ModelKey::MonteCarloHullWhite1F,
        crate::instruments::rates::snowball::SnowballHw1fMcPricer::default(),
    );
    registry.register(
        InstrumentType::Snowball,
        ModelKey::Discounting,
        crate::instruments::rates::snowball::SnowballDiscountingPricer,
    );

    // Callable Range Accrual - Hull-White 1F LSMC
    registry.register(
        InstrumentType::CallableRangeAccrual,
        ModelKey::MonteCarloHullWhite1F,
        crate::instruments::rates::callable_range_accrual::CallableRangeAccrualPricer::default(),
    );

    // Bermudan Swaption LSMC (Hull-White 1F Monte Carlo).
    //
    // Registered with `enforce_calibration`: uncalibrated
    // `HullWhiteParams::default()` (κ=3%, σ=1%) produces 10–30% errors
    // on early-exercise premia. Callers must supply calibrated params
    // via `HullWhiteParams::new(κ, σ)` or a pre-calibrated tree on
    // `BermudanSwaptionPricerConfig`; otherwise pricing returns
    // `PricingError::ModelFailure`.

    registry.register(
        InstrumentType::BermudanSwaption,
        ModelKey::MonteCarloHullWhite1F,
        crate::instruments::rates::swaption::BermudanSwaptionPricer::lsmc_with_config(
            crate::instruments::rates::swaption::BermudanSwaptionPricerConfig {
                enforce_calibration: true,
                ..Default::default()
            },
        ),
    );

    // Bermudan Swaption - Hull-White 1F Tree. See note on the LSMC
    // registration above for the calibration-requirement rationale.
    registry.register(
        InstrumentType::BermudanSwaption,
        ModelKey::HullWhite1F,
        crate::instruments::rates::swaption::BermudanSwaptionPricer::tree_with_config(
            crate::instruments::rates::swaption::BermudanSwaptionPricerConfig {
                enforce_calibration: true,
                ..Default::default()
            },
        ),
    );

    // Barrier Option - PDE Crank-Nicolson 1D
    registry.register(
        InstrumentType::BarrierOption,
        ModelKey::PdeCrankNicolson1D,
        crate::instruments::exotics::barrier_option::pde_pricer::BarrierOptionPdePricer::default(),
    );

    // Barrier Option - Monte Carlo Heston

    registry.register(
        InstrumentType::BarrierOption,
        ModelKey::MonteCarloHeston,
        crate::instruments::exotics::barrier_option::heston_mc_pricer::BarrierOptionHestonMcPricer::default(),
    );

    // Asian Option - Monte Carlo Heston

    registry.register(
        InstrumentType::AsianOption,
        ModelKey::MonteCarloHeston,
        crate::instruments::exotics::asian_option::heston_mc_pricer::AsianOptionHestonMcPricer::default(),
    );

    // Bermudan Swaption - LMM Monte Carlo

    registry.register(
        InstrumentType::BermudanSwaption,
        ModelKey::LmmMonteCarlo,
        crate::instruments::rates::swaption::lmm_pricer::BermudanSwaptionLmmPricer::default(),
    );

    // Bermudan Swaption - Cheyette Rough Vol Monte Carlo

    registry.register(
        InstrumentType::BermudanSwaption,
        ModelKey::MonteCarloCheyetteRoughVol,
        crate::instruments::rates::swaption::cheyette_rough_pricer::BermudanSwaptionCheyetteRoughPricer::default(),
    );

    // Exotic rate products require explicit stochastic or replication models.
}
