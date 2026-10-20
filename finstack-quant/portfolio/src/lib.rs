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
#![doc(test(attr(allow(clippy::expect_used))))]

//! Portfolio construction, valuation, metrics, attribution, scenarios, and
//! optimization on top of `finstack-quant-core` instruments.
//!
//! # Quick Start
//!
//! ```rust
//! use finstack_quant_portfolio::Portfolio;
//! use finstack_quant_portfolio::position::{Position, PositionUnit};
//! use finstack_quant_portfolio::types::Entity;
//! use finstack_quant_core::currency::Currency;
//! use finstack_quant_core::money::Money;
//! use finstack_quant_valuations::instruments::rates::deposit::Deposit;
//! use std::sync::Arc;
//! use time::macros::date;
//!
//! let as_of = date!(2024-01-01);
//!
//! let deposit = Deposit::builder()
//!     .id("DEP_1M".into())
//!     .notional(Money::from((1_000_000_i64, Currency::USD)))
//!     .start_date(as_of)
//!     .maturity(date!(2024-02-01))
//!     .day_count(finstack_quant_core::dates::DayCount::Act360)
//!     .discount_curve_id("USD".into())
//!     .build()
//!     .expect("test should succeed");
//!
//! let position = Position::new(
//!     "POS_001",
//!     "ACME_CORP",
//!     "DEP_1M",
//!     Arc::new(deposit),
//!     1.0,
//!     PositionUnit::Units,
//! ).expect("test should succeed")
//!  .with_attribute("asset_class", "cash");
//!
//! let portfolio = Portfolio::builder("MY_FUND")
//!     .base_currency(Currency::USD)
//!     .as_of(as_of)
//!     .entity(Entity::new("ACME_CORP"))
//!     .position(position)
//!     .build()
//!     .expect("test should succeed");
//! ```
//!
//! # References
//!
//! - Brinson-Fachler attribution: `docs/REFERENCES.md#brinson-fachler-1985`
//! - Euler capital allocation: `docs/REFERENCES.md#tasche-2008-capital-allocation`
//! - Liquidity-adjusted VaR: `docs/REFERENCES.md#bangia-1999-lvar`

macro_rules! define_string_id {
    ($(#[$meta:meta])* $vis:vis struct $name:ident;) => {
        $(#[$meta])*
        #[derive(Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
        #[repr(transparent)]
        $vis struct $name(String);

        impl $name {
            /// Wrap `id` as a typed identifier.
            ///
            /// # Arguments
            ///
            /// * `id` - Identifier string stored without extra validation.
            pub fn new(id: impl Into<String>) -> Self {
                Self(id.into())
            }

            /// Borrowed identifier string.
            #[inline]
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                self.0.fmt(f)
            }
        }

        impl From<&str> for $name {
            fn from(s: &str) -> Self {
                Self(s.to_string())
            }
        }

        impl From<String> for $name {
            fn from(s: String) -> Self {
                Self(s)
            }
        }

        impl std::borrow::Borrow<str> for $name {
            fn borrow(&self) -> &str {
                &self.0
            }
        }

        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                &self.0
            }
        }

        impl PartialEq<&str> for $name {
            fn eq(&self, other: &&str) -> bool {
                self.0 == *other
            }
        }

        impl PartialEq<str> for $name {
            fn eq(&self, other: &str) -> bool {
                self.0 == other
            }
        }
    };
}

// One public path per item: the four modules below hold the crate-defining
// types and stay private, so `Error`, `Result`, `Portfolio`, `PortfolioSpec`,
// `PortfolioBuilder` and `PortfolioResult` live at the crate root only. Every
// other item is reached through its public module and is not re-exported here.
mod builder;
mod error;
mod portfolio;
mod results;

pub use builder::PortfolioBuilder;
pub use error::{Error, Result};
pub use portfolio::{Portfolio, PortfolioSpec};
pub use results::PortfolioResult;

pub mod allocation;
pub mod attribution;
pub mod book;
pub mod brinson;
pub mod cashflows;
pub mod dataframe;
pub mod dependencies;
pub(crate) mod evaluation;
pub mod excess_return;
pub mod factor_brinson;
pub mod factor_model;
pub mod fi_attribution;
pub(crate) mod fx;
pub mod grid_attribution;
pub mod grouping;
pub mod margin;
pub mod materialization;
pub mod metrics;
/// Portfolio optimization engines and constraints.
pub mod optimization;
pub mod performance;
pub mod position;
pub mod primitive;
pub mod replay;
pub mod scenarios;
/// JSON Schema registry for the portfolio wire contracts.
#[cfg(feature = "json-schema")]
pub mod schema;
pub mod sensitivity;
pub mod types;
pub mod valuation;

#[cfg(test)]
mod test_utils;

/// Compiles the crate `README.md` Rust samples as doctests.
///
/// The README is *not* included in the rendered crate documentation — this
/// item exists only under `cfg(doctest)` so that every ` ```rust ` block in the
/// README is compiled and run by `cargo test --doc`. Without it those samples
/// are dead text and rot silently on any API change.
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;
