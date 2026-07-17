//! Tests for the surrounding crate component and its documented behavior.
//!
use std::collections::HashSet;

use finstack_quant_core::dates::Date;
use finstack_quant_core::dates::HolidayCalendar;
use time::Month;

/// Standard tolerance for day count year fraction comparisons.
/// Machine epsilon level for financial precision.
pub(crate) const DAYCOUNT_TOLERANCE: f64 = 1e-12;

/// Helper to construct `Date` instances.
pub(crate) fn make_date(year: i32, month: u8, day: u8) -> Date {
    Date::from_calendar_date(year, Month::try_from(month).unwrap(), day).unwrap()
}

/// Simple in-memory calendar used for testing adjustments.
#[derive(Default, Clone)]
pub(crate) struct TestCal {
    holidays: HashSet<Date>,
}

impl TestCal {
    pub(crate) fn new() -> Self {
        Self {
            holidays: HashSet::new(),
        }
    }

    pub(crate) fn with_holiday(mut self, date: Date) -> Self {
        self.holidays.insert(date);
        self
    }
}

impl HolidayCalendar for TestCal {
    fn is_holiday(&self, date: Date) -> bool {
        self.holidays.contains(&date)
    }
}
