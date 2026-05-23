#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![warn(clippy::new_without_default)]
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

//! Portfolio management and aggregation for finstack.
//!
//! This crate provides portfolio-level operations including:
//! - Entity and position management
//! - Valuation aggregation across positions
//! - Metrics aggregation with cross-currency support
//! - Attribute-based grouping and analysis
//! - Scenario application
//! - Tabular exports for analysis
//!
//! # Quick Start
//!
//! ```rust
//! use finstack_portfolio::Portfolio;
//! use finstack_portfolio::position::{Position, PositionUnit};
//! use finstack_portfolio::types::Entity;
//! use finstack_core::currency::Currency;
//! use finstack_core::money::Money;
//! use finstack_valuations::instruments::rates::deposit::Deposit;
//! use std::sync::Arc;
//! use time::macros::date;
//!
//! let as_of = date!(2024-01-01);
//!
//! // Create a deposit instrument
//! let deposit = Deposit::builder()
//!     .id("DEP_1M".into())
//!     .notional(Money::new(1_000_000.0, Currency::USD))
//!     .start_date(as_of)
//!     .maturity(date!(2024-02-01))
//!     .day_count(finstack_core::dates::DayCount::Act360)
//!     .discount_curve_id("USD".into())
//!     .build()
//!     .expect("test should succeed");
//!
//! // Create a position holding the deposit
//! let position = Position::new(
//!     "POS_001",
//!     "ACME_CORP",
//!     "DEP_1M",
//!     Arc::new(deposit),
//!     1.0,
//!     PositionUnit::Units,
//! ).expect("test should succeed")
//!  .with_text_attribute("asset_class", "cash");
//!
//! // Build the portfolio with the entity and position
//! let portfolio = Portfolio::builder("MY_FUND")
//!     .base_ccy(Currency::USD)
//!     .as_of(as_of)
//!     .entity(Entity::new("ACME_CORP"))
//!     .position(position)
//!     .build()
//!     .expect("test should succeed");
//! ```

macro_rules! define_string_id {
    ($(#[$meta:meta])* $vis:vis struct $name:ident;) => {
        $(#[$meta])*
        #[derive(Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
        #[repr(transparent)]
        $vis struct $name(String);

        impl $name {
            /// Create a new identifier.
            ///
            /// # Arguments
            ///
            /// * `id` - The identifier string.
            ///
            /// # Returns
            ///
            /// A strongly typed identifier wrapping the supplied string.
            pub fn new(id: impl Into<String>) -> Self {
                Self(id.into())
            }

            /// Get the identifier as a string slice.
            ///
            /// # Returns
            ///
            /// Borrowed view of the underlying identifier without allocating.
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

/// Portfolio-level PnL attribution and breakdowns.
pub mod attribution;
/// Book hierarchy and identifiers.
pub mod book;
/// Brinson-Fachler three-way benchmark-relative attribution with
/// Carino linking.
pub mod brinson;
/// Fluent portfolio construction helpers.
pub mod builder;
/// Tabular exports for portfolio results.
pub(crate) mod dataframe;
/// Error types for portfolio operations.
pub mod error;
/// Factor-model portfolio risk decomposition outputs and engines.
pub mod factor_model;
/// Shared FX conversion helpers (e.g. converting position values to base currency).
pub(crate) mod fx;
/// Grouping and aggregation by attributes or books.
pub mod grouping;
/// Liquidity risk metrics, spread estimation, and portfolio scoring.
pub mod liquidity;
/// Portfolio margin and netting set utilities.
pub(crate) mod margin;
/// Metrics aggregation and reporting.
pub mod metrics;
/// Portfolio optimization engines and constraints.
pub mod optimization;
/// TWRR / MWRR / GIPS-style return linking.
pub mod performance;
/// Portfolio container and state management.
pub mod portfolio;
/// Position primitives and units.
pub mod position;

/// Embedded portfolio defaults registries.
pub(crate) mod registry;
/// Result envelopes for portfolio operations.
pub mod results;
/// Factor sensitivity engines (delta-based + full-repricing) and JSON façade.
///
/// Hosts engines that bump-and-reprice `&dyn Instrument` against a
/// `MarketContext` to produce positions × factors sensitivity matrices and
/// scenario-grid P&L profiles. Originally lived in
/// `finstack-valuations::factor_model::sensitivity`; relocated here because
/// these are portfolio-level analytics with no per-instrument metric semantics.
pub mod sensitivity;
/// Core portfolio entity and ID types.
pub mod types;
/// Portfolio valuation APIs.
pub mod valuation;

/// Cashflow ladder and schedule aggregation utilities.
pub mod cashflows;
/// Market-factor dependency index for selective repricing.
pub(crate) mod dependencies;

#[cfg(test)]
mod test_utils;

/// Scenario application for portfolios.
pub mod scenarios;

/// Historical scenario replay for portfolios.
pub mod replay;

pub use builder::PortfolioBuilder;
pub use error::{Error, Result};
pub use portfolio::Portfolio;
pub use types::{AttributeTest, AttributeValue, ComparisonOp, Entity, EntityId, PositionId};

// Headline analytics types at the crate root so callers don't have to thread
// through long module paths for the most common workflows. Anything not
// re-exported here is still reachable via its module path; the goal is just
// to surface the canonical entry points.
pub use brinson::{
    brinson_fachler, carino_link, carino_link_from_sector_periods, BrinsonPeriodResult,
    CarinoLinkedAttribution, SectorEffect, SectorPeriod,
};
pub use dataframe::{
    aggregated_metrics_to_table, entities_to_table, metrics_to_table, positions_to_table,
};
pub use dependencies::{flatten_dependencies, DependencyIndex, MarketFactorKey};
pub use margin::{
    CurrencyMismatchError, NettingSet, NettingSetManager, NettingSetMargin,
    PortfolioMarginAggregator, PortfolioMarginResult,
};
pub use performance::{
    mwr_xirr, mwr_xirr_from_cashflows, twrr_linked, twrr_modified_dietz, DatedCashflow, DietzFlow,
    LinkedReturn, TwrrPeriod,
};
pub use portfolio::PortfolioSpec;
pub use position::{Position, PositionUnit};
pub use results::PortfolioResult;
pub use valuation::{
    revalue_affected, value_portfolio, PortfolioValuation, PortfolioValuationOptions,
    PositionValue, RequestedMetrics,
};
