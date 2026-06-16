//! Internal Rate of Return (IRR) utilities for revolving credit facilities.
//!
//! This module provides convenience functions for IRR calculation specific to
//! revolving credit cashflows. For general IRR calculations, use the functions
//! from `finstack_quant_core::cashflow`:
//!
//! - [`finstack_quant_core::cashflow::xirr_with_daycount`] - explicit-daycount XIRR helper
//!
//! # Facility-Specific Function
//!
//! - [`calculate_path_irr`]: Converts time-fraction cashflows (from MC paths) to dates
//!   and computes XIRR. This is useful when working with path-based data where
//!   cashflows are specified as `(time_in_years, amount)` tuples.
//!
//! # Usage
//!
//! ```text
//! use finstack_quant_valuations::instruments::fixed_income::revolving_credit::metrics::irr::calculate_path_irr;
//! use finstack_quant_core::cashflow::xirr_with_daycount;
//! use finstack_quant_core::dates::{Date, DayCount};
//! use time::Month;
//!
//! // For dated cashflows, use core's trait directly:
//! let dated_cashflows = vec![
//!     (Date::from_calendar_date(2025, Month::January, 1).unwrap(), -1_000_000.0),
//!     (Date::from_calendar_date(2026, Month::January, 1).unwrap(), 1_050_000.0),
//! ];
//! let irr = xirr_with_daycount(&dated_cashflows, DayCount::Act365F, None).ok();
//!
//! // For MC path data with time fractions:
//! let base_date = Date::from_calendar_date(2025, Month::January, 1).unwrap();
//! let path_cashflows = vec![(0.0, -1_000_000.0), (1.0, 1_050_000.0)];
//! let irr = calculate_path_irr(&path_cashflows, base_date, DayCount::Act365F);
//! ```

use finstack_quant_core::cashflow::xirr_with_daycount;
use finstack_quant_core::dates::{Date, DayCount};

/// Calculate IRR from Monte Carlo path cashflows (time fractions).
///
/// Convenience function that converts time-fraction cashflows to dates and
/// computes XIRR using `finstack_quant_core::cashflow::xirr_with_daycount`.
///
/// This is primarily useful for revolving credit Monte Carlo simulations where
/// cashflows are generated as `(time_in_years, amount)` tuples relative to
/// a base date.
///
/// For dated cashflows, use `finstack_quant_core::cashflow::xirr_with_daycount` directly.
///
/// # Arguments
///
/// * `cashflows` - Vector of (time_in_years, amount) tuples
/// * `base_date` - Base date for time calculations
/// * `day_count` - Day count convention (used to determine year basis for conversion)
///
/// # Returns
///
/// IRR as an annualized decimal, or None if IRR doesn't exist or calculation fails.
///
/// # Example
///
/// ```text
/// use finstack_quant_valuations::instruments::fixed_income::revolving_credit::metrics::irr::calculate_path_irr;
/// use finstack_quant_core::dates::{Date, DayCount};
/// use time::Month;
///
/// let base = Date::from_calendar_date(2025, Month::January, 1).unwrap();
/// let cashflows = vec![
///     (0.0, -1_000_000.0),    // Initial deployment
///     (0.25, 12_500.0),       // Q1 interest
///     (1.0, 1_012_500.0),     // Final payment
/// ];
///
/// let irr = calculate_path_irr(&cashflows, base, DayCount::Act365F);
/// ```
#[allow(dead_code)] // public API for external bindings
pub(crate) fn calculate_path_irr(
    cashflows: &[(f64, f64)],
    base_date: Date,
    day_count: DayCount,
) -> Option<f64> {
    if cashflows.len() < 2 {
        return None;
    }

    // Determine year basis from day count for time-to-date conversion
    let days_per_year = match day_count {
        DayCount::Act360 | DayCount::Thirty360 | DayCount::ThirtyE360 => 360.0,
        _ => 365.0, // Act365F, Act365L, ActAct, etc.
    };

    // Convert time fractions to dates
    let dated_cashflows: Vec<(Date, f64)> = cashflows
        .iter()
        .filter_map(|(time_years, amount)| {
            let days = (*time_years * days_per_year).round() as i64;
            base_date
                .checked_add(time::Duration::days(days))
                .map(|date| (date, *amount))
        })
        .collect();

    if dated_cashflows.len() < 2 {
        return None;
    }

    // Use the explicit-daycount XIRR path so the caller's convention is preserved.
    xirr_with_daycount(&dated_cashflows, day_count, None).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::Month;

    #[test]
    fn test_calculate_path_irr_simple() {
        let base = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");

        // Simple case: deploy 1M, receive 1.05M in 1 year = 5% IRR
        let cashflows = vec![(0.0, -1_000_000.0), (1.0, 1_050_000.0)];

        let irr = calculate_path_irr(&cashflows, base, DayCount::Act365F);
        assert!(irr.is_some());
        let irr_val = irr.expect("should have IRR");
        assert!(
            (irr_val - 0.05).abs() < 0.001,
            "Expected ~5% IRR, got {}",
            irr_val
        );
    }

    #[test]
    fn test_calculate_path_irr_quarterly_payments() {
        let base = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");

        // Quarterly payments: deploy 1M, receive 12.5k quarterly + 1M at end
        let cashflows = vec![
            (0.0, -1_000_000.0),
            (0.25, 12_500.0),
            (0.5, 12_500.0),
            (0.75, 12_500.0),
            (1.0, 1_012_500.0),
        ];

        let irr = calculate_path_irr(&cashflows, base, DayCount::Act365F);
        assert!(irr.is_some());
        let irr_val = irr.expect("should have IRR");
        assert!(
            irr_val > 0.04 && irr_val < 0.06,
            "Expected ~5% IRR, got {}",
            irr_val
        );
    }

    #[test]
    fn test_calculate_path_irr_no_sign_change() {
        let base = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");

        // All positive - no IRR
        let cashflows = vec![(0.0, 1_000_000.0), (1.0, 1_050_000.0)];

        let irr = calculate_path_irr(&cashflows, base, DayCount::Act365F);
        assert!(irr.is_none());
    }

    #[test]
    fn test_calculate_path_irr_respects_day_count() {
        let base = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let cashflows = vec![(0.0, -1_000_000.0), (0.5, 1_025_000.0)];

        let irr_365 = calculate_path_irr(&cashflows, base, DayCount::Act365F).expect("Act365F IRR");
        let irr_360 = calculate_path_irr(&cashflows, base, DayCount::Act360).expect("Act360 IRR");

        assert!(
            (irr_365 - irr_360).abs() > 1e-4,
            "expected day count to influence path IRR"
        );
    }
}
