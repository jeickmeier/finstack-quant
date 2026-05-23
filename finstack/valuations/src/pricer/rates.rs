//! Pricer registrations for rates instruments.
//!
//! Covers: Bond, IRS, FRA, BasisSwap, Deposit, InterestRateFuture, IrFutureOption,
//! BondFuture, CapFloor, Swaption, Repo, DCF.

use super::{register_generic, InstrumentType, ModelKey, PricerRegistry};

/// Register a minimal set of pricers for rates instruments.
///
/// Intended for environments (like WASM) where registering *all* pricers may be
/// too memory intensive.
pub(crate) fn register_rates_pricers(registry: &mut PricerRegistry) {
    // Bond pricers
    register_generic!(
        registry,
        InstrumentType::Bond,
        crate::instruments::fixed_income::bond::Bond
    );
    registry.register(
        InstrumentType::Bond,
        ModelKey::HazardRate,
        crate::instruments::fixed_income::bond::pricing::engine::SimpleBondHazardPricer,
    );
    registry.register(
        InstrumentType::Bond,
        ModelKey::Tree,
        crate::instruments::fixed_income::bond::pricing::engine::SimpleBondOasPricer,
    );
    registry.register(
        InstrumentType::Bond,
        ModelKey::MertonMc,
        crate::instruments::fixed_income::bond::pricing::engine::SimpleBondMertonMcPricer,
    );

    // Interest Rate Swaps
    register_generic!(
        registry,
        InstrumentType::IRS,
        crate::instruments::InterestRateSwap
    );

    // FRA
    register_generic!(
        registry,
        InstrumentType::FRA,
        crate::instruments::ForwardRateAgreement
    );

    // Basis Swap
    register_generic!(
        registry,
        InstrumentType::BasisSwap,
        crate::instruments::rates::basis_swap::BasisSwap
    );

    // Deposit
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

    // IR Future Option
    registry.register(
        InstrumentType::IrFutureOption,
        ModelKey::Discounting,
        crate::instruments::rates::ir_future_option::pricer::IrFutureOptionPricer::default(),
    );

    // Bond Future
    registry.register(
        InstrumentType::BondFuture,
        ModelKey::BondFutureCleanPriceProxy,
        crate::instruments::fixed_income::bond_future::pricer::BondFuturePricer,
    );

    // Cap/Floor
    registry.register(
        InstrumentType::CapFloor,
        ModelKey::Black76,
        crate::instruments::rates::cap_floor::pricing::pricer::SimpleCapFloorBlackPricer::default(),
    );
    registry.register(
        InstrumentType::CapFloor,
        ModelKey::Discounting,
        crate::instruments::rates::cap_floor::pricing::pricer::SimpleCapFloorBlackPricer::with_model(
            ModelKey::Discounting,
        ),
    );

    // Swaption
    registry.register(
        InstrumentType::Swaption,
        ModelKey::Black76,
        crate::instruments::rates::swaption::pricer::SimpleSwaptionBlackPricer::default(),
    );
    registry.register(
        InstrumentType::Swaption,
        ModelKey::Discounting,
        crate::instruments::rates::swaption::pricer::SimpleSwaptionBlackPricer::with_model(
            ModelKey::Discounting,
        ),
    );

    // Repo
    register_generic!(
        registry,
        InstrumentType::Repo,
        crate::instruments::rates::repo::Repo
    );

    // DCF (Discounted Cash Flow)
    registry.register(
        InstrumentType::DCF,
        ModelKey::Discounting,
        crate::instruments::equity::dcf_equity::pricer::DcfPricer,
    );

    // Swaption - Hull-White 1F Tree
    registry.register(
        InstrumentType::Swaption,
        ModelKey::HullWhite1F,
        crate::instruments::rates::swaption::hw_pricer::SwaptionHullWhitePricer::default(),
    );

    // Cap/Floor - Hull-White 1F
    registry.register(
        InstrumentType::CapFloor,
        ModelKey::HullWhite1F,
        crate::instruments::rates::cap_floor::hw_pricer::CapFloorHullWhitePricer,
    );
}
