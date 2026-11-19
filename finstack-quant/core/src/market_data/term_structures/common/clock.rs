//! Shared clock validation and positive-factor log-ratio math for curves.

use crate::dates::{Date, DayCount, HolidayCalendar, calendar_by_id_strict, canonical_calendar_id};
pub(crate) use crate::math::log_ratio::{checked_log_cross_ratio, stable_positive_log_ratio};

/// Registered holiday calendar that completes a business-day time axis.
///
/// A Bus/252 curve counts business days between its base date and a query
/// date; the calendar is part of the curve, not of the caller's input.
#[derive(Clone)]
pub(crate) struct CurveCalendar {
    /// Canonical registry identifier, persisted on the wire.
    pub(crate) id: String,
    /// Resolved registry calendar.
    pub(crate) calendar: &'static dyn HolidayCalendar,
}

impl CurveCalendar {
    /// Resolve a registered calendar id (including `+`-joined unions).
    pub(crate) fn resolve(id: &str) -> crate::Result<Self> {
        Ok(Self {
            calendar: calendar_by_id_strict(id)?,
            id: canonical_calendar_id(id)?,
        })
    }
}

impl std::fmt::Debug for CurveCalendar {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("CurveCalendar").field(&self.id).finish()
    }
}

/// Validate that a curve time axis uses a complete supported day count and resolved calendar.
pub(crate) fn validate_curve_clock(
    curve_label: &str,
    curve_id: &str,
    day_count: DayCount,
    calendar_id: Option<&str>,
) -> crate::Result<Option<CurveCalendar>> {
    day_count.validate_annualized_clock()?;
    if matches!(
        day_count,
        DayCount::ActActIsma | DayCount::Act365L | DayCount::ThirtyE360Isda
    ) {
        return Err(crate::Error::Validation(format!(
            "{curve_label} '{curve_id}' cannot use day count {day_count} because curve clocks do not store coupon or termination period metadata"
        )));
    }
    let calendar = calendar_id.map(CurveCalendar::resolve).transpose()?;
    if day_count == DayCount::Bus252 && calendar.is_none() {
        return Err(crate::Error::Validation(format!(
            "Bus/252 {curve_label} '{curve_id}' requires a holiday calendar"
        )));
    }
    Ok(calendar)
}

/// Compute signed year fraction using an optional resolved curve calendar.
#[inline]
pub(crate) fn signed_curve_year_fraction(
    base: Date,
    target: Date,
    day_count: DayCount,
    calendar: Option<&CurveCalendar>,
) -> crate::Result<f64> {
    let ctx = crate::dates::DayCountContext {
        calendar: calendar.map(|cal| cal.calendar),
        ..crate::dates::DayCountContext::default()
    };
    day_count.signed_year_fraction(base, target, ctx)
}
