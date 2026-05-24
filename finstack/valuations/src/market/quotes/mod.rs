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
//! - **Bump operations**: Quotes support bumping values for sensitivity calculations
//! - **TypeScript export**: Quotes can be exported to TypeScript when `ts_export` feature is enabled
//!
//! # Quick Example
//!
//! ```rust
//! use finstack_valuations::market::quotes::ids::{Pillar, QuoteId};
//! use finstack_valuations::market::quotes::rates::RateQuote;
//! use finstack_core::types::IndexId;
//!
//! # fn example() -> finstack_core::Result<()> {
//! let quote = RateQuote::Deposit {
//!     id: QuoteId::new("USD-SOFR-DEP-1M"),
//!     index: IndexId::new("USD-SOFR-1M"),
//!     pillar: Pillar::Tenor("1M".parse()?),
//!     rate: 0.0525,
//! };
//!
//! // Bump the rate by 1 basis point (0.0001 in decimal)
//! let bumped = quote.bump_rate_decimal(0.0001);
//! # Ok(())
//! # }
//! ```
//!
//! # See Also
//!
//! - [`crate::market::quotes::market_quote::MarketQuote`] for the unified quote enum
//! - [`crate::market::quotes::ids::QuoteId`] for quote identifiers
//! - [`crate::market::quotes::ids::Pillar`] for maturity pillars

/// Bond quotes.
pub mod bond;
/// Credit quotes (CDS).
pub mod cds;
/// CDS tranche quotes.
pub mod cds_tranche;
/// FX quotes.
pub mod fx;
/// Stable identifiers (QuoteId, Pillar).
pub mod ids;
/// Inflation quotes.
pub mod inflation;
/// Unified MarketQuote enum.
pub mod market_quote;
/// Interest rate quotes (Deposit, FRA, Futures, Swap).
pub mod rates;
/// Volatility quotes.
pub mod vol;
/// Cross-currency swap quotes.
pub mod xccy;
