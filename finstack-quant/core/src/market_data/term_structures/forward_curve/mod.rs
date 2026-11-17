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

use super::common::CurveCalendar;
use crate::dates::{Date, DayCount, Tenor};
use crate::math::interp::{ExtrapolationPolicy, Interp, InterpStyle};
use crate::types::CurveId;

/// Positive normalized factor curve for projecting floating-rate index fixings.
///
/// Stores strictly positive projection factors `(t_i, P_x(t_i))` anchored at
/// `(0.0, 1.0)` on an explicit [`DayCount`] clock (plus resolved holiday calendar
/// when using `Bus/252`), optional index tenor metadata ([`Tenor`]), an explicit
/// interpolation style, and an explicit extrapolation policy.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(try_from = "RawForwardCurve", into = "RawForwardCurve")]
pub struct ForwardCurve {
    pub(crate) id: CurveId,
    pub(crate) base: Date,
    /// Day-count basis mapping dates to projection factor times.
    pub(crate) day_count: DayCount,
    /// Business-day calendar of the time axis; required by Bus/252.
    pub(crate) calendar: Option<CurveCalendar>,
    /// Optional index accrual tenor metadata associated with this projection source.
    pub(crate) index_tenor: Option<Tenor>,
    /// Knot times in **years**.
    pub(crate) knots: Box<[f64]>,
    /// Normalized positive projection factors (unitless, `P_x(0) = 1.0`).
    pub(crate) factors: Box<[f64]>,
    pub(crate) interp: Interp,
    /// Exact accumulated conditioning and continuous zero-rate shock evaluation.
    transform: Option<evaluation::CurveTransform>,
    /// Exact typed recipe used to replay calibration after quote shocks.
    pub(crate) rate_calibration: Option<super::RateCalibrationRecipe>,
    /// Opaque FX policy stamp when bootstrap used cross-currency assumptions.
    pub(crate) fx_policy: Option<String>,
}

mod builder;
mod curve;
mod evaluation;
#[cfg(test)]
mod tests;
mod transform;
mod wire;

pub use builder::ForwardCurveBuilder;
use wire::RawForwardCurve;
