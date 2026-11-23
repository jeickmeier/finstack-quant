//! Factor-native forward projection curves for floating-rate indices.
//!
//! A [`ForwardCurve`] represents a normalized positive projection factor curve
//! `P_x(t) > 0` with `P_x(0) = 1.0`, sharing the positive-factor mathematical
//! foundation with [`DiscountCurve`](super::DiscountCurve) while keeping
//! projection sources explicitly distinct from funding/discount curves (it does
//! not implement [`Discounting`](crate::market_data::traits::Discounting)).
//!
//! # Simple Rate Projection
//!
//! Over any accrual period `[S, E]` with contractual index year fraction
//! `alpha > 0`:
//! ```text
//! L_x(S, E; alpha) = (P_x(S) / P_x(E) - 1) / alpha
//! ```
//! evaluated via [`ForwardCurve::rate_between`] and
//! [`ForwardCurve::rate_between_dates`].
//!
//! # Examples
//!
//! ```rust
//! use finstack_quant_core::dates::{Date, DayCount, Tenor};
//! use finstack_quant_core::market_data::term_structures::ForwardCurve;
//! use finstack_quant_core::math::interp::{ExtrapolationPolicy, InterpStyle};
//! use time::Month;
//!
//! let base = Date::from_calendar_date(2025, Month::January, 1).expect("Valid date");
//! let fc = ForwardCurve::builder("USD-SOFR-3M")
//!     .base_date(base)
//!     .day_count(DayCount::Act360)
//!     .index_tenor(Tenor::quarterly())
//!     .knots([(0.0, 1.0), (5.0, (-0.04_f64 * 5.0).exp())])
//!     .interp(InterpStyle::LogLinear)
//!     .extrapolation(ExtrapolationPolicy::Extend)
//!     .build()
//!     .expect("ForwardCurve builder should succeed");
//! assert!(fc.rate_between(0.0, 0.25, 0.25).expect("valid rate") > 0.0);
//! ```

pub(crate) mod builder;
pub(crate) mod curve;
#[cfg(test)]
mod tests;
mod transform;
mod wire;

pub(crate) use builder::ForwardCurveBuilder;
pub(crate) use curve::ForwardCurve;
