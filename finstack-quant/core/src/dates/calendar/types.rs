//! Calendar implementation using rule-based holiday evaluation.
//!
//! The `Calendar` struct provides a clean, efficient implementation supporting:
//! - Rule-based holiday definition (see [`super::rule::Rule`])
//! - A cache year range (`BASE_YEAR..=END_YEAR`); queries outside it are
//!   evaluated directly and fail when a required rule input (for example a
//!   lunar or equinox table year) is unavailable

use super::algo::{BASE_YEAR, END_YEAR};
use super::business_days::{CalendarMetadata, HolidayCalendar};
use super::rule::Rule;
use super::year_cache;
use crate::dates::DateExt;
use time::Date;

/// A holiday calendar implementation that evaluates holiday rules at runtime.
#[derive(Debug, Clone, Copy)]
pub struct Calendar {
    /// Unique identifier (e.g., "target2", "gblo")
    pub(crate) id: &'static str,

    /// Display name for the calendar
    pub(crate) name: &'static str,

    /// Whether weekends should be ignored in is_holiday checks
    pub(crate) ignore_weekends: bool,

    /// The rules defining this calendar's holidays
    pub(crate) rules: &'static [Rule],

    /// Optional inclusive `(from_year, to_year)` coverage bounds.
    pub(crate) supported_years: Option<(Option<i32>, Option<i32>)>,

    /// Exceptional trading dates on otherwise closed weekends.
    pub(super) open_dates: &'static [Date],
}

impl Calendar {
    /// Create a new calendar with the given parameters; weekends are
    /// Saturday and Sunday unless an exceptional opening is listed.
    pub(crate) const fn new(
        id: &'static str,
        name: &'static str,
        ignore_weekends: bool,
        rules: &'static [Rule],
        supported_years: Option<(Option<i32>, Option<i32>)>,
        open_dates: &'static [Date],
    ) -> Self {
        Self {
            id,
            name,
            ignore_weekends,
            rules,
            supported_years,
            open_dates,
        }
    }

    /// Reject queries outside this calendar's declared `(from_year, to_year)` bounds.
    pub(crate) fn check_supported_year(&self, year: i32) -> crate::Result<()> {
        if let Some((from_year, to_year)) = self.supported_years {
            let below = from_year.is_some_and(|from| year < from);
            let above = to_year.is_some_and(|to| year > to);
            if below || above {
                let range_desc = match (from_year, to_year) {
                    (Some(f), Some(t)) => format!("{f}..={t}"),
                    (Some(f), None) => format!("{f}.."),
                    (None, Some(t)) => format!("..={t}"),
                    (None, None) => "..".to_string(),
                };
                return Err(crate::Error::Validation(format!(
                    "calendar '{}' does not support year {year} (supported years: {range_desc})",
                    self.id
                )));
            }
        }
        Ok(())
    }

    pub(super) fn is_closed_weekend(&self, date: Date) -> bool {
        date.is_weekend() && !self.open_dates.contains(&date)
    }

    /// Evaluates the raw holiday rules without the cache or weekend overrides.
    ///
    /// Every rule is evaluated, so a failure is never hidden by an earlier hit.
    pub(super) fn holiday_from_rules(&self, date: Date) -> crate::Result<bool> {
        let mut holiday = false;
        for rule in self.rules {
            holiday |= rule.applies(date)?;
        }
        Ok(holiday)
    }
}

impl HolidayCalendar for Calendar {
    fn is_holiday(&self, date: Date) -> crate::Result<bool> {
        self.check_supported_year(date.year())?;
        let is_holiday = if (BASE_YEAR..=END_YEAR).contains(&date.year()) {
            year_cache::is_holiday_cached(self, date)?
        } else {
            self.holiday_from_rules(date)?
        };
        Ok(is_holiday && !(self.ignore_weekends && date.is_weekend()))
    }

    fn is_business_day(&self, date: Date) -> crate::Result<bool> {
        self.check_supported_year(date.year())?;
        if self.is_closed_weekend(date) {
            return Ok(false);
        }
        let holiday = if (BASE_YEAR..=END_YEAR).contains(&date.year()) {
            year_cache::is_holiday_cached(self, date)?
        } else {
            self.holiday_from_rules(date)?
        };
        Ok(!holiday)
    }

    fn count_business_days(&self, start: Date, end: Date) -> crate::Result<i32> {
        year_cache::count_business_days_cached(self, start, end)
    }

    fn metadata(&self) -> Option<CalendarMetadata> {
        Some(CalendarMetadata {
            id: self.id,
            name: self.name,
            ignore_weekends: self.ignore_weekends,
            supported_years: self.supported_years,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::Month;

    #[test]
    fn borrowed_and_trait_object_calendars_preserve_weekends_and_counts() {
        fn generic_business<C: HolidayCalendar>(calendar: C, date: Date) -> bool {
            calendar.is_business_day(date).expect("calendar lookup")
        }

        let calendar = Calendar::new("borrowed", "Borrowed calendar", true, &[], None, &[]);
        let borrowed = &calendar;
        let object: &dyn HolidayCalendar = &borrowed;
        for day in 1..=7 {
            let date =
                Date::from_calendar_date(2026, Month::October, day).expect("valid test date");
            let expected = !date.is_weekend();
            assert_eq!(
                calendar.is_business_day(date).expect("calendar lookup"),
                expected
            );
            assert_eq!(generic_business(borrowed, date), expected);
            assert_eq!(
                object.is_business_day(date).expect("calendar lookup"),
                expected
            );
            assert_eq!(
                object
                    .count_business_days(date, date.next_day().expect("next date"))
                    .expect("calendar lookup"),
                i32::from(expected)
            );
        }
    }

    #[test]
    fn indian_exchange_calendars_count_exceptional_trading_days() {
        use crate::dates::calendar::business_days::resolve_calendar;
        use time::macros::date;

        for id in ["bse", "nse"] {
            let calendar = resolve_calendar(id).expect("exchange calendar");
            for date in [
                date!(2024 - 01 - 20),
                date!(2024 - 03 - 02),
                date!(2024 - 05 - 18),
                date!(2025 - 02 - 01),
                date!(2026 - 02 - 01),
            ] {
                assert!(
                    calendar.is_business_day(date).expect("trading day"),
                    "{id} {date}"
                );
                assert!(
                    !calendar.is_holiday(date).expect("holiday lookup"),
                    "{id} {date}"
                );
                assert_eq!(
                    calendar
                        .count_business_days(date, date.next_day().expect("next date"))
                        .expect("business-day count"),
                    1,
                    "{id} {date}"
                );
            }
            for date in [
                date!(2024 - 01 - 21),
                date!(2024 - 01 - 22),
                date!(2024 - 05 - 20),
                date!(2024 - 11 - 20),
                date!(2026 - 01 - 15),
            ] {
                assert!(
                    !calendar.is_business_day(date).expect("business-day lookup"),
                    "{id} {date}"
                );
            }
        }
    }

    #[test]
    fn sifma_veterans_day_observes_sunday_only() {
        use crate::dates::calendar::business_days::resolve_calendar;
        use time::macros::date;

        let calendar = resolve_calendar("sifma").expect("bond market calendar");
        assert!(
            calendar
                .is_business_day(date!(2023 - 11 - 10))
                .expect("trading day")
        );
        assert!(
            calendar
                .is_business_day(date!(2028 - 11 - 10))
                .expect("trading day")
        );
        assert!(
            !calendar
                .is_business_day(date!(2029 - 11 - 12))
                .expect("observed holiday")
        );
    }

    #[test]
    fn calendar_weekends_are_saturday_and_sunday() {
        let cal = Calendar::new("weekend_test", "Weekend Test", true, &[], None, &[]);

        let saturday = Date::from_calendar_date(2025, Month::January, 4).expect("Valid test date");
        let sunday = Date::from_calendar_date(2025, Month::January, 5).expect("Valid test date");
        let friday = Date::from_calendar_date(2025, Month::January, 3).expect("Valid test date");

        assert!(!cal.is_business_day(saturday).expect("calendar lookup"));
        assert!(!cal.is_business_day(sunday).expect("calendar lookup"));
        assert!(cal.is_business_day(friday).expect("calendar lookup"));
    }
}
