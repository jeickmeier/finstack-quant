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
//! # Extension policy
//!
//! [`Instrument`] is a stable compatibility surface used behind
//! `Arc<dyn Instrument>` throughout portfolio and binding code. New optional
//! capabilities should therefore use focused provider traits, following
//! [`OptionGreeksProvider`], rather than adding more required methods to
//! `Instrument`. Removing existing `Instrument` methods is reserved for a
//! breaking release with a downstream migration plan.
//!
//! # Examples
//!
//! ## Basic Instrument Usage
//!
//! ```rust
//! use finstack_quant_valuations::instruments::Bond;
//! use finstack_quant_valuations::instruments::Instrument;
//! use finstack_quant_core::currency::Currency;
//! use finstack_quant_core::money::Money;
//! use finstack_quant_core::dates::create_date;
//! use finstack_quant_core::types::Rate;
//! use time::Month;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let issue = create_date(2025, Month::January, 15)?;
//! let maturity = create_date(2030, Month::January, 15)?;
//! let bond = Bond::fixed(
//!     "BOND-001",
//!     Money::from((1_000_000_i64, Currency::USD)),
//!     Rate::from_percent(5.0).expect("valid rate fixture"),
//!     issue,
//!     maturity,
//!     finstack_quant_core::dates::StubKind::None,
//!     "USD-OIS"
//! )?;
//!
//! // Instrument trait methods
//! assert_eq!(bond.id(), "BOND-001");
//! # Ok(())
//! # }
//! ```

mod instrument;
#[macro_use]
mod macros;
mod option_greeks;
mod pricing_options;

pub use instrument::Instrument;
pub use option_greeks::{
    GreekBumps, OptionGreekKind, OptionGreeks, OptionGreeksProvider, OptionGreeksRequest,
};
pub use pricing_options::PricingOptions;

pub use finstack_quant_core::types::Attributes;

pub(crate) use macros::{impl_focused_pricing_overrides, impl_instrument_base};
