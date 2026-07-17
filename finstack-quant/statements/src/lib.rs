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

//! # Finstack Quant Statements — Financial Statement Modeling Engine
//!
//! The `finstack-quant-statements` crate enables users to build financial statement models
//! as directed graphs of metrics evaluated over discrete periods (monthly, quarterly, annually).
//!
//! ## Key Features
//!
//! - **Declarative modeling** with a rich DSL for formulas
//! - **Time-series forecasting** with deterministic and statistical methods
//! - **Capital structure integration** for debt/equity tracking
//! - **Dynamic metric registry** (no recompilation needed)
//! - **Currency-safe arithmetic** with explicit FX handling
//! - **Deterministic evaluation**
//! - **EBITDA normalization & adjustments** with audited add-backs and cap policies
//!
//! ## Quick Start
//!
//! ```rust
//! use finstack_quant_statements::prelude::*;
//!
//! # fn main() -> Result<()> {
//! // Build a simple P&L model
//! let model = FinancialModelSpec::builder("Acme Corp")
//!     .periods("2025Q1..Q4", Some("2025Q2"))?
//!     .value("revenue", &[
//!         (PeriodId::quarter(2025, 1), AmountOrScalar::scalar(10_000_000.0)),
//!         (PeriodId::quarter(2025, 2), AmountOrScalar::scalar(11_000_000.0)),
//!     ])
//!     .compute("cogs", "revenue * 0.6")?
//!     .compute("gross_profit", "revenue - cogs")?
//!     .build()?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Architecture
//!
//! The crate is organized into:
//!
//! - **types**: Wire types for serialization (`NodeSpec`, `FinancialModelSpec`)
//! - **builder**: Type-safe builder API with compile-time state enforcement
//! - **error**: Error types with context
//! - **adjustments**: Normalization engine plus add-back specs/caps for adjusted EBITDA
//! - **dsl**: Formula DSL (parser, AST, compiler) for arithmetic, time-series, rolling and statistical functions
//! - **evaluator**: DAG-based evaluation with topological sort, precedence resolution, and capital structure integration
//! - **capital_structure**: Debt/equity structure modeling and the `cs.*` market-context namespace
//! - **forecast**: Deterministic and statistical forecast methods (growth, seasonal, Monte Carlo)
//! - **registry**: Dynamic metric registry with namespace management and built-in `fin.*` metrics
//! - **checks**: Financial-statement consistency checks framework
//! - **formula**: Public formula helpers for identifier extraction and qualification
//!
//! ## Analytics
//!
//! Higher-level analysis tools (sensitivity, scenario sets, DCF, etc.) and
//! concrete analytics extensions (e.g. `CorkscrewExtension`,
//! `CreditScorecardExtension`) live in the `finstack-quant-statements-analytics`
//! crate.

/// Normalization engine and add-back tracking for adjusted metrics.
pub mod adjustments;
/// Type-safe model builder API.
pub mod builder;
/// Debt and equity structure modeling.
pub mod capital_structure;
/// Financial statement checks framework.
pub mod checks;
/// Formula DSL parsing and compilation.
pub mod dsl;
/// Error types for statement modeling.
pub mod error;
/// Evaluation engine for metric graphs.
pub mod evaluator;
/// Forecast methods and time-series drivers.
pub mod forecast;
/// Public formula helper functions for identifier extraction and qualification.
pub mod formula;
/// Convenient re-exports for common statement types.
pub mod prelude;
/// Metric registry and namespace management.
pub mod registry;
/// Core statement model types.
pub mod types;
/// Internal utilities used within the crate implementation.
pub(crate) mod utils;

pub use error::{Error, Result};
pub use types::FinancialModelSpec;
