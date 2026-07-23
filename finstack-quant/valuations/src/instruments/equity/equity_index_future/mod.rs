//! Equity Index Future instrument module.
//!
//! This module provides the `EquityIndexFuture` instrument for pricing and risk
//! analysis of equity index futures such as E-mini S&P 500 (ES), E-mini Nasdaq-100 (NQ),
//! Euro Stoxx 50 (FESX), DAX (FDAX), FTSE 100 (Z), and Nikkei 225 (NK).
//!
//! # Overview
//!
//! Equity index futures are exchange-traded derivatives that allow market participants
//! to gain exposure to equity indices without owning the underlying stocks. They are
//! cash-settled contracts based on the value of the underlying index at expiration.
//!
//! # Pricing Modes
//!
//! The module supports two pricing modes:
//!
//! 1. **Mark-to-Market**: When a `quoted_price` is provided, the present value is
//!    calculated as the difference between the quoted price and entry price,
//!    scaled by contract terms.
//!
//! 2. **Fair Value**: When no quoted price is available, the cost-of-carry model
//!    is used: `F = S₀ × exp((r - q) × T)`
//!
//! # Supported Contracts
//!
//! | Contract | Exchange | Multiplier | Tick Size | Tick Value |
//! |----------|----------|------------|-----------|------------|
//! | ES (E-mini S&P 500) | CME | $50 | 0.25 | $12.50 |
//! | MES (Micro E-mini S&P) | CME | $5 | 0.25 | $1.25 |
//! | NQ (E-mini Nasdaq-100) | CME | $20 | 0.25 | $5.00 |
//! | FESX (Euro Stoxx 50) | Eurex | €10 | 1.0 | €10.00 |
//! | FDAX (DAX) | Eurex | €25 | 0.5 | €12.50 |
//! | Z (FTSE 100) | ICE | £10 | 0.5 | £5.00 |
//! | NK (Nikkei 225) | CME/OSE | ¥500 | 5.0 | ¥2,500 |
//!
//! # Examples
//!
//! ## Creating an E-mini S&P 500 Future
//!
//! ```rust
//! use finstack_quant_valuations::instruments::equity::equity_index_future::{
//!     EquityIndexFuture, EquityFutureSpecs,
//! };
//! use finstack_quant_valuations::instruments::Position;
//! use finstack_quant_core::currency::Currency;
//! use finstack_quant_core::dates::Date;
//! use finstack_quant_core::money::Money;
//! use finstack_quant_core::types::{CurveId, InstrumentId};
//! use time::Month;
//!
//! // Using the builder
//! let es_future = EquityIndexFuture::builder()
//!     .id(InstrumentId::new("ESH5"))
//!     .underlying_ticker("SPX".to_string())
//!     .notional(Money::new(2_250_000.0, Currency::USD))
//!     .expiry(Date::from_calendar_date(2025, Month::March, 21).unwrap())
//!     .last_trading_date(Date::from_calendar_date(2025, Month::March, 20).unwrap())
//!     .entry_price_opt(Some(4500.0))
//!     .quoted_price_opt(Some(4550.0))
//!     .position(Position::Long)
//!     .contract_specs(EquityFutureSpecs::sp500_emini())
//!     .discount_curve_id(CurveId::new("USD-OIS"))
//!     .spot_id("SPX-SPOT".into())
//!     .build()
//!     .expect("Valid future");
//!
//! // Using the convenience constructor
//! let es_future2 = EquityIndexFuture::sp500_emini(
//!     "ESH5",
//!     Money::new(2_250_000.0, Currency::USD),
//!     Date::from_calendar_date(2025, Month::March, 21).unwrap(),
//!     Date::from_calendar_date(2025, Month::March, 20).unwrap(),
//!     Some(4500.0),
//!     Position::Long,
//!     "USD-OIS",
//! ).expect("Valid future");
//! ```
//!
//! ## Calculating Delta
//!
//! ```rust
//! use finstack_quant_valuations::instruments::equity::equity_index_future::EquityIndexFuture;
//!
//! let future = EquityIndexFuture::example().unwrap();
//! let delta = future.delta().unwrap(); // errors if entry_price is unset
//! // For 10 long ES contracts: delta = 50 × 10 × 1 = 500
//! // This means $500 P&L per 1-point move in the futures price
//! assert_eq!(delta, 500.0);
//! ```
//!
//! # Market Data Requirements
//!
//! For mark-to-market pricing:
//! - Discount curve (for DV01 calculations)
//!
//! For fair value pricing:
//! - Discount curve (for risk-free rate and DV01)
//! - Spot index level (via `spot_id`)
//! - Optional: Dividend yield (via `div_yield_id`)
//!
//! # References
//!
//! - Hull, J. C. (2018). "Options, Futures, and Other Derivatives." Chapter 5.
//! - CME Group. "E-mini S&P 500 Futures Contract Specifications."
//! - Eurex. "EURO STOXX 50 Index Futures."

mod types;
pub use types::{EquityFutureSpecs, EquityIndexFuture};

pub(crate) mod pricer;

pub(crate) mod metrics;
