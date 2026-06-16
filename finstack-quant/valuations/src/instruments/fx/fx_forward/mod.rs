//! FX forward instrument module.
//!
//! This module provides the [`FxForward`] instrument for modeling outright
//! forward contracts on currency pairs. An FX forward is a single exchange
//! of currencies at a future date at a predetermined rate.
//!
//! # Overview
//!
//! FX forwards represent agreements to exchange one currency for another at
//! a future date at a rate agreed upon today. Unlike FX swaps which have
//! two legs (near and far), FX forwards have a single maturity date.
//!
//! # Pricing: Covered Interest Rate Parity (CIRP)
//!
//! Forward exchange rates are determined by no-arbitrage:
//!
//! ```text
//! F = S × DF_foreign(T) / DF_domestic(T)
//! ```
//!
//! where:
//! - S = spot FX rate (quote per base)
//! - DF_foreign(T) = discount factor in foreign (base) currency to maturity
//! - DF_domestic(T) = discount factor in domestic (quote) currency to maturity
//!
//! # Present Value
//!
//! For a long FX forward position (buying base currency):
//!
//! ```text
//! PV = notional × (F_market - F_contract) × DF_domestic(T)
//! ```
//!
//! # Market Usage
//!
//! - **Hedging**: Lock in future exchange rates for known cash flows
//! - **Speculation**: Take directional views on currency movements
//! - **Curve construction**: Calibrate cross-currency basis curves
//!
//! # Examples
//!
//! ```rust
//! use finstack_quant_valuations::instruments::fx::fx_forward::FxForward;
//! use finstack_quant_core::currency::Currency;
//!
//! // Create a EUR/USD forward
//! let forward = FxForward::example().unwrap();
//! assert_eq!(forward.base_currency, Currency::EUR);
//! assert_eq!(forward.quote_currency, Currency::USD);
//! ```
//!
//! # See Also
//!
//! - [`crate::instruments::fx::fx_swap::FxSwap`] for FX swap instruments
//! - [`crate::instruments::fx::fx_spot::FxSpot`] for spot FX positions
//! - [`crate::instruments::fx::fx_option::FxOption`] for FX options

mod types;

pub use types::FxForward;

/// Metrics submodule for FX forward risk measures.
pub(crate) mod metrics;
