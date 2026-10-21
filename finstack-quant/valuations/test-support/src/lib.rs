#![forbid(unsafe_code)]
// Test support: helpers panic on malformed fixtures by design.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

//! Shared test support for the valuations crates.
//!
//! Dev-only market-data builders, fixtures, tolerance constants and parity
//! assertions used by the integration tests of `finstack-quant-valuations`
//! and of every instrument-family crate. Everything here depends on
//! `finstack-quant-core` only: instrument-building helpers live with the
//! crate that owns the instrument.
//!
//! # Modules
//!
//! - [`assertions`]: sign and finiteness assertions with named diagnostics.
//! - [`builders`]: `TestMarketBuilder` for spot / vol / discount market contexts.
//! - [`fixtures`]: a standard base date and USD discount curves.
//! - [`tolerances`]: the tolerance tiers, strictest to loosest.
//! - [`parity`]: `ParityConfig`, `compare_values` and the `assert_parity!`
//!   macro for documented reference-value comparisons.
//! - [`test_helpers`]: standard dates, flat curve builders, swap and credit
//!   markets and money shorthands.
//! - [`date`], [`discount_forward_curves`], [`volatility`],
//!   [`commodity_curves`]: one-line curve and surface constructors.
//! - [`inflation`]: inflation curves, CPI index series and the standard
//!   inflation markets of the inflation-swap and cap/floor tests.
//!
//! # Example
//!
//! ```rust
//! use finstack_quant_valuations_test_support::builders::TestMarketBuilder;
//! use finstack_quant_valuations_test_support::fixtures::base_date;
//! use finstack_quant_valuations_test_support::tolerances;
//!
//! let as_of = base_date();
//! let market = TestMarketBuilder::new(as_of).spot(100.0).vol(0.25).rate(0.05).build();
//! assert!(market.get_discount("USD-OIS").is_ok());
//! assert!(tolerances::TIGHT < tolerances::ANALYTICAL);
//! ```

pub mod assertions;
pub mod builders;
/// Flat and contango commodity price curves.
pub mod commodity_curves;
/// One-line calendar date constructor.
pub mod date;
/// Flat discount and forward curves with a configurable far knot.
pub mod discount_forward_curves;
pub mod fixtures;
/// Inflation curves, CPI index series and inflation market contexts.
pub mod inflation;
pub mod parity;
pub mod test_helpers;
pub mod tolerances;
/// Constant volatility surfaces.
pub mod volatility;

/// Compiles the crate `README.md` Rust samples as doctests.
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;
