//! Payment delay conventions for agency MBS.
//!
//! Agency MBS have standardized stated delays measured from the **first day of
//! the accrual period** to the payment date. Post-Single Security Initiative
//! (June 2019), FNMA and FHLMC both issue UMBS with the same 55-day delay:
//!
//! - **FNMA / FHLMC (UMBS)**: 55-day stated delay — payment on the 25th of M+1
//! - **GNMA I**: 14-day stated delay — single-issuer pools, payment on the 15th of M
//! - **GNMA II**: 50-day stated delay — multi-issuer pools, payment on the 20th of M+1
//!
//! These constants match [`AgencyProgram::payment_lag_days`] and the
//! calendar-based [`AgencyProgram::payment_date_for_period`]. Legacy FHLMC
//! Gold PCs (45-day) and ARM PCs (75-day) predate UMBS and should be modeled
//! via the per-pool `payment_lag_days` override on `AgencyMbsPassthrough`.

use crate::instruments::fixed_income::mbs_passthrough::AgencyProgram;
use finstack_quant_core::dates::calendar::calendar_by_id;
use finstack_quant_core::dates::{BusinessDayConvention, Date};
use finstack_quant_core::Result;

/// Get the standard payment delay in days for an agency program.
///
/// # Arguments
///
/// * `agency` - Agency program (FNMA, FHLMC, GNMA)
///
/// # Returns
///
/// Payment delay in calendar days
///
/// # Examples
///
/// ```rust
/// use finstack_quant_valuations::instruments::fixed_income::mbs_passthrough::{
///     AgencyProgram,
///     delay::payment_lag_days,
/// };
///
/// assert_eq!(payment_lag_days(AgencyProgram::Fnma), 55);
/// assert_eq!(payment_lag_days(AgencyProgram::Fhlmc), 55);
/// assert_eq!(payment_lag_days(AgencyProgram::GnmaI), 14);
/// assert_eq!(payment_lag_days(AgencyProgram::GnmaII), 50);
/// ```
pub fn payment_lag_days(agency: AgencyProgram) -> u32 {
    agency.payment_lag_days()
}

/// Calculate the actual payment date by adding a stated delay to an accrual
/// anchor date.
///
/// The stated agency delay is measured from the **first day of the accrual
/// period**, so production callers pass the accrual period start. The function
/// itself simply adds `delay_days` calendar days to `anchor` and optionally
/// adjusts for weekends.
///
/// # Arguments
///
/// * `anchor` - Accrual anchor date (the accrual period start for agency
///   stated delays)
/// * `delay_days` - Number of calendar delay days to add
/// * `adjust_to_business` - Whether to adjust to next business day
///
/// # Returns
///
/// Actual payment date
///
/// # Examples
///
/// ```rust
/// use finstack_quant_valuations::instruments::fixed_income::mbs_passthrough::delay::actual_payment_date;
/// use finstack_quant_core::dates::Date;
/// use time::Month;
///
/// // 25-day delay from the accrual period start (Jan 1) → Jan 26.
/// let accrual_start = Date::from_calendar_date(2024, Month::January, 1).unwrap();
/// let payment_date = actual_payment_date(accrual_start, 25, false).unwrap();
/// assert_eq!(payment_date.day(), 26);
/// ```
pub fn actual_payment_date(
    anchor: Date,
    delay_days: u32,
    adjust_to_business: bool,
) -> Result<Date> {
    use time::Duration;

    let payment = anchor + Duration::days(delay_days as i64);

    if adjust_to_business {
        // Simple weekend adjustment (Following convention)
        let weekday = payment.weekday();
        let adjustment = match weekday {
            time::Weekday::Saturday => 2,
            time::Weekday::Sunday => 1,
            _ => 0,
        };
        Ok(payment + Duration::days(adjustment))
    } else {
        Ok(payment)
    }
}

/// Calculate payment date with calendar adjustment.
///
/// Uses a specific calendar for business day adjustment. The agency stated
/// delay is measured from the **accrual period start**, so `accrual_start`
/// should be the first day of the accrual period.
///
/// # Arguments
///
/// * `accrual_start` - Start (first day) of the accrual period
/// * `agency` - Agency program (determines delay)
/// * `calendar_id` - Calendar identifier for business day adjustment
/// * `bdc` - Business day convention
///
/// # Returns
///
/// Adjusted payment date
pub fn payment_date_with_calendar(
    accrual_start: Date,
    agency: AgencyProgram,
    calendar_id: Option<&str>,
    bdc: BusinessDayConvention,
) -> Result<Date> {
    use time::Duration;

    let delay = agency.payment_lag_days();
    let raw_payment = accrual_start + Duration::days(delay as i64);

    // Use holiday calendar when provided; fall back to weekend-only adjustment.
    if let Some(cal_id) = calendar_id {
        if let Some(cal) = calendar_by_id(cal_id) {
            return finstack_quant_core::dates::adjust(raw_payment, bdc, cal);
        }
    }

    // Weekend-only fallback
    match bdc {
        BusinessDayConvention::Following => {
            let weekday = raw_payment.weekday();
            let adjustment = match weekday {
                time::Weekday::Saturday => 2,
                time::Weekday::Sunday => 1,
                _ => 0,
            };
            Ok(raw_payment + Duration::days(adjustment))
        }
        BusinessDayConvention::ModifiedFollowing => {
            // Same as Following, but roll back if crosses month boundary
            let weekday = raw_payment.weekday();
            let adjustment = match weekday {
                time::Weekday::Saturday => 2,
                time::Weekday::Sunday => 1,
                _ => 0,
            };
            let adjusted = raw_payment + Duration::days(adjustment);
            if adjusted.month() != raw_payment.month() {
                // Roll back to previous business day
                let back_adjustment = match weekday {
                    time::Weekday::Saturday => -1,
                    time::Weekday::Sunday => -2,
                    _ => 0,
                };
                Ok(raw_payment + Duration::days(back_adjustment))
            } else {
                Ok(adjusted)
            }
        }
        _ => Ok(raw_payment),
    }
}

/// Generate payment schedule with delays for a series of accrual periods.
///
/// The agency stated delay is measured from the accrual period start, so the
/// input dates are accrual period starts.
///
/// # Arguments
///
/// * `accrual_starts` - Slice of accrual period start dates (first day of each
///   accrual period)
/// * `agency` - Agency program (determines delay)
///
/// # Returns
///
/// Vector of (accrual_start, payment_date) pairs
pub fn payment_schedule(
    accrual_starts: &[Date],
    agency: AgencyProgram,
) -> Result<Vec<(Date, Date)>> {
    let delay = agency.payment_lag_days();

    accrual_starts
        .iter()
        .map(|&accrual_start| {
            let payment = actual_payment_date(accrual_start, delay, false)?;
            Ok((accrual_start, payment))
        })
        .collect()
}

/// Calculate the time value impact of payment delay.
///
/// Returns the discount factor adjustment for the delay period.
///
/// # Arguments
///
/// * `delay_days` - Number of delay days
/// * `rate` - Annualized discount rate
///
/// # Returns
///
/// Discount factor for the delay (< 1.0 for positive rates)
pub fn delay_discount_factor(delay_days: u32, rate: f64) -> f64 {
    let years = delay_days as f64 / 365.0;
    (-rate * years).exp()
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::Month;

    #[test]
    fn test_payment_lag_days() {
        assert_eq!(payment_lag_days(AgencyProgram::Fnma), 55);
        assert_eq!(payment_lag_days(AgencyProgram::Fhlmc), 55);
        assert_eq!(payment_lag_days(AgencyProgram::Gnma), 50);
        assert_eq!(payment_lag_days(AgencyProgram::GnmaI), 14);
        assert_eq!(payment_lag_days(AgencyProgram::GnmaII), 50);
    }

    #[test]
    fn test_actual_payment_date() {
        // January 31 + 25 days = February 25
        let accrual_end = Date::from_calendar_date(2024, Month::January, 31).expect("valid date");
        let payment = actual_payment_date(accrual_end, 25, false).expect("valid date");

        assert_eq!(payment.month(), Month::February);
        assert_eq!(payment.day(), 25);
    }

    #[test]
    fn test_actual_payment_date_weekend_adjustment() {
        // Find a date where +25 lands on a weekend
        // Jan 6, 2024 is Saturday. So accrual end Dec 12, 2023 + 25 = Jan 6 (Saturday)
        let accrual_end = Date::from_calendar_date(2023, Month::December, 12).expect("valid date");
        let payment_no_adjust = actual_payment_date(accrual_end, 25, false).expect("valid date");
        let payment_adjusted = actual_payment_date(accrual_end, 25, true).expect("valid date");

        // Without adjustment: Jan 6, 2024 (Saturday)
        assert_eq!(payment_no_adjust.day(), 6);
        // With adjustment: Jan 8, 2024 (Monday)
        assert_eq!(payment_adjusted.day(), 8);
    }

    #[test]
    fn test_payment_schedule() {
        // Accrual period starts (first day of each month).
        let accrual_starts = vec![
            Date::from_calendar_date(2024, Month::January, 1).expect("valid"),
            Date::from_calendar_date(2024, Month::February, 1).expect("valid"),
            Date::from_calendar_date(2024, Month::March, 1).expect("valid"),
        ];

        let schedule = payment_schedule(&accrual_starts, AgencyProgram::Fnma).expect("valid");

        assert_eq!(schedule.len(), 3);

        // First payment: the 55-day stated delay from the Jan 1 accrual start
        // lands on Feb 25, 2024 — consistent with the FNMA "25th of M+1" rule.
        assert_eq!(schedule[0].0, accrual_starts[0]);
        assert_eq!(schedule[0].1.month(), Month::February);
        assert_eq!(schedule[0].1.day(), 25);
    }

    #[test]
    fn test_delay_discount_factor() {
        // 25 days at 5% rate
        let df = delay_discount_factor(25, 0.05);

        // Should be slightly less than 1.0
        assert!(df < 1.0);
        assert!(df > 0.99);

        // Approximate: exp(-0.05 * 25/365) ≈ 0.9966
        assert!((df - 0.9966).abs() < 0.001);
    }

    #[test]
    fn test_payment_date_with_calendar() {
        // Accrual period start (first day of the accrual month).
        let accrual_start = Date::from_calendar_date(2024, Month::January, 1).expect("valid");

        // FNMA with Following convention
        let payment = payment_date_with_calendar(
            accrual_start,
            AgencyProgram::Fnma,
            None,
            BusinessDayConvention::Following,
        )
        .expect("valid");

        // Jan 1 + 55-day stated delay = Feb 25, 2024 (Sunday → Following rolls
        // to Feb 26, Monday).
        assert_eq!(payment.month(), Month::February);
        assert_eq!(payment.day(), 26);
    }
}
