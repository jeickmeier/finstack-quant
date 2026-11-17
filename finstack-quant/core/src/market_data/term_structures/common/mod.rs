//! Internal helpers shared by one-dimensional term-structure builders:
//! curve clock validation, positive-factor log ratios, additive rate-shock
//! adjustments, and conditioning pillar validation.

mod adjustment;
mod clock;
pub(crate) mod factor;
mod roll;

pub(crate) use adjustment::{PiecewiseLinearAdjustment, RateShock};
pub(crate) use clock::{
    CurveCalendar, checked_log_cross_ratio, signed_curve_year_fraction, stable_positive_log_ratio,
    validate_curve_clock,
};
pub(crate) use factor::define_factor_curve_transform;
pub(crate) use roll::validate_pillars;
