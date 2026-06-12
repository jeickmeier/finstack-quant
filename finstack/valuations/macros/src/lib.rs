#![forbid(unsafe_code)]
#![warn(clippy::float_cmp)]
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![cfg_attr(
    test,
    allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::indexing_slicing,
        clippy::float_cmp,
    )
)]
// Allow expect() in doc tests (they are test code)
#![doc(test(attr(allow(clippy::expect_used))))]

//! Procedural macros for the finstack-valuations crate.
//!
//! This crate provides derive and attribute macros to reduce boilerplate
//! and improve type safety in the valuations module:
//!
//! - `FinancialBuilder`: Generates type-safe builder patterns for instruments

use proc_macro::TokenStream;

mod financial_builder;

/// Derives a builder pattern for financial instrument structs.
///
/// See the `financial_builder` module for detailed documentation.
#[proc_macro_derive(FinancialBuilder, attributes(builder))]
pub fn derive_financial_builder(input: TokenStream) -> TokenStream {
    financial_builder::derive_financial_builder_impl(input)
}
