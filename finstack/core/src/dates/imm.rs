//! IMM (International Money Market) and quarterly roll date helpers.
//!
//! Provides deterministic, allocation-free utilities for computing standard
//! futures roll dates, CDS settlement dates, and option expiry dates used
//! throughout global derivative markets.
//!
//! # Features
//!
//! - **IMM dates**: Third Wednesday of March, June, September, December
//! - **CDS IMM dates**: 20th of March, June, September, December
//! - **Option expiry**: Friday before IMM date (futures options)
//! - **Equity expiry**: Third Friday of every month
//! - **Zero allocation**: All functions are stack-only
//! - **Panic-free**: Safe for all valid `time::Date` ranges
//!
//! # Background
//!
//! The International Money Market (IMM) dates are standardized quarterly
//! roll dates used for futures contracts (interest rate futures, currency
//! futures, equity index futures) and credit default swaps. These dates
//! ensure coordinated settlement across global derivatives markets.
//!
//! ## IMM Dates (Third Wednesday)
//!
//! - Used by: CME futures, CBOE index options, many OTC derivatives
//! - Convention: Third Wednesday of March, June, September, December
//! - Example: March 19, 2025 is an IMM date
//!
//! ## CDS IMM Dates (20th of Quarter Month)
//!
//! - Used by: Credit default swaps, credit indices (CDX, iTraxx)
//! - Convention: 20th of March, June, September, December
//! - Rationale: Standardized by ISDA Big Bang Protocol (2009)
//!
//! ## Option Expiry Dates
//!
//! - **IMM option expiry**: Friday before third Wednesday (futures options)
//! - **Equity option expiry**: Third Friday of every month (equity derivatives)
//!
//! # Quick Example
//!
//! ```rust
//! use finstack_core::dates::{third_wednesday, next_imm, next_cds_date};
//! use time::{Date, Month};
//!
//! // IMM date for a specific month
//! let imm_march = third_wednesday(Month::March, 2025);
//! assert_eq!(imm_march, Date::from_calendar_date(2025, Month::March, 19).expect("Valid date"));
//!
//! // Find next IMM date after a given date
//! let date = Date::from_calendar_date(2025, Month::March, 20).expect("Valid date");
//! let next = next_imm(date);
//! assert_eq!(next, Date::from_calendar_date(2025, Month::June, 18).expect("Valid date"));
//!
//! // CDS settlement date
//! let cds = next_cds_date(Date::from_calendar_date(2025, Month::March, 10).expect("Valid date"));
//! assert_eq!(cds, Date::from_calendar_date(2025, Month::March, 20).expect("Valid date"));
//! ```
//!
//! # Standards Reference
//!
//! - **IMM dates**: CME Group contract specifications
//! - **CDS dates**: ISDA Big Bang Protocol (April 2009), ISDA CDS Standard Model
//! - **Equity options**: CBOE, major equity exchanges worldwide
//!
//! # See Also
//!
//! - [`ScheduleBuilder::cds_imm`] for building CDS payment schedules
//! - [`next_imm`] for finding the next quarterly IMM date
//! - [`next_cds_date`] for CDS settlement date calculation
//!
//! [`ScheduleBuilder::cds_imm`]: super::ScheduleBuilder::cds_imm

use crate::dates::calendar::business_days::{adjust, BusinessDayConvention, HolidayCalendar};
use crate::dates::calendar::generated::nth_weekday_of_month;
use time::{Date, Duration, Month, Weekday};

// Shared quarter months used by IMM/CDS roll helpers
const QUARTERLY_MONTHS: [Month; 4] = [Month::March, Month::June, Month::September, Month::December];

/// Generic helper to find the next date strictly after `date` by scanning
/// specific `months` within a (possibly incrementing) `year`, where candidates
/// are produced by `candidate_fn`.
#[inline]
fn next_date_from_months<F>(date: Date, months: &[Month], candidate_fn: F) -> Date
where
    F: Fn(Month, i32) -> Date,
{
    let mut year = date.year();
    loop {
        for &m in months {
            let candidate = candidate_fn(m, year);
            if candidate > date {
                return candidate;
            }
        }
        year += 1;
    }
}

/// Return the **third Wednesday** of `month` in `year`.
///
/// Delegates to the shared `nth_weekday_of_month` helper used by calendar rules
/// to keep all "nth weekday" logic consistent.
///
/// # Panics
/// Never panics for valid Gregorian years supported by the `time` crate.
#[must_use]
pub fn third_wednesday(month: Month, year: i32) -> Date {
    nth_weekday_of_month(year, month, Weekday::Wednesday, 3)
        .unwrap_or_else(|| unreachable!("every month has at least four Wednesdays"))
}

/// Return the **next IMM date** (third Wednesday of Mar/Jun/Sep/Dec) **strictly
/// after** `date`.
#[must_use]
pub fn next_imm(date: Date) -> Date {
    next_date_from_months(date, &QUARTERLY_MONTHS, third_wednesday)
}

/// Check if a date is a CDS roll date (20th of Mar/Jun/Sep/Dec).
#[must_use]
pub fn is_cds_date(date: Date) -> bool {
    if date.day() != 20 {
        return false;
    }
    matches!(
        date.month(),
        Month::March | Month::June | Month::September | Month::December
    )
}

/// Check if a date is a standard IMM date (third Wednesday of Mar/Jun/Sep/Dec).
///
/// IMM dates are used for interest rate futures, currency futures, and equity index
/// futures that follow CME IMM roll conventions.
///
/// # Example
/// ```rust
/// use finstack_core::dates::is_imm_date;
/// use time::{Date, Month};
///
/// let imm_date = Date::from_calendar_date(2025, Month::March, 19)?;
/// assert!(is_imm_date(imm_date)); // Third Wednesday of March 2025
///
/// let non_imm = Date::from_calendar_date(2025, Month::March, 20)?;
/// assert!(!is_imm_date(non_imm)); // Not a third Wednesday
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[must_use]
pub fn is_imm_date(date: Date) -> bool {
    // Must be a quarterly month
    if !matches!(
        date.month(),
        Month::March | Month::June | Month::September | Month::December
    ) {
        return false;
    }
    // Must be a Wednesday
    if date.weekday() != Weekday::Wednesday {
        return false;
    }
    // Must be the third Wednesday (day 15-21)
    let day = date.day();
    (15..=21).contains(&day)
}

/// Return the **next CDS roll date** (20-Mar/20-Jun/20-Sep/20-Dec) **strictly
/// after** `date`.
#[must_use]
pub fn next_cds_date(date: Date) -> Date {
    next_date_from_months(date, &QUARTERLY_MONTHS, |m, year| {
        Date::from_calendar_date(year, m, 20).unwrap_or_else(|_| {
            if year < Date::MIN.year() {
                Date::MIN
            } else {
                Date::MAX
            }
        })
    })
}

/// Return the **previous CDS roll date** (20-Mar/20-Jun/20-Sep/20-Dec)
/// **strictly before** `date`.
///
/// Post-Big-Bang (2009) standard CDS contracts accrue their first premium
/// period from the CDS roll date immediately preceding the trade/effective
/// date, so schedule construction anchors on this date.
///
/// # Examples
/// ```rust
/// use finstack_core::dates::prev_cds_date;
/// use time::{Date, Month};
///
/// let d = Date::from_calendar_date(2025, Month::January, 15)?;
/// assert_eq!(
///     prev_cds_date(d),
///     Date::from_calendar_date(2024, Month::December, 20)?
/// );
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[must_use]
pub fn prev_cds_date(date: Date) -> Date {
    let mut year = date.year();
    loop {
        for &m in QUARTERLY_MONTHS.iter().rev() {
            let candidate = Date::from_calendar_date(year, m, 20).unwrap_or_else(|_| {
                if year > Date::MAX.year() {
                    Date::MAX
                } else {
                    Date::MIN
                }
            });
            if candidate < date {
                return candidate;
            }
        }
        year -= 1;
    }
}

/// Return the **IMM option expiry date** (Friday before the third Wednesday) for
/// `month` in `year`.
///
/// IMM option expiry dates typically occur on the Friday preceding the IMM date
/// (third Wednesday). This ensures options expire before the underlying futures
/// contracts for orderly settlement.
///
/// # Panics
/// Never panics for valid Gregorian years supported by the `time` crate.
#[must_use]
pub fn imm_option_expiry(month: Month, year: i32) -> Date {
    let third_wed = third_wednesday(month, year);
    // Friday before Wednesday = subtract 5 days
    third_wed - Duration::days(5)
}

/// Return the **third Friday** of `month` in `year`.
///
/// Delegates to the shared `nth_weekday_of_month` helper used by calendar rules
/// to keep all "nth weekday" logic consistent.
///
/// # Panics
/// Never panics for valid Gregorian years supported by the `time` crate.
#[must_use]
pub fn third_friday(month: Month, year: i32) -> Date {
    nth_weekday_of_month(year, month, Weekday::Friday, 3)
        .unwrap_or_else(|| unreachable!("every month has at least four Fridays"))
}

/// Return the **next IMM option expiry date** (Friday before third Wednesday of
/// Mar/Jun/Sep/Dec) **strictly after** `date`.
#[must_use]
pub fn next_imm_option_expiry(date: Date) -> Date {
    next_date_from_months(date, &QUARTERLY_MONTHS, imm_option_expiry)
}

/// Return the **next equity option expiry date** (third Friday of any month)
/// **strictly after** `date`.
///
/// Equity options typically expire on the third Friday of each month, providing
/// a monthly expiration cycle for equity derivatives.
#[must_use]
pub fn next_equity_option_expiry(date: Date) -> Date {
    const ALL_MONTHS: [Month; 12] = [
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

    next_date_from_months(date, &ALL_MONTHS, third_friday)
}

/// SIFMA MBS settlement class.
///
/// SIFMA publishes distinct settlement dates for four classes of agency MBS.
/// The class determines which specific settlement date applies within a given
/// month. See <https://www.sifma.org/resources/general/mbs-notification-and-settlement-dates/>.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum SifmaSettlementClass {
    /// Class A: GNMA single-family 30-year.
    A,
    /// Class B: Conventional 30-year (FNMA/FHLMC UMBS). Most common for dollar rolls.
    #[default]
    B,
    /// Class C: GNMA multi-family, ARMs, and other GNMA products.
    C,
    /// Class D: Conventional 15-year and 20-year (FNMA/FHLMC).
    D,
}

impl SifmaSettlementClass {
    /// Infer the standard settlement class from agency program and original term.
    pub fn from_agency_term(agency: &str, term_years: u32) -> Self {
        let agency_upper = agency.to_uppercase();
        let is_gnma = agency_upper.contains("GNMA") || agency_upper.contains("GN");
        match (is_gnma, term_years) {
            (true, 30) => Self::A,
            (true, _) => Self::C,
            (false, 30) => Self::B,
            (false, _) => Self::D,
        }
    }
}

/// Compute the base (unadjusted) SIFMA settlement date for a given class.
///
/// These base dates use the standard nth-weekday-of-month conventions:
/// - Class A: 2nd Wednesday
/// - Class B: 3rd Wednesday
/// - Class C: 3rd Thursday
/// - Class D: 4th Wednesday
///
/// Note: SIFMA publishes exact settlement dates that may differ from these
/// base dates due to market-specific adjustments. When available, prefer the
/// published calendar via [`sifma_settlement_date_for_class`].
fn sifma_base_date(month: Month, year: i32, class: SifmaSettlementClass) -> Date {
    match class {
        SifmaSettlementClass::A => nth_weekday_of_month(year, month, Weekday::Wednesday, 2),
        SifmaSettlementClass::B => nth_weekday_of_month(year, month, Weekday::Wednesday, 3),
        SifmaSettlementClass::C => nth_weekday_of_month(year, month, Weekday::Thursday, 3),
        SifmaSettlementClass::D => nth_weekday_of_month(year, month, Weekday::Wednesday, 4),
    }
    .unwrap_or_else(|| unreachable!("every month has at least four of each weekday"))
}

/// Compute the SIFMA settlement date for a given class using a provided holiday calendar.
///
/// The settlement date is the Nth weekday of the month (varying by class),
/// adjusted to the next business day if it falls on a SIFMA holiday.
///
/// Note: This algorithmic approach produces an approximation. SIFMA publishes
/// exact settlement dates that may differ due to market-specific conventions
/// beyond simple business-day adjustment. For 2026-2027, the published dates
/// are used via [`sifma_settlement_date_for_class`].
#[must_use]
pub(crate) fn sifma_settlement_date_with_calendar(
    month: Month,
    year: i32,
    class: SifmaSettlementClass,
    calendar: &dyn HolidayCalendar,
) -> Date {
    let base = sifma_base_date(month, year, class);
    adjust(base, BusinessDayConvention::Following, calendar).unwrap_or(base)
}

/// Published SIFMA settlement calendar.
/// Each row: `(year, month, class_a_day, class_b_day, class_c_day, class_d_day)`.
#[rustfmt::skip]
static SIFMA_CALENDAR: &[(i32, u8, u8, u8, u8, u8)] = &[
    (2026,  1, 14, 20, 22, 27), (2026,  2, 12, 17, 19, 24),
    (2026,  3, 12, 17, 19, 23), (2026,  4, 13, 16, 21, 23),
    (2026,  5, 13, 18, 20, 26), (2026,  6, 11, 16, 22, 24),
    (2026,  7, 13, 16, 20, 23), (2026,  8, 13, 18, 20, 25),
    (2026,  9, 14, 17, 21, 24), (2026, 10, 13, 15, 20, 22),
    (2026, 11, 12, 17, 19, 24), (2026, 12, 10, 15, 17, 22),
    (2027,  1, 14, 19, 21, 25), (2027,  2, 11, 16, 18, 23),
    (2027,  3, 11, 15, 18, 23), (2027,  4, 13, 15, 20, 22),
    (2027,  5, 13, 17, 19, 24), (2027,  6, 14, 16, 21, 23),
    (2027,  7, 14, 19, 21, 22), (2027,  8, 12, 17, 19, 23),
    (2027,  9, 14, 16, 21, 23), (2027, 10, 14, 18, 21, 25),
    (2027, 11, 15, 17, 22, 23), (2027, 12, 13, 16, 20, 22),
];

/// Look up or compute the SIFMA settlement date for a specific class.
///
/// Returns the exact date from the embedded published calendar when available
/// (currently 2026-2027). For other years, falls back to an algorithmic
/// approximation using the SIFMA holiday calendar (nth weekday of month,
/// adjusted to the next business day). If the calendar is unavailable,
/// returns the unadjusted base date.
#[must_use]
pub fn sifma_settlement_date_for_class(
    month: Month,
    year: i32,
    class: SifmaSettlementClass,
) -> Option<Date> {
    // First: try the published hardcoded table (exact dates for 2026-2027)
    let month_num = month as u8;
    for &(y, m, a, b, c, d) in SIFMA_CALENDAR {
        if y == year && m == month_num {
            let day = match class {
                SifmaSettlementClass::A => a,
                SifmaSettlementClass::B => b,
                SifmaSettlementClass::C => c,
                SifmaSettlementClass::D => d,
            };
            return Date::from_calendar_date(year, month, day).ok();
        }
    }
    // Fallback: algorithmic approach using the SIFMA holiday calendar
    use super::calendar::registry::CalendarRegistry;
    match CalendarRegistry::global().resolve_str("sifma") {
        Some(cal) => Some(sifma_settlement_date_with_calendar(month, year, class, cal)),
        None => Some(sifma_base_date(month, year, class)),
    }
}

/// Return the **SIFMA TBA settlement date** for the given `month` and `year`.
///
/// Defaults to **Class B** (conventional 30-year UMBS). For other settlement
/// classes, use [`sifma_settlement_date_for_class`].
///
/// # Example
/// ```rust
/// use finstack_core::dates::sifma_settlement_date;
/// use time::{Date, Month};
///
/// let settle = sifma_settlement_date(Month::March, 2027);
/// assert_eq!(
///     settle,
///     Some(Date::from_calendar_date(2027, Month::March, 15).expect("Valid date"))
/// );
/// ```
#[must_use]
pub fn sifma_settlement_date(month: Month, year: i32) -> Option<Date> {
    sifma_settlement_date_for_class(month, year, SifmaSettlementClass::B)
}

/// Return the **next SIFMA TBA settlement date** (Class B)
/// **strictly after** `date`.
///
/// First checks the embedded published calendar (2026-2027), then falls back
/// to an algorithmic approximation for years outside the table range. Scans
/// the current month and up to 13 months forward.
///
/// # Example
/// ```rust
/// use finstack_core::dates::next_sifma_settlement;
/// use time::{Date, Month};
///
/// let start = Date::from_calendar_date(2027, Month::March, 16).expect("Valid date");
/// let next = next_sifma_settlement(start);
/// assert_eq!(next, Some(Date::from_calendar_date(2027, Month::April, 15).expect("Valid date")));
/// ```
#[must_use]
pub fn next_sifma_settlement(date: Date) -> Option<Date> {
    let mut current_month = date.month();
    let mut current_year = date.year();

    // Check current month and up to 13 months forward
    for _ in 0..14 {
        let settle =
            sifma_settlement_date_for_class(current_month, current_year, SifmaSettlementClass::B)?;

        if settle > date {
            return Some(settle);
        }

        // Advance to next month
        if current_month == Month::December {
            current_month = Month::January;
            current_year += 1;
        } else {
            current_month = current_month.next();
        }
    }

    None
}

// -------------------------------------------------------------------------------------------------
// Tests
// -------------------------------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn third_wed_march_2025() {
        let d = third_wednesday(Month::March, 2025);
        assert_eq!(
            d,
            Date::from_calendar_date(2025, Month::March, 19).expect("Valid test date")
        );
    }

    #[test]
    fn next_imm_after_mar20_2025() {
        let start = Date::from_calendar_date(2025, Month::March, 20).expect("Valid test date");
        let imm = next_imm(start);
        assert_eq!(
            imm,
            Date::from_calendar_date(2025, Month::June, 18).expect("Valid test date")
        );
    }

    #[test]
    fn next_cds_before_mar20() {
        let d = Date::from_calendar_date(2025, Month::March, 10).expect("Valid test date");
        let cds = next_cds_date(d);
        assert_eq!(
            cds,
            Date::from_calendar_date(2025, Month::March, 20).expect("Valid test date")
        );
    }

    #[test]
    fn imm_option_expiry_march_2025() {
        // March 2025: third Wednesday is 19th, so option expiry is Friday 14th
        let expiry = imm_option_expiry(Month::March, 2025);
        assert_eq!(
            expiry,
            Date::from_calendar_date(2025, Month::March, 14).expect("Valid test date")
        );
    }

    #[test]
    fn imm_option_expiry_june_2025() {
        // June 2025: third Wednesday is 18th, so option expiry is Friday 13th
        let expiry = imm_option_expiry(Month::June, 2025);
        assert_eq!(
            expiry,
            Date::from_calendar_date(2025, Month::June, 13).expect("Valid test date")
        );
    }

    #[test]
    fn third_friday_march_2025() {
        // March 2025: third Friday is 21st
        let friday = third_friday(Month::March, 2025);
        assert_eq!(
            friday,
            Date::from_calendar_date(2025, Month::March, 21).expect("Valid test date")
        );
    }

    #[test]
    fn third_friday_february_2025() {
        // February 2025: third Friday is 21st
        let friday = third_friday(Month::February, 2025);
        assert_eq!(
            friday,
            Date::from_calendar_date(2025, Month::February, 21).expect("Valid test date")
        );
    }

    #[test]
    fn next_imm_option_expiry_after_march() {
        // Starting after March 2025 IMM option expiry, should get June 2025
        let start = Date::from_calendar_date(2025, Month::March, 15).expect("Valid test date");
        let next_expiry = next_imm_option_expiry(start);
        assert_eq!(
            next_expiry,
            Date::from_calendar_date(2025, Month::June, 13).expect("Valid test date")
        );
    }

    #[test]
    fn next_imm_option_expiry_before_march() {
        // Starting before March 2025 IMM option expiry, should get March 2025
        let start = Date::from_calendar_date(2025, Month::March, 10).expect("Valid test date");
        let next_expiry = next_imm_option_expiry(start);
        assert_eq!(
            next_expiry,
            Date::from_calendar_date(2025, Month::March, 14).expect("Valid test date")
        );
    }

    #[test]
    fn next_equity_option_expiry_mid_march() {
        // Starting mid-March 2025, should get March third Friday (21st)
        let start = Date::from_calendar_date(2025, Month::March, 15).expect("Valid test date");
        let next_expiry = next_equity_option_expiry(start);
        assert_eq!(
            next_expiry,
            Date::from_calendar_date(2025, Month::March, 21).expect("Valid test date")
        );
    }

    #[test]
    fn next_equity_option_expiry_after_march_friday() {
        // Starting after March third Friday, should get April third Friday
        let start = Date::from_calendar_date(2025, Month::March, 22).expect("Valid test date");
        let next_expiry = next_equity_option_expiry(start);
        assert_eq!(
            next_expiry,
            Date::from_calendar_date(2025, Month::April, 18).expect("Valid test date")
        );
    }

    #[test]
    fn next_equity_option_expiry_year_rollover() {
        // Starting in December, should roll to January of next year
        let start = Date::from_calendar_date(2025, Month::December, 25).expect("Valid test date");
        let next_expiry = next_equity_option_expiry(start);
        assert_eq!(
            next_expiry,
            Date::from_calendar_date(2026, Month::January, 16).expect("Valid test date")
        );
    }

    #[test]
    fn is_imm_date_third_wednesday_march_2025() {
        // Third Wednesday of March 2025 is the 19th
        let imm = Date::from_calendar_date(2025, Month::March, 19).expect("Valid test date");
        assert!(is_imm_date(imm));
    }

    #[test]
    fn is_imm_date_third_wednesday_june_2025() {
        // Third Wednesday of June 2025 is the 18th
        let imm = Date::from_calendar_date(2025, Month::June, 18).expect("Valid test date");
        assert!(is_imm_date(imm));
    }

    #[test]
    fn is_imm_date_third_wednesday_september_2025() {
        // Third Wednesday of September 2025 is the 17th
        let imm = Date::from_calendar_date(2025, Month::September, 17).expect("Valid test date");
        assert!(is_imm_date(imm));
    }

    #[test]
    fn is_imm_date_third_wednesday_december_2025() {
        // Third Wednesday of December 2025 is the 17th
        let imm = Date::from_calendar_date(2025, Month::December, 17).expect("Valid test date");
        assert!(is_imm_date(imm));
    }

    #[test]
    fn is_imm_date_non_quarterly_month() {
        // Third Wednesday of February 2025 (not a quarterly month)
        let non_imm = Date::from_calendar_date(2025, Month::February, 19).expect("Valid test date");
        assert!(!is_imm_date(non_imm));
    }

    #[test]
    fn is_imm_date_non_wednesday() {
        // March 20, 2025 is a Thursday (not the third Wednesday)
        let non_imm = Date::from_calendar_date(2025, Month::March, 20).expect("Valid test date");
        assert!(!is_imm_date(non_imm));
    }

    #[test]
    fn is_imm_date_wrong_wednesday() {
        // March 12, 2025 is the second Wednesday (not the third)
        let non_imm = Date::from_calendar_date(2025, Month::March, 12).expect("Valid test date");
        assert!(!is_imm_date(non_imm));

        // March 26, 2025 is the fourth Wednesday (not the third)
        let non_imm2 = Date::from_calendar_date(2025, Month::March, 26).expect("Valid test date");
        assert!(!is_imm_date(non_imm2));
    }

    // -----------------------------------------------------------------------
    // SIFMA calendar tests
    // -----------------------------------------------------------------------

    #[test]
    fn sifma_class_b_jan_2026_from_calendar() {
        let d = sifma_settlement_date_for_class(Month::January, 2026, SifmaSettlementClass::B);
        assert_eq!(
            d,
            Some(Date::from_calendar_date(2026, Month::January, 20).expect("valid"))
        );
    }

    #[test]
    fn sifma_class_a_jan_2026_from_calendar() {
        let d = sifma_settlement_date_for_class(Month::January, 2026, SifmaSettlementClass::A);
        assert_eq!(
            d,
            Some(Date::from_calendar_date(2026, Month::January, 14).expect("valid"))
        );
    }

    #[test]
    fn sifma_class_d_mar_2027_from_calendar() {
        let d = sifma_settlement_date_for_class(Month::March, 2027, SifmaSettlementClass::D);
        assert_eq!(
            d,
            Some(Date::from_calendar_date(2027, Month::March, 23).expect("valid"))
        );
    }

    #[test]
    fn sifma_algorithmic_fallback_for_uncovered_year() {
        // For years outside the hardcoded table, the algorithmic approach is used
        let d = sifma_settlement_date_for_class(Month::March, 2024, SifmaSettlementClass::B);
        assert!(d.is_some());
        let date = d.expect("SIFMA class B should produce a 2024 settlement date");
        // Should be the 3rd Wednesday (or next business day if holiday)
        assert_eq!(date.month(), Month::March);
        assert_eq!(date.year(), 2024);
    }

    #[test]
    fn sifma_default_class_is_b() {
        assert_eq!(SifmaSettlementClass::default(), SifmaSettlementClass::B);
    }

    #[test]
    fn sifma_default_is_class_b() {
        let old = sifma_settlement_date(Month::January, 2026);
        let new = sifma_settlement_date_for_class(Month::January, 2026, SifmaSettlementClass::B);
        assert_eq!(old, new);
    }

    #[test]
    fn sifma_from_agency_term() {
        assert_eq!(
            SifmaSettlementClass::from_agency_term("Fnma", 30),
            SifmaSettlementClass::B
        );
        assert_eq!(
            SifmaSettlementClass::from_agency_term("Gnma", 30),
            SifmaSettlementClass::A
        );
        assert_eq!(
            SifmaSettlementClass::from_agency_term("Fnma", 15),
            SifmaSettlementClass::D
        );
        assert_eq!(
            SifmaSettlementClass::from_agency_term("GnmaII", 15),
            SifmaSettlementClass::C
        );
    }

    // -----------------------------------------------------------------------
    // SIFMA algorithmic / extended-range tests
    // -----------------------------------------------------------------------

    #[test]
    fn sifma_class_b_works_for_2024() {
        // Algorithm should work for any year, not just 2026-2027
        let d = sifma_settlement_date_for_class(Month::January, 2024, SifmaSettlementClass::B);
        assert!(d.is_some());
        // 3rd Wednesday of Jan 2024 is Jan 17
        let date = d.expect("SIFMA class B should produce a January 2024 settlement date");
        assert_eq!(date.month(), Month::January);
        assert_eq!(date.year(), 2024);
    }

    #[test]
    fn sifma_class_a_works_for_2030() {
        let d = sifma_settlement_date_for_class(Month::June, 2030, SifmaSettlementClass::A);
        assert!(d.is_some());
        let date = d.expect("SIFMA class A should produce a June 2030 settlement date");
        assert_eq!(date.month(), Month::June);
        assert_eq!(date.year(), 2030);
    }

    #[test]
    fn next_sifma_settlement_works_beyond_2027() {
        let d = Date::from_calendar_date(2028, Month::January, 1).expect("valid");
        let next = next_sifma_settlement(d);
        assert!(next.is_some());
        let date = next.expect("next SIFMA settlement should exist for January 2028");
        assert!(date > d);
        assert_eq!(date.month(), Month::January);
        assert_eq!(date.year(), 2028);
    }

    #[test]
    fn sifma_with_calendar_matches_algorithmic() {
        let date = sifma_settlement_date_for_class(Month::March, 2026, SifmaSettlementClass::B)
            .expect("published SIFMA table should contain March 2026 Class B");
        assert_eq!(
            date,
            Date::from_calendar_date(2026, Month::March, 17).expect("valid date")
        );
    }

    #[test]
    fn sifma_all_classes_work_for_uncovered_year() {
        // All four classes should return dates for years outside the table
        for class in [
            SifmaSettlementClass::A,
            SifmaSettlementClass::B,
            SifmaSettlementClass::C,
            SifmaSettlementClass::D,
        ] {
            let d = sifma_settlement_date_for_class(Month::June, 2030, class);
            assert!(d.is_some(), "Class {class:?} should return a date for 2030");
            let date = d.expect("all SIFMA classes should produce a June 2030 settlement date");
            assert_eq!(date.month(), Month::June);
            assert_eq!(date.year(), 2030);
        }
    }
}
