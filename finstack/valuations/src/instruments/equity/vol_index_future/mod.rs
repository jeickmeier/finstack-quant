//! Volatility index futures for VIX, VXN, VSTOXX, and similar indices.
//!
//! Volatility index futures are exchange-traded contracts on expected future
//! volatility levels. The most liquid market is VIX futures traded on CBOE,
//! which allow market participants to hedge or speculate on equity volatility.
//!
//! # Contract Types
//!
//! - **VIX futures**: Monthly and weekly expiries on CBOE VIX (S&P 500 vol)
//! - **Mini VIX**: Smaller contract size (1/10th of standard)
//! - **VXN futures**: NASDAQ-100 volatility
//! - **VSTOXX futures**: EURO STOXX 50 volatility
//!
//! # Pricing
//!
//! VIX futures are marked relative to the forward volatility curve:
//! ```text
//! NPV = (Forward_Vol - Quoted_Price) × Multiplier × Contracts × Position_Sign
//! ```
//! (a long gains when the forward mark rises above the entry price; the MTM
//! is undiscounted because the position is daily margined).
//!
//! Unlike equity or commodity futures, VIX futures:
//! - Do not require cost-of-carry adjustments
//! - Need no convexity adjustment **when the vol index curve is built from
//!   directly-quoted futures/forward vol levels** (see
//!   [`types`](self) module docs for the variance-derived caveat)
//! - Are directly linked to the volatility term structure
//!
//! # Term Structure
//!
//! VIX futures typically exhibit:
//! - **Contango**: Forward > Spot (normal conditions, roll cost for long positions)
//! - **Backwardation**: Forward < Spot (during volatility spikes)
//!
//! # Risk Metrics
//!
//! Key metrics for VIX futures:
//! - **DeltaVol**: Sensitivity to parallel shift in vol index curve
//! - **Theta**: Time decay (primarily from roll-down in contango)
//! - **Basis**: Difference between futures and spot VIX
//!
//! # References
//!
//! - CBOE (2019). "VIX Futures Contract Specifications."
//! - Alexander, C. (2008). *Pricing, Hedging and Trading Financial Instruments*.
//!   Chapter 12: Volatility indices and derivatives.
//! - Whaley, R. E. (2009). "Understanding the VIX." *Journal of Portfolio Management*.
//!
//! # See Also
//!
//! - [`VolatilityIndexFuture`] for instrument struct
//! - [`VolIndexContractSpecs`] for contract specifications
//! - [`crate::instruments::equity::vol_index_option`] for VIX options

pub(crate) mod metrics;
pub(crate) mod pricer;
mod types;

pub use types::{VolIndexContractSpecs, VolatilityIndexFuture};

// Builder provided by FinancialBuilder derive
