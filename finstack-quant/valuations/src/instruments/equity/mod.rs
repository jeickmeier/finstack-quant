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
//! use finstack_quant_valuations::instruments::equity::EquityOptionMarketData;
//! use finstack_quant_valuations::instruments::EquityOption;
//! use finstack_quant_core::currency::Currency;
//! use finstack_quant_core::money::Money;
//! use finstack_quant_core::types::{CurveId, PriceId};
//! use time::macros::date;
//!
//! // Create a 6-month ATM call option
//! let market_data = EquityOptionMarketData::new(
//!     CurveId::new("USD-OIS"),
//!     "EQUITY-SPOT",
//!     CurveId::new("EQUITY-VOL"),
//! )
//! .with_dividend_yield(PriceId::new("EQUITY-DIVYIELD"));
//! let option = EquityOption::european_call_with_market_data(
//!     "SPX-CALL-4500",
//!     "SPX",
//!     4500.0,
//!     date!(2025 - 07 - 15),
//!     Money::from((100_i64, Currency::USD)),
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
//! - Black, F., & Scholes, M. (1973). "The Pricing of Options and Corporate Liabilities." `docs/REFERENCES.md#black-scholes-1973`
//! - Heston, S. L. (1993). "A Closed-Form Solution for Options with Stochastic Volatility." `docs/REFERENCES.md#heston-1993`
//!
//! # See Also
//!
//! - [`EquityOption`] for vanilla options
//! - [`VarianceSwap`] for variance/volatility swaps
//! - [`Autocallable`] for structured notes
//! - [`PrivateMarketsFund`] for PE/credit fund valuation

/// Explicit stochastic model policy for path-dependent equity structures.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum EquityPathModel {
    /// Piecewise GBM calibrated to the ATM-forward total-variance term
    /// structure. This model does not represent strike skew or smile and must
    /// be selected explicitly as an approximation.
    AtmTermGbm,
}

/// Autocallable module - Autocallable structured notes.
pub mod autocallable;
/// Cliquet option module - Cliquet/ratchet options.
pub mod cliquet_option;
/// DCF equity module - Discounted cash flow for equity (renamed from dcf).
pub mod dcf_equity;
pub(crate) mod dividend01;
/// Exchange-listed equity, equity-index, and fixed-currency quanto futures.
pub mod equity_future;
/// Exchange-listed options on equity futures.
pub mod equity_future_option;
/// Equity option module - Vanilla equity options.
pub mod equity_option;
/// Exchange-listed equity and equity-index total-return futures.
pub mod equity_total_return_future;
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
/// Exchange-listed options on volatility-index futures.
pub mod vol_index_future_option;

pub use autocallable::{Autocallable, FinalPayoffType};
pub use cliquet_option::CliquetOption;
pub use dcf_equity::{DiscountedCashFlow, TerminalValueSpec};
pub use equity_future::{EquityFuture, EquityFutureQuantoSpec};
pub use equity_future_option::EquityFutureOption;
pub use equity_option::{
    EquityOption, EquityOptionExercise, EquityOptionMarketData, ThetaDayBasis,
};
pub use equity_total_return_future::EquityTotalReturnFuture;
pub use equity_trs::{EquityTotalReturnSwap, TrsDividendSettlement};
pub use pe_fund::PrivateMarketsFund;
pub use real_estate::{
    LeveredRealEstateEquity, RealEstateAsset, RealEstateFinancing, RealEstateValuationMethod,
};
pub use spot::Equity;
pub use variance_swap::{EquityPriceSeriesPolicy, VarianceSwap};
pub use vol_index_future::{VolIndexContractSpecs, VolatilityIndexFuture};
pub use vol_index_future_option::VolatilityIndexFutureOption;
