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
//! The [`Instrument`] documentation implements the trait for a one-payment
//! note and prices it; the instrument families in the umbrella crate are the
//! full-scale implementations.

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

pub use macros::{impl_focused_pricing_overrides, impl_instrument_base};
