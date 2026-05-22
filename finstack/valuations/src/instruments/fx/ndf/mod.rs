//! Non-Deliverable Forward (NDF) instrument module.
//!
//! This module provides the [`Ndf`] instrument for modeling non-deliverable
//! forward contracts on restricted currency pairs.
//!
//! # Overview
//!
//! NDFs are cash-settled forward contracts used in markets where foreign
//! exchange restrictions prevent physical delivery of the non-convertible
//! currency. Settlement occurs in a freely convertible currency (typically USD)
//! based on the difference between the contracted rate and a fixing rate.
//!
//! # Common NDF Markets
//!
//! - **CNY (Chinese Yuan)**: CNHFIX - RMB fixing by CFETS
//! - **INR (Indian Rupee)**: RBI reference rate
//! - **BRL (Brazilian Real)**: PTAX rate
//! - **KRW (Korean Won)**: KFTC fixing
//! - **TWD (Taiwan Dollar)**: TAIFX fixing
//!
//! # Quote Conventions
//!
//! NDFs support two quote conventions via [`crate::instruments::fx::ndf::NdfQuoteConvention`]:
//!
//! ## BasePerSettlement (default)
//!
//! Rate quoted as units of base (restricted) currency per one unit of settlement currency.
//! This is the standard convention for most Asian NDF markets.
//!
//! Example: USD/CNY = 7.25 means 7.25 CNY per 1 USD.
//!
//! ```text
//! Settlement = Notional_base × (1/F_contract - 1/F_fixing)
//! ```
//!
//! ## SettlementPerBase
//!
//! Rate quoted as units of settlement currency per one unit of base currency.
//!
//! Example: CNY/USD = 0.138 means 0.138 USD per 1 CNY.
//!
//! ```text
//! Settlement = Notional_base × (F_fixing - F_contract)
//! ```
//!
//! # Pricing
//!
//! ## Pre-Fixing (before fixing date)
//!
//! Forward rate is estimated via covered interest rate parity when foreign curve
//! is available, otherwise falls back to spot rate.
//!
//! ## Post-Fixing (after fixing date, before settlement)
//!
//! Uses the observed fixing rate for settlement calculation.
//!
//! **Note:** If valuation is past the fixing date but no fixing rate is set,
//! the pricer returns an error. Use `with_fixing_rate()` to set the observed rate.
//!
//! # Fixing Conventions
//!
//! - Fixing typically occurs T-2 before settlement (T-1 for KRW, PHP)
//! - Fixing source determines the official rate used
//! - Once fixed, the NDF becomes a simple cash flow
//!
//! Use [`crate::instruments::fx::ndf::NdfFixingSource`] for type-safe fixing source specification:
//!
//! | Currency | Fixing Source | Enum Variant |
//! |----------|---------------|--------------|
//! | CNY | PBOC | `NdfFixingSource::Pboc` |
//! | CNH | CNHFIX | `NdfFixingSource::Cnhfix` |
//! | INR | RBI | `NdfFixingSource::Rbi` |
//! | KRW | KFTC | `NdfFixingSource::Kftc` |
//! | BRL | PTAX | `NdfFixingSource::Ptax` |
//!
//! # Examples
//!
//! ```rust
//! use finstack_valuations::instruments::fx::ndf::{Ndf, NdfQuoteConvention};
//! use finstack_core::currency::Currency;
//!
//! // Create a USD/CNY NDF with BasePerSettlement convention
//! let ndf = Ndf::example();
//! assert_eq!(ndf.base_currency, Currency::CNY);
//! assert_eq!(ndf.settlement_currency, Currency::USD);
//! assert_eq!(ndf.quote_convention, NdfQuoteConvention::BasePerSettlement);
//! ```
//!
//! # See Also
//!
//! - [`crate::instruments::fx::fx_forward::FxForward`] for deliverable forwards
//! - [`crate::instruments::fx::fx_swap::FxSwap`] for FX swap instruments

mod types;

pub use types::{Ndf, NdfFixingSource, NdfQuoteConvention};

/// Metrics submodule for NDF risk measures.
pub(crate) mod metrics;
