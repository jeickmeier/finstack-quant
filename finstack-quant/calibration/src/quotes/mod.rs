//! Market quote schemas and definitions.
//!
//! This module provides stable, serializable schemas for market quotes across all instrument
//! types. Quotes are designed for calibration workflows and include identifiers, pillars,
//! and market values (rates, spreads, prices, volatilities).
//!
//! # Features
//!
//! - **Stable serde names**: All quote types use strict field names for long-lived pipelines
//! - **Type-safe identifiers**: `QuoteId` and convention IDs prevent mismatches
//! - **Pillar support**: Quotes support both tenor-based and date-based maturity pillars
//!
//! # Quick Example
//!
//! ```rust
//! use finstack_quant_calibration::quotes::{Pillar, QuoteId};
//! use finstack_quant_calibration::quotes::RateQuote;
//! use finstack_quant_core::types::IndexId;
//!
//! # fn example() -> finstack_quant_core::Result<()> {
//! let quote = RateQuote::Deposit {
//!     id: QuoteId::new("USD-SOFR-DEP-1M"),
//!     index: IndexId::new("USD-SOFR-1M"),
//!     pillar: Pillar::Tenor("1M".parse()?),
//!     rate: 0.0525,
//! };
//!
//! // Bump the rate by 1 basis point
//! let bumped = quote.bump_rate_bp(1.0);
//! # Ok(())
//! # }
//! ```
//!
//! # See Also
//!
//! - [`crate::quotes::MarketQuote`] for the unified quote enum
//! - [`crate::quotes::QuoteId`] for quote identifiers
//! - [`crate::quotes::Pillar`] for maturity pillars

/// Credit quotes (CDS).
mod cds;
/// CDS tranche quotes.
mod cds_tranche;
/// Stable identifiers (QuoteId, Pillar).
mod ids;
/// Inflation quotes.
mod inflation;
/// Unified MarketQuote enum.
mod market_quote;
/// Interest rate quotes (Deposit, FRA, Futures, Swap).
mod rates;
/// Volatility quotes.
mod vol;
/// Cross-currency swap quotes.
mod xccy;

pub use cds::CdsQuote;
pub use cds_tranche::CdsTrancheQuote;
pub use ids::{Pillar, QuoteId};
pub use inflation::InflationQuote;
pub use rates::RateQuote;
pub use vol::VolQuote;
pub use xccy::XccyQuote;

pub(crate) use market_quote::ExtractQuotes;
pub use market_quote::MarketQuote;
