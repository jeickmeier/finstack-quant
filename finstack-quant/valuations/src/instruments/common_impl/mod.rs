//! Common functionality shared across multiple instruments.
//!
//! This module contains utilities and types that are used
//! by multiple instrument implementations, including:
//! - Core instrument traits (Instrument)
//! - Common helper functions
//! - Shared data structures and enums

// Core instrument traits and metadata
pub(crate) mod traits;

// Unified dependency representation
pub(crate) mod dependencies;

/// Stable date constants for `example()` constructors. Defined once so all
/// instrument examples can rotate forward together.
pub(crate) mod example_constants {
    use finstack_quant_core::dates::Date;
    use time::macros::date;

    /// Far-future expiry used by long-dated examples (FX options, equity
    /// options, etc.). Currently `2030-06-21`. When this approaches the
    /// present, bump to the next round date and regenerate any docs that
    /// pin numeric outputs against examples.
    pub const FAR_EXPIRY: Date = date!(2030 - 06 - 21);
}

// Shared utilities and helper functions
pub(crate) mod helpers;
pub(crate) mod numeric;
/// Re-export core's serde guard under the historical internal path.
pub(crate) mod serde_guard {
    pub(crate) use finstack_quant_core::serde_guard::UnknownFieldGuard;
}
// Shared volatility override/surface resolution.
pub(crate) mod two_clock;
pub(crate) mod validation;
pub(crate) mod vol_resolution;

// Common parameter types shared across instruments
pub(crate) mod fx_dates;
pub(crate) mod parameters;

// Common pricing patterns and infrastructure
pub(crate) mod pricing;

// Enriched per-flow cashflow export with DF/SP/PV columns.
pub mod cashflow_export;

// Re-export pricer helper used by instrument pricer modules.
#[doc(hidden)]
pub(crate) use pricing::GenericInstrumentPricer;
