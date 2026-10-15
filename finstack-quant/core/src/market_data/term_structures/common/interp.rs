//! Shared defaults and calculations used by term-structure implementations.
//!
use crate::dates::{Date, DayCount, DayCountContext};
use crate::math::interp::types::Interp;
use crate::math::interp::{ExtrapolationPolicy, InterpStyle, ValidationPolicy};
use crate::Result;

/// Build an `Interp` with the requested style, extrapolation, and validation policy.
#[inline]
pub(crate) fn build_interp(
    style: InterpStyle,
    knots: Box<[f64]>,
    values: Box<[f64]>,
    extrapolation: ExtrapolationPolicy,
    validation: ValidationPolicy,
) -> Result<Interp> {
    style.build_enum(knots, values, extrapolation, validation)
}

/// Compute year fraction from a base date to a target date using the given day-count.
///
/// Returns `0.0` when `date == base` without invoking the day-count engine
/// (avoids edge-case issues for same-day lookups). This is the canonical
/// helper shared by all term-structure `*_on_date()` methods.
#[inline]
pub(crate) fn year_fraction_to(base: Date, date: Date, day_count: DayCount) -> Result<f64> {
    year_fraction_to_with_ctx(base, date, day_count, DayCountContext::default())
}

/// [`year_fraction_to`] for a curve whose time axis carries a holiday
/// calendar (Bus/252 discount curves).
#[inline]
pub(crate) fn year_fraction_to_with_ctx(
    base: Date,
    date: Date,
    day_count: DayCount,
    ctx: DayCountContext<'_>,
) -> Result<f64> {
    if date == base {
        Ok(0.0)
    } else {
        Ok(day_count.signed_year_fraction(base, date, ctx)?)
    }
}
