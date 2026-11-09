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
}

impl Calendar {
    /// Create a new calendar with the given parameters; weekends are
    /// Saturday and Sunday.
    pub(crate) const fn new(
        id: &'static str,
        name: &'static str,
        ignore_weekends: bool,
        rules: &'static [Rule],
    ) -> Self {
        Self {
            id,
            name,
            ignore_weekends,
            rules,
        }
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
        let is_holiday = if (BASE_YEAR..=END_YEAR).contains(&date.year()) {
            year_cache::is_holiday_cached(self, date)?
        } else {
            self.holiday_from_rules(date)?
        };
        Ok(is_holiday && !(self.ignore_weekends && date.is_weekend()))
    }

    fn is_business_day(&self, date: Date) -> crate::Result<bool> {
        if date.is_weekend() {
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

        let calendar = Calendar::new("borrowed", "Borrowed calendar", true, &[]);
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
    fn calendar_weekends_are_saturday_and_sunday() {
        let cal = Calendar::new("weekend_test", "Weekend Test", true, &[]);

        let saturday = Date::from_calendar_date(2025, Month::January, 4).expect("Valid test date");
        let sunday = Date::from_calendar_date(2025, Month::January, 5).expect("Valid test date");
        let friday = Date::from_calendar_date(2025, Month::January, 3).expect("Valid test date");

        assert!(!cal.is_business_day(saturday).expect("calendar lookup"));
        assert!(!cal.is_business_day(sunday).expect("calendar lookup"));
        assert!(cal.is_business_day(friday).expect("calendar lookup"));
    }
}
