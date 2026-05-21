//! Composite holiday calendars combining multiple market calendars.
//!
//! Allows combining multiple [`HolidayCalendar`] implementations into a single
//! logical calendar using union or intersection semantics. Useful for multi-market
//! instruments or cross-currency derivatives.
//!
//! # Combination Modes
//!
//! - **Union** (default): Holiday if ANY subcalendar is closed
//!   - Use for: Settlement requires ALL markets open
//!   - Example: Cross-currency swap settling in both USD and EUR
//!
//! - **Intersection**: Holiday only if ALL subcalendars are closed
//!   - Use for: Settlement when ANY market is open
//!   - Example: Multi-listed security trading on multiple exchanges
//!
//! # Performance
//!
//! Allocation-free design using borrowed slice of trait objects. Zero runtime
//! overhead beyond calling each subcalendar's `is_holiday` method.
//!
//! # Examples
//! ```
//! use finstack_core::dates::{CompositeCalendar, HolidayCalendar, create_date};
//! use finstack_core::dates::calendar::{TARGET2, GBLO};
//! use finstack_core::dates::CompositeMode;
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//!
//! let t2 = TARGET2;
//! let gb = GBLO;
//! let calendars = [&t2 as &dyn HolidayCalendar, &gb as &dyn HolidayCalendar];
//!
//! // Union (default) – treat the day as a holiday if *either* market is closed.
//! let cal_union = CompositeCalendar::new(&calendars);
//! let jan1_2025 = create_date(2025, time::Month::January, 1)?;
//! assert!(cal_union.is_holiday(jan1_2025));
//!
//! // Intersection – holiday only if *both* markets are closed.
//! let cal_inter = CompositeCalendar::with_mode(&calendars, CompositeMode::Intersection);
//! let may26_2025 = create_date(2025, time::Month::May, 26)?;
//! assert!(cal_union.is_holiday(may26_2025)); // U.K. spring bank holiday
//! assert!(!cal_inter.is_holiday(may26_2025));
//! # Ok(())
//! # }
//! ```

use crate::dates::calendar::HolidayCalendar;
use time::Date;

/// A lightweight view combining several holiday calendars.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[non_exhaustive]
pub enum CompositeMode {
    /// Holiday if any sub-calendar marks the date as holiday (set union).
    Union,
    /// Holiday only if all sub-calendars mark the date as holiday (set intersection).
    Intersection,
}

/// A lightweight view combining several holiday calendars.
#[derive(Clone, Copy)]
pub struct CompositeCalendar<'a> {
    calendars: &'a [&'a dyn HolidayCalendar],
    mode: CompositeMode,
}

impl core::fmt::Debug for CompositeCalendar<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("CompositeCalendar")
            .field("mode", &self.mode)
            .field("calendars_len", &self.calendars.len())
            .finish()
    }
}

impl<'a> CompositeCalendar<'a> {
    /// Create a new composite calendar using union semantics (strict by default).
    #[must_use]
    pub const fn new(calendars: &'a [&'a dyn HolidayCalendar]) -> Self {
        Self {
            calendars,
            mode: CompositeMode::Union,
        }
    }

    /// Construct a composite calendar with an explicit mode.
    /// When `CompositeMode::Intersection`, a date is a holiday only if all sub-calendars
    /// mark it as a holiday. With `CompositeMode::Union`, union semantics are used.
    #[must_use]
    pub const fn with_mode(calendars: &'a [&'a dyn HolidayCalendar], mode: CompositeMode) -> Self {
        Self { calendars, mode }
    }
}

impl HolidayCalendar for CompositeCalendar<'_> {
    fn is_holiday(&self, date: Date) -> bool {
        match self.mode {
            CompositeMode::Union => {
                // Empty slice ⇒ no holidays, so return false.
                self.calendars.iter().any(|c| c.is_holiday(date))
            }
            CompositeMode::Intersection => {
                if self.calendars.is_empty() {
                    return false;
                }
                self.calendars.iter().all(|c| c.is_holiday(date))
            }
        }
    }
}

// ----------------------------------------------------------------------------------------------
// Tests
// ----------------------------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;
    use crate::dates::calendar::{GBLO, TARGET2};
    use time::{Date, Month};

    #[test]
    fn union_vs_intersection() {
        let t2 = TARGET2;
        let gb = GBLO;
        let calendars = [&t2 as &dyn HolidayCalendar, &gb as &dyn HolidayCalendar];

        let cal_union = CompositeCalendar::new(&calendars);
        let cal_inter = CompositeCalendar::with_mode(&calendars, CompositeMode::Intersection);

        // Date that is holiday in both calendars (New Year's Day)
        let d1 = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");
        assert!(cal_union.is_holiday(d1));
        assert!(cal_inter.is_holiday(d1));

        // Date that is holiday only in GBLO (Spring bank holiday 26-May-2025)
        let d2 = Date::from_calendar_date(2025, Month::May, 26).expect("Valid test date");
        assert!(GBLO.is_holiday(d2));
        assert!(!TARGET2.is_holiday(d2));

        assert!(cal_union.is_holiday(d2));
        assert!(!cal_inter.is_holiday(d2));
    }

    #[test]
    fn test_composite_mode_serde_roundtrip() {
        use serde_json;

        // Test CompositeMode serialization
        let modes = vec![CompositeMode::Union, CompositeMode::Intersection];

        for mode in modes {
            let json =
                serde_json::to_string(&mode).expect("JSON serialization should succeed in test");
            let deserialized: CompositeMode =
                serde_json::from_str(&json).expect("JSON deserialization should succeed in test");
            assert_eq!(mode, deserialized);
        }
    }
}
