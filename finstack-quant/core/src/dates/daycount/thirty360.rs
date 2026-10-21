//! 30/360 convention implementations.

use time::{Date, Month};

use crate::dates::date_extensions::DateExt;

// 30/360 generalized helper
/// 30/360 day-count variants.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Thirty360Convention {
    /// 30U/360 (US SIA / Bond Basis).
    UsSia,
    /// 30/360 ISDA bond basis (ISDA 2006 §4.16(f); no February EOM rule).
    ///
    /// Reachable via the public [`days_30_360`] helper; the
    /// [`DayCount`](super::DayCount) enum exposes the SIA/PSA
    /// ([`DayCount::Thirty360`](super::DayCount::Thirty360)) and 30E/360 variants
    /// instead.
    Isda,
    /// 30E/360 (European).
    European,
    /// 30/360 Italian (QuantLib `Thirty360::Italian`).
    ///
    /// Day 31 becomes 30, and any February day after the 27th becomes 30.
    /// Distinct from US SIA (February EOM only when both ends are February
    /// EOM) and 30E/360 (no February-after-27 rule).
    Italian,
}

impl std::fmt::Display for Thirty360Convention {
    /// Writes the serde label (`us_sia`, `isda`, `european`, `italian`).
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&crate::wire::serde_label(self).map_err(|_| std::fmt::Error)?)
    }
}

impl std::str::FromStr for Thirty360Convention {
    type Err = crate::Error;

    /// Parses the serde label, ignoring surrounding whitespace and ASCII case.
    fn from_str(s: &str) -> crate::Result<Self> {
        crate::wire::serde_parse(&s.trim().to_ascii_lowercase())
    }
}

/// Compute day count between `start` (inclusive) and `end` (exclusive) under a 30/360 convention.
///
/// Precondition: `start <= end`. If violated, the returned value will be negative.
/// This helper is panic-free and allocation-free.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_core::dates::{days_30_360, Thirty360Convention};
/// use time::{Date, Month};
///
/// let start = Date::from_calendar_date(2025, Month::January, 31).expect("Valid date");
/// let end = Date::from_calendar_date(2025, Month::March, 31).expect("Valid date");
///
/// // ISDA 2006 §4.16(f): D1 31 → 30, then D2 31 → 30.
/// assert_eq!(days_30_360(start, end, Thirty360Convention::Isda), 60);
/// ```
///
/// # Arguments
///
/// * `start` - Inclusive accrual-period start date.
/// * `end` - Exclusive accrual-period end date. Earlier values produce a
///   negative count rather than an error.
/// * `convention` - 30/360 variant that determines February and month-end
///   adjustments.
#[inline]
pub fn days_30_360(start: Date, end: Date, convention: Thirty360Convention) -> i32 {
    let (y1, m1, d1) = (start.year(), start.month() as i32, start.day() as i32);
    let (y2, m2, d2) = (end.year(), end.month() as i32, end.day() as i32);

    let (d1_adj, d2_adj) = match convention {
        Thirty360Convention::UsSia => {
            // SIA/PSA 30/360 US Bond Basis:
            // - If D1 is 31 or last day of February, change D1 to 30
            // - If D2 is 31 and D1 was adjusted to 30, change D2 to 30
            // - If D2 is last day of Feb AND D1 was last day of Feb, change D2 to 30
            // (The Feb-EOM rule is SIA/PSA-specific; ISDA 2006 §4.16(f) omits it.)
            let d1_adj = if d1 == 31 || is_last_day_of_february(start) {
                30
            } else {
                d1
            };
            let d2_adj = if (d2 == 31 && d1_adj == 30)
                || (is_last_day_of_february(end) && is_last_day_of_february(start))
            {
                30
            } else {
                d2
            };
            (d1_adj, d2_adj)
        }
        Thirty360Convention::Isda => {
            let d1_adj = if d1 == 31 { 30 } else { d1 };
            let d2_adj = if d2 == 31 && d1_adj == 30 { 30 } else { d2 };
            (d1_adj, d2_adj)
        }
        Thirty360Convention::European => {
            // ISDA 2006 §4.16(g) - 30E/360:
            // - If D1 is 31, change D1 to 30
            // - If D2 is 31, change D2 to 30
            // Note: NO February EOM rule for European convention
            let d1_adj = if d1 == 31 { 30 } else { d1 };
            let d2_adj = if d2 == 31 { 30 } else { d2 };
            (d1_adj, d2_adj)
        }
        Thirty360Convention::Italian => {
            // QuantLib Thirty360::Italian:
            // D' = 30 if D == 31 or (month == February and D > 27)
            let d1_adj = if d1 == 31 || (start.month() == Month::February && d1 > 27) {
                30
            } else {
                d1
            };
            let d2_adj = if d2 == 31 || (end.month() == Month::February && d2 > 27) {
                30
            } else {
                d2
            };
            (d1_adj, d2_adj)
        }
    };

    (y2 - y1) * 360 + (m2 - m1) * 30 + (d2_adj - d1_adj)
}

/// Check if date is the last day of February (28 or 29 depending on leap year).
///
/// Per SIA/PSA Standard Formulas, the last day of February receives special
/// treatment in 30/360 US Bond Basis calculations.
#[inline]
fn is_last_day_of_february(date: Date) -> bool {
    date.month() == Month::February && date == date.end_of_month()
}

/// Compute the 30E/360 (ISDA) day count per ISDA 2006 §4.16(h).
///
/// Adjustment rules:
/// - D₁ becomes 30 when `start` is the last day of its month (including the
///   last day of February).
/// - D₂ becomes 30 when `end` is day 31, or when `end` is the last day of
///   February **and** `end_is_termination_date` is `false`.
///
/// The termination-date exception means the final accrual period of an
/// instrument maturing on the last day of February keeps the actual day
/// number (28/29); pass `end_is_termination_date = true` for that period.
/// [`DayCount::ThirtyE360Isda`](super::DayCount::ThirtyE360Isda) receives this flag from
/// [`DayCountContext::end_is_termination_date`](super::DayCountContext::end_is_termination_date).
///
/// Precondition: `start <= end`. If violated, the returned value will be
/// negative. This helper is panic-free and allocation-free.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_core::dates::days_30e_360_isda;
/// use time::{Date, Month};
///
/// let start = Date::from_calendar_date(2012, Month::January, 28).expect("Valid date");
/// let end = Date::from_calendar_date(2012, Month::February, 29).expect("Valid date");
///
/// // Intermediate coupon: end-of-Feb → 30; 30 + (30 - 28) = 32
/// assert_eq!(days_30e_360_isda(start, end, false), 32);
/// // Final period to maturity: Feb 29 kept; 30 + (29 - 28) = 31
/// assert_eq!(days_30e_360_isda(start, end, true), 31);
/// ```
///
/// # References
///
/// - ISDA (2006). "2006 ISDA Definitions." Section 4.16(h). `docs/REFERENCES.md#isda-2006-definitions`
///
/// # Arguments
///
/// * `start` - Inclusive accrual-period start date.
/// * `end` - Exclusive accrual-period end date. Earlier values produce a
///   negative count rather than an error.
/// * `end_is_termination_date` - Whether `end` is the instrument's final
///   maturity date, which preserves a February month-end under the ISDA rule.
#[inline]
pub fn days_30e_360_isda(start: Date, end: Date, end_is_termination_date: bool) -> i32 {
    let (y1, m1, d1) = (start.year(), start.month() as i32, start.day() as i32);
    let (y2, m2, d2) = (end.year(), end.month() as i32, end.day() as i32);

    let d1_adj = if start == start.end_of_month() {
        30
    } else {
        d1
    };
    let d2_adj = if d2 == 31 || (is_last_day_of_february(end) && !end_is_termination_date) {
        30
    } else {
        d2
    };

    (y2 - y1) * 360 + (m2 - m1) * 30 + (d2_adj - d1_adj)
}
