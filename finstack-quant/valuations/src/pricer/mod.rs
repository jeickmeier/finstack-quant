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
//!   [`crate::pricer::PricingErrorContext`]
//! - `registry`: [`crate::pricer::Pricer`], [`crate::pricer::PricerRegistry`],
//!   `expect_inst`
//! - `family`: [`crate::pricer::Family`], the registration unit
//!
//! Registration lives with the instruments. Each instrument family declares a
//! [`Family`](crate::pricer::Family) `static` beside its instruments, holding the pricer and metric
//! tables for the types it owns:
//! [`rates::RATES`](crate::instruments::rates::RATES),
//! [`fixed_income::FIXED_INCOME`](crate::instruments::fixed_income::FIXED_INCOME),
//! [`structured_credit::STRUCTURED_CREDIT`](crate::instruments::fixed_income::structured_credit::STRUCTURED_CREDIT),
//! [`credit_derivatives::CREDIT`](crate::instruments::credit_derivatives::CREDIT),
//! [`equity::EQUITY`](crate::instruments::equity::EQUITY),
//! [`fx::FX`](crate::instruments::fx::FX) and
//! [`exotics::EXOTICS`](crate::instruments::exotics::EXOTICS) (which also
//! owns the commodity instruments). [`STANDARD`](crate::pricer::STANDARD) merges them all and adds the
//! composite instruments (`Composite`, `Basket`) that can hold any leg;
//! [`standard_pricer_registry`](crate::pricer::standard_pricer_registry) is its pricer registry.

// Core submodules
#[doc(hidden)]
pub mod defaults;
mod enrichment;
mod errors;
mod family;
pub mod json;
mod keys;
mod registry;

pub use errors::{PricingError, PricingErrorContext};
pub use family::{Family, RegisterMetrics, RegisterPricers};
pub use json::{
    instrument_envelope_from_spec, instrument_from_spec, list_models, list_models_grouped,
    list_standard_metrics, list_standard_metrics_grouped, metric_metadata, metric_value,
    parse_boxed_instrument_from_json, parse_instrument_from_json, parse_model_key,
    parse_typed_instrument_json, present_standard_option_greeks, pretty_instrument_json,
    price_instrument, validate_instrument_json, validate_typed_instrument_json, MetricMetadata,
    ParsedInstrument, STANDARD_OPTION_GREEKS,
};
pub use keys::{InstrumentType, ModelKey, PricerKey};
#[doc(hidden)]
pub use registry::stamp_results_meta;
pub use registry::{expect_inst, Pricer, PricerRegistry, PricingDispatch};

use std::sync::Arc;

use crate::instruments::credit_derivatives::CREDIT;
use crate::instruments::equity::EQUITY;
use crate::instruments::exotics::EXOTICS;
use crate::instruments::fixed_income::structured_credit::STRUCTURED_CREDIT;
use crate::instruments::fixed_income::FIXED_INCOME;
use crate::instruments::fx::FX;
use crate::instruments::rates::RATES;
use crate::metrics::{MetricRegistry, MetricRegistryError};

/// Register a [`GenericInstrumentPricer`](crate::instruments::common_impl::GenericInstrumentPricer)
/// for an instrument type, collapsing the repetitive registration boilerplate
/// shared by every family table.
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
/// Both forms expand to a single `registry.register(...)` call.
#[macro_export]
macro_rules! register_generic {
    ($registry:expr, $inst:expr, $ty:ty $(,)?) => {
        $registry.register($crate::instruments::common_impl::GenericInstrumentPricer::<
            $ty,
        >::discounting($inst))?
    };
    ($registry:expr, $inst:expr, $ty:ty, $model:expr $(,)?) => {
        $registry.register($crate::instruments::common_impl::GenericInstrumentPricer::<
            $ty,
        >::new($inst, $model))?
    };
}

pub use register_generic;

/// The built-in instrument families, each owning a disjoint set of
/// instrument types.
///
/// The order is the order [`STANDARD`] registers them in; it has no effect on
/// lookups.
pub static FAMILIES: [&Family; 7] = [
    &RATES,
    &FIXED_INCOME,
    &STRUCTURED_CREDIT,
    &CREDIT,
    &EQUITY,
    &FX,
    &EXOTICS,
];

/// The umbrella family: every built-in family merged, plus the composite
/// instruments.
///
/// `Composite` and `Basket` hold legs of any family, so they belong here
/// rather than to one asset class. The JSON entry points and registry
/// introspection (`list_models`, `list_standard_metrics`) read this family's
/// registries.
pub static STANDARD: Family = Family::new(
    "standard",
    register_standard_pricers,
    register_standard_metrics,
);

fn register_standard_pricers(registry: &mut PricerRegistry) -> Result<(), PricingError> {
    for family in FAMILIES {
        family.register_pricers(registry)?;
    }
    register_generic!(
        registry,
        InstrumentType::Composite,
        crate::instruments::CompositeInstrument
    );
    register_generic!(
        registry,
        InstrumentType::Basket,
        crate::instruments::exotics::basket::Basket
    );
    Ok(())
}

fn register_standard_metrics(registry: &mut MetricRegistry) -> Result<(), MetricRegistryError> {
    for family in FAMILIES {
        family.register_metrics(registry)?;
    }
    crate::instruments::exotics::basket::metrics::register_basket_metrics(registry)
}

/// Return the shared standard pricer registry by reference.
///
/// This is the registry of the [`STANDARD`] family: every built-in pricer.
/// Callers that need to mutate a registry should start from
/// `standard_pricer_registry().clone()`.
pub fn standard_pricer_registry() -> &'static PricerRegistry {
    STANDARD.pricers()
}

/// Return the shared standard pricer registry as an `Arc` for pricing options
/// and dispatch handles.
pub(crate) fn shared_standard_registry() -> Arc<PricerRegistry> {
    STANDARD.shared_pricers()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::results::ValuationResult;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::money::Money;
    use std::collections::BTreeMap;
    use std::ptr;

    struct DummyPricer;

    impl Pricer for DummyPricer {
        fn key(&self) -> PricerKey {
            PricerKey::new(InstrumentType::Deposit, ModelKey::Tree)
        }

        fn price_dyn(
            &self,
            _instrument: &dyn crate::instruments::Instrument,
            _market: &finstack_quant_core::market_data::MarketContext,
            as_of: finstack_quant_core::dates::Date,
        ) -> std::result::Result<ValuationResult, PricingError> {
            Ok(ValuationResult::stamped(
                "dummy",
                as_of,
                Money::from((0_i64, Currency::USD)),
            ))
        }
    }

    #[test]
    fn registration_key_comes_from_the_pricer() {
        let mut registry = PricerRegistry::new();
        let key = DummyPricer.key();

        registry.register(DummyPricer).expect("registration");

        assert!(registry.get_pricer(key).is_some());
    }

    /// `register` returns a typed collision error without overwriting.
    #[test]
    fn register_rejects_duplicate_key() {
        let mut registry = PricerRegistry::new();
        let key = PricerKey::new(InstrumentType::Deposit, ModelKey::Tree);

        registry.register(DummyPricer).expect("first registration");
        let error = registry
            .register(DummyPricer)
            .expect_err("duplicate registration must fail");
        assert_eq!(error, PricingError::DuplicateRegistration { key });
        assert!(registry.get_pricer(key).is_some());
    }

    #[test]
    fn replace_is_the_explicit_overwrite_operation() {
        let mut registry = PricerRegistry::new();
        let key = PricerKey::new(InstrumentType::Deposit, ModelKey::Tree);
        registry.register(DummyPricer).expect("first registration");
        registry.replace(DummyPricer);
        assert!(registry.get_pricer(key).is_some());
    }

    #[test]
    fn standard_registry_returns_shared_singleton() {
        assert!(ptr::eq(
            standard_pricer_registry(),
            standard_pricer_registry()
        ));
        assert!(ptr::eq(
            standard_pricer_registry(),
            shared_standard_registry().as_ref(),
        ));
        assert!(ptr::eq(
            crate::metrics::standard_registry(),
            STANDARD.metrics()
        ));
    }

    #[test]
    fn cloned_standard_registry_is_independently_mutable() {
        let key = PricerKey::new(InstrumentType::Deposit, ModelKey::Tree);
        assert!(standard_pricer_registry().get_pricer(key).is_none());

        let mut cloned = standard_pricer_registry().clone();
        cloned
            .register(DummyPricer)
            .expect("new clone registration");

        assert!(cloned.get_pricer(key).is_some());
        assert!(standard_pricer_registry().get_pricer(key).is_none());
    }

    /// Every pricer has exactly one owning family, and the umbrella is the
    /// union of the families plus the composite instruments.
    #[test]
    fn families_own_disjoint_instrument_types_that_cover_the_standard_registry() {
        let mut owner: BTreeMap<InstrumentType, &str> = BTreeMap::new();
        for family in FAMILIES {
            let pricers = family
                .build_pricers()
                .unwrap_or_else(|error| panic!("{} pricer table: {error}", family.name()));
            for instrument_type in pricers.all_models_grouped().keys() {
                if let Some(previous) = owner.insert(*instrument_type, family.name()) {
                    panic!(
                        "{instrument_type:?} is registered by both {previous} and {}",
                        family.name()
                    );
                }
            }
        }
        owner.insert(InstrumentType::Composite, STANDARD.name());
        owner.insert(InstrumentType::Basket, STANDARD.name());

        let standard: Vec<InstrumentType> = standard_pricer_registry()
            .all_models_grouped()
            .keys()
            .copied()
            .collect();
        let owned: Vec<InstrumentType> = owner.keys().copied().collect();
        assert_eq!(owned, standard);
    }

    /// Resolving through a family answers exactly as the merged umbrella
    /// does for every instrument type the family owns: same models, same
    /// applicable metrics.
    #[test]
    fn family_registries_match_the_umbrella_for_owned_types() {
        let standard_models = standard_pricer_registry().all_models_grouped();
        let standard_metrics = crate::metrics::standard_registry();
        for family in FAMILIES {
            let models = family.pricers().all_models_grouped();
            assert!(!models.is_empty(), "{} registers no pricers", family.name());
            for (instrument_type, family_models) in &models {
                assert_eq!(
                    Some(family_models),
                    standard_models.get(instrument_type),
                    "{} models for {instrument_type:?}",
                    family.name()
                );
                assert_eq!(
                    family.metrics().metrics_for_instrument(*instrument_type),
                    standard_metrics.metrics_for_instrument(*instrument_type),
                    "{} metrics for {instrument_type:?}",
                    family.name()
                );
            }
        }
    }

    #[test]
    fn built_in_metric_owners_are_unique() {
        STANDARD
            .build_metrics()
            .expect("each built-in metric slot must have one owner");
    }
}
