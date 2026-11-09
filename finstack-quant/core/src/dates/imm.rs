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
//! use finstack_quant_core::dates::{third_wednesday, next_imm, next_cds_date};
//! use time::{Date, Month};
//!
//! // IMM date for a specific month
//! let imm_march = third_wednesday(Month::March, 2025)?;
//! assert_eq!(imm_march, Date::from_calendar_date(2025, Month::March, 19).expect("Valid date"));
//!
//! // Find next IMM date after a given date
//! let date = Date::from_calendar_date(2025, Month::March, 20).expect("Valid date");
//! let next = next_imm(date)?;
//! assert_eq!(next, Date::from_calendar_date(2025, Month::June, 18).expect("Valid date"));
//!
//! // CDS settlement date
//! let cds = next_cds_date(Date::from_calendar_date(2025, Month::March, 10).expect("Valid date"))?;
//! assert_eq!(cds, Date::from_calendar_date(2025, Month::March, 20).expect("Valid date"));
//! # Ok::<(), Box<dyn std::error::Error>>(())
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

use crate::dates::calendar::algo::nth_weekday_of_month;
use time::{Date, Month, Weekday};

// Shared quarter months used by IMM/CDS roll helpers
const QUARTERLY_MONTHS: [Month; 4] = [Month::March, Month::June, Month::September, Month::December];

/// Scan direction for [`scan_months`].
#[derive(Clone, Copy)]
enum Scan {
    /// First candidate strictly after `date`.
    After,
    /// First candidate on or after `date`.
    OnOrAfter,
    /// Last candidate strictly before `date`.
    Before,
    /// Last candidate on or before `date`.
    OnOrBefore,
}

/// Generic helper to find the nearest date to `date` (in the direction and
/// inclusivity given by `scan`) by walking `months` year by year, where
/// candidates are produced by `candidate_fn`.
#[inline]
fn scan_months<F>(date: Date, months: &[Month], scan: Scan, candidate_fn: F) -> crate::Result<Date>
where
    F: Fn(Month, i32) -> crate::Result<Date>,
{
    let forward = matches!(scan, Scan::After | Scan::OnOrAfter);
    let hit = |candidate: Date| match scan {
        Scan::After => candidate > date,
        Scan::OnOrAfter => candidate >= date,
        Scan::Before => candidate < date,
        Scan::OnOrBefore => candidate <= date,
    };
    let mut year = date.year();
    loop {
        for index in 0..months.len() {
            let month = months[if forward {
                index
            } else {
                months.len() - 1 - index
            }];
            let candidate = candidate_fn(month, year)?;
            if hit(candidate) {
                return Ok(candidate);
            }
        }
        if (forward && year == Date::MAX.year()) || (!forward && year == Date::MIN.year()) {
            return Err(crate::Error::Validation(format!(
                "no matching roll date {} {date} within the supported date range",
                if forward { "after" } else { "before" }
            )));
        }
        year += if forward { 1 } else { -1 };
    }
}

/// The actual 20th of `month` in `year`.
#[inline]
fn twentieth(month: Month, year: i32) -> crate::Result<Date> {
    super::create_date(year, month, 20)
}

/// Return the **third Wednesday** of `month` in `year`.
///
/// Delegates to the shared `nth_weekday_of_month` helper used by calendar rules
/// to keep all "nth weekday" logic consistent.
///
/// # Arguments
///
/// * `month` - Gregorian month for which to locate the third Wednesday.
/// * `year` - Proleptic Gregorian calendar year accepted by the `time` crate.
///
/// # Errors
/// Returns `Error::Validation` when `year` is outside the supported date range.
pub fn third_wednesday(month: Month, year: i32) -> crate::Result<Date> {
    third_weekday(month, year, Weekday::Wednesday)
}

/// Third `weekday` of `month` in `year`, validating the year range.
fn third_weekday(month: Month, year: i32, weekday: Weekday) -> crate::Result<Date> {
    nth_weekday_of_month(year, month, weekday, 3).ok_or_else(|| {
        crate::Error::Validation(format!("year {year} is outside the supported date range"))
    })
}

/// Return the **next IMM date** (third Wednesday of Mar/Jun/Sep/Dec) **strictly
/// after** `date`.
///
/// # Arguments
///
/// * `date` - Reference date to advance from. An IMM date supplied here returns
///   the following quarterly IMM date rather than itself.
/// # Errors
/// Returns `Error::Validation` when no later IMM date is representable.
pub fn next_imm(date: Date) -> crate::Result<Date> {
    scan_months(date, &QUARTERLY_MONTHS, Scan::After, third_wednesday)
}

/// First standard IMM date (third Wednesday of Mar/Jun/Sep/Dec) on or after
/// `date`; the anchor of a forward-generated IMM schedule.
pub(crate) fn imm_on_or_after(date: Date) -> crate::Result<Date> {
    scan_months(date, &QUARTERLY_MONTHS, Scan::OnOrAfter, third_wednesday)
}

/// Last standard IMM date on or before `date`; the anchor of a
/// backward-generated IMM schedule.
pub(crate) fn imm_on_or_before(date: Date) -> crate::Result<Date> {
    scan_months(date, &QUARTERLY_MONTHS, Scan::OnOrBefore, third_wednesday)
}

/// Check if a date is a CDS roll date (20th of Mar/Jun/Sep/Dec).
///
/// # Arguments
///
/// * `date` - Calendar date to test against the standard quarterly CDS roll
///   convention.
#[must_use]
pub fn is_cds_date(date: Date) -> bool {
    date.day() == 20 && QUARTERLY_MONTHS.contains(&date.month())
}

/// Check if a date is a standard IMM date (third Wednesday of Mar/Jun/Sep/Dec).
///
/// IMM dates are used for interest rate futures, currency futures, and equity index
/// futures that follow CME IMM roll conventions.
///
/// # Example
/// ```rust
/// use finstack_quant_core::dates::is_imm_date;
/// use time::{Date, Month};
///
/// let imm_date = Date::from_calendar_date(2025, Month::March, 19)?;
/// assert!(is_imm_date(imm_date)); // Third Wednesday of March 2025
///
/// let non_imm = Date::from_calendar_date(2025, Month::March, 20)?;
/// assert!(!is_imm_date(non_imm)); // Not a third Wednesday
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// # Arguments
///
/// * `date` - Calendar date to test against the quarterly third-Wednesday IMM
///   convention.
#[must_use]
pub fn is_imm_date(date: Date) -> bool {
    // Must be a quarterly month
    if !QUARTERLY_MONTHS.contains(&date.month()) {
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
///
/// # Arguments
///
/// * `date` - Reference date to advance from. A CDS roll date supplied here
///   returns the next quarterly roll date rather than itself.
/// # Errors
/// Returns `Error::Validation` when no later CDS roll date is representable.
pub fn next_cds_date(date: Date) -> crate::Result<Date> {
    scan_months(date, &QUARTERLY_MONTHS, Scan::After, twentieth)
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
/// use finstack_quant_core::dates::prev_cds_date;
/// use time::{Date, Month};
///
/// let d = Date::from_calendar_date(2025, Month::January, 15)?;
/// assert_eq!(
///     prev_cds_date(d)?,
///     Date::from_calendar_date(2024, Month::December, 20)?
/// );
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// # Arguments
///
/// * `date` - Reference date to step back from. A CDS roll date supplied here
///   returns the preceding quarterly roll date rather than itself.
/// # Errors
/// Returns `Error::Validation` when no earlier CDS roll date is representable.
pub fn prev_cds_date(date: Date) -> crate::Result<Date> {
    scan_months(date, &QUARTERLY_MONTHS, Scan::Before, twentieth)
}

/// Return the **semi-annual CDS roll date** (20-Mar / 20-Sep) **on or before**
/// `date`.
///
/// Since December 2015, single-name CDS (and the CDX/iTraxx indices) roll
/// **semi-annually** on 20 March and 20 September rather than quarterly. A
/// benchmark tenor quote (e.g. "5Y") references a contract whose term runs
/// from the semi-annual roll date on or before the trade date. A trade dated
/// exactly on 20-Mar/20-Sep uses that same date as its roll anchor.
///
/// # Standards Reference
///
/// - ISDA, "Amendments to Single Name CDS Roll Dates" (2015): effective
///   20 December 2015, new on-the-run maturities are created only twice a
///   year, on 20 March and 20 September.
///
/// # Examples
/// ```rust
/// use finstack_quant_core::dates::prev_cds_semiannual_roll;
/// use time::{Date, Month};
///
/// let trade = Date::from_calendar_date(2024, Month::January, 15)?;
/// assert_eq!(
///     prev_cds_semiannual_roll(trade)?,
///     Date::from_calendar_date(2023, Month::September, 20)?
/// );
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// # Arguments
///
/// * `date` - Trade or valuation date anchoring the on-the-run contract.
/// # Errors
/// Returns `Error::Validation` when no CDS semiannual roll on or before `date` is representable.
pub fn prev_cds_semiannual_roll(date: Date) -> crate::Result<Date> {
    const SEMIANNUAL_ROLL_MONTHS: [Month; 2] = [Month::March, Month::September];
    scan_months(date, &SEMIANNUAL_ROLL_MONTHS, Scan::OnOrBefore, twentieth)
}

/// Return the **standard CDS scheduled termination date** (20-Jun / 20-Dec)
/// **on or after** `date`.
///
/// Under the post-2015 semi-annual roll, on-the-run CDS maturities fall only
/// on 20 June and 20 December (the semi-annual roll anchor plus the tenor
/// lands on 20-Mar/20-Sep and is then extended to the following standard
/// maturity month). This helper snaps an unadjusted "roll + tenor" date to
/// that standard maturity grid. A date already on 20-Jun/20-Dec is returned
/// unchanged.
///
/// # Standards Reference
///
/// - ISDA, "Amendments to Single Name CDS Roll Dates" (2015).
///
/// # Examples
/// ```rust
/// use finstack_quant_core::dates::next_semiannual_cds_maturity;
/// use time::{Date, Month};
///
/// // 20-Sep roll + 5Y = 20-Sep-2028 -> standard maturity 20-Dec-2028
/// let raw = Date::from_calendar_date(2028, Month::September, 20)?;
/// assert_eq!(
///     next_semiannual_cds_maturity(raw)?,
///     Date::from_calendar_date(2028, Month::December, 20)?
/// );
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// # Arguments
///
/// * `date` - Unadjusted candidate maturity to snap forward to the standard
///   semi-annual maturity grid.
/// # Errors
/// Returns `Error::Validation` when no CDS maturity on or after `date` is representable.
pub fn next_semiannual_cds_maturity(date: Date) -> crate::Result<Date> {
    const MATURITY_MONTHS: [Month; 2] = [Month::June, Month::December];
    scan_months(date, &MATURITY_MONTHS, Scan::OnOrAfter, twentieth)
}

/// Return the **third Friday** of `month` in `year`.
///
/// Delegates to the shared `nth_weekday_of_month` helper used by calendar rules
/// to keep all "nth weekday" logic consistent.
///
/// # Arguments
///
/// * `month` - Gregorian month for which to locate the third Friday.
/// * `year` - Proleptic Gregorian calendar year accepted by the `time` crate.
///
/// # Errors
/// Returns `Error::Validation` when `year` is outside the supported date range.
pub fn third_friday(month: Month, year: i32) -> crate::Result<Date> {
    third_weekday(month, year, Weekday::Friday)
}

/// Return the next unadjusted third Friday of any month strictly after `date`.
///
/// This is a Gregorian monthly roll primitive. It does not apply exchange
/// holidays or determine a listed option's last trading or expiration date.
/// For example, April 2025 returns Good Friday, April 18, even though U.S.
/// listed monthly equity options used April 17 that month.
///
/// # Arguments
///
/// * `date` - Reference date to advance from. A third Friday supplied here
///   returns the following month's third Friday rather than itself.
/// # Errors
/// Returns `Error::Validation` when no later third Friday is representable.
pub fn next_third_friday(date: Date) -> crate::Result<Date> {
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

    scan_months(date, &ALL_MONTHS, Scan::After, third_friday)
}

/// SIFMA MBS settlement class.
///
/// SIFMA publishes distinct settlement dates for four classes of agency MBS.
/// The class determines which specific settlement date applies within a given
/// month. There is no default class: every lookup selects one explicitly. See <https://www.sifma.org/resources/general/mbs-notification-and-settlement-dates/>.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum SifmaSettlementClass {
    /// Class A: Conventional 30-year (FNMA/FHLMC UMBS).
    A,
    /// Class B: Fixed-rate 15-year agency MBS.
    B,
    /// Class C: GNMA single-family 30-year.
    C,
    /// Class D: Balloons, ARMs, multifamily, and other non-standard products.
    D,
}

impl std::fmt::Display for SifmaSettlementClass {
    /// Writes the serde label (`a` through `d`).
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&crate::wire::serde_label(self).map_err(|_| std::fmt::Error)?)
    }
}

impl std::str::FromStr for SifmaSettlementClass {
    type Err = crate::Error;

    /// Parses the serde label, ignoring surrounding whitespace and ASCII case.
    fn from_str(s: &str) -> crate::Result<Self> {
        crate::wire::serde_parse(&s.trim().to_ascii_lowercase())
    }
}

// Generated from `data/sifma_settlements.csv`. Rows may contain a subset of
// classes where SIFMA has published only partial coverage.
include!("../generated/sifma_settlements_generated.rs");

/// Look up the published SIFMA TBA settlement date for a settlement class.
///
/// Returns the maintained Settlement cell for the exact month, year and class,
/// unchanged: it is not re-rolled through any holiday calendar and never
/// estimated. Coverage is per class: a month may carry some classes and not
/// others.
///
/// # Arguments
///
/// * `month` - Settlement month to look up in the maintained SIFMA schedule.
/// * `year` - Settlement year to look up.
/// * `class` - Agency-MBS settlement class whose published date is requested.
///
/// # Errors
///
/// Returns `Error::Validation` when `year` is outside the supported date range
/// (checked before coverage), `InputError::NotFound` when the maintained data
/// has no Settlement cell for that month, year and class, and
/// `Error::Validation` when the stored cell is not a valid calendar date.
///
/// # Example
/// ```rust
/// use finstack_quant_core::dates::{sifma_settlement_date, SifmaSettlementClass};
/// use time::{Date, Month};
///
/// let settle = sifma_settlement_date(Month::March, 2027, SifmaSettlementClass::A)?;
/// assert_eq!(settle, Date::from_calendar_date(2027, Month::March, 11).expect("Valid date"));
/// assert!(sifma_settlement_date(Month::March, 1990, SifmaSettlementClass::A).is_err());
/// # Ok::<(), finstack_quant_core::Error>(())
/// ```
pub fn sifma_settlement_date(
    month: Month,
    year: i32,
    class: SifmaSettlementClass,
) -> crate::Result<Date> {
    if !(Date::MIN.year()..=Date::MAX.year()).contains(&year) {
        return Err(crate::Error::Validation(format!(
            "SIFMA settlement year {year} is outside the supported date range"
        )));
    }
    let class_index = match class {
        SifmaSettlementClass::A => 0,
        SifmaSettlementClass::B => 1,
        SifmaSettlementClass::C => 2,
        SifmaSettlementClass::D => 3,
    };
    let day = SIFMA_SETTLEMENTS
        .iter()
        .find(|(y, m, _)| *y == year && *m == month as u8)
        .and_then(|(_, _, class_days)| class_days[class_index])
        .ok_or_else(|| {
            crate::Error::from(crate::error::InputError::NotFound {
                id: format!(
                    "SIFMA Class {} settlement for {year}-{:02} (maintained data/sifma_settlements.csv{})",
                    class.to_string().to_ascii_uppercase(),
                    month as u8,
                    sifma_class_coverage(class_index)
                ),
            })
        })?;
    Date::from_calendar_date(year, month, day).map_err(|_| {
        crate::Error::Validation(format!(
            "maintained SIFMA Class {} settlement day {day} is not a valid date in {year}-{:02}",
            class.to_string().to_ascii_uppercase(),
            month as u8
        ))
    })
}

/// Describe the maintained month range for one class, for error context.
fn sifma_class_coverage(class_index: usize) -> String {
    let mut covered = SIFMA_SETTLEMENTS
        .iter()
        .filter(|(_, _, class_days)| class_days[class_index].is_some());
    match (covered.next(), covered.next_back()) {
        (Some(first), Some(last)) => format!(
            " covers this class from {}-{:02} to {}-{:02}, with gaps where cells are blank",
            first.0, first.1, last.0, last.1
        ),
        (Some(only), None) => format!(" covers this class only for {}-{:02}", only.0, only.1),
        (None, _) => " has no rows for this class".to_string(),
    }
}

/// Return the **next published SIFMA TBA settlement date** for `class`
/// **strictly after** `date`.
///
/// The current month's published date is used when it is later than `date`;
/// equality advances to the following month. Otherwise the immediately
/// following month's published date is returned. A missing required cell is
/// an error: unknown gaps are never skipped. On the last day of a month,
/// every current-month settlement is already non-qualifying, so the current
/// cell is not consulted.
///
/// # Example
/// ```rust
/// use finstack_quant_core::dates::{next_sifma_settlement, SifmaSettlementClass};
/// use time::{Date, Month};
///
/// let start = Date::from_calendar_date(2027, Month::March, 11).expect("Valid date");
/// let next = next_sifma_settlement(start, SifmaSettlementClass::A)?;
/// assert_eq!(next, Date::from_calendar_date(2027, Month::April, 13).expect("Valid date"));
/// # Ok::<(), finstack_quant_core::Error>(())
/// ```
///
/// # Arguments
///
/// * `date` - Reference date. A settlement on this date is not returned.
/// * `class` - Agency-MBS settlement class whose published dates are searched.
///
/// # Errors
///
/// Returns `InputError::NotFound` when a required current or following-month
/// cell is not in the maintained data, `Error::Validation` for an invalid
/// stored cell, and `Error::Validation` when `date` has no later month
/// representable as a date.
pub fn next_sifma_settlement(date: Date, class: SifmaSettlementClass) -> crate::Result<Date> {
    let month_end = date
        .next_day()
        .is_none_or(|next| next.month() != date.month());
    if !month_end {
        let current = sifma_settlement_date(date.month(), date.year(), class)?;
        if current > date {
            return Ok(current);
        }
    }
    let (year, month) = if date.month() == Month::December {
        let year = date
            .year()
            .checked_add(1)
            .filter(|year| *year <= Date::MAX.year());
        (year, Month::January)
    } else {
        (Some(date.year()), date.month().next())
    };
    let year = year.ok_or_else(|| {
        crate::Error::Validation(format!(
            "no SIFMA settlement month after {date} is within the supported date range"
        ))
    })?;
    sifma_settlement_date(month, year, class)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unit_enum_labels_round_trip_and_parse_leniently() {
        assert_eq!(SifmaSettlementClass::C.to_string(), "c");
        assert_eq!(
            " B ".parse::<SifmaSettlementClass>().unwrap(),
            SifmaSettlementClass::B
        );
    }

    #[test]
    fn roll_helpers_reject_unrepresentable_successors_and_predecessors() {
        for next in [
            next_imm,
            next_cds_date,
            next_semiannual_cds_maturity,
            next_third_friday,
        ] {
            assert!(matches!(next(Date::MAX), Err(crate::Error::Validation(_))));
        }
        for previous in [prev_cds_date, prev_cds_semiannual_roll] {
            assert!(matches!(
                previous(Date::MIN),
                Err(crate::Error::Validation(_))
            ));
        }

        let last_cds = twentieth(Month::December, Date::MAX.year()).expect("last CDS roll");
        assert!(next_cds_date(last_cds).is_err());
        assert_eq!(
            next_semiannual_cds_maturity(last_cds).expect("inclusive maturity"),
            last_cds
        );
        let first_cds = twentieth(Month::March, Date::MIN.year()).expect("first CDS roll");
        assert!(prev_cds_date(first_cds).is_err());
        assert_eq!(
            prev_cds_semiannual_roll(first_cds).expect("inclusive roll"),
            first_cds
        );
    }

    #[test]
    fn caller_supplied_years_are_checked_without_panics() {
        for year in [
            Date::MIN.year() - 1,
            Date::MAX.year() + 1,
            i32::MIN,
            i32::MAX,
        ] {
            assert!(third_wednesday(Month::January, year).is_err());
            assert!(third_friday(Month::December, year).is_err());
            assert!(sifma_settlement_date(Month::January, year, SifmaSettlementClass::A).is_err());
        }
        for year in [Date::MIN.year(), Date::MAX.year()] {
            for month in [Month::January, Month::December] {
                assert!(third_wednesday(month, year).is_ok());
                assert!(third_friday(month, year).is_ok());
            }
        }
    }

    #[test]
    fn third_wed_march_2025() {
        let d = third_wednesday(Month::March, 2025).expect("supported roll date");
        assert_eq!(
            d,
            Date::from_calendar_date(2025, Month::March, 19).expect("Valid test date")
        );
    }

    #[test]
    fn next_imm_after_mar20_2025() {
        let start = Date::from_calendar_date(2025, Month::March, 20).expect("Valid test date");
        let imm = next_imm(start).expect("supported roll date");
        assert_eq!(
            imm,
            Date::from_calendar_date(2025, Month::June, 18).expect("Valid test date")
        );
    }

    #[test]
    fn next_cds_before_mar20() {
        let d = Date::from_calendar_date(2025, Month::March, 10).expect("Valid test date");
        let cds = next_cds_date(d).expect("supported roll date");
        assert_eq!(
            cds,
            Date::from_calendar_date(2025, Month::March, 20).expect("Valid test date")
        );
    }

    #[test]
    fn prev_semiannual_roll_selects_mar_or_sep_on_or_before() {
        let d = |y, m, day| Date::from_calendar_date(y, m, day).expect("valid test date");
        // Mid-January -> prior September roll
        assert_eq!(
            prev_cds_semiannual_roll(d(2024, Month::January, 15)).expect("supported roll date"),
            d(2023, Month::September, 20)
        );
        // Between rolls (May) -> March roll of same year
        assert_eq!(
            prev_cds_semiannual_roll(d(2026, Month::May, 2)).expect("supported roll date"),
            d(2026, Month::March, 20)
        );
        // Exactly on the roll date -> that roll date (on-or-before semantics)
        assert_eq!(
            prev_cds_semiannual_roll(d(2025, Month::March, 20)).expect("supported roll date"),
            d(2025, Month::March, 20)
        );
        // Day before the roll -> prior September roll
        assert_eq!(
            prev_cds_semiannual_roll(d(2025, Month::March, 19)).expect("supported roll date"),
            d(2024, Month::September, 20)
        );
        // June 20 (quarterly but not semi-annual roll) -> March roll
        assert_eq!(
            prev_cds_semiannual_roll(d(2025, Month::June, 20)).expect("supported roll date"),
            d(2025, Month::March, 20)
        );
    }

    #[test]
    fn semiannual_maturity_snaps_to_jun_or_dec() {
        let d = |y, m, day| Date::from_calendar_date(y, m, day).expect("valid test date");
        // Roll + tenor lands on 20-Sep -> extend to 20-Dec
        assert_eq!(
            next_semiannual_cds_maturity(d(2028, Month::September, 20))
                .expect("supported roll date"),
            d(2028, Month::December, 20)
        );
        // Lands on 20-Mar -> extend to 20-Jun
        assert_eq!(
            next_semiannual_cds_maturity(d(2031, Month::March, 20)).expect("supported roll date"),
            d(2031, Month::June, 20)
        );
        // Already on 20-Jun -> unchanged (on-or-after semantics)
        assert_eq!(
            next_semiannual_cds_maturity(d(2030, Month::June, 20)).expect("supported roll date"),
            d(2030, Month::June, 20)
        );
        // Already on 20-Dec -> unchanged
        assert_eq!(
            next_semiannual_cds_maturity(d(2029, Month::December, 20))
                .expect("supported roll date"),
            d(2029, Month::December, 20)
        );
        // Late December date -> next June
        assert_eq!(
            next_semiannual_cds_maturity(d(2029, Month::December, 21))
                .expect("supported roll date"),
            d(2030, Month::June, 20)
        );
    }

    #[test]
    fn third_friday_march_2025() {
        // March 2025: third Friday is 21st
        let friday = third_friday(Month::March, 2025).expect("supported roll date");
        assert_eq!(
            friday,
            Date::from_calendar_date(2025, Month::March, 21).expect("Valid test date")
        );
    }

    #[test]
    fn third_friday_february_2025() {
        // February 2025: third Friday is 21st
        let friday = third_friday(Month::February, 2025).expect("supported roll date");
        assert_eq!(
            friday,
            Date::from_calendar_date(2025, Month::February, 21).expect("Valid test date")
        );
    }

    #[test]
    fn next_third_friday_remains_unadjusted_on_good_friday() {
        let date = Date::from_calendar_date(2025, Month::April, 1).unwrap();
        assert_eq!(
            next_third_friday(date).unwrap(),
            Date::from_calendar_date(2025, Month::April, 18).unwrap()
        );
    }

    #[test]
    fn next_third_friday_mid_march() {
        // Starting mid-March 2025, should get March third Friday (21st)
        let start = Date::from_calendar_date(2025, Month::March, 15).expect("Valid test date");
        let next_expiry = next_third_friday(start).expect("supported roll date");
        assert_eq!(
            next_expiry,
            Date::from_calendar_date(2025, Month::March, 21).expect("Valid test date")
        );
    }

    #[test]
    fn next_third_friday_after_march_friday() {
        // Starting after March third Friday, should get April third Friday
        let start = Date::from_calendar_date(2025, Month::March, 22).expect("Valid test date");
        let next_expiry = next_third_friday(start).expect("supported roll date");
        assert_eq!(
            next_expiry,
            Date::from_calendar_date(2025, Month::April, 18).expect("Valid test date")
        );
    }

    #[test]
    fn next_third_friday_year_rollover() {
        // Starting in December, should roll to January of next year
        let start = Date::from_calendar_date(2025, Month::December, 25).expect("Valid test date");
        let next_expiry = next_third_friday(start).expect("supported roll date");
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

    // SIFMA published settlement tests (fixed dates; no wall-clock dependence)

    fn ymd(year: i32, month: Month, day: u8) -> Date {
        Date::from_calendar_date(year, month, day).expect("valid date")
    }

    const ALL_CLASSES: [SifmaSettlementClass; 4] = [
        SifmaSettlementClass::A,
        SifmaSettlementClass::B,
        SifmaSettlementClass::C,
        SifmaSettlementClass::D,
    ];

    #[test]
    fn sifma_published_cells_are_returned_unchanged() {
        for (month, year, class, expected) in [
            (
                Month::January,
                2026,
                SifmaSettlementClass::A,
                ymd(2026, Month::January, 14),
            ),
            (
                Month::January,
                2026,
                SifmaSettlementClass::B,
                ymd(2026, Month::January, 20),
            ),
            (
                Month::March,
                2026,
                SifmaSettlementClass::B,
                ymd(2026, Month::March, 17),
            ),
            (
                Month::March,
                2027,
                SifmaSettlementClass::D,
                ymd(2027, Month::March, 23),
            ),
        ] {
            assert_eq!(
                sifma_settlement_date(month, year, class).expect("published cell"),
                expected
            );
        }
    }

    #[test]
    fn sifma_missing_cells_are_not_found_errors() {
        // 2025 carries Class A only: B/C/D cells are blank, not estimated.
        for class in [
            SifmaSettlementClass::B,
            SifmaSettlementClass::C,
            SifmaSettlementClass::D,
        ] {
            let error = sifma_settlement_date(Month::March, 2025, class).expect_err("gap");
            assert_eq!(error.kind(), crate::error::ErrorKind::NotFound);
        }
        for class in ALL_CLASSES {
            for (month, year) in [
                (Month::June, 2030),
                (Month::March, 2024),
                (Month::January, 2028),
            ] {
                let error = sifma_settlement_date(month, year, class).expect_err("uncovered");
                assert_eq!(error.kind(), crate::error::ErrorKind::NotFound);
            }
        }
    }

    #[test]
    fn sifma_next_is_strict_and_class_specific() {
        let a_march_2027 = ymd(2027, Month::March, 11);
        // Before the current-month settlement: current month.
        assert_eq!(
            next_sifma_settlement(ymd(2027, Month::March, 1), SifmaSettlementClass::A)
                .expect("next"),
            a_march_2027
        );
        // Equality advances to the following month.
        assert_eq!(
            next_sifma_settlement(a_march_2027, SifmaSettlementClass::A).expect("next"),
            ymd(2027, Month::April, 13)
        );
        // Class B in the same month has its own, later date.
        assert_eq!(
            next_sifma_settlement(a_march_2027, SifmaSettlementClass::B).expect("next"),
            sifma_settlement_date(Month::March, 2027, SifmaSettlementClass::B).expect("cell")
        );
    }

    #[test]
    fn sifma_next_errors_on_required_gaps_without_skipping() {
        // After the last maintained month the following cell is required.
        let error = next_sifma_settlement(ymd(2027, Month::December, 20), SifmaSettlementClass::A)
            .expect_err("January 2028 is not maintained");
        assert_eq!(error.kind(), crate::error::ErrorKind::NotFound);
        // Early December 2025 Class B: the current 2025 cell is a gap, so the
        // search fails rather than jumping to January 2026.
        let error = next_sifma_settlement(ymd(2025, Month::December, 1), SifmaSettlementClass::B)
            .expect_err("current-month gap");
        assert_eq!(error.kind(), crate::error::ErrorKind::NotFound);
        // On the month's last day the current gap is bypassed.
        assert_eq!(
            next_sifma_settlement(ymd(2025, Month::December, 31), SifmaSettlementClass::B)
                .expect("January 2026 Class B"),
            ymd(2026, Month::January, 20)
        );
    }

    #[test]
    fn sifma_next_at_date_max_errors_without_lookup() {
        let error =
            next_sifma_settlement(Date::MAX, SifmaSettlementClass::A).expect_err("no later month");
        assert_eq!(error.kind(), crate::error::ErrorKind::Validation);
    }

    #[test]
    fn sifma_class_labels_are_explicit() {
        for (class, label) in ALL_CLASSES.into_iter().zip(["a", "b", "c", "d"]) {
            assert_eq!(class.to_string(), label);
            assert_eq!(label.parse::<SifmaSettlementClass>().expect("label"), class);
        }
        assert!("e".parse::<SifmaSettlementClass>().is_err());
        assert!("".parse::<SifmaSettlementClass>().is_err());
    }

    #[test]
    fn sifma_class_a_2025_uses_published_dates() {
        let expected_days = [14, 13, 13, 14, 13, 12, 14, 13, 15, 14, 13, 11];
        let months = [
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
        for (month, expected_day) in months.into_iter().zip(expected_days) {
            let date = sifma_settlement_date(month, 2025, SifmaSettlementClass::A)
                .expect("published 2025 Class A date");
            assert_eq!(date.day(), expected_day);
        }
    }
}
