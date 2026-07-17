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

//! Procedural macros for the finstack-quant-valuations crate.
//!
//! This crate provides derive and attribute macros to reduce boilerplate
//! and improve type safety in the valuations module:
//!
//! - `FinancialBuilder`: Generates type-safe builder patterns for instruments
//! - `FocusedPricingOverrides`: Preserves the legacy pricing-overrides wire shape

use proc_macro::TokenStream;

mod financial_builder;
mod focused_pricing_overrides;

/// Derives a builder pattern for financial instrument structs.
///
/// See the `financial_builder` module for detailed documentation.
#[proc_macro_derive(FinancialBuilder, attributes(builder))]
pub fn derive_financial_builder(input: TokenStream) -> TokenStream {
    financial_builder::derive_financial_builder_impl(input)
}

/// Derives serde and schema implementations for instruments that store the
/// three focused pricing-override categories separately at runtime.
///
/// The runtime struct must contain fields named `instrument_pricing_overrides`,
/// `metric_pricing_overrides`, and `scenario_pricing_overrides`. On the wire,
/// those fields are represented by the single legacy `pricing_overrides`
/// property.
#[proc_macro_derive(
    FocusedPricingOverrides,
    attributes(pricing_overrides, serde, schemars)
)]
pub fn derive_focused_pricing_overrides(input: TokenStream) -> TokenStream {
    focused_pricing_overrides::derive_focused_pricing_overrides_impl(input)
}
