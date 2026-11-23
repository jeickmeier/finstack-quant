//! Credit hazard rate curves for default probability modeling.
//!
//! A hazard curve represents the piecewise-constant default intensity `λ(t)`
//! on ending intervals `(tᵢ₋₁, tᵢ]` with `t₀ = 0` and constant tail intensity
//! `λₙ` beyond `tₙ`.
//!
//! # Examples
//!
//! ```rust
//! use finstack_quant_core::dates::{Date, DayCount};
//! use finstack_quant_core::market_data::term_structures::HazardCurve;
//! use time::Month;
//!
//! let base = Date::from_calendar_date(2025, Month::January, 1).expect("Valid date");
//! let hc = HazardCurve::builder("USD-CREDIT")
//!     .base_date(base)
//!     .day_count(DayCount::Act365F)
//!     .recovery_rate(0.40)
//!     .knots([(1.0, 0.01), (10.0, 0.015)])
//!     .build()
//!     .expect("HazardCurve builder should succeed");
//! assert!(hc.sp(5.0).expect("valid survival") < 1.0);
//! ```
//!
//! # References
//!
//! - O'Kane, D. (2008). *Modelling Single-name and Multi-name Credit Derivatives*.
//!   Wiley Finance. Chapters 3-5. `docs/REFERENCES.md#o-kane-2008`
//! - ISDA (2009). "ISDA CDS Standard Model." Version 1.8.2.
//!   `docs/REFERENCES.md#isda-cds-standard-model`

pub(crate) mod builder;
pub(crate) mod curve;
#[cfg(test)]
mod tests;
mod transform;
mod wire;

pub(crate) use builder::HazardCurveBuilder;
pub(super) use curve::HazardConditionSource;
pub(crate) use curve::{HazardCurve, Seniority};
