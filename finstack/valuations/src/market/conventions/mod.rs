//! Market convention registries and definitions.
//!
//! This module provides market convention definitions and a global registry for looking up
//! conventions by identifier. Conventions define day count, business day adjustments, payment
//! frequencies, and other market-standard parameters required for instrument construction.
//!
//! # Features
//!
//! - **Convention definitions**: Structured types for all convention categories
//! - **Stable identifiers**: Type-safe IDs for convention lookups
//! - **Global registry**: Singleton registry loaded from embedded JSON data
//! - **Strict validation**: Missing conventions cause explicit errors
//!
//! # Quick Example
//!
//! ```rust
//! use finstack_valuations::market::conventions::ConventionRegistry;
//! use finstack_core::types::IndexId;
//!
//! let registry = ConventionRegistry::try_global()?;
//! let conv = registry.require_rate_index(&IndexId::new("USD-SOFR-OIS"))?;
//! assert_eq!(conv.currency, finstack_core::currency::Currency::USD);
//! # Ok::<(), finstack_core::Error>(())
//! ```
//!
//! # See Also
//!
//! - `ConventionRegistry` for convention lookups
//! - convention `ids` module for convention identifiers

/// Data structures for conventions.
pub(crate) mod defs;
/// Stable identifiers (typed keys).
pub mod ids;

/// Convention loaders and parsers.
pub(crate) mod loaders;
/// Registry for looking up conventions.
pub(crate) mod registry;

// Re-export convention types for external use
pub use defs::{
    BondConventions, CdsConventions, FxConventions, FxOptionConventions, InflationSwapConventions,
    IrFutureConventions, OptionConventions, RateIndexConventions, RateIndexKind,
    SwaptionConventions, XccyConventions,
};
pub use registry::ConventionRegistry;
