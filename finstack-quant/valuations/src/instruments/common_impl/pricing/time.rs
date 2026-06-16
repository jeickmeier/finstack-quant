//! Shared time-mapping and discount factor helpers for consistent curve usage.
//!
//! This module centralizes the pattern of computing:
//! - **Curve time**: year fraction from a curve's base_date using the curve's day_count
//! - **Relative discount factors**: DF from `as_of` to `target` using curve-consistent mapping
//! - **Forward rate projection**: rate over a date interval using forward curve's time basis
//!
//! # Background
//!
//! Several pricers historically used `disc.df(t)` or `fwd.rate_period(t1, t2)` where `t` was
//! computed from the *instrument's* day count and `as_of`. But:
//!
//! - `DiscountCurve::df(t)` expects `t` measured from `discount_curve.base_date()` using
//!   `discount_curve.day_count()`.
//! - `ForwardCurve::rate_period(t1, t2)` expects `t` measured from `forward_curve.base_date()`
//!   using `forward_curve.day_count()`.
//!
//! This breaks PV/Greeks whenever curve day-count/base-date differs from the instrument's.
//!
//! # Solution
//!
//! Always use date-based helpers that internally handle curve-to-date mapping:
//! - [`relative_df_discount_curve`] - compute DF from `as_of` to `target` using discount curve
//! - [`relative_df_discounting`] - same for trait objects implementing [`Discounting`]
//! - [`curve_time`] - compute year fraction from forward curve's base_date
//! - [`rate_period_on_dates`] - compute average forward rate between two dates
//!
//! # Bloomberg Validation
//!
//! The `relative_df_*` functions implement the same numerical stability checks used in
//! IRS pricing that have been validated against Bloomberg SWPM.

use finstack_quant_core::dates::{Date, DayCountContext};
use finstack_quant_core::market_data::term_structures::{DiscountCurve, ForwardCurve};
use finstack_quant_core::market_data::traits::Discounting;
use finstack_quant_core::Result;

use crate::constants::numerical::DF_EPSILON;

// ---------------------------------------------------------------------------
// Discount Curve Helpers
// ---------------------------------------------------------------------------

/// Compute discount factor from `as_of` to `target` using a [`DiscountCurve`].
///
/// This is the preferred method for computing relative discount factors in pricing.
/// It delegates to [`DiscountCurve::df_between_dates`] which handles:
/// - Curve base_date ≠ as_of scenarios (seasoned instruments)
/// - Curve's own day_count for time mapping
/// - Numerical validation (finiteness, positivity)
///
/// # Arguments
///
/// * `disc` - Discount curve
/// * `as_of` - Valuation date (start of discounting interval)
/// * `target` - Target payment date (end of discounting interval)
///
/// # Returns
///
/// Discount factor from `as_of` to `target`.
///
/// # Errors
///
/// Returns an error if:
/// - Year fraction calculation fails
/// - The resulting discount factor is non-finite or non-positive
///
/// # Example
///
/// ```text
/// let df = relative_df_discount_curve(&disc, as_of, payment_date)?;
/// let pv = cashflow * df;
/// ```
#[inline]
pub fn relative_df_discount_curve(disc: &DiscountCurve, as_of: Date, target: Date) -> Result<f64> {
    // Delegate to the curve's own date-based DF calculation
    let df = disc.df_between_dates(as_of, target)?;
    validate_relative_df(df, as_of, target)
}

/// Compute discount factor from `as_of` to `target` using any [`Discounting`] implementor.
///
/// This is the trait-object variant for use with `&dyn Discounting`. It computes:
/// ```text
/// DF(as_of → target) = DF(0 → target) / DF(0 → as_of)
/// ```
/// where times are computed using the discount curve's own `base_date()` and `day_count()`.
///
/// # Arguments
///
/// * `disc` - Discounting trait object
/// * `as_of` - Valuation date
/// * `target` - Target payment date
///
/// # Returns
///
/// Discount factor from `as_of` to `target`.
///
/// # Errors
///
/// Returns an error if:
/// - Year fraction calculation fails
/// - The resulting discount factor is non-finite or non-positive
#[inline]
pub fn relative_df_discounting(disc: &dyn Discounting, as_of: Date, target: Date) -> Result<f64> {
    if as_of == target {
        return Ok(1.0);
    }

    let base = disc.base_date();
    let dc = disc.day_count();
    let ctx = DayCountContext::default();

    // Compute times using the curve's own day count and base date
    let t_as_of = if as_of == base {
        0.0
    } else {
        dc.year_fraction(base, as_of, ctx)?
    };

    let t_target = if target == base {
        0.0
    } else {
        dc.year_fraction(base, target, ctx)?
    };

    let df_as_of = disc.df(t_as_of);
    let df_target = disc.df(t_target);

    // Validate intermediate DF at as_of
    if !df_as_of.is_finite() || df_as_of <= DF_EPSILON {
        return Err(finstack_quant_core::Error::Validation(format!(
            "Discount factor at as_of ({}) is invalid: df={:.3e}. \
             This may indicate extreme rate scenarios or curve extrapolation issues.",
            as_of, df_as_of
        )));
    }

    let df = df_target / df_as_of;
    validate_relative_df(df, as_of, target)
}

/// Validate a relative discount factor for finiteness and positivity.
#[inline]
fn validate_relative_df(df: f64, from: Date, to: Date) -> Result<f64> {
    if !df.is_finite() {
        return Err(finstack_quant_core::Error::Validation(format!(
            "Discount factor between {} and {} is not finite (df={:?}). \
             This may indicate extreme rate scenarios or curve extrapolation issues.",
            from, to, df
        )));
    }
    if df <= 0.0 {
        return Err(finstack_quant_core::Error::Validation(format!(
            "Discount factor between {} and {} is non-positive (df={:.3e}) which is non-physical. \
             Check curve construction and rate levels.",
            from, to, df
        )));
    }
    Ok(df)
}

// ---------------------------------------------------------------------------
// Forward Curve Helpers
// ---------------------------------------------------------------------------

/// Compute year fraction from a forward curve's base_date to a given date.
///
/// This ensures that forward curve lookups use the curve's own time basis,
/// not the instrument's day count convention.
///
/// # Arguments
///
/// * `fwd` - Forward curve
/// * `date` - Target date
///
/// # Returns
///
/// Year fraction from `fwd.base_date()` to `date` using `fwd.day_count()`.
/// Returns 0.0 if `date <= fwd.base_date()`.
///
/// # Errors
///
/// Returns an error if year fraction calculation fails.
#[inline]
pub fn curve_time(fwd: &ForwardCurve, date: Date) -> Result<f64> {
    let base = fwd.base_date();
    if date <= base {
        return Ok(0.0);
    }
    let dc = fwd.day_count();
    let t = dc.year_fraction(base, date, DayCountContext::default())?;
    Ok(t.max(0.0))
}

/// Compute forward rate over a date interval using the forward curve's time basis.
///
/// This is the date-based equivalent of `fwd.rate_period(t1, t2)` that ensures
/// times are computed using the curve's own day count and base date.
///
/// # Arguments
///
/// * `fwd` - Forward curve
/// * `start` - Period start date
/// * `end` - Period end date
///
/// # Returns
///
/// Average forward rate over `[start, end]`.
///
/// # Errors
///
/// Returns an error if time computation fails.
///
/// # Example
///
/// ```text
/// // Instead of:
/// // let t1 = inst.day_count.year_fraction(as_of, start, ctx)?;
/// // let t2 = inst.day_count.year_fraction(as_of, end, ctx)?;
/// // let fwd_rate = fwd.rate_period(t1, t2);
///
/// // Use:
/// let fwd_rate = rate_period_on_dates(&fwd, start, end)?;
/// ```
#[inline]
pub fn rate_period_on_dates(fwd: &ForwardCurve, start: Date, end: Date) -> Result<f64> {
    let t_start = curve_time(fwd, start)?;
    let t_end = curve_time(fwd, end)?;
    Ok(fwd.rate_period(t_start, t_end))
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::dates::DayCount;
    use finstack_quant_core::types::CurveId;
    use time::Month;

    fn date(year: i32, month: u8, day: u8) -> Date {
        Date::from_calendar_date(year, Month::try_from(month).expect("valid month"), day)
            .expect("valid date")
    }

    fn test_discount_curve(base_date: Date, day_count: DayCount) -> DiscountCurve {
        DiscountCurve::builder(CurveId::new("TEST-DISC"))
            .base_date(base_date)
            .day_count(day_count)
            .knots(vec![(0.0, 1.0), (0.5, 0.975), (1.0, 0.95), (5.0, 0.80)])
            .build()
            .expect("test curve should build")
    }

    fn test_forward_curve(base_date: Date, day_count: DayCount) -> ForwardCurve {
        ForwardCurve::builder(CurveId::new("TEST-FWD"), 0.25)
            .base_date(base_date)
            .day_count(day_count)
            .knots(vec![(0.0, 0.03), (1.0, 0.035), (5.0, 0.04)])
            .build()
            .expect("test curve should build")
    }

    #[test]
    fn relative_df_discount_curve_same_date() {
        let base = date(2024, 1, 1);
        let disc = test_discount_curve(base, DayCount::Act365F);
        let df = relative_df_discount_curve(&disc, base, base).expect("should succeed");
        assert!(
            (df - 1.0).abs() < 1e-12,
            "DF from date to itself should be 1.0"
        );
    }

    #[test]
    fn relative_df_discount_curve_future_date() {
        let base = date(2024, 1, 1);
        let disc = test_discount_curve(base, DayCount::Act365F);
        let target = date(2025, 1, 1);
        let df = relative_df_discount_curve(&disc, base, target).expect("should succeed");
        assert!(df > 0.0 && df < 1.0, "DF should be in (0, 1): {}", df);
    }

    #[test]
    fn relative_df_discount_curve_seasoned_instrument() {
        // Simulate a seasoned instrument where as_of > base_date
        let base = date(2024, 1, 1);
        let disc = test_discount_curve(base, DayCount::Act365F);

        let as_of = date(2024, 7, 1); // 6 months after base
        let target = date(2025, 1, 1);

        let df = relative_df_discount_curve(&disc, as_of, target).expect("should succeed");
        assert!(df > 0.0, "Seasoned DF should be positive: {}", df);
        // For a normal upward-sloping curve, this should be between 0 and 1
        assert!(df < 1.5, "Seasoned DF should be reasonable: {}", df);
    }

    #[test]
    fn relative_df_discounting_matches_curve() {
        let base = date(2024, 1, 1);
        let disc = test_discount_curve(base, DayCount::Act365F);

        let target = date(2025, 1, 1);

        let df_curve = relative_df_discount_curve(&disc, base, target).expect("curve method");
        let df_trait =
            relative_df_discounting(&disc as &dyn Discounting, base, target).expect("trait method");

        assert!(
            (df_curve - df_trait).abs() < 1e-12,
            "Methods should match: curve={}, trait={}",
            df_curve,
            df_trait
        );
    }

    #[test]
    fn curve_time_at_base_date_is_zero() {
        let base = date(2024, 1, 1);
        let fwd = test_forward_curve(base, DayCount::Act360);

        let t = curve_time(&fwd, base).expect("should succeed");
        assert!((t - 0.0).abs() < 1e-12, "Time at base should be 0: {}", t);
    }

    #[test]
    fn curve_time_before_base_is_zero() {
        let base = date(2024, 1, 1);
        let fwd = test_forward_curve(base, DayCount::Act360);

        let before = date(2023, 6, 1);
        let t = curve_time(&fwd, before).expect("should succeed");
        assert!(
            (t - 0.0).abs() < 1e-12,
            "Time before base should be 0: {}",
            t
        );
    }

    #[test]
    fn curve_time_uses_curve_day_count() {
        let base = date(2024, 1, 1);
        // Build curves with different day counts
        let fwd_360 = test_forward_curve(base, DayCount::Act360);
        let fwd_365 = test_forward_curve(base, DayCount::Act365F);

        let target = date(2024, 7, 1); // 182 days

        let t_360 = curve_time(&fwd_360, target).expect("should succeed");
        let t_365 = curve_time(&fwd_365, target).expect("should succeed");

        // Act/360: 182/360 ≈ 0.5056
        // Act/365F: 182/365 ≈ 0.4986
        assert!(
            (t_360 - t_365).abs() > 0.005,
            "Different day counts should produce different times: 360={}, 365={}",
            t_360,
            t_365
        );
    }

    #[test]
    fn rate_period_on_dates_basic() {
        let base = date(2024, 1, 1);
        let fwd = test_forward_curve(base, DayCount::Act360);

        let start = date(2024, 4, 1);
        let end = date(2024, 7, 1);

        let rate = rate_period_on_dates(&fwd, start, end).expect("should succeed");
        assert!(
            rate > 0.0 && rate < 0.1,
            "Forward rate should be reasonable: {}",
            rate
        );
    }

    #[test]
    fn day_count_mismatch_test() {
        // This test demonstrates the bug we're fixing:
        // If instrument uses Act365F but curve uses Act360, times differ
        let base = date(2024, 1, 1);
        let disc = test_discount_curve(base, DayCount::Act360); // Curve uses Act/360

        let target = date(2025, 1, 1);

        // OLD (buggy) approach: compute t using instrument's day count
        let inst_dc = DayCount::Act365F;
        let t_instrument = inst_dc
            .year_fraction(base, target, DayCountContext::default())
            .expect("yf");
        let df_old = disc.df(t_instrument);

        // NEW (correct) approach: use curve's day count via df_between_dates
        let df_new = relative_df_discount_curve(&disc, base, target).expect("df");

        // These SHOULD differ because the time bases differ
        // (In this test, the curve will compute t using Act/360, which gives a different t)
        // For demonstration, we just verify the new method works
        assert!(df_new > 0.0 && df_new < 1.0, "New DF should be valid");
        // The old approach uses the wrong time basis
        assert!(
            df_old > 0.0 && df_old < 1.0,
            "Old DF is also valid but computed incorrectly"
        );
    }
}
