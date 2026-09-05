//! Extension traits for date and datetime convenience methods.
//!
//! Provides ergonomic extensions to `time::Date`
//! for common financial operations like weekend checking, quarter calculation,
//! and business day arithmetic. All methods are allocation-free.

#![allow(clippy::wrong_self_convention)]

use crate::dates::calendar::business_days::{
    seek_business_day, BusinessDayConvention, MAX_BUSINESS_DAY_SEARCH_DAYS,
};
use crate::dates::periods::FiscalConfig;
use time::{Date, Duration, Month, Weekday};

const MONTHS_BY_INDEX: [Month; 12] = [
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

#[inline]
fn saturating_calendar_date(year: i32, month: Month, day: u8) -> Date {
    Date::from_calendar_date(year, month, day).unwrap_or_else(|_| {
        if year < Date::MIN.year() {
            Date::MIN
        } else {
            Date::MAX
        }
    })
}

/// Convenience extensions for [`time::Date`].
pub trait DateExt: Sized {
    /// Returns true if the date falls on a weekend (**Saturday** or **Sunday**).
    fn is_weekend(self) -> bool;

    /// Calendar quarter of the date (1‥=4).
    fn quarter(self) -> u8;

    /// Fiscal year corresponding to the date based on the provided fiscal configuration.
    ///
    /// Uses the fiscal year start month and day from `FiscalConfig` to determine
    /// which fiscal year this date belongs to.
    fn fiscal_year(self, config: FiscalConfig) -> i32;

    /// Add `months` to the date, clamping to the last valid day of the target month.
    ///
    /// Handles negative month offsets correctly and clamps the day to the last
    /// valid day for the target month (e.g. Jan 31 + 1 month → Feb 28/29).
    ///
    /// # Example
    /// ```
    /// use finstack_quant_core::dates::{Date, DateExt};
    /// use time::Month;
    /// let date = Date::from_calendar_date(2024, Month::January, 31).expect("Valid date");
    /// assert_eq!(date.add_months(1), Date::from_calendar_date(2024, Month::February, 29).expect("Valid date"));
    /// ```
    fn add_months(self, months: i32) -> Self;

    /// Return the last day-of-month date for the month containing this date.
    ///
    /// # Example
    /// ```
    /// use finstack_quant_core::dates::{Date, DateExt};
    /// use time::Month;
    /// let date = Date::from_calendar_date(2024, Month::February, 15).expect("Valid date");
    /// assert_eq!(date.end_of_month(), Date::from_calendar_date(2024, Month::February, 29).expect("Valid date"));
    /// ```
    fn end_of_month(self) -> Self;

    /// Add / subtract a number of **weekdays** (`n`) to the date.
    ///
    /// This naive algorithm only skips Saturdays & Sundays, and does NOT
    /// account for holidays. For true business day adjustments that respect
    /// holidays, use [`DateExt::add_business_days`] with a `HolidayCalendar`.
    /// Positive `n` moves forward, negative `n` moves backward. Zero returns
    /// the input unchanged.
    fn add_weekdays(self, n: i32) -> Self;

    /// Add / subtract a number of **business days** (`n`) to the date using
    /// the provided `calendar` for holiday lookup.
    ///
    /// This algorithm skips weekends AND holidays according to the calendar.
    /// Positive `n` moves forward, negative `n` moves backward. Zero returns
    /// the input unchanged.
    ///
    /// Returns an error if no business day is found within the bounded search window.
    ///
    /// Example:
    /// ```
    /// use finstack_quant_core::dates::{Date, DateExt};
    /// use finstack_quant_core::dates::calendar::TARGET2;
    /// use time::Month;
    /// let cal = TARGET2;
    /// let start = Date::from_calendar_date(2025, Month::June, 27).expect("Valid date"); // Friday
    /// let next = start.add_business_days(3, &cal).expect("Business days calculation should succeed");
    /// assert_eq!(next, Date::from_calendar_date(2025, Month::July, 2).expect("Valid date"));
    /// ```
    fn add_business_days<C: crate::dates::HolidayCalendar + ?Sized>(
        self,
        n: i32,
        cal: &C,
    ) -> crate::Result<Self>;

    /// Calculate the number of whole months between two dates.
    ///
    /// Returns the difference as `(other.year - self.year) * 12 + (other.month - self.month)`.
    /// If `other` is before `self`, returns `0`.
    ///
    /// This is commonly used to calculate loan seasoning (age) in months for
    /// structured credit instruments.
    ///
    /// # Example
    /// ```rust
    /// use finstack_quant_core::dates::{Date, DateExt};
    /// use time::Month;
    ///
    /// let start = Date::from_calendar_date(2020, Month::January, 15).expect("Valid date");
    /// let end = Date::from_calendar_date(2022, Month::March, 10).expect("Valid date");
    /// assert_eq!(start.months_until(end), 25);
    ///
    /// // Returns 0 if end is before start
    /// assert_eq!(end.months_until(start), 0);
    /// ```
    fn months_until(self, other: Self) -> u32;

    /// Returns `true` if the date is a business day according to the provided
    /// `cal` (see `HolidayCalendar`).
    fn is_business_day<C: crate::dates::HolidayCalendar + ?Sized>(self, cal: &C) -> bool;
}

impl DateExt for Date {
    fn is_weekend(self) -> bool {
        matches!(self.weekday(), Weekday::Saturday | Weekday::Sunday)
    }

    fn quarter(self) -> u8 {
        ((self.month() as u8 - 1) / 3) + 1
    }

    fn fiscal_year(self, config: FiscalConfig) -> i32 {
        let year = self.year();

        // Fast path: Calendar year
        if config.start_month == 1 && config.start_day == 1 {
            return year;
        }

        let current_month = self.month() as u8;

        if current_month > config.start_month {
            // Strictly after the start month -> belongs to next fiscal year
            return year + 1;
        } else if current_month < config.start_month {
            // Strictly before the start month -> belongs to current calendar year
            return year;
        }

        // We are in the start month. Check the day.
        // We must handle the edge case where config.start_day exceeds the month length
        // (e.g. config="Feb 30" implies "last day of Feb").
        let threshold_day = if config.start_day <= 28 {
            config.start_day
        } else {
            let month_len = self.month().length(year);
            config.start_day.min(month_len)
        };

        if self.day() >= threshold_day {
            year + 1
        } else {
            year
        }
    }

    fn add_months(self, months: i32) -> Self {
        let (year, month, _) = self.to_calendar_date();
        let total_months = year * 12 + (month as i32 - 1) + months;
        let new_year = total_months.div_euclid(12);
        let new_month_idx = total_months.rem_euclid(12);
        let new_month = MONTHS_BY_INDEX[new_month_idx as usize];

        let days_in_new_month = new_month.length(new_year);
        let new_day = self.day().min(days_in_new_month);
        saturating_calendar_date(new_year, new_month, new_day)
    }

    fn end_of_month(self) -> Self {
        let days = self.month().length(self.year());
        saturating_calendar_date(self.year(), self.month(), days)
    }

    fn add_weekdays(self, mut n: i32) -> Self {
        if n == 0 {
            return self;
        }

        let step = if n > 0 { 1 } else { -1 };
        let mut date = self;

        // Phase 1: land on a weekday (at most two steps from a weekend start).
        while date.is_weekend() {
            date += Duration::days(step as i64);
            if !date.is_weekend() {
                n -= step;
            }
            if n == 0 {
                return date;
            }
        }

        // Phase 2: jump whole weeks (5 weekdays = 7 calendar days).
        let weeks = n / 5;
        let remainder = n % 5;

        if weeks != 0 {
            date += Duration::days(weeks as i64 * 7);
        }

        // Phase 3: remaining weekdays (at most 4).
        let mut rem = remainder;
        while rem != 0 {
            date += Duration::days(step as i64);
            if !date.is_weekend() {
                rem -= step;
            }
        }

        date
    }

    fn add_business_days<C: crate::dates::HolidayCalendar + ?Sized>(
        self,
        n: i32,
        cal: &C,
    ) -> crate::Result<Self> {
        if n == 0 {
            return Ok(self);
        }

        let step = if n > 0 { 1 } else { -1 };
        let mut current = self;
        for _ in 0..n.unsigned_abs() {
            // move at least one day in the desired direction, then seek to a business day
            let start = current + Duration::days(step as i64);
            let conv = if step > 0 {
                BusinessDayConvention::Following
            } else {
                BusinessDayConvention::Preceding
            };
            current = seek_business_day(start, step, MAX_BUSINESS_DAY_SEARCH_DAYS, cal).ok_or({
                crate::Error::Input(crate::error::InputError::AdjustmentFailed {
                    date: self,
                    convention: conv,
                    max_days: MAX_BUSINESS_DAY_SEARCH_DAYS,
                })
            })?;
        }
        Ok(current)
    }

    fn is_business_day<C: crate::dates::HolidayCalendar + ?Sized>(self, cal: &C) -> bool {
        cal.is_business_day(self)
    }

    fn months_until(self, other: Self) -> u32 {
        let mut months =
            (other.year() - self.year()) * 12 + (other.month() as i32 - self.month() as i32);
        if self.day() > other.day() {
            let self_eom = self.day() == self.month().length(self.year());
            let other_eom = other.day() == other.month().length(other.year());
            if !(self_eom && other_eom) {
                months -= 1;
            }
        }
        months.max(0) as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::Date;

    fn make_date(y: i32, m: u8, d: u8) -> Date {
        Date::from_calendar_date(y, time::Month::try_from(m).expect("Valid month (1-12)"), d)
            .expect("Valid test date")
    }

    #[test]
    fn test_is_weekend() {
        let sat = make_date(2025, 6, 28);
        let sun = make_date(2025, 6, 29);
        let fri = make_date(2025, 6, 27);
        assert!(sat.is_weekend());
        assert!(sun.is_weekend());
        assert!(!fri.is_weekend());
    }
    #[test]
    fn test_add_weekdays_forward() {
        let start = make_date(2025, 6, 27); // Friday
        let result = start.add_weekdays(3);
        assert_eq!(result, make_date(2025, 7, 2)); // Fri +3 weekdays = Wed (skip weekend)
    }

    #[test]
    fn test_add_weekdays_backward() {
        let start = make_date(2025, 6, 29); // Sunday
        let result = start.add_weekdays(-2);
        assert_eq!(result, make_date(2025, 6, 26)); // Sun -2 weekdays = Thu (skip weekend)
    }

    #[test]
    fn test_fiscal_year_calendar_year() {
        let date = make_date(2025, 6, 15);
        let config = FiscalConfig::calendar_year();
        assert_eq!(date.fiscal_year(config), 2025);
    }

    #[test]
    fn test_fiscal_year_us_federal() {
        let config = FiscalConfig::us_federal(); // October 1 start

        // Date before fiscal year start (e.g., September) belongs to previous FY
        let sept_date = make_date(2024, 9, 15);
        assert_eq!(sept_date.fiscal_year(config), 2024);

        // Date on or after fiscal year start belongs to current FY
        let oct_date = make_date(2024, 10, 1);
        assert_eq!(oct_date.fiscal_year(config), 2025);

        let dec_date = make_date(2024, 12, 15);
        assert_eq!(dec_date.fiscal_year(config), 2025);
    }

    #[test]
    fn test_fiscal_year_uk() {
        let config = FiscalConfig::uk(); // April 6 start

        // Date before fiscal year start belongs to previous FY
        let march_date = make_date(2025, 3, 15);
        assert_eq!(march_date.fiscal_year(config), 2025);

        // Date on or after fiscal year start belongs to current FY
        let april_date = make_date(2025, 4, 6);
        assert_eq!(april_date.fiscal_year(config), 2026);

        let may_date = make_date(2025, 5, 15);
        assert_eq!(may_date.fiscal_year(config), 2026);
    }

    #[test]
    fn test_add_business_days_forward() {
        use crate::dates::calendar::TARGET2;

        let cal = TARGET2;

        // Start on Friday, add 3 business days should land on Wednesday (skip weekend)
        let friday = make_date(2025, 6, 27);
        let result = friday
            .add_business_days(3, &cal)
            .expect("Business days calculation should succeed in test");
        assert_eq!(result, make_date(2025, 7, 2)); // Wednesday
    }

    #[test]
    fn test_add_business_days_backward() {
        use crate::dates::calendar::TARGET2;

        let cal = TARGET2;

        // Start on Monday, subtract 3 business days should land on Wednesday previous week
        let monday = make_date(2025, 6, 30);
        let result = monday
            .add_business_days(-3, &cal)
            .expect("Business days calculation should succeed in test");
        assert_eq!(result, make_date(2025, 6, 25)); // Wednesday
    }

    #[test]
    fn test_add_business_days_zero() {
        use crate::dates::calendar::TARGET2;

        let cal = TARGET2;
        let date = make_date(2025, 6, 27);
        let result = date
            .add_business_days(0, &cal)
            .expect("Business days calculation should succeed in test");
        assert_eq!(result, date);
    }

    #[test]
    fn test_add_business_days_with_holidays() {
        use crate::dates::calendar::TARGET2;
        use crate::dates::HolidayCalendar;

        let cal = TARGET2;

        // Test around a known holiday period (Christmas/New Year)
        // December 24, 2024 is Tuesday
        let christmas_eve = make_date(2024, 12, 24);
        let result = christmas_eve
            .add_business_days(1, &cal)
            .expect("Business days calculation should succeed in test");

        // Should skip Christmas Day (Dec 25), Boxing Day (Dec 26), and weekends
        // Landing on the next available business day
        assert!(result > christmas_eve);
        assert!(cal.is_business_day(result));
    }

    #[test]
    fn test_add_business_days_error_on_all_holidays() {
        // A calendar that marks every day as a holiday to trigger bounded search failure
        struct AllHolidaysCal;
        impl crate::dates::HolidayCalendar for AllHolidaysCal {
            fn is_holiday(&self, _date: Date) -> bool {
                true
            }
        }

        let cal = AllHolidaysCal;
        let start = make_date(2025, 1, 1);
        let err = start
            .add_business_days(1, &cal)
            .expect_err("Should fail with AllHolidaysCal");
        match err {
            crate::Error::Input(crate::error::InputError::AdjustmentFailed {
                max_days, ..
            }) => {
                assert_eq!(max_days, MAX_BUSINESS_DAY_SEARCH_DAYS);
            }
            other => panic!("Expected AdjustmentFailed error, got {:?}", other),
        }
    }

    #[test]
    fn test_months_until() {
        // Jan 15 → Mar 10: day 15 > day 10, so 26 - 1 = 25 completed months
        let start = make_date(2020, 1, 15);
        let end = make_date(2022, 3, 10);
        assert_eq!(start.months_until(end), 25);

        // Same date = 0 months
        assert_eq!(start.months_until(start), 0);

        // End before start = 0 (clamped)
        assert_eq!(end.months_until(start), 0);

        // Exactly one month (same day-of-month)
        let one_month_later = make_date(2020, 2, 15);
        assert_eq!(start.months_until(one_month_later), 1);

        // Cross year boundary (same day-of-month)
        let dec = make_date(2024, 12, 1);
        let jan = make_date(2025, 1, 1);
        assert_eq!(dec.months_until(jan), 1);

        // Negative year handling (for completeness)
        let ancient = make_date(-500, 6, 1);
        let later = make_date(-498, 6, 1);
        assert_eq!(ancient.months_until(later), 24);

        // Jan 31 → Feb 1: only 1 day apart, not a completed month
        let jan31 = make_date(2020, 1, 31);
        let feb1 = make_date(2020, 2, 1);
        assert_eq!(jan31.months_until(feb1), 0);
    }
}
