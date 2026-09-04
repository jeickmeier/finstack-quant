//! Pricer registrations for rates instruments.
//!
//! Covers: Bond, IRS, FRA, BasisSwap, Deposit, InterestRateFuture, InterestRateFutureOption,
//! BondFuture, CapFloor, Swaption, Repo, DCF.

use super::{register_generic, InstrumentType, ModelKey, PricerRegistry};

/// Register the rates-instrument shard of the standard pricer registry.
pub(crate) fn register_rates_pricers(
    registry: &mut PricerRegistry,
) -> std::result::Result<(), crate::pricer::PricingError> {
    // Bond pricers
    for model in [
        ModelKey::Discounting,
        ModelKey::Tree,
        ModelKey::HazardRate,
        ModelKey::RatesCredit,
    ] {
        registry.register(
            crate::instruments::fixed_income::bond::pricing::engine::BondPricer::new(model),
        )?;
    }
    registry.register(
        crate::instruments::fixed_income::bond::pricing::engine::SimpleBondMertonMcPricer,
    )?;

    // Interest Rate Swaps
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

    // Basis Swap
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

    // Interest Rate Future
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

    // Bond Future
    registry.register(crate::instruments::fixed_income::bond_future::pricer::BondFuturePricer)?;

    // Cap/Floor
    registry.register(
        crate::instruments::rates::cap_floor::pricing::pricer::SimpleCapFloorBlackPricer::default(),
    )?;

    registry.register(crate::instruments::rates::swaption::pricer::SimpleSwaptionBlackPricer)?;
    registry.register(crate::instruments::rates::swaption::pricer::SimpleSwaptionNormalPricer)?;

    register_generic!(
        registry,
        InstrumentType::Repo,
        crate::instruments::rates::repo::Repo
    );

    // Swaption - Hull-White 1F Tree
    registry.register(
        crate::instruments::rates::swaption::hw_pricer::SwaptionHullWhitePricer::default(),
    )?;

    // Cap/Floor - Hull-White 1F
    registry.register(crate::instruments::rates::cap_floor::hw_pricer::CapFloorHullWhitePricer)?;
    Ok(())
}
