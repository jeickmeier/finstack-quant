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

//! Financial primitives and date utilities for the Finstack workspace.
//!
//! This crate exposes reusable building blocks used by pricing, risk, scenario,
//! portfolio, and binding crates:
//!
//! * [`currency::Currency`] – ISO-4217 codes with numeric identifiers and metadata
//! * [`money::Money`] – type-safe monetary amounts that refuse to mix currencies
//! * [`dates`] – date/time scaffolding (business calendars, day-count, schedules)
//! * [`market_data`] – curves, surfaces, scalars, and [`market_data::MarketContext`]
//! * [`math`] – interpolation, solvers, integration, statistics, and random numbers
//!
//! This crate uses the Rust standard library.
//!
//! # Quick start
//! ```
//! use finstack_core::currency::Currency;
//! use finstack_core::money::Money;
//! # fn main() -> finstack_core::Result<()> {
//!
//! // Parse ISO-4217 codes (case-insensitive)
//! let eur = "eur"
//!     .parse::<Currency>()
//!     .expect("valid ISO-4217 currency");
//!
//! // Perform arithmetic that refuses to mix currencies
//! let subtotal = Money::new(49.50, Currency::EUR);
//! let tax      = Money::new( 9.90, Currency::EUR);
//! let total    = subtotal.checked_add(tax)?;
//! assert_eq!(format!("{}", total), "EUR 59.40");
//! # Ok(())
//! # }
//! ```
//!
//! # API surface
//!
//! - [`currency`]: Currency types and ISO-4217 codes
//! - [`mod@money`]: Monetary amounts with currency safety
//! - [`dates`]: Date handling, calendars, and schedules
//! - [`market_data`]: Term structures and market data containers
//! - [`config`]: Configuration and global settings
//! - [`types`]: Core type definitions (IDs, rates, etc.)
//! - [`prelude`]: Convenient re-exports of commonly used types
//! - [`cashflow`]: Cashflow primitives and discounting
//! - [`credit`]: Credit risk primitives (migration models, generator extraction, CTMC simulation)
//! - [`math`]: Numerical utilities and interpolation
//! - [`expr`]: Expression engine for formula evaluation
//! - [`explain`]: Computation tracing and debugging
//! - [`error`]: Error types and result handling
//! - [`decimal`]: `f64 ↔ Decimal` conversion helpers with explicit error propagation
//! - [`rating_scales`]: Shared credit rating-scale registry
//! - [`table`]: Serializable columnar table envelope for host-language bindings
//! - [`validation`]: Generic invariant-checking helpers
//! - [`versions`]: Canonical model-version strings for calibration reports
//!
//! For most users, importing `use finstack_core::prelude::*;` provides
//! all commonly needed types.
//!
//! # Cargo features
//! Serde support is always enabled in this crate; no feature flags are required.
//!
//! # Minimum Supported Rust Version (MSRV)
//! This crate targets **Rust 1.90**.  It is tested in CI and follows the
//! standard *cargo-semver* guideline: MSRV may only bump in a **minor** release.

// Core modules
//
// API note: `collections` is intentionally kept as an internal module to avoid
// committing to a public submodule layout. Downstream crates should import the
// aliases directly from the crate root (`finstack_core::HashMap`).
/// Foundational cashflow primitives and discounting helpers.
pub mod cashflow;
pub(crate) mod collections;
/// Global configuration and environment settings.
pub mod config;
/// Credit risk modeling primitives (migration models, generator extraction, CTMC simulation).
pub mod credit;
/// Currency types and ISO-4217 definitions.
pub mod currency;
/// Date & calendar helpers (facade over the `time` crate)
pub mod dates;
/// Decimal conversion utilities (`f64 ↔ Decimal`) with explicit error propagation.
pub mod decimal;

/// Shared loader for embedded JSON registries with config override support.
///
/// Provides cached parsing and validation for versioned compile-time JSON assets.
pub(crate) mod embedded_registry;
/// Error types for finstack-core.
///
/// The crate uses a unified `Error` enum with specific variants for
/// different error categories (validation, market data, computation, etc.).
pub mod error;
/// Explainability infrastructure for computation tracing.
///
/// Provides opt-in tracing for debugging and auditing financial computations.
pub mod explain;
/// Expression engine (AST, planning, and evaluation).
///
/// Internal expression engine used by statements for formula evaluation and
/// time-series operations.
pub mod expr;
/// Market data curves, surfaces, scalars, and context storage.
pub mod market_data;
/// Numerical helpers (root finding, summation, stats)
pub mod math;
/// Currency-tagged monetary amounts with safe arithmetic
pub mod money;
/// Label normalization for human-entered identifiers.
pub(crate) mod parse;
/// Convenient re-exports of commonly used types
pub mod prelude;
/// Shared credit rating-scale registry.
pub mod rating_scales;
/// Serializable columnar table envelope for host-language bindings.
pub mod table;
/// Core type definitions (phantom-typed IDs, rates, etc.)
pub mod types;
/// Generic validation helpers for checking invariants.
pub mod validation;
/// Canonical model-version strings for calibration reports.
pub mod versions;

/// Hash map type alias used across Finstack.
///
/// Uses `rustc_hash::FxHashMap` for fast deterministic hashing.
pub use collections::HashMap;
/// Hash set type alias used across Finstack.
///
/// Uses `rustc_hash::FxHashSet` for fast deterministic hashing.
pub use collections::HashSet;

// Re-export main error types for convenience.
pub use error::{Error, InputError, NonFiniteKind, Result};
