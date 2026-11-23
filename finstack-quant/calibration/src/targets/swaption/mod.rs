//! Calibration target construction and shared input validation.
//!

mod target;
#[cfg(test)]
mod tests;

pub(crate) use target::{
    build_fixed_leg_periods, build_float_leg_periods, resolve_quote_leg_conventions,
    resolve_underlying_dates, solve,
};
