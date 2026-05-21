//! Commodity forward/futures instrument module.
//!
//! This module provides the [`CommodityForward`] instrument for modeling
//! forward contracts on physical commodities (energy, metals, agricultural).
//!
//! # Overview
//!
//! Commodity forwards represent agreements to buy or sell a specified quantity
//! of a commodity at a predetermined price on a future date. They can be:
//!
//! - **Physically settled**: Actual delivery of the commodity
//! - **Cash settled**: Payment of the price difference at settlement
//!
//! # Pricing
//!
//! Forward pricing uses the cost-of-carry model:
//! ```text
//! F(T) = S × exp((r - y + u) × T)
//! ```
//! where:
//! - S = Spot price
//! - r = Risk-free rate
//! - y = Convenience yield
//! - u = Storage costs
//! - T = Time to settlement
//!
//! # Examples
//!
//! ```rust
//! use finstack_valuations::instruments::commodity::commodity_forward::CommodityForward;
//! use finstack_core::currency::Currency;
//!
//! // Create a WTI crude oil forward
//! let forward = CommodityForward::example();
//! assert_eq!(forward.underlying.ticker, "CL");
//! ```

mod types;

pub use types::CommodityForward;
pub use types::Position;
pub use types::SettlementType;

/// Metrics submodule for commodity forward risk measures.
pub(crate) mod metrics;
