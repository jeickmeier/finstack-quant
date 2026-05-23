//! Core instrument traits and metadata infrastructure.
//!
//! Provides the fundamental [`Instrument`] trait that all financial instruments
//! implement, along with [`Attributes`] for tagging, selection, and scenario filtering.
//!
//! # Key Types
//!
//! - [`Instrument`]: Unified trait combining identity, attributes, and pricing methods
//! - [`Attributes`]: Tag-based metadata for categorization and scenario selection
//!
//! # Examples
//!
//! ## Basic Instrument Usage
//!
//! ```rust
//! use finstack_valuations::instruments::Bond;
//! use finstack_valuations::instruments::Instrument;
//! use finstack_core::currency::Currency;
//! use finstack_core::money::Money;
//! use finstack_core::dates::create_date;
//! use finstack_core::types::Rate;
//! use time::Month;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let issue = create_date(2025, Month::January, 15)?;
//! let maturity = create_date(2030, Month::January, 15)?;
//! let bond = Bond::fixed(
//!     "BOND-001",
//!     Money::new(1_000_000.0, Currency::USD),
//!     Rate::from_percent(5.0),
//!     issue,
//!     maturity,
//!     "USD-OIS"
//! )?;
//!
//! // Instrument trait methods
//! assert_eq!(bond.id(), "BOND-001");
//! # Ok(())
//! # }
//! ```
//!
//! ## Attributes and Selection
//!
//! ```rust
//! use finstack_valuations::instruments::Attributes;
//!
//! let attrs = Attributes::new()
//!     .with_tag("high-yield")
//!     .with_tag("energy")
//!     .with_meta("sector", "oil-gas")
//!     .with_meta("rating", "BB+");
//!
//! assert!(attrs.has_tag("high-yield"));
//! assert_eq!(attrs.get_meta("sector"), Some("oil-gas"));
//!
//! // Selector matching
//! assert!(attrs.matches_selector("tag:energy"));
//! assert!(attrs.matches_selector("meta:rating=BB+"));
//! assert!(attrs.matches_selector("*")); // Matches all
//! ```

mod curve_dependencies;
mod equity_dependencies;
mod instrument;
#[macro_use]
mod macros;
mod option_greeks;
mod pricing_options;

// Re-export all public items to preserve the existing API surface.
pub use curve_dependencies::{
    CurveDependencies, InstrumentCurves, InstrumentCurvesBuilder, RatesCurveKind,
};
pub use equity_dependencies::{
    EquityDependencies, EquityInstrumentDeps, EquityInstrumentDepsBuilder,
};
pub use instrument::Instrument;
pub use option_greeks::{OptionGreekKind, OptionGreeks, OptionGreeksProvider, OptionGreeksRequest};
pub use pricing_options::{CurveIdVec, DynInstrument, PricingOptions};

/// Metadata for instrument categorization, tagging, and scenario selection.
///
/// Attributes provide a flexible tagging system for organizing instruments,
/// applying scenarios, and filtering portfolios. Tags are simple strings for
/// broad categorization, while metadata key-value pairs store structured information.
///
/// # Tag-Based Selection
///
/// Tags enable coarse-grained filtering:
/// - Asset class: "equity", "fixed-income", "credit"
/// - Risk profile: "high-yield", "investment-grade"
/// - Sector: "technology", "financials", "energy"
/// - Custom: Any domain-specific categories
///
/// # Metadata Pairs
///
/// Key-value metadata stores structured attributes:
/// - Credit ratings: `("rating", "AA+")`
/// - Geographic region: `("region", "north-america")`
/// - Counterparty: `("counterparty", "JPMORGAN")`
/// - Desk/book: `("desk", "rates-trading")`
///
/// # Selector Patterns
///
/// Attributes support pattern-based selection for scenarios:
/// - `"*"`: Matches all instruments
/// - `"tag:high-yield"`: Matches instruments with the "high-yield" tag
/// - `"meta:sector=technology"`: Matches instruments with sector metadata
///
/// # Examples
///
/// ```rust
/// use finstack_valuations::instruments::Attributes;
///
/// let mut attrs = Attributes::new()
///     .with_tag("corporate")
///     .with_tag("investment-grade")
///     .with_meta("issuer", "AAPL")
///     .with_meta("rating", "AA+");
///
/// // Check tags
/// assert!(attrs.has_tag("corporate"));
/// assert!(!attrs.has_tag("high-yield"));
///
/// // Access metadata
/// assert_eq!(attrs.get_meta("issuer"), Some("AAPL"));
/// assert_eq!(attrs.get_meta("rating"), Some("AA+"));
///
/// // Pattern matching
/// assert!(attrs.matches_selector("tag:corporate"));
/// assert!(attrs.matches_selector("meta:issuer=AAPL"));
/// assert!(!attrs.matches_selector("tag:high-yield"));
/// ```
pub use finstack_core::types::Attributes;
