//! Exotic and path-dependent options.
//!
//! This module provides exotic option instruments whose payoffs depend on
//! the path of the underlying asset, not just its terminal value. These
//! instruments require either analytical approximations or Monte Carlo
//! simulation for pricing.
//!
//! # Features
//!
//! - **Asian Options**: Payoff based on average price over option life
//! - **Barrier Options**: Knock-in/knock-out based on barrier crossing
//! - **Lookback Options**: Payoff based on path extremum (max or min)
//! - **Basket Options**: Multi-asset options on weighted basket
//!
//! # Pricing Models
//!
//! | Option Type | Analytical | Monte Carlo |
//! |-------------|------------|-------------|
//! | Asian (Geometric) | Kemna-Vorst (exact) | ✓ |
//! | Asian (Arithmetic) | Turnbull-Wakeman (approx) | ✓ |
//! | Barrier (Continuous) | Reiner-Rubinstein | ✓ |
//! | Barrier (Discrete) | Broadie-Glasserman correction | ✓ |
//! | Lookback | Conze-Viswanathan | ✓ |
//! | Basket | — | ✓ (correlation) |
//!
//! # Monte Carlo Requirements
//!
//! Path-dependent exotics use the workspace Monte Carlo infrastructure for simulation-based
//! pricing. Analytical formulas are available for some exotic types when applicable.
//!
//! # Quick Example
//!
//! ```rust
//! use finstack_quant_valuations::instruments::exotics::{AsianOption, AveragingMethod};
//!
//! // Use the example Asian option (arithmetic average call)
//! let asian = AsianOption::example().unwrap();
//! ```
//!
//! Or build a custom option:
//!
//! ```ignore
//! use finstack_quant_valuations::instruments::exotics::{AsianOption, AveragingMethod};
//! use finstack_quant_valuations::instruments::OptionType;
//! use finstack_quant_core::currency::Currency;
//! use finstack_quant_core::dates::DayCount;
//! use finstack_quant_core::money::Money;
//! use finstack_quant_core::types::{CurveId, InstrumentId};
//! use time::macros::date;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let fixing_dates = vec![
//!     date!(2025-01-31), date!(2025-02-28), date!(2025-03-31),
//!     date!(2025-04-30), date!(2025-05-31), date!(2025-06-30),
//! ];
//!
//! let asian = AsianOption::builder()
//!     .id(InstrumentId::new("ASIAN-SPX"))
//!     .underlying_ticker("SPX".to_string())
//!     .strike(4500.0)
//!     .option_type(OptionType::Call)
//!     .averaging_method(AveragingMethod::Arithmetic)
//!     .expiry(date!(2025-06-30))
//!     .fixing_dates(fixing_dates)
//!     .notional(Money::new(100_000.0, Currency::USD))
//!     .day_count(DayCount::Act365F)
//!     .discount_curve_id(CurveId::new("USD-OIS"))
//!     .spot_id("SPX-SPOT".to_string().into())
//!     .vol_surface_id(CurveId::new("SPX-VOL"))
//!     .build()?;
//! # Ok(()) }
//! ```
//!
//! # Academic References
//!
//! - Kemna, A. G. Z., & Vorst, A. C. F. (1990). "A Pricing Method for Options
//!   Based on Average Asset Values." *Journal of Banking & Finance*.
//! - Turnbull, S. M., & Wakeman, L. M. (1991). "A Quick Algorithm for Pricing
//!   European Average Options."
//! - Reiner, E., & Rubinstein, M. (1991). "Breaking Down the Barriers."
//! - Conze, A., & Viswanathan (1991). "Path Dependent Options: The Case of
//!   Lookback Options."
//!
//! # See Also
//!
//! - [`AsianOption`] for average price options
//! - [`BarrierOption`] for knock-in/knock-out options
//! - [`LookbackOption`] for path extremum options
//! - [`Basket`] for multi-asset options
//! - [`crate::models::closed_form`] for analytical formulas

/// Asian option module - Average price/strike options.
pub mod asian_option;
/// Barrier option module - Knock-in/knock-out options.
pub mod barrier_option;
/// Basket module - Multi-underlying basket instruments.
pub mod basket;
/// Lookback option module - Path-dependent lookback options.
pub mod lookback_option;

// Re-export primary types
pub use asian_option::{AsianOption, AveragingMethod};
pub use barrier_option::{BarrierOption, BarrierType};
pub use basket::Basket;
pub use lookback_option::{LookbackOption, LookbackType};
