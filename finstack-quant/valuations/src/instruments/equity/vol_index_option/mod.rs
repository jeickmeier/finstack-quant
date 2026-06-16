//! Volatility index options for VIX, VXN, VSTOXX, and similar indices.
//!
//! Volatility index options are European-style, cash-settled options on
//! volatility indices. The most liquid market is VIX options on CBOE.
//!
//! # Pricing Model
//!
//! VIX options use the Black model (not Black-Scholes) because:
//! 1. The underlying is the VIX forward, not spot
//! 2. VIX has no cost of carry or dividends
//! 3. The forward price is directly observable from futures
//!
//! The key input is the "vol-of-vol" - the volatility of the volatility index.
//! This is typically quoted in a 2D surface indexed by expiry and strike.
//!
//! # Contract Types
//!
//! - **VIX options**: Monthly expiries on CBOE VIX
//! - **VSTOXX options**: Options on EURO STOXX 50 volatility
//! - **Weekly VIX options**: Short-dated weekly expiries
//!
//! # Risk Metrics
//!
//! Key Greeks for VIX options:
//! - **Delta**: Sensitivity to vol index forward level
//! - **Gamma**: Second derivative w.r.t. forward level
//! - **Vega (Vol-of-Vol)**: Sensitivity to vol-of-vol
//! - **Theta**: Time decay
//!
//! # Term Structure Effects
//!
//! VIX options exhibit unique characteristics:
//! - **Contango/Backwardation**: Forward term structure affects option value
//! - **Vol-of-vol smile**: Strike-dependent implied vol
//! - **Mean reversion**: Long-dated options reflect vol mean reversion
//!
//! # References
//!
//! - Carr, P., & Lee, R. (2009). "Volatility Derivatives."
//! - CBOE (2019). "VIX Options Contract Specifications."
//!
//! # See Also
//!
//! - [`VolatilityIndexOption`] for instrument struct
//! - [`VolIndexOptionSpecs`] for contract specifications
//! - [`crate::instruments::equity::vol_index_future`] for VIX futures

pub(crate) mod metrics;
pub(crate) mod pricer;
mod types;

pub use types::{VolIndexOptionSpecs, VolatilityIndexOption};
