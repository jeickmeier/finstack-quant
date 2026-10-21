//! Algorithmic holiday helpers for calendar computations.
//!
//! This module provides deterministic, allocation-free implementations of
//! holiday date calculations used across multiple calendar modules. Each
//! algorithm is defined once and reused to ensure consistency.
//!
//! # Features
//!
//! - **Easter Monday**: Anonymous Gregorian algorithm for Western Easter
//! - **Chinese New Year**: Pre-computed lookup table (1970-2150)
//! - **Nth weekday of month**: O(1) form used by IMM dates and calendar rules
//! - **Validated year range**: `BASE_YEAR..=END_YEAR` constants shared by the
//!   holiday bitset cache and rule validation
//! - **Zero allocation**: All functions are stack-only
//! - **Panic-free**: Safe for all valid `time::Date` ranges
//!
//! # Supported Range
//!
//! Chinese New Year dates are available for years 1970-2150. Easter Monday
//! can be computed for any valid Gregorian year.

use time::{Date, Duration, Month, Weekday};

/// Inclusive first year of the holiday cache and the lunar/equinox tables.
///
/// `build/generate_reference_data.py` and `build/gen_lunar_festivals.py` read
/// this pair from this file.
pub(crate) const BASE_YEAR: i32 = 1970;
/// Inclusive last year of the holiday cache and the lunar/equinox tables.
pub(crate) const END_YEAR: i32 = 2150;

// Easter

/// Computes Easter Monday for a given Gregorian year.
///
/// Uses the Anonymous Gregorian algorithm (also known as Meeus/Jones/Butcher
/// algorithm) to calculate Easter Sunday, then returns the following Monday.
/// Easter Monday is a public holiday in many European and Commonwealth countries.
///
/// # Algorithm
///
/// The algorithm computes Easter Sunday using purely arithmetic operations
/// without iteration, based on the Metonic cycle (19-year lunar cycle) and
/// solar corrections for the Gregorian calendar.
///
/// # Arguments
///
/// * `year` - Gregorian calendar year (valid range: any year supported by `time::Date`)
///
/// # Returns
///
/// The `Date` of Easter Monday (the day after Easter Sunday) for the given
/// year, or `None` when the year is outside the proleptic Gregorian range the
/// `time` crate represents or the algorithm's integer arithmetic does not
/// yield a calendar date (which happens for negative years).
///
/// For every supported positive year Easter falls between March 22 and
/// April 25 (Sunday), so Easter Monday falls between March 23 and April 26.
///
/// # References
///
/// - Meeus, J. (1991). *Astronomical Algorithms*. Willmann-Bell. Chapter 8. `docs/REFERENCES.md#meeus-1991`
/// - Butcher, S. (1876). "Ecclesiastical Calendar." In *The Calculation of Easter*. `docs/REFERENCES.md#meeus-1991`
/// - Algorithm widely known as "Anonymous Gregorian Algorithm" or "Meeus/Jones/Butcher" `docs/REFERENCES.md#meeus-1991`
#[inline]
pub(crate) fn easter_monday(year: i32) -> Option<Date> {
    // Anonymous Gregorian algorithm
    let a = year % 19;
    let b = year / 100;
    let c = year % 100;
    let d = b / 4;
    let e = b % 4;
    let f = (b + 8) / 25;
    let g = (b - f + 1) / 3;
    let h = (19 * a + b - d - g + 15) % 30;
    let i = c / 4;
    let k = c % 4;
    let l = (32 + 2 * e + 2 * i - h - k) % 7;
    let m = (a + 11 * h + 22 * l) / 451;
    let month_num = (h + l - 7 * m + 114) / 31; // 3=March 4=April
    let day = ((h + l - 7 * m + 114) % 31) + 1; // Easter Sunday
    let month = if month_num == 3 {
        Month::March
    } else {
        Month::April
    };
    let easter_sunday = Date::from_calendar_date(year, month, u8::try_from(day).ok()?).ok()?;
    easter_sunday.checked_add(Duration::days(1)) // Easter Monday = Sunday + 1
}

// Chinese New Year, lunar festivals and Japanese equinoxes (generated lookups,
// 1970-2150): `cny_date` and one `*_date(year) -> Option<Date>` per festival.
include!("../../generated/cny_generated.rs");
include!("../../generated/festivals_generated.rs");

// Nth weekday of month

/// Helper to compute nth weekday of month.
///
/// Returns `None` when the requested occurrence does not exist in the month
/// (e.g. a 5th Monday in a month with only four Mondays), rather than spilling
/// into the adjacent month.
///
/// Also `None` when the month lies outside the date range the `time` crate
/// represents, so a rule evaluated at the edge of that range is simply not a
/// holiday rather than a panic.
#[inline]
pub(crate) fn nth_weekday_of_month(
    year: i32,
    month: Month,
    weekday: Weekday,
    n: i8,
) -> Option<Date> {
    let result = if n > 0 {
        let first = Date::from_calendar_date(year, month, 1).ok()?;
        // Days to step forward from the 1st to reach `weekday`, in 0..=6.
        let offset =
            (7 + weekday.number_days_from_monday() - first.weekday().number_days_from_monday()) % 7;
        first
            .checked_add(Duration::days(i64::from(offset)))?
            .checked_add(Duration::weeks(i64::from(n) - 1))?
    } else {
        let (ny, nm) = if month == Month::December {
            (year.checked_add(1)?, Month::January)
        } else {
            (year, month.next())
        };
        let last = Date::from_calendar_date(ny, nm, 1)
            .ok()?
            .checked_sub(Duration::days(1))?;
        // Days to step backward from the last day to reach `weekday`, in 0..=6.
        let offset =
            (7 + last.weekday().number_days_from_monday() - weekday.number_days_from_monday()) % 7;
        let pos = i64::from(-n); // 1=last, 2=second-last
        last.checked_sub(Duration::days(i64::from(offset)))?
            .checked_sub(Duration::weeks(pos - 1))?
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
    /// (year, month, weekday, n) in the cache calendar range.
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

    #[test]
    fn easter_monday_is_none_instead_of_panicking_outside_the_algorithm_range() {
        // Negative years feed negative remainders into the Gregorian
        // arithmetic; some of them name no calendar date and must come back as
        // `None`, never a panic.
        let negative: Vec<Option<Date>> = (-400..=0).map(easter_monday).collect();
        assert!(negative.iter().any(Option::is_none));
        assert!((1583..=Date::MAX.year()).all(|year| easter_monday(year).is_some()));
        assert!(easter_monday(i32::MIN).is_none());
        assert!(easter_monday(i32::MAX).is_none());
    }

    #[test]
    fn nth_weekday_is_none_at_the_date_range_edge() {
        let last_year = Date::MAX.year();
        // The last Monday of the final December needs the successor month.
        assert_eq!(
            nth_weekday_of_month(last_year, Month::December, Weekday::Monday, -1),
            None
        );
        assert!(nth_weekday_of_month(last_year, Month::November, Weekday::Monday, -1).is_some());
        assert_eq!(
            nth_weekday_of_month(i32::MAX, Month::December, Weekday::Friday, 1),
            None
        );
        assert_eq!(
            nth_weekday_of_month(i32::MIN, Month::January, Weekday::Friday, -1),
            None
        );
    }
}
