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
#![doc(test(attr(allow(clippy::expect_used))))]

//! Reusable analytical, numerical, credit, correlation, and stochastic models.
//!
//! Instrument definitions, market resolution, instrument-specific pricing
//! orchestration, metrics, and valuation results remain in
//! `finstack-quant-valuations`. Product-independent pricing workflows and
//! runtime defaults live beside their model engines here.
//!
//! # Module Organization
//!
//! - `closed_form`: Closed-form and semi-analytical pricing formulas (Black-Scholes Greeks,
//!   Asian, Barrier, Lookback, Quanto, Heston)
//! - `fourier`: Characteristic functions and COS-method pricing engines
//! - `factor`: Factor definitions, matching, covariance, sensitivity matrices,
//!   and hierarchical credit-factor calibration
//! - `rates`: Interest-rate models and dynamic term-structure engines
//! - `liquidity`: Liquidity risk estimators and market-impact models
//! - `volatility`: Volatility models (SABR) and Black-Scholes helper functions
//! - `trees`: Tree-based methods (Binomial, Trinomial, Multi-factor, Short-rate)
//! - `pde`: Finite difference PDE methods (1D Crank-Nicolson, 2D Craig-Sneyd ADI, Heston, American penalty)
//!
//! Credit copulas, recovery models, and factor models live in [`crate::correlation`].

extern crate self as finstack_quant_models;

pub mod closed_form;
pub mod correlation;
pub mod credit;
pub mod factor;
pub mod fourier;
pub mod liquidity;
pub mod monte_carlo;
pub mod pde;
pub mod rates;
pub mod trees;
pub mod types;
pub mod volatility;

pub use closed_form::{
    black76_implied_vol, bs_greeks, bs_implied_vol, bs_price, heston_call_price_fourier,
    heston_put_price_fourier, vanilla_expiry_payoff, BsGreeks, HestonPricingParams, ONE_PERCENT,
};
pub use trees::{
    short_rate_keys, single_factor_equity_state, state_keys, BinomialTree, EvolutionParams,
    HullWhiteTree, HullWhiteTreeConfig, NodeState, ShortRateTree, ShortRateTreeConfig,
    TreeCompounding, TreeGreeks, TreeModel, TreeValuator,
};
pub use types::{ExerciseStyle, OptionMarketParams, OptionType};
pub use volatility::{
    d1, d1_black76, d1_d2_black76, d2_black76, vega_weight, SabrCalibrator, SabrModel,
    SabrParameters, SabrShift, SabrSmile,
};

/// Compiles the crate `README.md` Rust samples as doctests.
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;
