use crate::dates::{Date, DayCount, DayCountContext};
use crate::math::interp::types::Interp;
use crate::math::interp::{ExtrapolationPolicy, InterpStyle, ValidationPolicy};
use crate::Result;

/// Shared default base date for term-structure builders.
#[inline]
pub(crate) fn default_curve_base_date() -> Date {
    // Epoch date - unwrap_or provides defensive fallback for an effectively infallible operation.
    Date::from_calendar_date(1970, time::Month::January, 1).unwrap_or(time::Date::MIN)
}

/// Build an `Interp` with unified error mapping (crate::Result) for callers
/// whose builders return `crate::Result<T>` (Forward/Inflation).
///
/// Preserves the original interpolation error context for better diagnostics.
#[inline]
pub(crate) fn build_interp(
    style: InterpStyle,
    knots: Box<[f64]>,
    values: Box<[f64]>,
    extrapolation: ExtrapolationPolicy,
) -> Result<Interp> {
    style.build_enum(knots, values, extrapolation, ValidationPolicy::Strict)
}

/// Build an `Interp` allowing any values (including negative forward rates).
///
/// This is used by forward curves where negative rates are allowed
/// (e.g., EUR, CHF, JPY markets since 2014).
///
/// Preserves the original interpolation error context for better diagnostics.
#[inline]
pub(crate) fn build_interp_allow_any_values(
    style: InterpStyle,
    knots: Box<[f64]>,
    values: Box<[f64]>,
    extrapolation: ExtrapolationPolicy,
) -> Result<Interp> {
    style.build_enum(
        knots,
        values,
        extrapolation,
        ValidationPolicy::AllowNegative,
    )
}

/// Build an `Interp` mapping errors to `InputError` for discount curve builders.
#[inline]
pub(crate) fn build_interp_input_error(
    style: InterpStyle,
    knots: Box<[f64]>,
    values: Box<[f64]>,
    extrapolation: ExtrapolationPolicy,
    skip_validation: bool,
) -> crate::Result<Interp> {
    // Preserve the original interpolation error (usually an InputError).
    let validation = if skip_validation {
        // Allow domain-specific builders (e.g., discount curves) to defer value validation
        // to downstream helpers while still validating knot shape upstream.
        ValidationPolicy::AllowNegative
    } else {
        ValidationPolicy::Strict
    };
    style.build_enum(knots, values, extrapolation, validation)
}

/// Compute year fraction from a base date to a target date using the given day-count.
///
/// Returns `0.0` when `date == base` without invoking the day-count engine
/// (avoids edge-case issues for same-day lookups). This is the canonical
/// helper shared by all term-structure `*_on_date()` methods.
#[inline]
pub(crate) fn year_fraction_to(base: Date, date: Date, day_count: DayCount) -> Result<f64> {
    if date == base {
        Ok(0.0)
    } else {
        Ok(day_count.year_fraction(base, date, DayCountContext::default())?)
    }
}

/// Convenience to split points (t, v) into separate vectors.
#[inline]
pub(crate) fn split_points(points: Vec<(f64, f64)>) -> (Vec<f64>, Vec<f64>) {
    points.into_iter().unzip()
}
