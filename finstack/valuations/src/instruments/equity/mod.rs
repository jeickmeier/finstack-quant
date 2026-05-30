//! Equity instruments and equity derivatives.
//!
//! This module provides equity-linked instruments spanning vanilla options,
//! structured products, volatility derivatives, and private markets. Pricing
//! models include Black-Scholes, Monte Carlo, and binomial trees.
//!
//! # Features
//!
//! - **Vanilla Options**: European calls/puts with analytical Greeks
//! - **Volatility Products**: Variance swaps, VIX futures/options
//! - **Structured Products**: Autocallables, cliquets, equity-linked notes
//! - **Total Return Swaps**: Equity TRS with financing legs
//! - **Private Markets**: PE funds, real estate with waterfall distributions
//! - **Corporate Valuation**: DCF models with terminal value
//!
//! # Pricing Models
//!
//! | Instrument | Models Available |
//! |------------|------------------|
//! | Vanilla Options | Black-Scholes, Heston Fourier, Monte Carlo |
//! | Autocallables | Monte Carlo with early exercise |
//! | Variance Swaps | Replication via log contract |
//! | Equity TRS | Discounted cashflows |
//! | Private Markets | DCF with distribution waterfall |
//!
//! # Quick Example
//!
//! ```rust
//! use finstack_valuations::instruments::equity::EquityOptionMarketData;
//! use finstack_valuations::instruments::EquityOption;
//! use finstack_core::currency::Currency;
//! use finstack_core::money::Money;
//! use finstack_core::types::CurveId;
//! use time::macros::date;
//!
//! // Create a 6-month ATM call option
//! let market_data = EquityOptionMarketData::new(
//!     CurveId::new("USD-OIS"),
//!     "EQUITY-SPOT",
//!     CurveId::new("EQUITY-VOL"),
//! )
//! .with_dividend_yield(CurveId::new("EQUITY-DIVYIELD"));
//! let option = EquityOption::european_call_with_market_data(
//!     "SPX-CALL-4500",
//!     "SPX",
//!     4500.0,
//!     date!(2025 - 07 - 15),
//!     Money::new(100.0, Currency::USD),
//!     market_data,
//! )
//! .expect("valid option");
//! ```
//!
//! # Greeks
//!
//! Equity options support full analytical Greeks:
//! - **Delta**: ∂V/∂S (spot sensitivity)
//! - **Gamma**: ∂²V/∂S² (delta convexity)
//! - **Vega**: ∂V/∂σ (volatility sensitivity)
//! - **Theta**: ∂V/∂t (time decay)
//! - **Rho**: ∂V/∂r (rate sensitivity)
//!
//! # References
//!
//! - Black, F., & Scholes, M. (1973). "The Pricing of Options and Corporate Liabilities."
//! - Heston, S. L. (1993). "A Closed-Form Solution for Options with Stochastic Volatility."
//!
//! # See Also
//!
//! - [`EquityOption`] for vanilla options
//! - [`VarianceSwap`] for variance/volatility swaps
//! - [`Autocallable`] for structured notes
//! - [`PrivateMarketsFund`] for PE/credit fund valuation

/// Autocallable module - Autocallable structured notes.
pub mod autocallable;
/// Cliquet option module - Cliquet/ratchet options.
pub mod cliquet_option;
/// DCF equity module - Discounted cash flow for equity (renamed from dcf).
pub mod dcf_equity;
/// Equity index future module.
pub mod equity_index_future;
/// Equity option module - Vanilla equity options.
pub mod equity_option;
/// Equity TRS module - Equity total return swaps.
pub mod equity_trs;
/// PE fund module - Private equity/markets funds (renamed from private_markets_fund).
pub mod pe_fund;
/// Shared piecewise-constant GBM process and forward-vol/rate bootstrap for the
/// equity path-dependent MC pricers (cliquet, autocallable).
pub(crate) mod piecewise_gbm;
/// Real estate module - Real estate asset valuation.
pub mod real_estate;
/// Equity spot module - Equity spot positions.
pub mod spot;
/// Variance swap module - Variance and volatility swaps.
pub mod variance_swap;
/// Volatility index future module.
pub mod vol_index_future;
/// Volatility index option module.
pub mod vol_index_option;

// Re-export primary types
pub use autocallable::{Autocallable, FinalPayoffType};
pub use cliquet_option::CliquetOption;
pub use dcf_equity::{DiscountedCashFlow, TerminalValueSpec};
pub use equity_index_future::{EquityFutureSpecs, EquityIndexFuture};
pub use equity_option::{EquityOption, EquityOptionMarketData};
pub use equity_trs::EquityTotalReturnSwap;
pub use pe_fund::PrivateMarketsFund;
pub use real_estate::{LeveredRealEstateEquity, RealEstateAsset, RealEstateValuationMethod};
pub use spot::Equity;
pub use variance_swap::VarianceSwap;
pub use vol_index_future::{VolIndexContractSpecs, VolatilityIndexFuture};
pub use vol_index_option::{VolIndexOptionSpecs, VolatilityIndexOption};
