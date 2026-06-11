//! Generated holiday support: validated year range and shared date helpers.
//!
//! Design:
//! - Years covered: `BASE_YEAR..=END_YEAR` (build-generated constants); holiday
//!   rules are validated within this range and evaluated at runtime.

use time::{Date, Duration, Month, Weekday};

// Include generated constants directly from src/generated for IDE discoverability.
include!("../../generated/holiday_generated.rs");

/// Helper to compute nth weekday of month.
///
/// Returns `None` when the requested occurrence does not exist in the month
/// (e.g. a 5th Monday in a month with only four Mondays). Previously the raw
/// arithmetic result silently spilled into the adjacent month, which could
/// mark phantom holiday dates (2026-06-09 core quant review, Minor/Dates).
#[inline]
pub(crate) fn nth_weekday_of_month(
    year: i32,
    month: Month,
    weekday: Weekday,
    n: i8,
) -> Option<Date> {
    let result = if n > 0 {
        let mut d = Date::from_calendar_date(year, month, 1)
            .unwrap_or_else(|_| unreachable!("first day of month is a valid Gregorian date"));
        while d.weekday() != weekday {
            d += Duration::days(1);
        }
        d + Duration::weeks((n as i64) - 1)
    } else {
        let (ny, nm) = if month == Month::December {
            (year + 1, Month::January)
        } else {
            (
                year,
                Month::try_from(month as u8 + 1).unwrap_or_else(|_| {
                    unreachable!("successor month exists for non-December months")
                }),
            )
        };
        let mut d = Date::from_calendar_date(ny, nm, 1).unwrap_or_else(|_| {
            unreachable!("first day of successor month is a valid Gregorian date")
        }) - Duration::days(1);
        while d.weekday() != weekday {
            d -= Duration::days(1);
        }
        let pos = (-n) as i64; // 1=last, 2=second-last
        d - Duration::weeks(pos - 1)
    };
    (result.year() == year && result.month() == month).then_some(result)
}
