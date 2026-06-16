//! Equity spot position instruments with market data integration.
//!
//! Represents spot equity positions (individual stocks, ETFs, indices) with
//! pricing from market data feeds and risk metric calculations including
//! dividend sensitivity.
//!
//! # Structure
//!
//! - **Ticker**: Symbol identifier (e.g., "AAPL", "SPY")
//! - **Shares**: Number of shares held
//! - **Price source**: Market data lookup or explicit quote
//! - **Dividend yield**: For forward pricing and metrics
//!
//! # Pricing
//!
//! Spot equity value:
//!
//! ```text
//! PV = Shares × Spot_Price
//! ```
//!
//! Forward price for derivatives:
//!
//! ```text
//! F = S × e^((r - q)T)
//! ```
//!
//! where q is the continuous dividend yield.
//!
//! # Market Data Integration
//!
//! Equity pricing requires:
//! - **Spot price**: From market data feed or explicit quote
//! - **Dividend yield**: Historical or implied from options
//! - **Discount curve**: For present value calculations
//!
//! # Key Metrics
//!
//! - **Price per share**: Current market price
//! - **Total value**: Shares × Price
//! - **Forward price**: Dividend-adjusted forward
//! - **Dividend yield**: Annualized yield
//!
//! # See Also
//!
//! - [`Equity`] for instrument struct
//! - [`String`] for symbol type
//! - [`crate::instruments::equity::equity_option`] for options on equities

pub(crate) mod metrics;
pub(crate) mod pricer;
mod types;

pub use pricer::EquityPricer;
pub use types::Equity;
