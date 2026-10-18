//! Internal helpers shared by one-dimensional term-structure builders.
//!
//! The module keeps curve builders small by extracting common logic for
//! interpolation setup and knot splitting. Hazard curves do not rely on the
//! interpolation engine and only reuse the knot helpers.

mod adjustment;
mod interp;
mod knot_ops;
mod roll;

pub(crate) use adjustment::{PiecewiseLinearAdjustment, RateShock};
pub(crate) use interp::{year_fraction_to, year_fraction_to_with_ctx};
pub(crate) use knot_ops::{roll_knots, triangular_weight, validate_unit_range};
pub(crate) use roll::{validate_pillars, InterpRollState};
