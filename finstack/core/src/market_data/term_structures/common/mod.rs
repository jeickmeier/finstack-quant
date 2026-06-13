//! Internal helpers shared by one-dimensional term-structure builders.
//!
//! The module keeps curve builders small by extracting common logic for
//! interpolation setup and knot splitting. Hazard curves do not rely on the
//! interpolation engine and only reuse the knot helpers.

mod conventions;
mod interp;
mod knot_ops;

pub(crate) use conventions::{infer_discount_curve_day_count, infer_forward_curve_defaults};
pub(crate) use interp::{
    build_interp, build_interp_allow_any_values, build_interp_input_error, default_curve_base_date,
    split_points, year_fraction_to,
};
pub(crate) use knot_ops::{
    bump_knots_parallel, bump_knots_percentage, bump_knots_triangular, infer_spot_from_knots,
    roll_knots, triangular_weight, validate_non_negative_knots, validate_triangular_bucket_grid,
    validate_unit_range,
};
