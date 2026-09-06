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

//! Financial primitives and date utilities for the Finstack Quant workspace.
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
//! # Quick start
//! ```
//! use finstack_quant_core::currency::Currency;
//! use finstack_quant_core::money::Money;
//! # fn main() -> finstack_quant_core::Result<()> {
//!
//! // Parse ISO-4217 codes (case-insensitive)
//! let eur = "eur"
//!     .parse::<Currency>()
//!     .expect("valid ISO-4217 currency");
//!
//! // Perform arithmetic that refuses to mix currencies
//! let subtotal = Money::new(49.50, Currency::EUR).expect("valid money fixture");
//! let tax = Money::new(9.90, Currency::EUR).expect("valid money fixture");
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
//! - [`canonical`]: Deterministic JSON bytes and content hashes
//! - [`contract`]: Persisted-contract descriptors, limits, and diagnostics
//! - [`math`]: Numerical utilities and interpolation
//! - [`expr`]: Expression engine for formula evaluation
//! - [`explain`]: Computation tracing and debugging
//! - [`error`]: Error types and result handling
//! - [`decimal`]: `f64 ↔ Decimal` conversion helpers with explicit error propagation
//! - [`rating_scales`]: Shared credit rating-scale registry
//! - [`serde_guard`]: `deny_unknown_fields` enforcement for `#[serde(flatten)]` structs
//! - [`table`]: Serializable columnar table envelope for host-language bindings
//! - [`validation`]: Generic invariant-checking helpers
//!
//! For most users, importing `use finstack_quant_core::prelude::*;` provides
//! all commonly needed types.
//!
//! # Cargo features
//!
//! - `json-schema` (default): `schemars` derives and the `schema` generation module.
//! - `ts_export`: `ts_rs::TS` derives on contract diagnostics types.
//!
//! Serde is always enabled. WASM builds this crate with `default-features = false`.
//!
//! # Minimum Supported Rust Version (MSRV)
//! This crate targets **Rust 1.90**.  It is tested in CI and follows the
//! standard *cargo-semver* guideline: MSRV may only bump in a **minor** release.
//!
//! # References
//!
//! Canonical sources: `docs/REFERENCES.md`.
//!
//! - Day-count and business-day conventions: `docs/REFERENCES.md#isda-2006-definitions`
//! - Bond-market accrued-interest conventions: `docs/REFERENCES.md#icma-rule-book`
//! - Discounting and curve construction: `docs/REFERENCES.md#andersen-piterbarg-interest-rate-modeling`
//! - Interpolation: `docs/REFERENCES.md#hagan-west-monotone-convex`

// `collections` stays crate-private; import `HashMap`/`HashSet` from the crate root.
/// Deterministic JSON canonicalization and content hashing.
pub mod canonical;
/// Foundational cashflow primitives and discounting helpers.
pub mod cashflow;
pub(crate) mod collections;
/// Caller-supplied configuration and rounding policy.
pub mod config;
/// Persisted-contract descriptors, loading limits, and diagnostics.
pub mod contract;
/// Currency types and ISO-4217 definitions.
pub mod currency;
/// Date & calendar helpers (facade over the `time` crate)
pub mod dates;
/// Decimal conversion utilities (`f64 ↔ Decimal`) with explicit error propagation.
pub mod decimal;

/// Shared loader for embedded JSON registries with config override support.
pub mod embedded_registry;
/// Error types for finstack-quant-core.
pub mod error;
/// Explainability infrastructure for computation tracing.
pub mod explain;
/// Expression engine (AST, planning, and evaluation).
pub mod expr;
/// Market data curves, surfaces, scalars, and context storage.
pub mod market_data;
/// Numerical helpers (root finding, summation, stats)
pub mod math;
/// Currency-tagged monetary amounts with safe arithmetic
pub mod money;
/// Convenient re-exports of commonly used types
pub mod prelude;
/// Shared credit rating-scale registry.
pub mod rating_scales;
/// Deterministic JSON Schema assembly helpers.
#[cfg(feature = "json-schema")]
pub mod schema;
/// `deny_unknown_fields` enforcement for structs that use `#[serde(flatten)]`.
pub mod serde_guard;
/// Serializable columnar table envelope for host-language bindings.
pub mod table;
/// Core type definitions (phantom-typed IDs, rates, etc.)
pub mod types;
/// Generic validation helpers for checking invariants.
pub mod validation;
/// Canonical serde representations used by generated JSON contracts.
pub mod wire;

/// Hash map type alias used across Finstack.
///
/// Uses `rustc_hash::FxHashMap` for fast deterministic hashing.
pub use collections::HashMap;
/// Hash set type alias used across Finstack.
///
/// Uses `rustc_hash::FxHashSet` for fast deterministic hashing.
pub use collections::HashSet;

pub use canonical::{
    canonical_bytes_of_value, content_hash, to_canonical_bytes, CANONICAL_VERSION,
};
pub use contract::{
    ContractDescriptor, ContractError, Diagnostic, LoadLimits, LoadPhase, Severity,
    ValidationReport, DEFAULT_MAX_ARTIFACTS, DEFAULT_MAX_BYTES, DEFAULT_MAX_DEPTH,
    DEFAULT_MAX_DIAGNOSTICS, DEFAULT_MAX_POSITIONS,
};

pub use error::{Error, InputError, NonFiniteKind, Result};

/// Compiles the crate `README.md` Rust samples as doctests.
///
/// The README is *not* included in the rendered crate documentation — this
/// item exists only under `cfg(doctest)` so that every ` ```rust ` block in the
/// README is compiled and run by `cargo test --doc`. Without it those samples
/// are dead text and rot silently on any API change.
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;
