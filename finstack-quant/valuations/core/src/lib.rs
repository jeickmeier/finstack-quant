#![forbid(unsafe_code)]
#![warn(clippy::float_cmp)]
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![deny(clippy::unreachable)]
#![cfg_attr(
    test,
    allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::unreachable,
        clippy::indexing_slicing,
        clippy::float_cmp,
    )
)]
// Allow expect() in doc tests (they are test code)
#![doc(test(attr(allow(clippy::expect_used))))]

//! Pricing framework of the valuations stack: the instrument contract, pricing
//! dispatch, metric calculators, market conventions and result envelopes.
//!
//! Instruments implement [`crate::instruments::Instrument`], consume market data from
//! [`finstack_quant_core::market_data::MarketContext`], and dispatch through registered pricing
//! models. Pricing returns a currency-tagged present value plus requested metrics in
//! [`crate::results::ValuationResult`]. This crate holds no instrument type: the instrument
//! families and the closed-world assembly (JSON loading, the standard registries, composite
//! instruments, schemas) live in the umbrella crate `finstack-quant-valuations`, which
//! re-exports everything here under the same paths.
//!
//! # Documentation Conventions
//!
//! - Prefer typed rates and spreads where the API accepts them, and always state the
//!   representation and units of financial inputs.
//! - Treat [`crate::results::ValuationResult::value`] as monetary present value; interpret other
//!   measures using the units, sign, and bump contracts on [`crate::metrics::MetricId`].
//! - Document day-count, calendar, compounding, settlement, quote, and curve-role conventions at
//!   the public API that owns the behavior.
//! - Cite named models and market conventions with canonical entries.
//!
//! # Modules
//!
//! - [`crate::instruments`]: the [`Instrument`](crate::instruments::Instrument) trait, pricing
//!   options and overrides, shared parameter types, the generic pricer and the cashflow exporter.
//! - [`crate::market`]: market quotes and conventions.
//! - [`crate::pricer`]: pricing dispatch, registries and the [`Family`](crate::pricer::Family)
//!   registration unit.
//! - [`crate::metrics`]: risk metric identifiers, calculators, and registries.
//! - [`crate::results`]: valuation result envelopes and metadata.
//! - [`crate::constants`]: shared numerical constants.
//! - [`crate::error`]: valuation error types; [`crate::Error`] and [`crate::Result`] are also
//!   available at the crate root.
//!
//! # Example
//!
//! [`crate::instruments::Instrument::price_with_metrics`] is the canonical pricing entry point;
//! the [`Instrument`](crate::instruments::Instrument) documentation implements the trait for a
//! one-payment note and prices it. With an instrument family in scope (the umbrella crate), the
//! same call prices a bond, a swap or an option.

extern crate self as finstack_quant_valuations_core;

/// Shared numerical constants and basis point helpers.
pub mod constants;
#[doc(hidden)]
pub mod contract_specs;
/// Error types for pricing and valuation workflows.
pub mod error;
/// Market quotes and conventions
pub mod market;
/// Pricing dispatch and registry infrastructure.
pub mod pricer;
/// Contracts for quote-driven market recalibration supplied by the calibration crate.
pub mod recalibration;
/// Valuation result envelopes and metadata.
pub mod results;
#[doc(hidden)]
pub mod serde_defaults;

/// Hidden support paths for exported macros.
#[doc(hidden)]
pub mod __private {
    /// Cashflow crate path used by exported macros without exposing `cashflow`.
    pub use finstack_quant_cashflows;
}

#[macro_use]
/// Financial instrument definitions and builders.
pub mod instruments;
/// Risk metric calculators and registries.
pub mod metrics;

pub use error::{Error, Result};

/// Compiles the crate `README.md` Rust samples as doctests.
///
/// The README is *not* included in the rendered crate documentation — this
/// item exists only under `cfg(doctest)` so that every ` ```rust ` block in the
/// README is compiled and run by `cargo test --doc`. Without it those samples
/// are dead text and rot silently on any API change.
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;
