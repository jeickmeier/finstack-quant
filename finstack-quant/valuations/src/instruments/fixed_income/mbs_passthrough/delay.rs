//! Payment delay conventions for agency MBS.
//!
//! Agency MBS have standardized stated delays measured from the **first day of
//! the accrual period** to the payment date. Post-Single Security Initiative
//! (June 2019), FNMA and FHLMC both issue UMBS with the same 55-day delay:
//!
//! - **FNMA / FHLMC (UMBS)**: 55-day stated delay — payment on the 25th of M+1
//! - **GNMA I**: 45-day stated delay — single-issuer pools, payment on the 15th of M+1
//! - **GNMA II**: 50-day stated delay — multi-issuer pools, payment on the 20th of M+1
//!
//! These constants match [`AgencyProgram::payment_lag_days`] and the
//! calendar-based [`AgencyProgram::payment_date_for_period`]. Legacy FHLMC
//! Gold PCs (45-day) and ARM PCs (75-day) predate UMBS and should be modeled
//! via the per-pool `payment_lag_days` override on `AgencyMbsPassthrough`.

use crate::instruments::fixed_income::mbs_passthrough::AgencyProgram;
use finstack_quant_core::dates::Date;
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
/// assert_eq!(payment_lag_days(AgencyProgram::GnmaI), 45);
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

/// Generate payment schedule with delays for a series of accrual periods.
///
/// Uses the agency's following-month payment day and Federal Reserve holiday
/// calendar, rather than adding an approximate stated delay.
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
    accrual_starts
        .iter()
        .map(|&accrual_start| {
            let payment =
                agency.payment_date_for_period(accrual_start.year(), accrual_start.month())?;
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
        assert_eq!(payment_lag_days(AgencyProgram::GnmaI), 45);
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

        // February 25, 2024 is Sunday: the FNMA payment follows on Monday.
        assert_eq!(schedule[0].0, accrual_starts[0]);
        assert_eq!(schedule[0].1.month(), Month::February);
        assert_eq!(schedule[0].1.day(), 26);
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
}
