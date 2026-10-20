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
//! use finstack_quant_valuations::instruments::equity::equity_option::{EquityOptionMarketData, EquityOptionParams};
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
//! .with_div_yield_id(PriceId::new("EQUITY-DIVYIELD"));
//! let option = EquityOption::european_with_market_data(
//!     "SPX-CALL-4500",
//!     "SPX",
//!     EquityOptionParams::call(4500.0, date!(2025 - 07 - 15), 100.0, Currency::USD),
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
//!
//! The family lives in `finstack-quant-valuations-equity` and is re-exported
//! here unchanged. `real_estate` stays in this crate: `RealEstateFinancing`
//! holds `Bond`, `TermLoan`, `RevolvingCredit` and `Repo` legs from other
//! families, so it registers with [`STANDARD`](crate::pricer::STANDARD) rather
//! than with [`EQUITY`](finstack_quant_valuations_equity::EQUITY).

pub use finstack_quant_valuations_equity::*;

/// Real estate assets and levered real-estate equity.
pub mod real_estate;
