//! Holiday calendars and business day adjustment conventions.
//!
//! This module implements industry-standard business day adjustment rules used
//! for cashflow date adjustments in fixed income and derivatives markets.
//!
//! # Core Components
//!
//! 1. [`HolidayCalendar`]: Trait for querying market-specific holidays
//! 2. [`BusinessDayConvention`]: ISDA-standard adjustment rules
//! 3. [`adjust`]: Date adjustment function applying conventions
//!
//! # Business Day Conventions
//!
//! Following ISDA 2006 Definitions Section 4.12:
//! - **Following**: Move to next business day
//! - **Preceding**: Move to previous business day
//! - **ModifiedFollowing**: Following, unless crosses month boundary
//! - **ModifiedPreceding**: Preceding, unless crosses month boundary
//! - **Nearest**: closer business day; a tie rolls Following (FpML / QuantLib)
//!
//! # Built-in Calendars
//!
//! Standard market calendars included:
//! - **TARGET2**: European Central Bank / SEPA
//! - **NYSE**: New York Stock Exchange
//! - **GBLO**: United Kingdom (London) bank holidays
//! - **USNY**: US Federal Reserve (New York)
//! - **JPTO**: Japan (Tokyo) bank holidays
//!
//! # Standards References
//!
//! - **ISDA**: 2006 ISDA Definitions, Section 4.12 (Business Day Conventions)
//! - **FpML**: BusinessDayConventionEnum
//! - **ISO 20022**: Business Day Convention codes
//!
//! # Examples
//!
//! ```rust
//! use finstack_quant_core::dates::{adjust, calendar_by_id_strict, BusinessDayConvention, Date};
//! use time::Month;
//!
//! let target2 = calendar_by_id_strict("target2")?;
//! let date = Date::from_calendar_date(2025, Month::January, 1).expect("Valid date"); // New Year (holiday)
//! let adjusted = adjust(date, BusinessDayConvention::Following, target2)?;
//! assert!(adjusted > date); // Moved to next business day
//! # Ok::<(), finstack_quant_core::Error>(())
//! ```

use crate::{Error, InputError};
use time::{Date, Duration};

/// Shared upper bound used across business-day search helpers to avoid runaway loops.
pub(crate) const MAX_BUSINESS_DAY_SEARCH_DAYS: i32 = 100;

/// Seek the nearest business day from `date` moving in steps of `step_days` (±1),
/// searching up to `max_days` steps using the provided `cal`.
///
/// Returns `Some(date)` when found, or `None` if no business day is found within the window.
#[inline]
pub(crate) fn seek_business_day<C: HolidayCalendar + ?Sized>(
    mut date: Date,
    step_days: i32,
    max_days: i32,
    cal: &C,
) -> Option<Date> {
    let mut searched = 0;
    while !cal.is_business_day(date) {
        date = date.checked_add(Duration::days(i64::from(step_days)))?;
        searched += 1;
        if searched > max_days {
            return None;
        }
    }
    Some(date)
}

/// Trait for market-specific holiday calendars.
///
/// Implementors define market-specific holidays in [`is_holiday`](Self::is_holiday)
/// and state their weekend rule in [`is_business_day`](Self::is_business_day).
///
/// # Implementation Guide
///
/// When implementing a custom calendar:
/// 1. Return `true` from [`is_holiday`](Self::is_holiday) for market-specific
///    non-working days (exchanges closed, bank holidays)
/// 2. Implement [`is_business_day`](Self::is_business_day) as "not a weekend
///    under the market's weekend convention and not a holiday"
///
/// # Examples
///
/// ```rust
/// use finstack_quant_core::dates::{DateExt, HolidayCalendar};
/// use time::Date;
///
/// struct CustomCalendar;
///
/// impl HolidayCalendar for CustomCalendar {
///     fn is_holiday(&self, date: Date) -> bool {
///         // Example: Only New Year's Day
///         date.month() == time::Month::January && date.day() == 1
///     }
///
///     fn is_business_day(&self, date: Date) -> bool {
///         !date.is_weekend() && !self.is_holiday(date)
///     }
/// }
///
/// let cal = CustomCalendar;
/// let new_year = Date::from_calendar_date(2025, time::Month::January, 1).expect("Valid date");
/// assert!(cal.is_holiday(new_year));
/// assert!(!cal.is_business_day(new_year)); // Holiday = not business day
/// ```
pub trait HolidayCalendar: Send + Sync {
    /// Returns `true` if `date` is a holiday according to this calendar.
    ///
    /// Notes:
    /// - Many built-in calendars also return `true` for weekends.
    /// - Some calendars intentionally ignore weekends and will return `false`
    ///   for Saturday/Sunday. This only affects `is_holiday`; use
    ///   [`HolidayCalendar::is_business_day`] for business-day logic.
    fn is_holiday(&self, date: Date) -> bool;

    /// Returns `true` if the `date` is a business day according to the
    /// calendar (i.e. not weekend and not holiday).
    ///
    /// Each calendar applies its own weekend convention here (every built-in
    /// calendar closes on Saturday and Sunday, see
    /// [`DateExt::is_weekend`](crate::dates::DateExt::is_weekend)); it is
    /// independent of how the calendar labels weekends in [`Self::is_holiday`].
    ///
    /// # Arguments
    ///
    /// * `date` - Calendar date to classify.
    fn is_business_day(&self, date: Date) -> bool;

    /// Counts business days in the half-open interval `[start, end)`.
    ///
    /// Returns zero when `start >= end`. Implementors may override this method
    /// with an equivalent optimized count.
    ///
    /// # Arguments
    ///
    /// * `start` - First date included in the count.
    /// * `end` - Boundary date excluded from the count.
    fn count_business_days(&self, start: Date, end: Date) -> i32 {
        count_business_days_by_scan(self, start, end)
    }

    /// Optional human-friendly metadata for the calendar.
    #[inline]
    fn metadata(&self) -> Option<CalendarMetadata> {
        None
    }

    /// Canonical identifier that reconstructs this calendar through the
    /// registered calendar resolver.
    ///
    /// Custom calendars should return `None` unless their identifier is
    /// registered and resolves to identical behavior.
    fn calendar_id(&self) -> Option<String> {
        self.metadata().map(|metadata| metadata.id.to_string())
    }
}

impl<T: HolidayCalendar + ?Sized> HolidayCalendar for &T {
    fn is_holiday(&self, date: Date) -> bool {
        (**self).is_holiday(date)
    }

    fn is_business_day(&self, date: Date) -> bool {
        (**self).is_business_day(date)
    }

    fn count_business_days(&self, start: Date, end: Date) -> i32 {
        (**self).count_business_days(start, end)
    }

    fn metadata(&self) -> Option<CalendarMetadata> {
        (**self).metadata()
    }

    fn calendar_id(&self) -> Option<String> {
        (**self).calendar_id()
    }
}

/// Business days in `[start, end)` by testing each date with
/// [`HolidayCalendar::is_business_day`]; zero when `start >= end`.
pub(super) fn count_business_days_by_scan<C: HolidayCalendar + ?Sized>(
    calendar: &C,
    start: Date,
    end: Date,
) -> i32 {
    let mut count = 0_i32;
    let mut current = start;
    while current < end {
        if calendar.is_business_day(current) {
            count += 1;
        }
        current += Duration::days(1);
    }
    count
}

/// Basic metadata describing a holiday calendar.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
pub struct CalendarMetadata {
    /// Lowercase identifier (stable code).
    pub id: &'static str,
    /// Human-readable name of the calendar.
    pub name: &'static str,
    /// Whether weekends are ignored when classifying holidays.
    pub ignore_weekends: bool,
}

/// Business day adjustment conventions per ISDA standards.
///
/// Defines how dates are adjusted when they fall on non-business days
/// (weekends or holidays). Used throughout fixed income and derivatives
/// markets for determining payment dates, fixing dates, and maturity dates.
///
/// # Standards References
///
/// - **ISDA**: 2006 ISDA Definitions, Section 4.12
/// - **FpML**: BusinessDayConventionEnum
/// - **ISO 20022**: Business Day Convention codes
///
/// # Examples
///
/// ```rust
/// use finstack_quant_core::dates::{adjust, calendar_by_id_strict, BusinessDayConvention, Date};
/// use time::Month;
///
/// let target2 = calendar_by_id_strict("target2")?;
/// // Saturday, January 4, 2025
/// let weekend = Date::from_calendar_date(2025, Month::January, 4).expect("Valid date");
///
/// // Following: moves to next Monday (Jan 6)
/// let adj = adjust(weekend, BusinessDayConvention::Following, target2)?;
/// assert_eq!(adj.day(), 6);
///
/// // Preceding: moves to previous Friday (Jan 3)
/// let adj = adjust(weekend, BusinessDayConvention::Preceding, target2)?;
/// assert_eq!(adj.day(), 3);
/// # Ok::<(), finstack_quant_core::Error>(())
/// ```
///
/// # Default
///
/// `BusinessDayConvention::default()` is `ModifiedFollowing`, the ISDA 2006
/// Definitions Section 4.12(c) convention used for swap and bond period dates.
/// Host bindings use it whenever a caller omits the convention.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize,
)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum BusinessDayConvention {
    /// No adjustment - date remains as specified.
    ///
    /// **ISDA**: Section 4.12(a) - "Unadjusted"
    /// **FpML**: "NONE"
    Unadjusted,

    /// Adjust to next business day (may cross month boundary).
    ///
    /// **ISDA**: Section 4.12(b) - "Following Business Day Convention"
    /// **FpML**: "FOLLOWING"
    /// **ISO 20022**: "FWNG"
    Following,

    /// Following, unless that crosses into next month, then preceding.
    ///
    /// Ensures month-end dates don't shift to next month, important for
    /// consistent period calculations in swaps and bonds.
    ///
    /// **ISDA**: Section 4.12(c) - "Modified Following Business Day Convention"
    /// **FpML**: "MODFOLLOWING"
    /// **ISO 20022**: "MODF"
    #[default]
    ModifiedFollowing,

    /// Adjust to previous business day (may cross month boundary).
    ///
    /// **ISDA**: Section 4.12(d) - "Preceding Business Day Convention"
    /// **FpML**: "PRECEDING"
    /// **ISO 20022**: "PREC"
    Preceding,

    /// Preceding, unless that crosses into previous month, then following.
    ///
    /// **ISDA**: Section 4.12(e) - "Modified Preceding Business Day Convention"
    /// **FpML**: "MODPRECEDING"
    ModifiedPreceding,

    /// Adjust to the closer business day. A tie (equal calendar-day distance)
    /// rolls Following, matching FpML `NEAREST` and QuantLib `Nearest`.
    ///
    /// **FpML**: "NEAREST"
    Nearest,
}

impl BusinessDayConvention {
    /// Stable snake_case wire name, identical to the serde representation.
    #[inline]
    pub const fn as_str(self) -> &'static str {
        match self {
            BusinessDayConvention::Unadjusted => "unadjusted",
            BusinessDayConvention::Following => "following",
            BusinessDayConvention::ModifiedFollowing => "modified_following",
            BusinessDayConvention::Preceding => "preceding",
            BusinessDayConvention::ModifiedPreceding => "modified_preceding",
            BusinessDayConvention::Nearest => "nearest",
        }
    }
}

/// `Display` emits the snake_case serde name so `to_string()` round-trips
/// through `FromStr` and matches the JSON wire form.
impl core::fmt::Display for BusinessDayConvention {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Parses the canonical snake_case serde name, ignoring surrounding whitespace
/// and ASCII case.
impl core::str::FromStr for BusinessDayConvention {
    type Err = crate::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        crate::wire::serde_parse(&s.trim().to_ascii_lowercase()).map_err(|_| {
            crate::Error::Validation(format!(
                "Unknown business day convention: '{s}'; expected one of unadjusted, following, modified_following, preceding, modified_preceding, nearest"
            ))
        })
    }
}

/// Adjust `date` according to `conv` utilising `cal` for holiday lookup.
///
/// Returns an error if no business day is found within 100 days of the input date.
/// This prevents infinite loops when using composite calendars that mark
/// all days as holidays in a range.
///
/// Example:
/// ```
/// use finstack_quant_core::dates::{adjust, calendar_by_id_strict, BusinessDayConvention, Date};
/// use time::Month;
/// let cal = calendar_by_id_strict("target2").expect("built-in calendar");
/// let sat = Date::from_calendar_date(2025, Month::January, 4).expect("Valid date");
/// let adj = adjust(sat, BusinessDayConvention::Following, cal).expect("Adjustment should succeed");
/// assert_eq!(adj, Date::from_calendar_date(2025, Month::January, 6).expect("Valid date"));
/// ```
///
/// # Arguments
///
/// * `date` - Unadjusted date to test and, if necessary, roll to a business day.
/// * `conv` - Business-day convention that selects the roll direction and
///   month-crossing treatment.
/// * `cal` - Holiday calendar used to determine whether candidate dates are
///   business days.
pub fn adjust<C: HolidayCalendar + ?Sized>(
    date: Date,
    conv: BusinessDayConvention,
    cal: &C,
) -> Result<Date, Error> {
    if cal.is_business_day(date) {
        return Ok(date);
    }
    let seek = |step: i32| seek_business_day(date, step, MAX_BUSINESS_DAY_SEARCH_DAYS, cal);
    let failed = || adjustment_failed(date, conv);
    // Search direction and whether crossing the month end reverses it.
    let (step, modified) = match conv {
        BusinessDayConvention::Unadjusted => return Ok(date),
        BusinessDayConvention::Following => (1, false),
        BusinessDayConvention::ModifiedFollowing => (1, true),
        BusinessDayConvention::Preceding => (-1, false),
        BusinessDayConvention::ModifiedPreceding => (-1, true),
        BusinessDayConvention::Nearest => {
            return match (seek(1), seek(-1)) {
                (Some(f), Some(b)) => {
                    let df = (f - date).whole_days().unsigned_abs();
                    let db = (date - b).whole_days().unsigned_abs();
                    Ok(if df <= db { f } else { b })
                }
                (Some(f), None) => Ok(f),
                (None, Some(b)) => Ok(b),
                (None, None) => Err(failed()),
            };
        }
    };
    let candidate = seek(step).ok_or_else(failed)?;
    if modified && candidate.month() != date.month() {
        return seek(-step).ok_or_else(failed);
    }
    Ok(candidate)
}

/// The error for a business-day search that found nothing within
/// [`MAX_BUSINESS_DAY_SEARCH_DAYS`].
pub(crate) fn adjustment_failed(date: Date, convention: BusinessDayConvention) -> Error {
    Error::Input(InputError::AdjustmentFailed {
        date,
        convention,
        max_days: MAX_BUSINESS_DAY_SEARCH_DAYS,
    })
}

// Runtime discovery helpers

/// Returns the identifiers of all built-in holiday calendars that have been
/// compiled into the crate.
///
/// Identifiers are lowercase, stable market codes (e.g., `"gblo"`, `"target2"`).
/// They are suitable for serialization and long-lived pipelines.
///
/// Example using the strict resolution API:
/// ```
/// use finstack_quant_core::dates::{available_calendars, calendar_by_id_strict};
/// use finstack_quant_core::types::CalendarId;
/// let ids = available_calendars();
/// let id = CalendarId::from(ids[0]);
/// assert!(calendar_by_id_strict(id.as_str()).is_ok());
/// ```
#[inline]
pub const fn available_calendars() -> &'static [&'static str] {
    super::calendars_generated::registry::ALL_IDS
}

/// Resolve a calendar ID strictly, naming close matches on a miss.
///
/// This is the one resolver behind every optional-calendar API in the
/// workspace; use it instead of pairing the generated registry's `calendar_by_id` with
/// `Error::calendar_not_found_with_suggestions` by hand.
///
/// # Arguments
///
/// * `id` - Canonical lowercase calendar identifier (for example `"nyse"` or
///   `"target2"`); see [`available_calendars`] for the registry. Joining
///   identifiers with `+` (for example `"nyse+gblo"`) resolves to a union
///   calendar that is a business day only when every member is.
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
    super::calendars_generated::registry::calendar_by_id(id)
        .ok_or_else(|| crate::Error::calendar_not_found_with_suggestions(id, available_calendars()))
}

/// Canonical identifier of the calendar an id resolves to.
///
/// A built-in calendar reports its registry id (`"USNY"` gives `"usny"`); a
/// `+`-joined union reports its members trimmed, lower-cased, sorted and
/// de-duplicated (`"GBLO + nyse"` gives `"gblo+nyse"`), which is the key the
/// union is interned under. Two ids with the same canonical form resolve to
/// the same calendar.
///
/// # Arguments
///
/// * `id` - Calendar identifier accepted by [`calendar_by_id_strict`]: a
///   built-in id in any case, or `+`-joined built-in ids.
///
/// # Errors
///
/// Returns `InputError::CalendarNotFound` (with suggestions) when `id` or any
/// `+`-joined member is not a built-in calendar.
pub fn canonical_calendar_id(id: &str) -> crate::Result<String> {
    let calendar = calendar_by_id_strict(id)?;
    Ok(match calendar.metadata() {
        Some(metadata) => metadata.id.to_string(),
        None => joint_members(id).join("+"),
    })
}

/// Members of a `+`-joined calendar id: trimmed, lower-cased, sorted, de-duplicated.
pub(super) fn joint_members(id: &str) -> Vec<String> {
    let mut parts: Vec<String> = id
        .split('+')
        .map(|p| p.trim().to_ascii_lowercase())
        .filter(|p| !p.is_empty())
        .collect();
    parts.sort_unstable();
    parts.dedup();
    parts
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
    let parts = joint_members(id);
    if parts.is_empty() {
        return Err(crate::Error::calendar_not_found_with_suggestions(
            id,
            available_calendars(),
        ));
    }
    let members = parts
        .iter()
        .map(|p| {
            super::calendars_generated::registry::calendar_by_id(p).ok_or_else(|| {
                crate::Error::calendar_not_found_with_suggestions(p, available_calendars())
            })
        })
        .collect::<crate::Result<Vec<&'static dyn HolidayCalendar>>>()?;
    if let [single] = members.as_slice() {
        return Ok(*single);
    }
    let key = parts.join("+");
    let mut interned = crate::recover_lock(JOINT_CALENDARS.get_or_init(Default::default).lock());
    if let Some(existing) = interned.get(&key) {
        return Ok(*existing);
    }
    let member_slice: &'static [&'static dyn HolidayCalendar] =
        Box::leak(members.into_boxed_slice());
    let composite: &'static super::composite::CompositeCalendar<'static> = Box::leak(Box::new(
        super::composite::CompositeCalendar::new(member_slice),
    ));
    interned.insert(key, composite);
    Ok(composite)
}

/// Resolve a calendar ID to a calendar reference.
///
/// # Errors
///
/// Returns an error if the calendar ID is not recognized.
/// If `cal_id` is `None`, returns the weekends-only calendar (does not error).
///
/// # Examples
///
/// ```
/// # use finstack_quant_core::dates::resolve_calendar;
/// // Valid calendar ID
/// let cal = resolve_calendar(Some("nyse")).expect("NYSE calendar should exist");
///
/// // Explicit None uses weekends-only calendar (no error)
/// let weekends = resolve_calendar(None).expect("None should use weekends-only");
///
/// // Unknown calendar ID errors
/// let err = resolve_calendar(Some("unknown_cal"));
/// assert!(err.is_err());
/// ```
///
/// # Arguments
///
/// * `cal_id` - Optional canonical calendar ID. `None` explicitly selects the
///   weekends-only calendar; an unrecognized ID returns an error. `+`-joined
///   ids such as `"nyse+gblo"` resolve to a union calendar (see
///   [`calendar_by_id_strict`]).
pub fn resolve_calendar(cal_id: Option<&str>) -> crate::Result<&'static dyn HolidayCalendar> {
    match cal_id {
        Some(id) => calendar_by_id_strict(id),
        // Only use weekends_only if explicitly None (not as fallback)
        None => Ok(&super::WEEKENDS_ONLY),
    }
}

/// Resolve the joint calendar of a currency pair.
///
/// Both ids present resolve to the interned union calendar (`"base+quote"`,
/// a business day only when both members are); one id resolves to that
/// calendar alone (every built-in calendar already closes on weekends); none
/// resolves to weekends only.
/// Step and adjust on the result with [`crate::dates::DateExt::add_business_days`]
/// and [`adjust`]; the USD-aware market spot rule lives in
/// [`crate::dates::fx::fx_spot_date`].
///
/// # Arguments
///
/// * `base_cal_id` - Base-currency calendar id, or `None` for weekends only.
/// * `quote_cal_id` - Quote-currency calendar id, or `None` for weekends only.
///
/// # Errors
///
/// Returns `InputError::CalendarNotFound` when an id is not a built-in calendar.
pub fn resolve_joint_calendar(
    base_cal_id: Option<&str>,
    quote_cal_id: Option<&str>,
) -> crate::Result<&'static dyn HolidayCalendar> {
    match (base_cal_id, quote_cal_id) {
        (None, None) => Ok(&super::WEEKENDS_ONLY),
        (Some(id), None) | (None, Some(id)) => calendar_by_id_strict(id),
        (Some(base), Some(quote)) => calendar_by_id_strict(&format!("{base}+{quote}")),
    }
}

#[cfg(test)]
mod serde_tests {
    use super::*;
    use crate::dates::{create_date, DateExt};
    use time::macros::date;
    use time::Month;

    struct Jan29Holiday;
    struct Jan30And31Holidays;

    impl HolidayCalendar for Jan29Holiday {
        fn is_holiday(&self, date: Date) -> bool {
            date.year() == 2025 && date.month() == Month::January && date.day() == 29
        }

        fn is_business_day(&self, date: Date) -> bool {
            !date.is_weekend() && !self.is_holiday(date)
        }
    }

    impl HolidayCalendar for Jan30And31Holidays {
        fn is_holiday(&self, date: Date) -> bool {
            date.year() == 2025 && date.month() == Month::January && matches!(date.day(), 30 | 31)
        }

        fn is_business_day(&self, date: Date) -> bool {
            !date.is_weekend() && !self.is_holiday(date)
        }
    }

    static JAN_29_HOLIDAY: Jan29Holiday = Jan29Holiday;
    static JAN_30_AND_31_HOLIDAYS: Jan30And31Holidays = Jan30And31Holidays;

    #[test]
    fn test_business_day_convention_serde_roundtrip() {
        use serde_json;

        // Test all BusinessDayConvention variants
        let conventions = vec![
            BusinessDayConvention::Unadjusted,
            BusinessDayConvention::Following,
            BusinessDayConvention::ModifiedFollowing,
            BusinessDayConvention::Preceding,
            BusinessDayConvention::ModifiedPreceding,
            BusinessDayConvention::Nearest,
        ];

        for conv in conventions {
            let json =
                serde_json::to_string(&conv).expect("JSON serialization should succeed in test");
            let deserialized: BusinessDayConvention =
                serde_json::from_str(&json).expect("JSON deserialization should succeed in test");
            assert_eq!(conv, deserialized);
        }
    }

    #[test]
    fn test_business_day_convention_snake_case() {
        use serde_json;

        // Test that the snake_case renaming works
        let conv = BusinessDayConvention::ModifiedFollowing;
        let json = serde_json::to_string(&conv).expect("JSON serialization should succeed in test");
        assert_eq!(json, "\"modified_following\"");

        let conv = BusinessDayConvention::ModifiedPreceding;
        let json = serde_json::to_string(&conv).expect("JSON serialization should succeed in test");
        assert_eq!(json, "\"modified_preceding\"");
    }

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

    #[test]
    fn canonical_id_is_registry_id_for_builtin_calendars() {
        assert_eq!(canonical_calendar_id("NYSE").expect("nyse"), "nyse");
    }

    #[test]
    fn canonical_id_normalizes_union_members() {
        assert_eq!(
            canonical_calendar_id(" nyse + GBLO+nyse").expect("union"),
            "gblo+nyse"
        );
        // A union of one distinct member is that built-in calendar.
        assert_eq!(canonical_calendar_id("nyse+NYSE").expect("single"), "nyse");
    }

    #[test]
    fn canonical_id_rejects_unknown_members() {
        assert!(canonical_calendar_id("nyse+nope").is_err());
        assert!(canonical_calendar_id("+").is_err());
    }

    #[test]
    fn test_resolve_calendar_unknown_id() {
        let result = resolve_calendar(Some("unknown_calendar_id"));
        assert!(result.is_err(), "Unknown calendar ID should error");
        let err = result.err().expect("unknown calendar must error");
        assert!(
            matches!(err, Error::Input(ref e) if matches!(
                e,
                crate::InputError::CalendarNotFound { .. }
            )),
            "Should be CalendarNotFound error, got: {:?}",
            err
        );
    }

    #[test]
    fn test_resolve_calendar_explicit_none() {
        let result = resolve_calendar(None);
        assert!(result.is_ok(), "Explicit None should not error");
        let cal = result.unwrap();
        let saturday = create_date(2024, Month::January, 13).unwrap();
        let sunday = create_date(2024, Month::January, 14).unwrap();
        let monday = create_date(2024, Month::January, 15).unwrap();
        assert!(!cal.is_business_day(saturday));
        assert!(!cal.is_business_day(sunday));
        assert!(cal.is_business_day(monday));
    }

    #[test]
    fn modified_following_is_evaluated_on_the_union_calendar() {
        let members: [&dyn HolidayCalendar; 2] = [&JAN_29_HOLIDAY, &JAN_30_AND_31_HOLIDAYS];
        let joint = crate::dates::calendar::composite::CompositeCalendar::new(&members);

        let date = create_date(2025, Month::January, 29).unwrap();
        let adjusted = adjust(date, BusinessDayConvention::ModifiedFollowing, &joint)
            .expect("Union calendar adjustment should succeed");

        let expected = create_date(2025, Month::January, 28).unwrap();
        assert_eq!(
            adjusted, expected,
            "ModifiedFollowing must be evaluated on the joint calendar, not sequentially"
        );
    }
}
