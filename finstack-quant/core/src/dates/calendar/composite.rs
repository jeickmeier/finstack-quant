//! Composite holiday calendars combining multiple market calendars.
//!
//! Allows combining multiple [`HolidayCalendar`] implementations into a single
//! logical calendar with union semantics: a date is a holiday if ANY
//! sub-calendar is closed, so settlement requires ALL markets to be open
//! (e.g. a cross-currency swap settling in both USD and EUR).
//!
//! # Performance
//!
//! Borrowed slice of trait objects: the holiday checks allocate nothing (only
//! `calendar_id()` builds its joined id) and add no overhead beyond calling
//! each subcalendar's `is_holiday` method.
//!
//! Public callers reach a composite through a `+`-joined calendar id.
//!
//! # Examples
//! ```
//! use finstack_quant_core::dates::{calendar_by_id_strict, create_date};
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//!
//! // Treat the day as a holiday if *either* market is closed.
//! let cal_union = calendar_by_id_strict("target2+gblo")?;
//! let jan1_2025 = create_date(2025, time::Month::January, 1)?;
//! assert!(cal_union.is_holiday(jan1_2025));
//! let may26_2025 = create_date(2025, time::Month::May, 26)?;
//! assert!(cal_union.is_holiday(may26_2025)); // U.K. spring bank holiday
//! # Ok(())
//! # }
//! ```

use crate::dates::{canonical_calendar_id, HolidayCalendar, WeekendRule};
use time::Date;

/// A lightweight view combining several holiday calendars (union semantics).
#[derive(Clone, Copy)]
pub(crate) struct CompositeCalendar<'a> {
    calendars: &'a [&'a dyn HolidayCalendar],
}

impl core::fmt::Debug for CompositeCalendar<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("CompositeCalendar")
            .field("calendars_len", &self.calendars.len())
            .finish()
    }
}

impl<'a> CompositeCalendar<'a> {
    /// Create a new composite calendar: a date is a holiday if any sub-calendar
    /// marks it as one.
    #[must_use]
    pub const fn new(calendars: &'a [&'a dyn HolidayCalendar]) -> Self {
        Self { calendars }
    }
}

impl HolidayCalendar for CompositeCalendar<'_> {
    fn is_holiday(&self, date: Date) -> bool {
        // Empty slice ⇒ no holidays, so return false.
        self.calendars.iter().any(|c| c.is_holiday(date))
    }

    /// A date is a business day only if it is one on **all** sub-calendars,
    /// so each member's weekend rule (e.g. Friday/Saturday Middle East
    /// calendars) is respected.
    ///
    /// An empty calendar list falls back to the Saturday/Sunday weekend rule.
    fn is_business_day(&self, date: Date) -> bool {
        if self.calendars.is_empty() {
            return !WeekendRule::SaturdaySunday.is_weekend(date.weekday());
        }
        self.calendars.iter().all(|c| c.is_business_day(date))
    }

    fn calendar_id(&self) -> Option<String> {
        let mut member_ids = self
            .calendars
            .iter()
            .map(|calendar| calendar.calendar_id())
            .collect::<Option<Vec<_>>>()?;
        if member_ids.is_empty() {
            return None;
        }
        member_ids.sort_unstable();
        member_ids.dedup();
        canonical_calendar_id(&member_ids.join("+")).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dates::calendar::{GBLO, NYSE, TARGET2, USNY};
    use time::{Date, Month};

    fn make_date(y: i32, m: u8, d: u8) -> Date {
        Date::from_calendar_date(y, Month::try_from(m).unwrap(), d).unwrap()
    }

    #[test]
    fn union_marks_any_subcalendar_holiday() {
        let t2 = TARGET2;
        let gb = GBLO;
        let calendars = [&t2 as &dyn HolidayCalendar, &gb as &dyn HolidayCalendar];

        let cal_union = CompositeCalendar::new(&calendars);

        // Date that is holiday in both calendars (New Year's Day)
        let d1 = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");
        assert!(cal_union.is_holiday(d1));

        // Date that is holiday only in GBLO (Spring bank holiday 26-May-2025)
        let d2 = Date::from_calendar_date(2025, Month::May, 26).expect("Valid test date");
        assert!(GBLO.is_holiday(d2));
        assert!(!TARGET2.is_holiday(d2));
        assert!(cal_union.is_holiday(d2));
    }

    #[test]
    fn composite_respects_subcalendar_weekend_rules() {
        use crate::dates::calendar::types::{Calendar, WeekendRule};

        // Friday/Saturday weekend calendar (e.g. Middle East), no holiday rules.
        const FRI_SAT: Calendar = Calendar::new("fri_sat", "Fri/Sat weekend", false, &[])
            .with_weekend_rule(WeekendRule::FridaySaturday);
        let gb = GBLO;
        let calendars = [
            &FRI_SAT as &dyn HolidayCalendar,
            &gb as &dyn HolidayCalendar,
        ];

        let cal_union = CompositeCalendar::new(&calendars);

        // Friday 2025-06-06: weekend for FRI_SAT, business day for GBLO.
        let friday = Date::from_calendar_date(2025, Month::June, 6).expect("Valid test date");
        assert_eq!(friday.weekday(), time::Weekday::Friday);
        assert!(!FRI_SAT.is_business_day(friday));
        assert!(GBLO.is_business_day(friday));

        // Business day only if ALL sub-calendars are open → closed.
        assert!(!cal_union.is_business_day(friday));

        // Sunday 2025-06-08: weekend for GBLO, business day for FRI_SAT.
        let sunday = Date::from_calendar_date(2025, Month::June, 8).expect("Valid test date");
        assert!(FRI_SAT.is_business_day(sunday));
        assert!(!GBLO.is_business_day(sunday));
        assert!(!cal_union.is_business_day(sunday));

        // Wednesday 2025-06-04: business day everywhere.
        let wednesday = Date::from_calendar_date(2025, Month::June, 4).expect("Valid test date");
        assert!(cal_union.is_business_day(wednesday));
    }

    #[test]
    fn composite_union_any_holiday() {
        let t2 = TARGET2;
        let gb = GBLO;
        let calendars = [&t2 as &dyn HolidayCalendar, &gb as &dyn HolidayCalendar];

        let composite = CompositeCalendar::new(&calendars);

        // Jan 1 is a holiday in both
        let jan1 = make_date(2025, 1, 1);
        assert!(composite.is_holiday(jan1));

        // Spring bank holiday (May 26, 2025) is only in GBLO
        let may26 = make_date(2025, 5, 26);
        assert!(GBLO.is_holiday(may26));
        assert!(!TARGET2.is_holiday(may26));
        assert!(composite.is_holiday(may26)); // Union includes it

        // Regular business day in both
        let regular_day = make_date(2025, 6, 18); // Wednesday
        assert!(!composite.is_holiday(regular_day));
    }
    #[test]
    fn composite_empty_calendars_union() {
        let calendars: &[&dyn HolidayCalendar] = &[];
        let composite = CompositeCalendar::new(calendars);

        // Empty union should have no holidays
        let any_date = make_date(2025, 1, 1);
        assert!(!composite.is_holiday(any_date));
    }
    #[test]
    fn composite_single_calendar_behaves_like_original() {
        let t2 = TARGET2;
        let calendars = [&t2 as &dyn HolidayCalendar];

        let composite_union = CompositeCalendar::new(&calendars);

        // Test several dates
        for day in 1..=28 {
            let date = make_date(2025, 1, day);
            let original = t2.is_holiday(date);
            assert_eq!(composite_union.is_holiday(date), original);
        }
    }

    #[test]
    fn composite_multiple_calendars_union() {
        let nyse = NYSE;
        let usny = USNY;
        let t2 = TARGET2;

        let calendars = [
            &nyse as &dyn HolidayCalendar,
            &usny as &dyn HolidayCalendar,
            &t2 as &dyn HolidayCalendar,
        ];

        let composite = CompositeCalendar::new(&calendars);

        // Any US or EU holiday should be marked
        let us_labor_day = make_date(2025, 9, 1); // First Monday of Sept
        assert!(composite.is_holiday(us_labor_day));

        // Christmas is a holiday everywhere
        let christmas = make_date(2025, 12, 25);
        assert!(composite.is_holiday(christmas));
    }

    #[test]
    fn composite_business_day_trait_implementation() {
        let t2 = TARGET2;
        let gb = GBLO;
        let calendars = [&t2 as &dyn HolidayCalendar, &gb as &dyn HolidayCalendar];

        let composite = CompositeCalendar::new(&calendars);

        // Weekends should not be business days
        let saturday = make_date(2025, 1, 4);
        assert!(!composite.is_business_day(saturday));

        let sunday = make_date(2025, 1, 5);
        assert!(!composite.is_business_day(sunday));

        // Holiday should not be business day
        let holiday = make_date(2025, 1, 1);
        assert!(!composite.is_business_day(holiday));

        // Regular weekday should be business day
        let weekday = make_date(2025, 1, 2);
        assert!(composite.is_business_day(weekday));
    }
}
