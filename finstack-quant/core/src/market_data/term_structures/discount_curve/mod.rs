//! Discount factor curves for present value calculations.
//!
//! A discount curve represents the time value of money, mapping curve times and
//! calendar dates to finite strictly positive discount factors `D(t) > 0`.
//!
//! # Examples
//! ```rust
//! use finstack_quant_core::dates::{Date, DayCount};
//! use finstack_quant_core::market_data::term_structures::DiscountCurve;
//! use finstack_quant_core::math::interp::{ExtrapolationPolicy, InterpStyle};
//! use time::Month;
//!
//! let curve = DiscountCurve::builder("USD-OIS")
//!     .base_date(Date::from_calendar_date(2025, Month::January, 1).expect("Valid date"))
//!     .day_count(DayCount::Act365F)
//!     .knots([(0.0, 1.0), (5.0, 0.9)])
//!     .interp(InterpStyle::MonotoneConvex)
//!     .extrapolation(ExtrapolationPolicy::Extend)
//!     .build()
//!     .expect("DiscountCurve builder should succeed");
//! assert!(curve.df(3.0).expect("valid df") < 1.0);
//! ```

pub(crate) mod builder;
pub(crate) mod curve;
#[cfg(test)]
mod tests;
mod traits;
mod transform;
mod wire;

pub(crate) use builder::DiscountCurveBuilder;
pub(crate) use curve::DiscountCurve;
