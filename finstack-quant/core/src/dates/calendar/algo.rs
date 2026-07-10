//! Algorithmic holiday helpers for calendar computations.
//!
//! This module provides deterministic, allocation-free implementations of
//! holiday date calculations used across multiple calendar modules. Each
//! algorithm is defined once and reused to ensure consistency.
//!
//! # Features
//!
//! - **Easter Monday**: Anonymous Gregorian algorithm for Western Easter
//! - **Chinese New Year**: Pre-computed lookup table (1970-2150)
//! - **Zero allocation**: All functions are stack-only
//! - **Panic-free**: Safe for all valid `time::Date` ranges
//!
//! # Supported Range
//!
//! Chinese New Year dates are available for years 1970-2150. Easter Monday
//! can be computed for any valid Gregorian year.

use time::{Date, Duration, Month};

// -------------------------------------------------------------------------------------------------
// Easter
// -------------------------------------------------------------------------------------------------

/// Computes Easter Monday for a given Gregorian year.
///
/// Uses the Anonymous Gregorian algorithm (also known as Meeus/Jones/Butcher
/// algorithm) to calculate Easter Sunday, then returns the following Monday.
/// Easter Monday is a public holiday in many European and Commonwealth countries.
///
/// # Algorithm
///
/// The algorithm computes Easter Sunday using purely arithmetic operations
/// without iteration, based on the Metonic cycle (19-year lunar cycle) and
/// solar corrections for the Gregorian calendar.
///
/// # Arguments
///
/// * `year` - Gregorian calendar year (valid range: any year supported by `time::Date`)
///
/// # Returns
///
/// The `Date` of Easter Monday (the day after Easter Sunday) for the given year.
///
/// # Panics
///
/// Never panics for valid Gregorian years within the `time` crate's supported range.
/// The algorithm guarantees Easter falls between March 22 and April 25 (Sunday),
/// so Easter Monday falls between March 23 and April 26.
///
/// # References
///
/// - Meeus, J. (1991). *Astronomical Algorithms*. Willmann-Bell. Chapter 8.
/// - Butcher, S. (1876). "Ecclesiastical Calendar." In *The Calculation of Easter*.
/// - Algorithm widely known as "Anonymous Gregorian Algorithm" or "Meeus/Jones/Butcher"
///
/// # Examples
///
/// ```rust,compile_fail
/// // This helper is internal (pub(crate)); it is not part of the public API.
/// use finstack_quant_core::dates::calendar::algo::easter_monday;
/// let _ = easter_monday(2025);
/// ```
#[inline]
pub(crate) fn easter_monday(year: i32) -> Date {
    // Anonymous Gregorian algorithm
    let a = year % 19;
    let b = year / 100;
    let c = year % 100;
    let d = b / 4;
    let e = b % 4;
    let f = (b + 8) / 25;
    let g = (b - f + 1) / 3;
    let h = (19 * a + b - d - g + 15) % 30;
    let i = c / 4;
    let k = c % 4;
    let l = (32 + 2 * e + 2 * i - h - k) % 7;
    let m = (a + 11 * h + 22 * l) / 451;
    let month_num = (h + l - 7 * m + 114) / 31; // 3=March 4=April
    let day = ((h + l - 7 * m + 114) % 31) + 1; // Easter Sunday
    let month = if month_num == 3 {
        Month::March
    } else {
        Month::April
    };
    // Easter algorithm always produces valid March 22-April 25 dates.
    let easter_sunday = Date::from_calendar_date(year, month, day as u8).unwrap_or_else(|_| {
        unreachable!("Anonymous Gregorian algorithm produces valid March/April dates")
    });
    easter_sunday + Duration::days(1) // Easter Monday = Sunday + 1
}

// -------------------------------------------------------------------------------------------------
// Chinese New Year (generated lookup, 1970-2150)
// -------------------------------------------------------------------------------------------------

// The generated table provides `cny_date_for_year` and `is_cny_date` helpers.
include!("../../generated/cny_generated.rs");

/// Tests whether a given date is Chinese New Year (Spring Festival).
///
/// Chinese New Year is celebrated on the second new moon after winter solstice,
/// typically falling between January 21 and February 20 in the Gregorian calendar.
///
/// This function uses a pre-computed lookup table generated from astronomical
/// calculations for years 1970-2150.
///
/// # Arguments
///
/// * `date` - The date to check
///
/// # Returns
///
/// `true` if `date` is Chinese New Year, `false` otherwise. Returns `false`
/// for years outside the supported range (1970-2150).
///
/// # Examples
///
/// ```rust,compile_fail
/// // This helper is internal (pub(crate)); it is not part of the public API.
/// use finstack_quant_core::dates::calendar::algo::is_cny;
/// let _ = is_cny(time::macros::date!(2025 - 01 - 29));
/// ```
///
/// # References
///
/// - Dates computed from Chinese lunar calendar astronomical algorithms
/// - Generated table covers 1970-2150 (standard financial system date range)
#[inline]
pub(crate) fn is_cny(date: Date) -> bool {
    is_cny_date(date.year(), date.month() as u8, date.day())
}

/// Returns the Chinese New Year date for a given year, if available.
///
/// Chinese New Year (Spring Festival, 春节) is the most important traditional
/// Chinese holiday, celebrated on the first day of the Chinese lunar calendar.
///
/// This function uses a pre-computed lookup table for years 1970-2150.
///
/// # Arguments
///
/// * `year` - Gregorian calendar year (supported: 1970-2150)
///
/// # Returns
///
/// `Some(Date)` with the Chinese New Year date for the given year, or `None`
/// if the year is outside the supported range.
///
/// # Examples
///
/// ```rust,compile_fail
/// // This helper is internal (pub(crate)); it is not part of the public API.
/// use finstack_quant_core::dates::calendar::algo::cny_date;
/// let _ = cny_date(2025);
/// ```
///
/// # References
///
/// - Dates computed from Chinese lunar calendar astronomical algorithms
/// - Generated table covers 1970-2150 (standard financial system date range)
#[inline]
pub(crate) fn cny_date(year: i32) -> Option<Date> {
    cny_date_for_year(year)
        .and_then(|(m, d)| Date::from_calendar_date(year, Month::try_from(m).ok()?, d).ok())
}

// -------------------------------------------------------------------------------------------------
// Dragon Boat / Mid-Autumn (generated lookup, 1970-2150)
// -------------------------------------------------------------------------------------------------

// The generated table provides `dragon_boat_date_for_year`, `is_dragon_boat_date`,
// `mid_autumn_date_for_year`, and `is_mid_autumn_date` helpers.
include!("../../generated/festivals_generated.rs");

/// Returns the Dragon Boat Festival (端午节) date for a given year, if available.
///
/// Celebrated on the 5th day of the 5th Chinese lunar month (typically late May
/// to mid June). Uses a pre-computed lookup table for years 1970-2150.
#[inline]
pub(crate) fn dragon_boat_date(year: i32) -> Option<Date> {
    dragon_boat_date_for_year(year)
        .and_then(|(m, d)| Date::from_calendar_date(year, Month::try_from(m).ok()?, d).ok())
}

/// Tests whether a given date is the Dragon Boat Festival.
///
/// Returns `false` for years outside the supported range (1970-2150).
#[inline]
pub(crate) fn is_dragon_boat(date: Date) -> bool {
    is_dragon_boat_date(date.year(), date.month() as u8, date.day())
}

/// Returns the Mid-Autumn Festival (中秋节) date for a given year, if available.
///
/// Celebrated on the 15th day of the 8th Chinese lunar month (typically mid
/// September to early October). Uses a pre-computed lookup table for years
/// 1970-2150.
#[inline]
pub(crate) fn mid_autumn_date(year: i32) -> Option<Date> {
    mid_autumn_date_for_year(year)
        .and_then(|(m, d)| Date::from_calendar_date(year, Month::try_from(m).ok()?, d).ok())
}

/// Tests whether a given date is the Mid-Autumn Festival.
///
/// Returns `false` for years outside the supported range (1970-2150).
#[inline]
pub(crate) fn is_mid_autumn(date: Date) -> bool {
    is_mid_autumn_date(date.year(), date.month() as u8, date.day())
}
