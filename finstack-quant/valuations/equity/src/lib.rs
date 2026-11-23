#![forbid(unsafe_code)]
#![warn(clippy::float_cmp)]
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![deny(clippy::unreachable)]
#![cfg_attr(
    test,
    allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::unreachable,
        clippy::indexing_slicing,
        clippy::float_cmp,
    )
)]
// Allow expect() in doc tests (they are test code)
#![doc(test(attr(allow(clippy::expect_used))))]

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
//! use finstack_quant_valuations_equity::equity_option::{EquityOptionMarketData, EquityOptionParams};
//! use finstack_quant_valuations_equity::equity_option::EquityOption;
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
//! - [`EquityOption`](crate::equity_option::EquityOption) for vanilla options
//! - [`VarianceSwap`](crate::variance_swap::VarianceSwap) for variance/volatility swaps
//! - [`Autocallable`](crate::autocallable::Autocallable) for structured notes
//! - [`PrivateMarketsFund`](crate::private_markets_fund::PrivateMarketsFund) for PE/credit fund valuation

/// Autocallable module - Autocallable structured notes.
pub mod autocallable;
/// Cliquet option module - Cliquet/ratchet options.
pub mod cliquet_option;
/// DCF equity module - Discounted cash flow for equity.
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
/// Shared Monte Carlo discretization defaults.
mod mc_defaults;
/// Observed-fixing lookup and validation shared by the seasoned instruments.
pub(crate) mod past_fixings;
/// Shared piecewise-constant GBM process and forward-vol/rate bootstrap for the
/// equity path-dependent MC pricers (cliquet, autocallable).
pub(crate) mod piecewise_gbm;
/// PE fund module - Private equity/markets funds.
pub mod private_markets_fund;
/// Real estate asset valuation (unlevered DCF and direct capitalization).
pub mod real_estate;
#[doc(hidden)]
pub mod rf_component_dv01;
/// Equity spot module - Equity spot positions.
pub mod spot;
/// Spot-pinned finite-difference Greeks for products struck at inception spot.
pub(crate) mod spot_pinned;
/// Equity underlying asset parameters.
pub(crate) mod underlying;
/// Variance swap module - Variance and volatility swaps.
pub mod variance_swap;
#[doc(hidden)]
pub mod vega;
/// Volatility index future module.
pub mod vol_index_future;
/// Exchange-listed options on volatility-index futures.
pub mod vol_index_future_option;

pub(crate) use underlying::equity_vol_dependencies;
pub use underlying::{EquityPathModel, EquityUnderlyingParams};

mod family;

pub use family::EQUITY;

/// Compiles the crate `README.md` Rust samples as doctests.
///
/// The README is *not* included in the rendered crate documentation — this
/// item exists only under `cfg(doctest)` so that every ` ```rust ` block in the
/// README is compiled and run by `cargo test --doc`.
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;
