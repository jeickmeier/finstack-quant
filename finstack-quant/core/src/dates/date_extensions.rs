//! Extension traits for date and datetime convenience methods.
//!
//! Provides ergonomic extensions to `time::Date`
//! for common financial operations like weekend checking, quarter calculation,
//! and business day arithmetic. All methods are allocation-free.

use crate::dates::calendar::business_days::{
    BusinessDayConvention, adjustment_failed, seek_business_day_after,
};
use crate::dates::periods::FiscalConfig;
use time::{Date, Duration, Month};

/// Convenience extensions for [`time::Date`].
pub trait DateExt: Copy {
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
    /// Date-range overflow is rejected instead of clamping the year.
    ///
    /// # Arguments
    ///
    /// * `months` - Signed number of calendar months to move; the target day is clamped to the target month's final valid day.
    ///
    /// # Errors
    ///
    /// Returns a validation error if the target year is outside the supported date range.
    ///
    /// # Example
    /// ```
    /// use finstack_quant_core::dates::{Date, DateExt};
    /// use time::Month;
    /// let date = Date::from_calendar_date(2024, Month::January, 31).expect("Valid date");
    /// assert_eq!(date.add_months(1)?, Date::from_calendar_date(2024, Month::February, 29).expect("Valid date"));
    /// # Ok::<(), finstack_quant_core::Error>(())
    /// ```
    fn add_months(self, months: i32) -> crate::Result<Self>;

    /// Add a signed number of calendar days, rejecting date-range overflow.
    ///
    /// # Arguments
    ///
    /// * `days` - Signed calendar-day offset; zero returns the input unchanged.
    ///
    /// # Errors
    ///
    /// Returns a validation error if the shifted date cannot be represented.
    fn add_days(self, days: i64) -> crate::Result<Self>;

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
    /// This algorithm only skips Saturdays & Sundays, and does NOT
    /// account for holidays. For true business day adjustments that respect
    /// holidays, use [`DateExt::add_business_days`] with a `HolidayCalendar`.
    /// Positive `n` moves forward, negative `n` moves backward. Zero returns
    /// the input unchanged.
    ///
    /// # Arguments
    ///
    /// * `n` - Signed weekday offset, excluding Saturdays and Sundays; zero leaves the date unchanged.
    ///
    /// # Errors
    ///
    /// Returns a validation error if the shifted date is outside the supported range.
    fn add_weekdays(self, n: i32) -> crate::Result<Self>;

    /// Add / subtract a number of **business days** (`n`) to the date using
    /// the provided `calendar` for holiday lookup.
    ///
    /// This algorithm skips weekends AND holidays according to the calendar.
    /// Positive `n` moves forward, negative `n` moves backward. Zero returns
    /// the input unchanged.
    ///
    /// The start is excluded and is not pre-rolled when closed. There is no
    /// closed-day cap: each step searches until it reaches a business day.
    ///
    /// # Errors
    ///
    /// Returns a validation error when fewer than `|n|` calendar days fit in
    /// the supported date range, [`crate::InputError::AdjustmentFailed`] when
    /// a step reaches the end of that range without a business day, and the
    /// calendar's error when membership of a needed date cannot be determined.
    ///
    /// # Arguments
    ///
    /// * `n` - Signed number of business days to move under `cal`; zero leaves the date unchanged.
    /// * `cal` - Holiday and weekend calendar used to determine each counted business day.
    ///
    /// Example:
    /// ```
    /// use finstack_quant_core::dates::{calendar_by_id_strict, Date, DateExt};
    /// use time::Month;
    /// let cal = calendar_by_id_strict("target2").expect("built-in calendar");
    /// let start = Date::from_calendar_date(2025, Month::June, 27).expect("Valid date"); // Friday
    /// let next = start.add_business_days(3, cal).expect("Business days calculation should succeed");
    /// assert_eq!(next, Date::from_calendar_date(2025, Month::July, 2).expect("Valid date"));
    /// ```
    fn add_business_days<C: crate::dates::HolidayCalendar + ?Sized>(
        self,
        n: i32,
        cal: &C,
    ) -> crate::Result<Self>;

    /// Completed calendar months from `self` to `other`.
    ///
    /// The greatest `k >= 0` with `self.add_months(k) <= other`, each
    /// candidate evaluated from the original `self` (last-valid-day
    /// clamping, CORE-CONVENTIONS-007 #4). Equal dates give zero. This is a
    /// calendar measure; it does not define loan seasoning, payment counts or
    /// coupon periods.
    ///
    /// # Arguments
    ///
    /// * `other` - End date; must not precede `self`.
    ///
    /// # Errors
    ///
    /// Returns `InputError::InvalidDateRange` when `other` precedes `self`.
    ///
    /// # Example
    /// ```rust
    /// use finstack_quant_core::dates::{Date, DateExt};
    /// use time::Month;
    ///
    /// let start = Date::from_calendar_date(2020, Month::January, 15).expect("Valid date");
    /// let end = Date::from_calendar_date(2022, Month::March, 10).expect("Valid date");
    /// assert_eq!(start.months_until(end)?, 25);
    ///
    /// // January 30 + 1M clamps to February 28, so one month is complete.
    /// let jan30 = Date::from_calendar_date(2025, Month::January, 30).expect("Valid date");
    /// let feb28 = Date::from_calendar_date(2025, Month::February, 28).expect("Valid date");
    /// assert_eq!(jan30.months_until(feb28)?, 1);
    ///
    /// // A reversed range is an error, not zero.
    /// assert!(end.months_until(start).is_err());
    /// # Ok::<(), finstack_quant_core::Error>(())
    /// ```
    fn months_until(self, other: Self) -> crate::Result<u32>;
}

impl DateExt for Date {
    fn is_weekend(self) -> bool {
        matches!(
            self.weekday(),
            time::Weekday::Saturday | time::Weekday::Sunday
        )
    }

    fn quarter(self) -> u8 {
        ((self.month() as u8 - 1) / 3) + 1
    }

    fn fiscal_year(self, config: FiscalConfig) -> i32 {
        let year = self.year();

        // Fast path: Calendar year
        if config.start_month() == 1 && config.start_day() == 1 {
            return year;
        }

        let current_month = self.month() as u8;

        if current_month > config.start_month() {
            // Strictly after the start month -> belongs to next fiscal year
            return year + 1;
        } else if current_month < config.start_month() {
            // Strictly before the start month -> belongs to current calendar year
            return year;
        }

        // We are in the start month. Check the day.
        // We must handle the edge case where config.start_day() exceeds the month length
        // (e.g. config="Feb 30" implies "last day of Feb").
        let threshold_day = if config.start_day() <= 28 {
            config.start_day()
        } else {
            let month_len = self.month().length(year);
            config.start_day().min(month_len)
        };

        if self.day() >= threshold_day {
            year + 1
        } else {
            year
        }
    }

    fn add_months(self, months: i32) -> crate::Result<Self> {
        let (year, month, _) = self.to_calendar_date();
        let total_months = i64::from(year) * 12 + (month as i64 - 1) + i64::from(months);
        let new_year = total_months.div_euclid(12) as i32;
        let new_month_idx = total_months.rem_euclid(12);
        let new_month = Month::January.nth_next(new_month_idx as u8);

        let days_in_new_month = new_month.length(new_year);
        let new_day = self.day().min(days_in_new_month);
        Date::from_calendar_date(new_year, new_month, new_day).map_err(|_| {
            crate::Error::Validation("month offset exceeds the supported date range".into())
        })
    }

    fn add_days(self, days: i64) -> crate::Result<Self> {
        i64::from(self.to_julian_day())
            .checked_add(days)
            .and_then(|day| i32::try_from(day).ok())
            .and_then(|day| Date::from_julian_day(day).ok())
            .ok_or_else(|| {
                crate::Error::Validation("day offset exceeds the supported date range".into())
            })
    }

    fn end_of_month(self) -> Self {
        let days = self.month().length(self.year());
        self + Duration::days(i64::from(days - self.day()))
    }

    fn add_weekdays(self, mut n: i32) -> crate::Result<Self> {
        if n == 0 {
            return Ok(self);
        }

        let step = if n > 0 { 1 } else { -1 };
        let mut date = self;

        // Phase 1: land on a weekday (at most two steps from a weekend start).
        while date.is_weekend() {
            date = date.add_days(i64::from(step))?;
            if !date.is_weekend() {
                n -= step;
            }
            if n == 0 {
                return Ok(date);
            }
        }

        // Phase 2: jump whole weeks (5 weekdays = 7 calendar days).
        let weeks = n / 5;
        let remainder = n % 5;

        if weeks != 0 {
            date = date.add_days(i64::from(weeks) * 7)?;
        }

        // Phase 3: remaining weekdays (at most 4).
        let mut rem = remainder;
        while rem != 0 {
            date = date.add_days(i64::from(step))?;
            if !date.is_weekend() {
                rem -= step;
            }
        }

        Ok(date)
    }

    fn add_business_days<C: crate::dates::HolidayCalendar + ?Sized>(
        self,
        n: i32,
        cal: &C,
    ) -> crate::Result<Self> {
        if n == 0 {
            return Ok(self);
        }

        // A business-day shift cannot require fewer calendar days than counted days.
        self.add_days(i64::from(n))?;
        let (step, conv) = if n > 0 {
            (1, BusinessDayConvention::Following)
        } else {
            (-1, BusinessDayConvention::Preceding)
        };
        let mut current = self;
        for _ in 0..n.unsigned_abs() {
            current = seek_business_day_after(current, step, cal, |_| true)?
                .ok_or_else(|| adjustment_failed(self, conv))?;
        }
        Ok(current)
    }

    fn months_until(self, other: Self) -> crate::Result<u32> {
        if other < self {
            return Err(crate::InputError::InvalidDateRange.into());
        }
        // The year/month difference bounds the answer; clamping can only make
        // the final candidate overshoot `other` by part of one month.
        let mut months =
            (other.year() - self.year()) * 12 + (other.month() as i32 - self.month() as i32);
        while months > 0 && self.add_months(months)? > other {
            months -= 1;
        }
        u32::try_from(months)
            .map_err(|_| crate::Error::Validation("month count is negative".into()))
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
        let result = start.add_weekdays(3).expect("valid date shift");
        assert_eq!(result, make_date(2025, 7, 2)); // Fri +3 weekdays = Wed (skip weekend)
    }

    #[test]
    fn test_add_weekdays_backward() {
        let start = make_date(2025, 6, 29); // Sunday
        let result = start.add_weekdays(-2).expect("valid date shift");
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
        let config = FiscalConfig::new(10, 1).expect("valid fiscal start"); // October 1 start

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
        let config = FiscalConfig::new(4, 6).expect("valid fiscal start"); // April 6 start

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
        use crate::dates::HolidayCalendar;
        use crate::dates::calendar::TARGET2;

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
        assert!(cal.is_business_day(result).expect("calendar lookup"));
    }

    #[test]
    fn test_add_business_days_error_on_all_holidays() {
        // Every day closed: the uncapped search fails only at the end of the
        // supported date range.
        struct AllHolidaysCal;
        impl crate::dates::HolidayCalendar for AllHolidaysCal {
            fn is_holiday(&self, _date: Date) -> crate::Result<bool> {
                Ok(true)
            }

            fn is_business_day(&self, _date: Date) -> crate::Result<bool> {
                Ok(false)
            }
        }

        let cal = AllHolidaysCal;
        let start = make_date(2025, 1, 1);
        let err = start
            .add_business_days(1, &cal)
            .expect_err("Should fail with AllHolidaysCal");
        match err {
            crate::Error::Input(crate::InputError::AdjustmentFailed { date, convention }) => {
                assert_eq!(date, start);
                assert_eq!(convention, BusinessDayConvention::Following);
            }
            other => panic!("Expected AdjustmentFailed error, got {:?}", other),
        }
    }

    #[test]
    fn add_business_days_i32_min_is_a_range_error() {
        use crate::dates::calendar::TARGET2;

        let err = make_date(2025, 1, 2)
            .add_business_days(i32::MIN, &TARGET2)
            .expect_err("i32::MIN business days cannot fit in the date range");
        assert!(matches!(err, crate::Error::Validation(_)), "{err:?}");
    }

    #[test]
    fn add_business_days_crosses_a_closure_longer_than_100_days() {
        struct ClosedUntilMay;
        impl crate::dates::HolidayCalendar for ClosedUntilMay {
            fn is_holiday(&self, date: Date) -> crate::Result<bool> {
                Ok(date < make_date(2025, 5, 1))
            }

            fn is_business_day(&self, date: Date) -> crate::Result<bool> {
                Ok(!self.is_holiday(date)?)
            }
        }

        let next = make_date(2025, 1, 1)
            .add_business_days(1, &ClosedUntilMay)
            .expect("a 120-day closure is searched through");
        assert_eq!(next, make_date(2025, 5, 1));
    }

    #[test]
    fn test_months_until() {
        let months = |a: Date, b: Date| a.months_until(b).expect("forward range");
        // Jan 15 → Mar 10: Jan 15 + 26M = Mar 15 > Mar 10, so 25 completed months
        let start = make_date(2020, 1, 15);
        let end = make_date(2022, 3, 10);
        assert_eq!(months(start, end), 25);

        // Same date = 0 months
        assert_eq!(months(start, start), 0);

        // End before start is an input error, not a zero clamp
        assert!(end.months_until(start).is_err());

        // Exactly one month (same day-of-month)
        assert_eq!(months(start, make_date(2020, 2, 15)), 1);

        // Cross year boundary (same day-of-month)
        assert_eq!(months(make_date(2024, 12, 1), make_date(2025, 1, 1)), 1);

        // Negative year handling (for completeness)
        assert_eq!(months(make_date(-500, 6, 1), make_date(-498, 6, 1)), 24);

        // Jan 31 → Feb 1: Jan 31 + 1M clamps to Feb 29, after Feb 1
        assert_eq!(months(make_date(2020, 1, 31), make_date(2020, 2, 1)), 0);

        // Jan 30 + 1M clamps to Feb 28, so one month is complete (007 #4)
        assert_eq!(months(make_date(2025, 1, 30), make_date(2025, 2, 28)), 1);
    }
}
