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

//! Portfolio management and aggregation for finstack_quant.
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
//! // Create a deposit instrument
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
/// Duration-cell base-return tables from a reference universe (Lehman App. B).
pub mod excess_return;
/// Factor-Brinson unified attribution over continuous factor exposures
/// (Jeet & Partani 2023).
pub mod factor_brinson;
/// Factor-model portfolio risk decomposition outputs and engines.
pub mod factor_model;
/// Campisi-style benchmark-relative fixed-income attribution
/// (carry / treasury / spread / selection with Carino linking).
pub mod fi_attribution;
/// Shared FX conversion helpers (e.g. converting position values to base currency).
pub(crate) mod fx;
/// Hierarchical duration-cell x sector grid attribution (Lehman/Dynkin-Hyman-Vankudre 1998).
pub mod grid_attribution;
/// Grouping and aggregation by attributes or books.
pub mod grouping;
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
/// Primitive exposure and overlapping-concentration reports.
pub mod primitive;
#[cfg(feature = "json-schema")]
pub mod schema;

/// Result envelopes for portfolio operations.
pub mod results;
/// Factor sensitivity engines (delta-based + full-repricing) and JSON façade.
///
/// Hosts engines that bump-and-reprice `&dyn Instrument` against a
/// `MarketContext` to produce positions × factors sensitivity matrices and
/// scenario-grid P&L profiles. Originally lived in
/// `finstack-quant-valuations::factor_model::sensitivity`; relocated here because
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
/// Request-scoped portfolio evaluation planning and execution.
pub(crate) mod evaluation;

#[cfg(test)]
mod test_utils;

/// Scenario application for portfolios.
pub mod scenarios;

/// Historical scenario replay for portfolios.
pub mod replay;

pub use builder::PortfolioBuilder;
pub use error::{Error, Result};
pub use portfolio::Portfolio;
pub use types::{AttributeTest, AttributeValue, ComparisonOp, Entity, PositionId};

// Headline analytics types at the crate root so callers don't have to thread
// through long module paths for the most common workflows. Anything not
// re-exported here is still reachable via its module path; the goal is just
// to surface the canonical entry points.
pub use brinson::{
    brinson_fachler, carino_link, carino_link_from_sector_periods, BrinsonPeriodResult,
    CarinoLinkedAttribution, SectorPeriod,
};
pub use dataframe::{
    aggregated_metrics_to_table, entities_to_table, metrics_to_table, positions_to_table,
};
pub use dependencies::{flatten_dependencies, DependencyIndex, MarketFactorKey};
pub use excess_return::{
    cell_returns_from_curves, cell_returns_from_reference, excess_returns, CellConfig,
    DurationCellTable, ExcessReturnPosition, ExcessReturnResult, ReferenceReturn,
};
pub use factor_brinson::{
    factor_brinson_attribution, FactorBrinsonInput, FactorBrinsonResult, FactorContribution,
};
pub use factor_model::{
    allocate_weights, allocate_weights_json, validate_allocation_json, WeightAllocationResult,
    WeightAllocationSpec,
};
pub use fi_attribution::{
    campisi_attribution, campisi_carino_link, campisi_carino_link_from_snapshots,
    FiAttributionConfig, FiAttributionResult, FiCarinoLinkedResult, FiPeriodInput,
    FiPositionSnapshot, FiReconciliationReport,
};
pub use grid_attribution::{
    grid_attribution, grid_carino_link, GridAttributionResult, GridCarinoLinkedResult, GridPosition,
};
pub use margin::{NettingSetMargin, PortfolioMarginAggregator, PortfolioMarginResult};
pub use performance::{
    mwr_xirr, mwr_xirr_from_cashflows, twrr_linked, twrr_modified_dietz, DatedCashflow,
    LinkedReturn, TwrrPeriod,
};
pub use portfolio::PortfolioSpec;
pub use position::{Position, PositionUnit};
pub use primitive::primitive_exposure_report;
pub use results::PortfolioResult;
pub use valuation::{
    value_portfolio, PortfolioValuation, PortfolioValuationOptions, RequestedMetrics,
};

/// Strict versioned portfolio materialization bundles and native bulk loading.
pub mod materialization;
pub use materialization::{
    InstrumentArtifact, InstrumentArtifactCache, MaterializationPhases, MaterializationReport,
    MaterializedPosition, MaterializerInfo, PortfolioHeader, PortfolioMaterializationEnvelope,
    PortfolioMaterializationSchema,
};

/// Compiles the crate `README.md` Rust samples as doctests.
///
/// The README is *not* included in the rendered crate documentation — this
/// item exists only under `cfg(doctest)` so that every ` ```rust ` block in the
/// README is compiled and run by `cargo test --doc`. Without it those samples
/// are dead text and rot silently on any API change.
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;
