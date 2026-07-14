//! Interest rate caps and floors with lognormal, shifted-lognormal, or normal pricing.
//!
//! Interest rate caps and floors are portfolios of caplets/floorlets providing
//! protection against rising or falling interest rates. Widely used for hedging
//! floating-rate debt or managing interest rate exposure.
//!
//! # Cap and Floor Structures
//!
//! - **Cap**: Portfolio of caplets (call options on forward rates)
//!   - Pays max(Rate - Strike, 0) on each reset date
//!   - Protects against rising rates
//!
//! - **Floor**: Portfolio of floorlets (put options on forward rates)
//!   - Pays max(Strike - Rate, 0) on each reset date
//!   - Protects against falling rates
//!
//! - **Collar**: Long cap + short floor (or vice versa)
//!
//! # Pricing Models
//!
//! [`CapFloorVolType::Lognormal`] uses Black (1976),
//! [`CapFloorVolType::ShiftedLognormal`] applies Black to `F + shift` and
//! `K + shift`, and [`CapFloorVolType::Normal`] uses Bachelier. Normal pricing
//! supports zero and negative forwards.
//!
//! For Black (1976), each caplet/floorlet is priced as:
//!
//! **Caplet (Call on forward rate):**
//! ```text
//! Caplet = N · τ · DF(T) · [F · N(d₁) - K · N(d₂)]
//! ```
//!
//! **Floorlet (Put on forward rate):**
//! ```text
//! Floorlet = N · τ · DF(T) · [K · N(-d₂) - F · N(-d₁)]
//! ```
//!
//! where:
//! ```text
//! d₁ = [ln(F/K) + 0.5σ²T] / (σ√T)
//! d₂ = d₁ - σ√T
//! ```
//!
//! and:
//! - N = notional
//! - τ = accrual fraction (day count)
//! - DF(T) = discount factor to payment date
//! - F = forward rate for the period
//! - K = strike rate (cap/floor rate)
//! - σ = implied volatility
//! - T = time to option expiration
//!
//! Compounded overnight-RFR coupons may specify lookback/observation shift,
//! rate cutoff, and a business-day payment delay. Daily compounding uses
//! business-day-adjusted accrual boundaries, and discounting runs to the
//! delayed payment date rather than the contractual accrual end.
//!
//! # Market Conventions
//!
//! Standard cap/floor conventions by currency:
//!
//! - **USD SOFR**: ACT/360, Quarterly or Semi-annual
//! - **EUR EURIBOR**: ACT/360, Quarterly or Semi-annual
//! - **GBP SONIA**: ACT/365, Quarterly or Semi-annual
//!
//! # References
//!
//! - Black, F. (1976). "The Pricing of Commodity Contracts." *Journal of
//!   Financial Economics*, 3(1-2), 167-179.
//!   (Black model for options on forwards/futures)
//!
//! - Rebonato, R. (2004). *Volatility and Correlation: The Perfect Hedger and
//!   the Fox* (2nd ed.). Wiley.
//!   (Market practice for caps/floors)
//!
//! - Brigo, D., & Mercurio, F. (2006). *Interest Rate Models - Theory and Practice*
//!   (2nd ed.). Springer. Chapters 1-2.
//!
//! # Examples
//!
//! See [`CapFloor`] for construction and usage examples.
//!
//! # See Also
//!
//! - [`CapFloor`] for cap/floor instrument struct
//! - [`RateOptionType`] for cap vs floor distinction
//! - cap/floor metrics module for risk metrics (DV01, vega)

pub(crate) mod hw_pricer;
pub(crate) mod metrics;
pub(crate) mod parameters;
pub(crate) mod pricing;
mod types;

pub use parameters::CapFloorParams;
pub use types::{
    CapFloor, CapFloorVolType, OvernightCouponConvention, OvernightSpreadCompounding,
    RateOptionType,
};
