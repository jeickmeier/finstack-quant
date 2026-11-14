//! Internal helpers shared by one-dimensional term-structure builders:
//! additive rate-shock adjustments and roll-forward state.

mod adjustment;
mod roll;

pub(crate) use adjustment::{PiecewiseLinearAdjustment, RateShock};
pub(crate) use roll::{InterpRollState, MIN_ROLLED_KNOT_GAP, roll_knots, validate_pillars};
