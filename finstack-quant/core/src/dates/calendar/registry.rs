//! Calendar registry for resolving calendars by identifier.
//!
//! Provides a global registry for looking up built-in holiday calendars by
//! their standard codes (e.g., "target2", "nyse", "gblo"). Supports both
//! string-based and typed ID resolution.

use crate::dates::calendar::calendar_by_id;
use crate::dates::calendar::HolidayCalendar;
use crate::types::CalendarId;
use core::marker::PhantomData;
use std::sync::OnceLock;

/// Global, immutable registry for resolving calendars by typed ID.
///
/// The registry is process-wide, lock-free after initialization, and contains
/// only built-in calendars generated at build time or registered by the crate.
pub struct CalendarRegistry<'a> {
    _marker: PhantomData<&'a ()>,
}

impl CalendarRegistry<'_> {
    /// Obtain the global registry instance.
    #[inline]
    pub fn global() -> &'static CalendarRegistry<'static> {
        static INSTANCE: OnceLock<CalendarRegistry> = OnceLock::new();
        INSTANCE.get_or_init(|| CalendarRegistry {
            _marker: PhantomData,
        })
    }

    /// Resolve a calendar by its lowercase code string (e.g., "gblo").
    #[inline]
    pub fn resolve_str(&self, code: &str) -> Option<&'static dyn HolidayCalendar> {
        calendar_by_id(code)
    }

    /// Resolve a calendar by the canonical typed calendar identifier.
    #[inline]
    pub fn resolve(&self, id: &CalendarId) -> Option<&'static dyn HolidayCalendar> {
        self.resolve_str(id.as_str())
    }

    /// Resolve many calendars by id, returning them as an owned `Vec`.
    ///
    /// Unknown ids are silently dropped; the returned order matches input order
    /// for the ids that did resolve. Build a
    /// [`CompositeCalendar`](crate::dates::CompositeCalendar) by borrowing the
    /// returned `Vec` as a slice:
    ///
    /// ```
    /// # use finstack_quant_core::dates::{CalendarRegistry, CompositeCalendar, CompositeMode};
    /// # use finstack_quant_core::dates::calendar::{TARGET2, GBLO};
    /// # use finstack_quant_core::types::CalendarId;
    /// # use finstack_quant_core::dates::HolidayCalendar;
    /// let ids = [
    ///     CalendarId::from(TARGET2.id()),
    ///     CalendarId::from(GBLO.id()),
    /// ];
    /// let regs = CalendarRegistry::global();
    /// let v = regs.resolve_many_vec(&ids);
    /// let composite = CompositeCalendar::with_mode(&v[..], CompositeMode::Union);
    /// # let _ = composite.is_holiday(time::Date::from_calendar_date(2025, time::Month::January, 1).expect("Valid date"));
    /// ```
    #[inline]
    pub fn resolve_many_vec(&self, ids: &[CalendarId]) -> Vec<&'static dyn HolidayCalendar> {
        let mut out: Vec<&'static dyn HolidayCalendar> = Vec::with_capacity(ids.len());
        for id in ids {
            if let Some(c) = self.resolve(id) {
                out.push(c);
            }
        }
        out
    }

    /// Return the list of available calendar identifiers.
    #[inline]
    pub fn available_ids(&self) -> &'static [&'static str] {
        crate::dates::available_calendars()
    }
}

// ----------------------------------------------------------------------------------------------
// Tests
// ----------------------------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;
    use crate::dates::calendar::{CompositeCalendar, CompositeMode, GBLO, TARGET2};
    use crate::types::CalendarId;
    use time::{Date, Month};

    #[test]
    fn resolve_many_vec_builds_composite_without_leak() {
        let ids = [CalendarId::from(TARGET2.id()), CalendarId::from(GBLO.id())];
        let regs = CalendarRegistry::global();
        let v = regs.resolve_many_vec(&ids);
        assert_eq!(v.len(), 2);

        let composite = CompositeCalendar::with_mode(&v[..], CompositeMode::Union);

        // Jan 1 is a holiday for both; union should be holiday.
        let d1 = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");
        assert!(composite.is_holiday(d1));

        // Date that is holiday in GBLO but not necessarily in Target2 (e.g., 26-May-2025)
        let d2 = Date::from_calendar_date(2025, Month::May, 26).expect("Valid test date");
        assert!(GBLO.is_holiday(d2));
        assert!(composite.is_holiday(d2));
    }
}
