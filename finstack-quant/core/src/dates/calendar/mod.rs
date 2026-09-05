//! Holiday calendar system for financial markets.
//!
//! Provides deterministic, high-performance holiday calendars for scheduling
//! cashflows, adjusting payment dates, and validating business days across
//! global financial markets.
//!
//! # Features
//!
//! - **26 built-in market calendars**: major exchanges, central banks, and
//!   settlement systems, generated at build time from `data/calendars/*.json`
//!   (see [`available_calendars`] for the exact identifier list)
//! - **Rule-based definitions**: JSON-defined rules for transparency and auditability
//! - **Cached rule evaluation**: validated years are materialized lazily into a
//!   process-wide holiday bitset and business-day prefix sums; out-of-range
//!   dates continue to scan the calendar's `&'static` rules directly
//! - **Composite calendars**: Combine multiple calendars for multi-currency schedules
//! - **Business day adjustments**: Following, Modified Following, Preceding,
//!   Modified Preceding, Nearest conventions
//!
//! # Lookup Cost
//!
//! The first lookup for a calendar year inside the validated range materializes
//! a 366-bit raw rule-holiday mask and business-day prefix sums. Subsequent
//! holiday and business-day predicates are constant-time bit lookups, while
//! interval counts combine at most one prefix-sum lookup per year. Dates outside
//! the validated range retain direct rule scanning.
//!
//! The cache is an implementation detail: public predicates are unchanged.
//! [`HolidayCalendar::is_holiday`] still applies each calendar's
//! `ignore_weekends` behavior, and business-day checks still combine raw rule
//! holidays with the calendar's configured weekend rule.
//!
//! # Supported Date Range
//!
//! Holiday rules are validated for years **1970-2150**. Years outside this range
//! still evaluate via the same rules (a one-time warning is emitted), but their
//! accuracy is not guaranteed.
//!
//! # Key Concepts
//!
//! ## Holiday vs. Business Day
//!
//! - **Holiday**: Non-working date as defined by a specific market calendar
//!   (e.g., Christmas, Lunar New Year, bank holidays)
//! - **Business day**: Any day that is not a weekend (Saturday/Sunday) AND not
//!   a market-specific holiday
//!
//! Many calendars include weekends in their holiday definitions for convenience,
//! while others intentionally omit them. Regardless, [`HolidayCalendar::is_business_day`]
//! always treats Saturday/Sunday as non-business days.
//!
//! **Guideline**: Use `is_business_day` for scheduling and date adjustments.
//! Use `is_holiday` only when you need market-specific holiday information.
//!
//! # Quick Example
//!
//! ```rust
//! use finstack_quant_core::dates::{adjust, BusinessDayConvention, HolidayCalendar};
//! use finstack_quant_core::dates::calendar_by_id;
//! use time::{Date, Month};
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//!
//! // Get New York Stock Exchange calendar
//! let nyse = calendar_by_id("nyse")
//!     .ok_or("NYSE calendar not found")?;
//!
//! // Check if a date is a business day
//! let date = Date::from_calendar_date(2025, Month::December, 25)?;
//! assert!(!nyse.is_business_day(date)); // Christmas is not a business day
//!
//! // Adjust date to next business day
//! let adjusted = adjust(date, BusinessDayConvention::Following, nyse)?;
//! assert_eq!(adjusted, Date::from_calendar_date(2025, Month::December, 26)?);
//! # Ok(())
//! # }
//! ```
//!
//! # Calendar Types
//!
//! - **Exchange calendars**: NYSE, LSE, TSE, HKEX, etc.
//! - **Settlement calendars**: TARGET (Eurozone), USGS (US Government Securities)
//! - **Central bank calendars**: Federal Reserve, ECB, BOE, BOJ
//! - **Country calendars**: Nationwide holidays (US, UK, JP, etc.)
//!
//! # Architecture
//!
//! - `rule`: Rule-based holiday definitions (Easter, IMM, lunar calendars)
//! - generated free functions for calendar lookup and discovery
//! - `business_days`: Business day adjustment and counting
//! - `composite`: Multi-calendar union support
//! - `generated`: Build-time generated year-range constants and shared date helpers
//! - `year_cache`: Lazy year holiday bitsets and business-day prefix sums
//!
//! # See Also
//!
//! - [`HolidayCalendar`] for the core trait
//! - [`calendar_by_id`] for calendar lookup by code
//! - [`available_calendars`] for discovery of supported calendar identifiers
//! - `BusinessDayConvention` for adjustment conventions
//! - `CompositeCalendar` for combining calendars

pub(crate) mod algo;
pub(crate) mod business_days;
pub(crate) mod composite;
pub(crate) mod rule;
pub(crate) mod types;
mod year_cache;

// `finstack_quant_core::dates::*` is the canonical facade for adjustment, registry, and
// calendar traits. This namespace keeps the generated calendars and related
// implementation types available for callers that need them directly.
pub use business_days::{adjust, available_calendars, BusinessDayConvention, HolidayCalendar};
pub use composite::CompositeCalendar;
pub use rule::{Direction, Observed, Rule};
pub use types::{Calendar, WeekendRule};

// Isolate generated `use` imports from this module's public re-exports.
mod calendars_generated {
    include!(concat!(env!("OUT_DIR"), "/calendars.rs"));
}

pub use calendars_generated::*;

/// Resolve a calendar ID strictly, naming close matches on a miss.
///
/// This is the one resolver behind every optional-calendar API in the
/// workspace; use it instead of pairing [`calendar_by_id`] with
/// `Error::calendar_not_found_with_suggestions` by hand.
///
/// # Arguments
///
/// * `id` - Canonical lowercase calendar identifier (for example `"nyse"` or
///   `"target2"`); see [`available_calendars`] for the registry. Joining
///   identifiers with `+` (for example `"nyse+gblo"`) resolves to a union
///   [`CompositeCalendar`] that is a business day only when every member is.
///
/// # Errors
///
/// Returns `InputError::CalendarNotFound` carrying fuzzy suggestions drawn
/// from [`available_calendars`] when `id` (or any `+`-joined member) is not
/// a built-in calendar.
pub fn calendar_by_id_strict(id: &str) -> crate::Result<&'static dyn HolidayCalendar> {
    if id.contains('+') {
        return joint_calendar(id);
    }
    calendar_by_id(id)
        .ok_or_else(|| crate::Error::calendar_not_found_with_suggestions(id, available_calendars()))
}

/// Interned union calendars keyed by their normalized `a+b` identifier.
///
/// Composite calendars borrow their members, so a `'static` handle needs a
/// `'static` member slice; leaking one boxed composite per distinct
/// combination is bounded by the number of combinations a process ever asks
/// for and keeps the resolver signature identical for built-in and joint ids.
static JOINT_CALENDARS: std::sync::OnceLock<
    std::sync::Mutex<std::collections::HashMap<String, &'static dyn HolidayCalendar>>,
> = std::sync::OnceLock::new();

/// Resolve a `+`-joined identifier such as `"nyse+gblo"` to a union calendar.
///
/// Members are trimmed, lower-cased, sorted and de-duplicated, so
/// `"GBLO + nyse"` and `"nyse+gblo"` share one interned composite; a single
/// distinct member resolves to that built-in calendar directly.
fn joint_calendar(id: &str) -> crate::Result<&'static dyn HolidayCalendar> {
    let mut parts: Vec<String> = id
        .split('+')
        .map(|p| p.trim().to_ascii_lowercase())
        .filter(|p| !p.is_empty())
        .collect();
    parts.sort_unstable();
    parts.dedup();
    if parts.is_empty() {
        return Err(crate::Error::calendar_not_found_with_suggestions(
            id,
            available_calendars(),
        ));
    }
    let members = parts
        .iter()
        .map(|p| {
            calendar_by_id(p).ok_or_else(|| {
                crate::Error::calendar_not_found_with_suggestions(p, available_calendars())
            })
        })
        .collect::<crate::Result<Vec<&'static dyn HolidayCalendar>>>()?;
    if let [single] = members.as_slice() {
        return Ok(*single);
    }
    let key = parts.join("+");
    let mut interned = JOINT_CALENDARS
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some(existing) = interned.get(&key) {
        return Ok(*existing);
    }
    let member_slice: &'static [&'static dyn HolidayCalendar] =
        Box::leak(members.into_boxed_slice());
    let composite: &'static CompositeCalendar<'static> =
        Box::leak(Box::new(CompositeCalendar::new(member_slice)));
    interned.insert(key, composite);
    Ok(composite)
}

/// Resolve typed calendar identifiers strictly and preserve their input order.
///
/// # Errors
///
/// Returns an error naming the first unknown identifier. Unlike the removed
/// registry helper, unknown identifiers are never silently dropped.
///
/// # Arguments
///
/// * `ids` - Typed calendar IDs to resolve in order. Each ID must name a
///   built-in calendar; the output preserves this input order.
pub fn calendars_by_ids(
    ids: &[crate::types::CalendarId],
) -> crate::Result<Vec<&'static dyn HolidayCalendar>> {
    ids.iter()
        .map(|id| calendar_by_id_strict(id.as_str()))
        .collect()
}

#[cfg(test)]
mod joint_tests {
    use super::*;
    use time::macros::date;

    #[test]
    fn joint_id_resolves_to_union_calendar_and_is_interned() {
        // 2025-07-04 (US Independence Day): NYSE holiday, GBLO business day.
        let july4 = date!(2025 - 07 - 04);
        let joint = calendar_by_id_strict("nyse+gblo").expect("joint");
        assert!(!joint.is_business_day(july4));
        assert!(calendar_by_id_strict("gblo")
            .expect("gblo")
            .is_business_day(july4));
        // 2025-08-25 (UK Summer bank holiday): GBLO holiday, NYSE open.
        assert!(!joint.is_business_day(date!(2025 - 08 - 25)));

        let same = calendar_by_id_strict("GBLO + nyse").expect("normalized");
        assert!(std::ptr::eq(
            joint as *const dyn HolidayCalendar as *const u8,
            same as *const dyn HolidayCalendar as *const u8
        ));
        assert!(calendar_by_id_strict("nyse+bogus").is_err());
        assert!(calendar_by_id_strict("+").is_err());
    }
}
