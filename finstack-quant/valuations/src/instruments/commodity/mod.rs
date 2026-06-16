//! Commodity derivatives: forwards, swaps, and options.
//!
//! This module provides commodity derivative instruments for energy, metals,
//! and agricultural products. Pricing uses forward curves with cost-of-carry
//! adjustments for physical settlement considerations.
//!
//! # Features
//!
//! - **Forwards/Futures**: Physical and cash-settled commodity forwards
//! - **Swaps**: Fixed-for-floating commodity price swaps
//! - **Options**: European options on commodity forwards
//!
//! # Commodity Markets
//!
//! Supports major commodity classes:
//! - **Energy**: Crude oil (WTI, Brent), natural gas, refined products
//! - **Metals**: Gold, silver, copper, aluminum
//! - **Agriculture**: Corn, wheat, soybeans
//!
//! # Pricing Framework
//!
//! Commodity instruments are priced using:
//! - **Forward curves**: Term structure of forward prices
//! - **Convenience yield**: Storage cost and availability premium
//! - **Black (1976)**: Standard model for commodity options
//!
//! # Quick Example
//!
//! ```rust
//! use finstack_quant_valuations::instruments::commodity::CommodityForward;
//! use finstack_quant_valuations::instruments::CommodityUnderlyingParams;
//! use finstack_quant_core::currency::Currency;
//! use finstack_quant_core::types::{CurveId, InstrumentId};
//! use time::macros::date;
//!
//! let forward = CommodityForward::builder()
//!     .id(InstrumentId::new("WTI-DEC25"))
//!     .underlying(CommodityUnderlyingParams::new("Energy", "CL", "BBL", Currency::USD))
//!     .quantity(1000.0)
//!     .multiplier(1.0)
//!     .maturity(date!(2025-12-15))
//!     .forward_curve_id(CurveId::new("CL-FORWARD"))
//!     .discount_curve_id(CurveId::new("USD-OIS"))
//!     .build()
//!     .expect("Valid forward");
//! ```
//!
//! # References
//!
//! - Black, F. (1976). "The Pricing of Commodity Contracts."
//! - Schwartz, E. S. (1997). "The Stochastic Behavior of Commodity Prices."
//!
//! # See Also
//!
//! - [`CommodityForward`] for forwards and futures
//! - [`CommoditySwap`] for fixed-float swaps
//! - [`CommodityOption`] for commodity options

/// Shared business-day averaging helpers for floating legs.
pub(crate) mod averaging;
/// Commodity Asian option module.
pub mod commodity_asian_option;
/// Commodity forward module.
pub mod commodity_forward;
/// Commodity option module.
pub mod commodity_option;
/// Commodity spread option module.
pub mod commodity_spread_option;
/// Commodity swap module.
pub mod commodity_swap;
/// Commodity swaption module.
pub mod commodity_swaption;

// Re-export primary types
pub use commodity_asian_option::CommodityAsianOption;
pub use commodity_forward::CommodityForward;
pub use commodity_option::CommodityOption;
pub use commodity_spread_option::CommoditySpreadOption;
pub use commodity_swap::CommoditySwap;
pub use commodity_swaption::CommoditySwaption;
