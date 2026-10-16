//! Internal helpers shared by one-dimensional term-structure builders.
//!
//! The module keeps curve builders small by extracting common logic for
//! interpolation setup and knot splitting. Hazard curves do not rely on the
//! interpolation engine and only reuse the knot helpers.

mod adjustment;
mod conventions;
mod interp;
mod knot_ops;

pub(crate) use adjustment::PiecewiseLinearAdjustment;
pub(crate) use conventions::infer_forward_curve_defaults;
pub(crate) use interp::{year_fraction_to, year_fraction_to_with_ctx};
pub(crate) use knot_ops::{
    bump_knots_parallel, bump_knots_percentage, bump_knots_triangular, infer_spot_from_knots,
    roll_knots, triangular_weight, validate_non_negative_knots, validate_triangular_bucket_grid,
    validate_unit_range,
};
