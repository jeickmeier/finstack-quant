//! Shared clock validation and positive-factor log-ratio math for curves.

use crate::dates::{Date, DayCount, HolidayCalendar, calendar_by_id_strict, canonical_calendar_id};
use crate::math::stats;

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

#[inline]
fn frexp_f64(x: f64) -> (f64, i32) {
    let exp = stats::frexp_exp(x);
    (stats::ldexp(x, -exp), exp)
}

#[inline]
fn diff_of_products(a: f64, b: f64, c: f64, d: f64) -> f64 {
    let cd = c * d;
    let err = (-c).mul_add(d, cd);
    let dop = a.mul_add(b, -cd);
    dop + err
}

/// Evaluate `ln((a * b) / (c * d))` for strictly positive finite inputs with `frexp` scaling and FMA.
pub(crate) fn checked_log_cross_ratio(a: f64, b: f64, c: f64, d: f64) -> f64 {
    let (ma, ea) = frexp_f64(a);
    let (mb, eb) = frexp_f64(b);
    let (mc, ec) = frexp_f64(c);
    let (md, ed) = frexp_f64(d);
    let exp_diff = (ea + eb) - (ec + ed);

    if (-2..=2).contains(&exp_diff) {
        let a_scaled = stats::ldexp(ma, exp_diff);
        let num = a_scaled * mb;
        let den = mc * md;
        let ratio = num / den;
        if (0.5..=2.0).contains(&ratio) {
            let diff = diff_of_products(a_scaled, mb, mc, md);
            return (diff / den).ln_1p();
        }
    }

    ((ma * mb) / (mc * md)).ln() + f64::from(exp_diff) * std::f64::consts::LN_2
}

/// Evaluate `ln(num / den)` stably for finite strictly positive `num` and `den`.
pub(crate) fn stable_positive_log_ratio(num: f64, den: f64) -> crate::Result<f64> {
    if !num.is_finite() || num <= 0.0 || !den.is_finite() || den <= 0.0 {
        return Err(crate::Error::Validation(format!(
            "positive-factor log ratio requires finite positive values, got {num} / {den}"
        )));
    }
    if num.to_bits() == den.to_bits() {
        return Ok(0.0);
    }
    let ratio = checked_log_cross_ratio(num, 1.0, den, 1.0);
    if !ratio.is_finite() {
        return Err(crate::Error::Validation(format!(
            "positive-factor log ratio overflowed f64 for {num} / {den}"
        )));
    }
    Ok(if ratio == 0.0 { 0.0 } else { ratio })
}
