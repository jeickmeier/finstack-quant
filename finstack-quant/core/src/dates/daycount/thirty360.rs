//! 30/360 synthetic-day kernels behind [`super::DayCount`].
//!
//! Every variant counts `N = 360(Y₂-Y₁) + 30(M₂-M₁) + D₂' - D₁'` exactly in
//! `i64` (it cannot overflow over the supported `Date` range) and differs only
//! in the day adjustments. These are implementation details: the public
//! interval semantics (validation, equal dates, reversal) belong to
//! [`super::DayCount::year_fraction`] and
//! [`super::DayCount::signed_year_fraction`].

use time::{Date, Month};

use crate::dates::date_extensions::DateExt;

/// Day adjustments of the 30/360 conventions without a termination input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Thirty360Variant {
    /// Fixed US 30U/360 EOM: paired February month-end adjustment.
    UsEom,
    /// ISDA 2006 §4.16(f) bond basis: no February month-end rule.
    IsdaBondBasis,
    /// ISDA 2006 §4.16(g) 30E/360 (European).
    European,
    /// QuantLib `Thirty360::Italian`.
    Italian,
}

/// `N = 360(Y₂-Y₁) + 30(M₂-M₁) + (D₂' - D₁')` over the original years and
/// months of a forward interval.
fn synthetic_days(start: Date, end: Date, d1_adj: u8, d2_adj: u8) -> i64 {
    360 * (i64::from(end.year()) - i64::from(start.year()))
        + 30 * (i64::from(end.month() as u8) - i64::from(start.month() as u8))
        + (i64::from(d2_adj) - i64::from(d1_adj))
}

/// Synthetic day count of the forward interval `[start, end)` under `variant`.
pub(super) fn thirty_360_days(start: Date, end: Date, variant: Thirty360Variant) -> i64 {
    let (d1, d2) = (start.day(), end.day());
    let (d1_adj, d2_adj) = match variant {
        Thirty360Variant::UsEom => {
            // Day 31 or February month-end start becomes 30; day 31 end becomes
            // 30 after an adjusted-30 start; February month-end end becomes 30
            // when the start is also February month-end.
            let d1_adj = if d1 == 31 || is_last_day_of_february(start) {
                30
            } else {
                d1
            };
            let d2_adj = if (d2 == 31 && d1_adj == 30)
                || (is_last_day_of_february(end) && is_last_day_of_february(start))
            {
                30
            } else {
                d2
            };
            (d1_adj, d2_adj)
        }
        Thirty360Variant::IsdaBondBasis => {
            let d1_adj = d1.min(30);
            let d2_adj = if d2 == 31 && d1_adj == 30 { 30 } else { d2 };
            (d1_adj, d2_adj)
        }
        Thirty360Variant::European => (d1.min(30), d2.min(30)),
        Thirty360Variant::Italian => {
            let italian = |date: Date, day: u8| {
                if day == 31 || (date.month() == Month::February && day > 27) {
                    30
                } else {
                    day
                }
            };
            (italian(start, d1), italian(end, d2))
        }
    };
    synthetic_days(start, end, d1_adj, d2_adj)
}

/// Synthetic day count of the forward interval `[start, end)` under 30E/360
/// ISDA (ISDA 2006 §4.16(h)).
///
/// The start becomes 30 when it is the last day of its month. The end becomes
/// 30 when it is day 31, or the last day of February unless
/// `end_is_termination_date` (the end is the contractual termination date).
pub(super) fn thirty_e_360_isda_days(start: Date, end: Date, end_is_termination_date: bool) -> i64 {
    let d1_adj = if start == start.end_of_month() {
        30
    } else {
        start.day()
    };
    let d2 = end.day();
    let d2_adj = if d2 == 31 || (is_last_day_of_february(end) && !end_is_termination_date) {
        30
    } else {
        d2
    };
    synthetic_days(start, end, d1_adj, d2_adj)
}

/// Whether `date` is the last day of February (28 or 29).
fn is_last_day_of_february(date: Date) -> bool {
    date.month() == Month::February && date == date.end_of_month()
}
