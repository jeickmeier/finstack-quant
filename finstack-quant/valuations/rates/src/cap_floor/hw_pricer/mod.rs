//! Hull-White 1-factor closed-form pricer for interest rate caps and floors.
//!
//! Prices a cap/floor as a sum of caplet/floorlet values. Standard term-index
//! caplets use the exact HW1F zero-coupon-bond option equivalence. Options on
//! compounded overnight coupons use the explicitly documented moment-matched
//! normal approximation below.
//!
//! # Algorithm
//!
//! For each caplet/floorlet period `[T_start, T_end]`:
//!
//! 1. Term-index caplets are transformed into options on `P(T,S)` and priced
//!    exactly under HW1F.
//! 2. Compounded-RFR coupons are projected from every contractual overnight
//!    factor. Their normal volatility is moment-matched from each factor's
//!    product derivative, affine bond loading, and pairwise OU state covariance.
//! 3. The cap/floor value is the sum of all caplet/floorlet values.
//!
//! # References
//!
//! - Hull, J. & White, A. (1990). "Pricing Interest-Rate-Derivative Securities."
//!   *Review of Financial Studies*, 3(4), 573-592. `docs/REFERENCES.md#hull-white-1990-pricing-ird`
//! - Brigo, D. & Mercurio, F. (2006). *Interest Rate Models - Theory and Practice*,
//!   Chapter 3: One-factor Short-Rate Models, Section 3.3.2 (Gaussian forward-rate
//!   dynamics underpinning the closed-form caplet normal volatility). `docs/REFERENCES.md#brigo-mercurio-2006-interest-rate-models`

mod pricer;
#[cfg(test)]
mod tests;

pub(crate) use pricer::{CapFloorHullWhitePricer, resolve_capfloor_hw1f_model_params};
