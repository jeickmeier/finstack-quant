//! Common functionality shared across multiple instruments.
//!
//! This module contains utilities and types that are used
//! by multiple instrument implementations, including:
//! - Core instrument traits (Instrument)
//! - Common helper functions
//! - Shared data structures and enums

// Core instrument traits and metadata
pub mod traits;

// Unified dependency representation
pub mod dependencies;

/// Stable date constants for `example()` constructors. Defined once so all
/// instrument examples can rotate forward together.
pub mod example_constants {
    use finstack_quant_core::dates::Date;
    use time::macros::date;

    /// Far-future expiry used by long-dated examples (FX options, equity
    /// options, etc.). Currently `2030-06-21`. When this approaches the
    /// present, bump to the next round date and regenerate any docs that
    /// pin numeric outputs against examples.
    pub const FAR_EXPIRY: Date = date!(2030 - 06 - 21);
}

// Shared utilities and helper functions
pub mod helpers;
pub mod listed;
pub mod margin;
// Shared volatility override/surface resolution.
pub mod vol_resolution;

// Common parameter types shared across instruments
pub mod parameters;

// Common pricing patterns and infrastructure
pub mod pricing;

// Enriched per-flow cashflow export with DF/SP/PV columns.

#[doc(hidden)]
pub use pricing::GenericInstrumentPricer;
