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
/// (e.g. a 5th Monday in a month with only four Mondays), rather than spilling
/// into the adjacent month.
#[inline]
#[allow(clippy::unreachable)] // Gregorian month boundaries used below are valid by construction.
pub(crate) fn nth_weekday_of_month(
    year: i32,
    month: Month,
    weekday: Weekday,
    n: i8,
) -> Option<Date> {
    let result = if n > 0 {
        let first = Date::from_calendar_date(year, month, 1)
            .unwrap_or_else(|_| unreachable!("first day of month is a valid Gregorian date"));
        // Days to step forward from the 1st to reach `weekday`, in 0..=6.
        let offset =
            (7 + weekday.number_days_from_monday() - first.weekday().number_days_from_monday()) % 7;
        first + Duration::days(i64::from(offset)) + Duration::weeks((n as i64) - 1)
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
        let last = Date::from_calendar_date(ny, nm, 1).unwrap_or_else(|_| {
            unreachable!("first day of successor month is a valid Gregorian date")
        }) - Duration::days(1);
        // Days to step backward from the last day to reach `weekday`, in 0..=6.
        let offset =
            (7 + last.weekday().number_days_from_monday() - weekday.number_days_from_monday()) % 7;
        let pos = (-n) as i64; // 1=last, 2=second-last
        last - Duration::days(i64::from(offset)) - Duration::weeks(pos - 1)
    };
    (result.year() == year && result.month() == month).then_some(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Brute-force reference: the day-stepping implementation that
    /// [`nth_weekday_of_month`] replaced. Kept only as a test oracle.
    fn nth_weekday_reference(year: i32, month: Month, weekday: Weekday, n: i8) -> Option<Date> {
        let result = if n > 0 {
            let mut d = Date::from_calendar_date(year, month, 1).unwrap();
            while d.weekday() != weekday {
                d += Duration::days(1);
            }
            d + Duration::weeks((n as i64) - 1)
        } else {
            let (ny, nm) = if month == Month::December {
                (year + 1, Month::January)
            } else {
                (year, Month::try_from(month as u8 + 1).unwrap())
            };
            let mut d = Date::from_calendar_date(ny, nm, 1).unwrap() - Duration::days(1);
            while d.weekday() != weekday {
                d -= Duration::days(1);
            }
            d - Duration::weeks(((-n) as i64) - 1)
        };
        (result.year() == year && result.month() == month).then_some(result)
    }

    /// The O(1) form must agree with the day-stepping reference for every
    /// (year, month, weekday, n) in the validated calendar range.
    #[test]
    fn nth_weekday_matches_day_stepping_reference_over_full_year_range() {
        const MONTHS: [Month; 12] = [
            Month::January,
            Month::February,
            Month::March,
            Month::April,
            Month::May,
            Month::June,
            Month::July,
            Month::August,
            Month::September,
            Month::October,
            Month::November,
            Month::December,
        ];
        const WEEKDAYS: [Weekday; 7] = [
            Weekday::Monday,
            Weekday::Tuesday,
            Weekday::Wednesday,
            Weekday::Thursday,
            Weekday::Friday,
            Weekday::Saturday,
            Weekday::Sunday,
        ];

        for year in BASE_YEAR..=END_YEAR {
            for month in MONTHS {
                for weekday in WEEKDAYS {
                    for n in [-5i8, -4, -3, -2, -1, 1, 2, 3, 4, 5] {
                        assert_eq!(
                            nth_weekday_of_month(year, month, weekday, n),
                            nth_weekday_reference(year, month, weekday, n),
                            "mismatch at year={year} month={month:?} weekday={weekday:?} n={n}"
                        );
                    }
                }
            }
        }
    }
}
