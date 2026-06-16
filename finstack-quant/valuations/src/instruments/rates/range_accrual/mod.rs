//! Range accrual notes with path-dependent coupon accrual.
//!
//! Range accrual notes pay coupons that accrue only when a reference rate
//! stays within a specified range. Popular for investors with views on
//! range-bound markets or mean reversion.
//!
//! # Structure
//!
//! - **Reference rate**: Typically LIBOR, SOFR, or other floating index
//! - **Range**: [Lower bound, Upper bound]
//! - **Observation frequency**: Daily, weekly, or business days
//! - **Accrual formula**: Coupon × (Days in range / Total days)
//!
//! # Pricing Method
//!
//! This module implements two pricing methods:
//! 1.  **Static Replication (Default)**: Replicates the payoff as a portfolio of digital options
//!     (binary call spreads). This method naturally captures volatility skew/smile and term structure
//!     from the volatility surface, making it the market standard for vanilla range accruals.
//! 2.  **Monte Carlo**: Used for complex path-dependent features or when explicitly requested.
//!     Supports Quanto drift adjustment and flat/term volatility.
//!
//! # Market Usage
//!
//! Common structures:
//! - **Digital range accruals**: Full coupon or zero (no proportional accrual)
//! - **Corridor notes**: Range defined by strike levels
//! - **Target redemption notes**: Early redemption when cumulative coupons hit target
//!
//! # See Also
//!
//! - [`RangeAccrual`] for instrument struct
//! - range accrual pricer module for Static Replication and MC implementations
//!

pub(crate) mod metrics;
pub mod monte_carlo;
pub(crate) mod pricer;
pub(crate) mod types;

pub use types::{BoundsType, RangeAccrual};
