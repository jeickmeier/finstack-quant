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
//! tables for the types it owns (`RATES`, `FIXED_INCOME`, `STRUCTURED_CREDIT`, `CREDIT`,
//! `EQUITY`, `FX`, `EXOTICS` in the umbrella crate). The umbrella's `STANDARD` family merges
//! them all and adds the composite instruments (`Composite`, `Basket`) that can hold any leg;
//! its `finstack_quant_valuations::pricer::STANDARD.pricers()` is that pricer registry.

// Core submodules
#[doc(hidden)]
pub mod defaults;
mod enrichment;
mod errors;
mod family;
mod keys;
mod registry;

pub use errors::{PricingError, PricingErrorContext};
pub use family::{Family, RegisterMetrics, RegisterPricers};
pub use keys::{InstrumentType, ModelKey, PricerKey, parse_model_key};
pub use registry::{Pricer, PricerRegistry, PricingDispatch, expect_inst};

/// Register a [`GenericInstrumentPricer`](crate::instruments::pricing::GenericInstrumentPricer)
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
    ($registry:expr_2021, $inst:expr_2021, $ty:ty $(,)?) => {
        $registry.register(
            $crate::instruments::pricing::GenericInstrumentPricer::<$ty>::discounting($inst),
        )?
    };
    ($registry:expr_2021, $inst:expr_2021, $ty:ty, $model:expr_2021 $(,)?) => {
        $registry.register(
            $crate::instruments::pricing::GenericInstrumentPricer::<$ty>::new($inst, $model),
        )?
    };
}
