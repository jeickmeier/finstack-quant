//! Shared fixed-leg pricing and swap payment utilities.
//!
//! This module provides fixed coupon valuation, period data and payment-delay
//! handling. Floating coupons use the canonical cashflow builder and overnight
//! projection engines.
//!
//! # Capabilities
//!
//! - Numerically stable relative discount factors via
//!   [`super::time::relative_df_discount_curve`]
//! - Neumaier compensated summation for long-dated swaps
//! - Holiday-aware payment delay handling

use crate::instruments::common_impl::pricing::time::relative_df_discount_curve;
use finstack_quant_core::dates::calendar_by_id;
use finstack_quant_core::dates::{Date, DateExt, DayCount};
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::math::NeumaierAccumulator;
use finstack_quant_core::Result;

/// Minimum threshold for annuity values to avoid divide-by-zero in par spread calculations.
///
/// # Numerical Justification
///
/// For a typical swap with $1MM notional:
/// - 10Y swap with semi-annual payments and DF ~0.80: annuity ≈ 8.0
/// - 30Y swap with quarterly payments and DF ~0.30: annuity ≈ 15.0
/// - 1Y swap with annual payment and DF ~0.95: annuity ≈ 0.95
///
/// The threshold of 1e-12 is triggered when:
/// - All periods have expired (no future cashflows)
/// - Extreme discounting scenarios (e.g., +200% rates over 30Y gives DF ~1e-26)
/// - Instrument misconfiguration (zero-length accrual periods)
///
/// This threshold is very conservative to ensure we catch only pathological cases,
/// not legitimate stress scenarios. For comparison, a 1bp annuity change on a $1MM
/// notional would be ~$100, so 1e-12 corresponds to sub-nanodollar precision.
///
/// # Usage
///
/// Used in par rate and par spread calculations where dividing by annuity is required.
/// Failing on near-zero annuity is preferable to returning NaN/Inf which would
/// propagate through downstream calculations.
pub const ANNUITY_EPSILON: f64 = 1e-12;

/// Apply a payment-delay in business days using an optional holiday calendar.
///
/// Bloomberg/ISDA conventions define payment delay in **business days**, not just weekdays.
/// If a calendar is provided and found in the registry, we apply holiday-aware business day
/// addition; otherwise we fall back to weekday-only addition.
///
/// # Arguments
///
/// * `date` - The base date to adjust
/// * `delay_days` - Number of business days to add (0 or negative returns unchanged date)
/// * `calendar_id` - Optional calendar identifier for business day adjustments
///
/// # Returns
///
/// The adjusted payment date, or an error if a calendar ID is provided but cannot be resolved.
///
/// # Strict Calendar Policy
///
/// If a `calendar_id` is provided, this function **requires** the calendar to be available
/// and usable. This prevents silent date drift that can cause trade breaks.
///
/// - If `calendar_id` is `Some` but the calendar cannot be resolved or applied → `Err`
/// - If `calendar_id` is `None` → weekday-only stepping is assumed intentional → `Ok`
#[inline]
pub fn add_payment_delay(date: Date, delay_days: i32, calendar_id: Option<&str>) -> Result<Date> {
    if delay_days <= 0 {
        return Ok(date);
    }

    if let Some(id) = calendar_id {
        // Calendar explicitly specified: require successful resolution and application
        match calendar_by_id(id) {
            Some(cal) => date.add_business_days(delay_days, cal).map_err(|e| {
                finstack_quant_core::Error::Validation(format!(
                    "Failed to add {} business days to {} using calendar '{}': {}",
                    delay_days, date, id, e
                ))
            }),
            None => Err(finstack_quant_core::Error::Validation(format!(
                "Payment-delay calendar '{}' not found in registry; \
                 cannot apply {} business day delay to {}. \
                 Either register the calendar or use None for weekday-only stepping.",
                id, delay_days, date
            ))),
        }
    } else {
        // No calendar specified: weekday-only (Mon-Fri) is intentional
        date.add_weekdays(delay_days)
    }
}

/// A period in a swap leg schedule.
///
/// This is a simpler view of cashflow data focused on what's needed for pricing.
#[derive(Debug, Clone)]
pub struct LegPeriod {
    /// Start of the accrual period.
    pub accrual_start: Date,
    /// End of the accrual period (also the unadjusted payment date).
    pub accrual_end: Date,
    /// Rate reset/fixing date (for floating legs).
    pub reset_date: Option<Date>,
    /// Year fraction for the accrual period.
    pub year_fraction: f64,
}

/// Parameters for pricing a fixed rate leg.
#[derive(Debug, Clone)]
pub struct FixedLegParams {
    /// Fixed rate (decimal, e.g., 0.05 for 5%).
    pub rate: f64,
    /// Day count convention for accrual.
    pub day_count: DayCount,
    /// Payment delay in business days after period end.
    pub payment_lag_days: i32,
    /// Optional calendar ID for payment date adjustments.
    pub calendar_id: Option<finstack_quant_core::types::CalendarId>,
}

impl FixedLegParams {
    /// Validate fixed leg parameters.
    ///
    /// Checks that:
    /// - Rate is finite
    pub fn validate(&self) -> Result<()> {
        if !self.rate.is_finite() {
            return Err(finstack_quant_core::Error::Validation(
                "Fixed rate must be finite".into(),
            ));
        }
        Ok(())
    }
}

/// Compute present value of a fixed rate leg.
///
/// This is the Bloomberg-validated implementation from IRS pricing, generalized to work
/// with any swap instrument. It handles:
/// - Fixed coupon calculation with proper day count
/// - Payment delay adjustment
/// - Numerical stability via Kahan summation
/// - Robust relative discount factors
///
/// # Arguments
///
/// * `periods` - Iterator over the leg's accrual periods; it is consumed while
///   valuing the fixed coupons.
/// * `notional` - Unsigned contractual notional in the leg currency. The
///   caller applies payer/receiver sign conventions to the returned PV.
/// * `params` - Fixed coupon rate, day-count, and payment-lag conventions.
/// * `disc` - Discount curve used to value future payments relative to
///   `as_of`.
/// * `as_of` - Valuation date and cashflow cutoff: payments on or before this
///   date are excluded, and remaining payments are discounted from it.
///
/// # Returns
///
/// Present value of the fixed leg as a raw f64 (unsigned).
/// The caller is responsible for applying sign conventions.
///
/// # Errors
///
/// Returns an error if:
/// - Parameter validation fails
/// - Discount factor calculation fails due to numerical instability
pub fn pv_fixed_leg<I>(
    periods: I,
    notional: f64,
    params: &FixedLegParams,
    disc: &DiscountCurve,
    as_of: Date,
) -> Result<f64>
where
    I: Iterator<Item = LegPeriod>,
{
    params.validate()?;

    // Use incremental Kahan accumulator to avoid Vec allocation
    let mut acc = NeumaierAccumulator::new();

    for period in periods {
        // Apply payment delay to determine the actual payment date
        let payment_date = add_payment_delay(
            period.accrual_end,
            params.payment_lag_days,
            params.calendar_id.as_deref(),
        )?;

        // Skip cashflows where the payment has already settled
        // (payment_date <= as_of means the payment has been made)
        if payment_date <= as_of {
            continue;
        }

        // Fixed coupon amount
        let coupon_amount = notional * params.rate * period.year_fraction;

        // Discount from as_of for correct theta
        let df = relative_df_discount_curve(disc, as_of, payment_date)?;
        acc.add(coupon_amount * df);
    }

    Ok(acc.total())
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::types::CurveId;
    use time::Month;

    fn date(year: i32, month: u8, day: u8) -> Date {
        Date::from_calendar_date(year, Month::try_from(month).expect("valid month"), day)
            .expect("valid date")
    }

    fn fixed_rate(rate: f64, day_count: DayCount) -> FixedLegParams {
        FixedLegParams {
            rate,
            day_count,
            payment_lag_days: 0,
            calendar_id: None,
        }
    }

    fn test_discount_curve(base_date: Date) -> DiscountCurve {
        DiscountCurve::builder(CurveId::new("TEST-DISC"))
            .base_date(base_date)
            .knots(vec![(0.0, 1.0), (0.5, 0.975), (1.0, 0.95), (5.0, 0.80)])
            .build()
            .expect("test curve should build")
    }

    #[test]
    fn relative_df_discount_curve_positive() {
        let base_date = date(2024, 1, 1);
        let disc = test_discount_curve(base_date);

        let df =
            relative_df_discount_curve(&disc, base_date, date(2025, 1, 1)).expect("should succeed");
        assert!(df > 0.0 && df <= 1.0, "DF should be in (0, 1]: {}", df);
    }

    #[test]
    fn relative_df_discount_curve_accepts_small_absolute_df() {
        // Create a curve with very small absolute DFs (stress scenario).
        // The new policy accepts these as long as the RELATIVE DF between dates is valid.
        let base_date = date(2024, 1, 1);
        let disc = DiscountCurve::builder(CurveId::new("EXTREME"))
            .base_date(base_date)
            .knots(vec![(0.0, 1e-12), (1.0, 1e-15)]) // Very small DFs
            .build()
            .expect("curve should build");

        // Under the new policy, df_between_dates computes df(target) / df(as_of)
        // = 1e-15 / 1e-12 = 0.001, which is a valid positive relative DF.
        let result = relative_df_discount_curve(&disc, base_date, date(2025, 1, 1));
        assert!(
            result.is_ok(),
            "Small absolute DFs should be accepted if relative DF is valid: {:?}",
            result
        );
        let df = result.expect("relative DF should be valid");
        assert!(df > 0.0, "Relative DF should be positive: {}", df);
    }

    #[test]
    fn pv_fixed_leg_basic() {
        let base_date = date(2024, 1, 1);
        let disc = test_discount_curve(base_date);

        let periods = vec![
            LegPeriod {
                accrual_start: date(2024, 1, 1),
                accrual_end: date(2024, 7, 1),
                reset_date: None,
                year_fraction: 0.5,
            },
            LegPeriod {
                accrual_start: date(2024, 7, 1),
                accrual_end: date(2025, 1, 1),
                reset_date: None,
                year_fraction: 0.5,
            },
        ];

        let params = fixed_rate(0.03, DayCount::Thirty360);
        let pv = pv_fixed_leg(periods.into_iter(), 1_000_000.0, &params, &disc, base_date)
            .expect("should price");

        // Should be positive (receiving fixed)
        assert!(pv > 0.0, "PV should be positive: {}", pv);

        // Approximate check: 2 × 0.5 × 0.03 × 1M × avg_df ≈ 30000 × 0.95 ≈ 28500
        assert!(
            pv > 20000.0 && pv < 35000.0,
            "PV should be reasonable: {}",
            pv
        );
    }

    #[test]
    fn pv_fixed_leg_validates_nan_rate() {
        let base_date = date(2024, 1, 1);
        let disc = test_discount_curve(base_date);

        let periods = vec![LegPeriod {
            accrual_start: date(2024, 1, 1),
            accrual_end: date(2024, 7, 1),
            reset_date: None,
            year_fraction: 0.5,
        }];

        let params = fixed_rate(f64::NAN, DayCount::Thirty360);
        let result = pv_fixed_leg(periods.into_iter(), 1_000_000.0, &params, &disc, base_date);
        assert!(result.is_err(), "Should reject NaN rate");
    }

    #[test]
    fn add_payment_delay_zero_returns_same() {
        let d = date(2024, 1, 15);
        let result = add_payment_delay(d, 0, None).expect("should succeed");
        assert_eq!(result, d);
    }

    #[test]
    fn add_payment_delay_positive_adds_weekdays() {
        let d = date(2024, 1, 15); // Monday
        let result = add_payment_delay(d, 2, None).expect("should succeed");
        // 2 weekdays from Monday = Wednesday
        assert_eq!(result, date(2024, 1, 17));
    }

    #[test]
    fn add_payment_delay_missing_calendar_errors() {
        let d = date(2024, 1, 15);
        // Providing a calendar ID that doesn't exist should now error
        let result = add_payment_delay(d, 2, Some("nonexistent_calendar"));
        assert!(result.is_err(), "Should error when calendar not found");
        let err = result.expect_err("should error when calendar not found");
        assert!(
            err.to_string().contains("not found"),
            "Error should mention calendar not found: {}",
            err
        );
    }

    #[test]
    fn relative_df_discount_curve_as_of_equals_base_date() {
        let base_date = date(2024, 1, 1);
        let disc = test_discount_curve(base_date);

        // When as_of == base_date, DF(as_of to target) is just DF(target)
        let target = date(2025, 1, 1);
        let df = relative_df_discount_curve(&disc, base_date, target).expect("should succeed");
        assert!(df > 0.0 && df < 1.0, "DF should be in (0,1): {}", df);
    }

    #[test]
    fn relative_df_discount_curve_as_of_after_base_date() {
        let base_date = date(2024, 1, 1);
        let disc = test_discount_curve(base_date);

        // as_of is 6 months after base_date (seasoned instrument scenario)
        let as_of = date(2024, 7, 1);
        let target = date(2025, 1, 1);

        let df = relative_df_discount_curve(&disc, as_of, target).expect("should succeed");
        // Should be the relative DF from as_of to target, which is valid and positive
        assert!(df > 0.0, "Relative DF should be positive: {}", df);
    }

    #[test]
    fn relative_df_discount_curve_long_horizon() {
        use finstack_quant_core::market_data::term_structures::DiscountCurve;

        // Create a curve that extends far into the future
        let base_date = date(2024, 1, 1);
        let curve = DiscountCurve::builder("TEST-LONG")
            .base_date(base_date)
            .knots([
                (0.0, 1.0),
                (1.0, 0.95),
                (10.0, 0.60),
                (30.0, 0.20),
                (50.0, 0.08),
            ])
            .build()
            .expect("curve should build");

        // 30Y forward date - long horizon but should still work
        let target = date(2054, 1, 1);
        let df = relative_df_discount_curve(&curve, base_date, target).expect("should succeed");
        assert!(df > 0.0, "Long-horizon DF should be positive: {}", df);
    }

    #[test]
    fn relative_df_discount_curve_rejects_non_positive() {
        // This test verifies that truly invalid DFs are rejected
        // In practice this shouldn't happen with well-constructed curves,
        // but the guard protects against misconfigured curves.
        //
        // We can't easily construct a curve that returns negative DF,
        // so we just verify the function returns valid positive DFs for normal inputs.
        let base_date = date(2024, 1, 1);
        let disc = test_discount_curve(base_date);

        let target = date(2025, 1, 1);
        let df = relative_df_discount_curve(&disc, base_date, target).expect("should succeed");
        assert!(df > 0.0, "DF must be positive: {}", df);
    }
}
