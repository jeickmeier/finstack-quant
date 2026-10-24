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
//! - [`PrivateMarketsFund`](crate::pe_fund::PrivateMarketsFund) for PE/credit fund valuation

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
/// Observed-fixing lookup and validation shared by the seasoned instruments.
pub(crate) mod past_fixings;
/// PE fund module - Private equity/markets funds.
pub mod pe_fund;
/// Shared piecewise-constant GBM process and forward-vol/rate bootstrap for the
/// equity path-dependent MC pricers (cliquet, autocallable).
pub(crate) mod piecewise_gbm;
/// Equity spot module - Equity spot positions.
pub mod spot;
/// Spot-pinned finite-difference Greeks for products struck at inception spot.
pub(crate) mod spot_pinned;
/// Variance swap module - Variance and volatility swaps.
pub mod variance_swap;
/// Volatility index future module.
pub mod vol_index_future;
/// Exchange-listed options on volatility-index futures.
pub mod vol_index_future_option;

mod family;

/// Market dependencies of a single-name equity vol product: discount curve,
/// spot scalar, volatility surface keyed on spot (and `strike` when the
/// surface lookup is strike-specific), and the optional dividend-yield scalar.
///
/// # Arguments
///
/// * `discount_curve_id` - Discount curve for the payoff currency.
/// * `spot_id` - Spot price scalar.
/// * `vol_surface_id` - Implied-volatility surface.
/// * `strike` - Absolute strike for the volatility lookup, or `None` for ATM-forward products.
/// * `div_yield_id` - Continuous dividend-yield scalar, when the product uses one.
pub(crate) fn equity_vol_dependencies(
    discount_curve_id: &finstack_quant_core::types::CurveId,
    spot_id: &finstack_quant_core::types::PriceId,
    vol_surface_id: &finstack_quant_core::types::CurveId,
    strike: Option<f64>,
    div_yield_id: Option<&finstack_quant_core::types::PriceId>,
) -> finstack_quant_valuations_core::instruments::MarketDependencies {
    let mut deps = finstack_quant_valuations_core::instruments::MarketDependencies::new();
    deps.add_discount_curve(discount_curve_id.clone());
    deps.add_market_scalar_id(spot_id.as_str());
    deps.add_volatility_dependency(
        finstack_quant_valuations_core::instruments::VolatilityDependency::new(
            vol_surface_id.clone(),
            Some(spot_id.clone()),
            strike,
        ),
    );
    if let Some(dividend_yield) = div_yield_id {
        deps.add_market_scalar_id(dividend_yield.as_str());
    }
    deps
}
pub use family::EQUITY;

/// Compiles the crate `README.md` Rust samples as doctests.
///
/// The README is *not* included in the rendered crate documentation — this
/// item exists only under `cfg(doctest)` so that every ` ```rust ` block in the
/// README is compiled and run by `cargo test --doc`.
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;
