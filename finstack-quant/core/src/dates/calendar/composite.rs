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
//! assert!(cal_union.is_holiday(jan1_2025).expect("calendar lookup"));
//! let may26_2025 = create_date(2025, time::Month::May, 26)?;
//! assert!(cal_union.is_holiday(may26_2025).expect("calendar lookup")); // U.K. spring bank holiday
//! # Ok(())
//! # }
//! ```

use super::business_days::joint_members;
use crate::dates::HolidayCalendar;
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
    /// Construct a composite calendar combining one or more sub-calendars.
    ///
    /// A date is a holiday if any member marks it as a holiday, and a business day
    /// only when every member marks it as a business day.
    ///
    /// # Arguments
    ///
    /// * `calendars` - Non-empty slice of member calendars with non-disjoint
    ///   declared coverage ranges.
    ///
    /// # Errors
    ///
    /// Returns [`crate::Error::Validation`] when `calendars` is empty or when
    /// member `supported_years` intervals do not overlap.
    pub(crate) fn try_new(calendars: &'a [&'a dyn HolidayCalendar]) -> crate::Result<Self> {
        if calendars.is_empty() {
            return Err(crate::Error::Validation(
                "composite calendar requires at least one member calendar".to_string(),
            ));
        }
        let mut max_from: Option<i32> = None;
        let mut min_to: Option<i32> = None;
        for calendar in calendars {
            if let Some((from, to)) = calendar.supported_years() {
                max_from = match (max_from, from) {
                    (Some(a), Some(b)) => Some(a.max(b)),
                    (Some(a), None) => Some(a),
                    (None, Some(b)) => Some(b),
                    (None, None) => None,
                };
                min_to = match (min_to, to) {
                    (Some(a), Some(b)) => Some(a.min(b)),
                    (Some(a), None) => Some(a),
                    (None, Some(b)) => Some(b),
                    (None, None) => None,
                };
            }
        }
        if let (Some(from), Some(to)) = (max_from, min_to)
            && from > to
        {
            return Err(crate::Error::Validation(format!(
                "composite calendar members have disjoint supported years ({from} > {to})"
            )));
        }
        Ok(Self { calendars })
    }
}

impl HolidayCalendar for CompositeCalendar<'_> {
    fn is_holiday(&self, date: Date) -> crate::Result<bool> {
        // Every member is evaluated so a member failure is never hidden by an earlier hit.
        let mut holiday = false;
        for calendar in self.calendars {
            let member = calendar.is_holiday(date)?;
            holiday |= member;
        }
        Ok(holiday)
    }

    /// A date is a business day only if it is one on **all** sub-calendars,
    /// so each member's own `is_business_day` (weekends included) is respected.
    fn is_business_day(&self, date: Date) -> crate::Result<bool> {
        let mut business = true;
        for calendar in self.calendars {
            let member = calendar.is_business_day(date)?;
            business &= member;
        }
        Ok(business)
    }

    fn supported_years(&self) -> Option<(Option<i32>, Option<i32>)> {
        let mut max_from: Option<i32> = None;
        let mut min_to: Option<i32> = None;
        for calendar in self.calendars {
            if let Some((from, to)) = calendar.supported_years() {
                max_from = match (max_from, from) {
                    (Some(a), Some(b)) => Some(a.max(b)),
                    (Some(a), None) => Some(a),
                    (None, Some(b)) => Some(b),
                    (None, None) => None,
                };
                min_to = match (min_to, to) {
                    (Some(a), Some(b)) => Some(a.min(b)),
                    (Some(a), None) => Some(a),
                    (None, Some(b)) => Some(b),
                    (None, None) => None,
                };
            }
        }
        if max_from.is_some() || min_to.is_some() {
            Some((max_from, min_to))
        } else {
            None
        }
    }

    fn calendar_id(&self) -> Option<String> {
        let member_ids = self
            .calendars
            .iter()
            .map(|calendar| calendar.calendar_id())
            .collect::<Option<Vec<_>>>()?;
        if member_ids.is_empty() {
            return None;
        }
        Some(joint_members(&member_ids.join("+")).join("+"))
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

        let cal_union = CompositeCalendar::try_new(&calendars).expect("non-empty composite");

        // Date that is holiday in both calendars (New Year's Day)
        let d1 = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");
        assert!(cal_union.is_holiday(d1).expect("calendar lookup"));

        // Date that is holiday only in GBLO (Spring bank holiday 26-May-2025)
        let d2 = Date::from_calendar_date(2025, Month::May, 26).expect("Valid test date");
        assert!(GBLO.is_holiday(d2).expect("calendar lookup"));
        assert!(!TARGET2.is_holiday(d2).expect("calendar lookup"));
        assert!(cal_union.is_holiday(d2).expect("calendar lookup"));
    }

    #[test]
    fn composite_union_any_holiday() {
        let t2 = TARGET2;
        let gb = GBLO;
        let calendars = [&t2 as &dyn HolidayCalendar, &gb as &dyn HolidayCalendar];

        let composite = CompositeCalendar::try_new(&calendars).expect("non-empty composite");

        // Jan 1 is a holiday in both
        let jan1 = make_date(2025, 1, 1);
        assert!(composite.is_holiday(jan1).expect("calendar lookup"));

        // Spring bank holiday (May 26, 2025) is only in GBLO
        let may26 = make_date(2025, 5, 26);
        assert!(GBLO.is_holiday(may26).expect("calendar lookup"));
        assert!(!TARGET2.is_holiday(may26).expect("calendar lookup"));
        assert!(composite.is_holiday(may26).expect("calendar lookup")); // Union includes it

        // Regular business day in both
        let regular_day = make_date(2025, 6, 18); // Wednesday
        assert!(!composite.is_holiday(regular_day).expect("calendar lookup"));
    }
    #[test]
    fn composite_empty_calendars_rejected() {
        let calendars: &[&dyn HolidayCalendar] = &[];
        assert!(CompositeCalendar::try_new(calendars).is_err());
    }
    #[test]
    fn composite_single_calendar_behaves_like_original() {
        let t2 = TARGET2;
        let calendars = [&t2 as &dyn HolidayCalendar];

        let composite_union = CompositeCalendar::try_new(&calendars).expect("non-empty composite");

        // Test several dates
        for day in 1..=28 {
            let date = make_date(2025, 1, day);
            let original = t2.is_holiday(date).expect("calendar lookup");
            assert_eq!(
                composite_union.is_holiday(date).expect("calendar lookup"),
                original
            );
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

        let composite = CompositeCalendar::try_new(&calendars).expect("non-empty composite");

        // Any US or EU holiday should be marked
        let us_labor_day = make_date(2025, 9, 1); // First Monday of Sept
        assert!(composite.is_holiday(us_labor_day).expect("calendar lookup"));

        // Christmas is a holiday everywhere
        let christmas = make_date(2025, 12, 25);
        assert!(composite.is_holiday(christmas).expect("calendar lookup"));
    }

    #[test]
    fn composite_business_day_trait_implementation() {
        let t2 = TARGET2;
        let gb = GBLO;
        let calendars = [&t2 as &dyn HolidayCalendar, &gb as &dyn HolidayCalendar];

        let composite = CompositeCalendar::try_new(&calendars).expect("non-empty composite");

        // Weekends should not be business days
        let saturday = make_date(2025, 1, 4);
        assert!(
            !composite
                .is_business_day(saturday)
                .expect("calendar lookup")
        );

        let sunday = make_date(2025, 1, 5);
        assert!(!composite.is_business_day(sunday).expect("calendar lookup"));

        // Holiday should not be business day
        let holiday = make_date(2025, 1, 1);
        assert!(!composite.is_business_day(holiday).expect("calendar lookup"));

        // Regular weekday should be business day
        let weekday = make_date(2025, 1, 2);
        assert!(composite.is_business_day(weekday).expect("calendar lookup"));
    }

    #[test]
    fn composite_intersects_supported_years_and_rejects_disjoint_members() {
        use crate::dates::calendar::rule::Rule;
        use crate::dates::calendar::types::Calendar;
        use crate::dates::calendar::{BSE, SGSI};

        let sgsi_nyse_cals: [&dyn HolidayCalendar; 2] = [&SGSI, &NYSE];
        let sgsi_nyse = CompositeCalendar::try_new(&sgsi_nyse_cals).expect("overlapping coverage");
        assert_eq!(sgsi_nyse.supported_years(), Some((Some(2024), Some(2027))));
        assert!(sgsi_nyse.metadata().is_none());

        let sgsi_bse_cals: [&dyn HolidayCalendar; 2] = [&SGSI, &BSE];
        let sgsi_bse = CompositeCalendar::try_new(&sgsi_bse_cals).expect("overlapping coverage");
        assert_eq!(sgsi_bse.supported_years(), Some((Some(2024), Some(2026))));

        static EMPTY_RULES: [Rule; 0] = [];
        let old_cal = Calendar::new(
            "old",
            "Old",
            false,
            &EMPTY_RULES,
            Some((None, Some(2023))),
            &[],
        );
        let calendars: [&dyn HolidayCalendar; 2] = [&old_cal, &SGSI];
        assert!(
            CompositeCalendar::try_new(&calendars).is_err(),
            "disjoint coverage bounds must be rejected"
        );
    }
}
