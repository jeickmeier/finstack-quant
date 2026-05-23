//! Pricer infrastructure: type-safe pricing dispatch via registry pattern.
//!
//! This module provides a registry-based pricing system that maps
//! (instrument type, model) pairs to specific pricer implementations.
//! The system uses enum-based dispatch for type safety rather than string
//! comparisons.
//!
//! # Module structure
//!
//! Core types are split into focused submodules:
//! - `keys`: [`crate::pricer::InstrumentType`], [`crate::pricer::ModelKey`],
//!   [`crate::pricer::PricerKey`]
//! - `errors`: [`crate::pricer::PricingError`],
//!   [`crate::pricer::PricingErrorContext`], [`crate::pricer::PricingResult`]
//! - `registry`: [`crate::pricer::Pricer`], [`crate::pricer::PricerRegistry`],
//!   `expect_inst`
//!
//! The registration logic is split into asset-class submodules:
//! - `rates`: Bond, IRS, FRA, BasisSwap, Deposit, CapFloor, Swaption, Repo, DCF, IR futures
//! - `credit`: CDS, CDSIndex, CDSTranche, CDSOption, StructuredCredit
//! - `equity`: Equity, EquityOption, EquityTRS, VarianceSwap, EquityIndexFuture, RealEstate, PE fund
//! - `fx`: FxSpot, FxSwap, XccySwap, FxOption, FxVarianceSwap, FxForward, NDF, FX barrier/digital/touch
//! - `fixed_income`: FIIndexTRS, Convertible, InflationLinkedBond, RevolvingCredit, TermLoan, MBS, TBA, CMO
//! - `inflation`: InflationSwap, YoYInflationSwap, InflationCapFloor
//! - `exotics`: Basket, Asian, Barrier, Lookback, Quanto, Autocallable, CMS, Cliquet, RangeAccrual, BermudanSwaption
//! - `commodity`: CommodityForward, CommoditySwap, CommodityOption, CommoditySwaption, CommoditySpreadOption

// Core submodules
mod errors;
pub mod json;
mod keys;
mod registry;

pub use crate::instruments::cashflow_export::instrument_cashflows_json;
pub(crate) use errors::actionable_unknown_pricer_message;
pub use errors::{PricingError, PricingErrorContext, PricingResult};
pub use json::{
    canonical_instrument_json, canonical_instrument_json_from_str, list_standard_metrics,
    list_standard_metrics_grouped, metric_value_from_instrument_json, parse_as_of_date,
    parse_boxed_instrument_json, parse_instrument_json, parse_model_key,
    present_metric_values_from_instrument_json,
    present_standard_option_greeks_from_instrument_json, pretty_instrument_json,
    price_instrument_json, price_instrument_json_with_metrics,
    price_instrument_json_with_metrics_and_history, validate_instrument_json,
    STANDARD_OPTION_GREEKS,
};
pub use keys::{InstrumentType, ModelKey, PricerKey};
pub use registry::{expect_inst, Pricer, PricerRegistry};

// Fourier pricing via the Fang-Oosterlee (2008) COS method.
//
// A Lewis (2001) pricer was previously exposed alongside COS but was known-
// divergent off-ATM and silently dropped non-finite integrand panels behind a
// `max(0.0)` clamp. It was removed; COS is the only Fourier method finstack
// exposes, so the implementation lives directly in `pricer::cos` rather than
// inside a single-member `pricer::fourier` namespace.
pub mod cos;

// Asset-class registration submodules
mod commodity;
mod credit;
mod equity;
mod exotics;
mod fixed_income;
mod fx;
mod inflation;
mod rates;

use std::sync::{Arc, OnceLock};

/// Register a [`GenericInstrumentPricer`](crate::instruments::common_impl::GenericInstrumentPricer)
/// for an instrument type, collapsing the repetitive registration boilerplate
/// shared by every asset-class shard.
///
/// Two forms are supported:
///
/// - `register_generic!(registry, InstrumentType::Foo, crate::instruments::FooType);`
///   registers the discounting pricer (`GenericInstrumentPricer::<T>::discounting`)
///   under `ModelKey::Discounting`.
/// - `register_generic!(registry, InstrumentType::Foo, crate::instruments::FooType, ModelKey::Bar);`
///   registers `GenericInstrumentPricer::<T>::new(InstrumentType::Foo, ModelKey::Bar)`
///   under the explicit model key.
///
/// Both forms expand to a single `registry.register(...)` call with behavior
/// byte-identical to the hand-written registrations they replace.
macro_rules! register_generic {
    ($registry:expr, $inst:expr, $ty:ty $(,)?) => {
        $registry.register(
            $inst,
            $crate::pricer::ModelKey::Discounting,
            $crate::instruments::common_impl::GenericInstrumentPricer::<$ty>::discounting($inst),
        )
    };
    ($registry:expr, $inst:expr, $ty:ty, $model:expr $(,)?) => {
        $registry.register(
            $inst,
            $model,
            $crate::instruments::common_impl::GenericInstrumentPricer::<$ty>::new($inst, $model),
        )
    };
}

pub(crate) use register_generic;

/// Register all standard pricers explicitly.
///
/// This function keeps the full registration list in one visible place while
/// delegating concrete registration tables to the asset-class submodules below.
/// This explicit approach provides better IDE support, easier debugging, and
/// clearer dependency tracking compared to auto-registration.
fn register_all_pricers(registry: &mut PricerRegistry) {
    register_rates_pricers(registry);
    register_credit_pricers(registry);
    register_equity_pricers(registry);
    register_fx_pricers(registry);
    register_fixed_income_pricers(registry);
    register_inflation_pricers(registry);
    register_exotic_pricers(registry);
    register_commodity_pricers(registry);
}

pub use commodity::register_commodity_pricers;
pub use credit::register_credit_pricers;
pub use equity::register_equity_pricers;
pub use exotics::register_exotic_pricers;
pub use fixed_income::register_fixed_income_pricers;
pub use fx::register_fx_pricers;
pub use inflation::register_inflation_pricers;
pub use rates::register_rates_pricers;

/// Build a standard pricer registry with all registered pricers.
///
/// This helper explicitly registers all instrument pricers into a fresh registry.
/// The explicit registration approach provides better visibility, IDE support, and
/// debugging capabilities compared to the previous auto-registration system.
///
/// All 40+ instrument pricers are registered in the `register_all_pricers` function.
/// Note: All pricers now use standardized parameter ordering: (instrument, market, as_of).
///
/// A duplicate `(instrument, model)` registration in the standard registry is a
/// bug: the second `register` call silently overwrites the first. Because this
/// runs inside a `OnceLock` initializer it cannot return a `Result`, so any
/// collision is surfaced as a hard `tracing::error!`. The
/// `register_all_pricers_has_no_duplicate_keys` test fails CI on the same
/// condition.
fn build_standard_registry() -> PricerRegistry {
    let mut registry = PricerRegistry::new();
    register_all_pricers(&mut registry);
    for key in registry.duplicate_keys() {
        tracing::error!(
            ?key,
            "standard pricer registry built with a duplicate (instrument, model) \
             registration; a shard overwrote a previously registered pricer"
        );
    }
    registry
}

static STANDARD_PRICER_REGISTRY: OnceLock<Arc<PricerRegistry>> = OnceLock::new();

/// Return the shared standard pricer registry by reference.
///
/// This is the primary public entry point for accessing the built-in pricer set.
/// Callers that need to mutate a registry should start from `standard_registry().clone()`.
pub fn standard_registry() -> &'static PricerRegistry {
    STANDARD_PRICER_REGISTRY
        .get_or_init(|| Arc::new(build_standard_registry()))
        .as_ref()
}

/// Return the shared standard pricer registry.
///
/// The registry is initialized once and then cloned via `Arc` for cheap reuse
/// across instrument-side pricing calls.
pub fn shared_standard_registry() -> Arc<PricerRegistry> {
    STANDARD_PRICER_REGISTRY
        .get_or_init(|| Arc::new(build_standard_registry()))
        .clone()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::results::ValuationResult;
    use finstack_core::currency::Currency;
    use finstack_core::money::Money;
    use std::ptr;

    struct DummyPricer;

    impl Pricer for DummyPricer {
        fn key(&self) -> PricerKey {
            PricerKey::new(InstrumentType::Deposit, ModelKey::Tree)
        }

        fn price_dyn(
            &self,
            _instrument: &dyn crate::instruments::Instrument,
            _market: &finstack_core::market_data::MarketContext,
            as_of: finstack_core::dates::Date,
        ) -> PricingResult<ValuationResult> {
            Ok(ValuationResult::stamped(
                "dummy",
                as_of,
                Money::new(0.0, Currency::USD),
            ))
        }
    }

    /// CI guard: the standard registry must not contain any duplicate
    /// `(instrument, model)` registration.
    ///
    /// A duplicate means one shard's `register` call silently overwrote
    /// another pricer. `build_standard_registry` records every such collision
    /// in `duplicate_keys`; this test asserts that set is empty so the bug is
    /// caught before it ships.
    #[test]
    fn register_all_pricers_has_no_duplicate_keys() {
        let registry = build_standard_registry();
        let duplicates = registry.duplicate_keys();
        assert!(
            duplicates.is_empty(),
            "standard pricer registry has duplicate (instrument, model) registrations: \
             {duplicates:?} — a shard's `register` call overwrote an existing pricer",
        );
    }

    /// `try_register` rejects a colliding key instead of overwriting, and
    /// leaves the originally registered pricer in place.
    #[test]
    fn try_register_rejects_duplicate_key() {
        let mut registry = PricerRegistry::new();
        let key = PricerKey::new(InstrumentType::Deposit, ModelKey::Tree);

        assert!(registry
            .try_register(InstrumentType::Deposit, ModelKey::Tree, DummyPricer)
            .is_ok());
        assert_eq!(
            registry.try_register(InstrumentType::Deposit, ModelKey::Tree, DummyPricer),
            Err(key),
            "second try_register for the same key must return the conflicting key",
        );
        assert!(
            registry.get_pricer(key).is_some(),
            "rejected try_register must leave the first pricer registered",
        );
    }

    #[test]
    fn standard_registry_returns_shared_singleton() {
        assert!(ptr::eq(standard_registry(), standard_registry()));
        assert!(ptr::eq(
            standard_registry(),
            shared_standard_registry().as_ref(),
        ));
    }

    #[test]
    fn cloned_standard_registry_is_independently_mutable() {
        let key = PricerKey::new(InstrumentType::Deposit, ModelKey::Tree);
        assert!(standard_registry().get_pricer(key).is_none());

        let mut cloned = standard_registry().clone();
        cloned.register(InstrumentType::Deposit, ModelKey::Tree, DummyPricer);

        assert!(cloned.get_pricer(key).is_some());
        assert!(standard_registry().get_pricer(key).is_none());
    }

    #[test]
    fn standard_registry_exposes_range_accrual_analytic_and_mc_models() {
        let registry = standard_registry();
        assert!(registry
            .get_pricer(PricerKey::new(
                InstrumentType::RangeAccrual,
                ModelKey::StaticReplication,
            ))
            .is_some());
        assert!(registry
            .get_pricer(PricerKey::new(
                InstrumentType::RangeAccrual,
                ModelKey::MonteCarloGBM,
            ))
            .is_some());
        assert!(registry
            .get_pricer(PricerKey::new(
                InstrumentType::Tarn,
                ModelKey::MonteCarloHullWhite1F,
            ))
            .is_some());
        assert!(registry
            .get_pricer(PricerKey::new(
                InstrumentType::Snowball,
                ModelKey::MonteCarloHullWhite1F,
            ))
            .is_some());
        assert!(registry
            .get_pricer(PricerKey::new(
                InstrumentType::Snowball,
                ModelKey::Discounting,
            ))
            .is_some());
        assert!(registry
            .get_pricer(PricerKey::new(
                InstrumentType::CallableRangeAccrual,
                ModelKey::MonteCarloHullWhite1F,
            ))
            .is_some());
        assert!(registry
            .get_pricer(PricerKey::new(
                InstrumentType::CmsSpreadOption,
                ModelKey::StaticReplication,
            ))
            .is_some());
    }
}
