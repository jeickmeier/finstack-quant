//! Date-aware ACT/365F model time and rebased discounting.
use finstack_quant_core::dates::{Date, DayCount, DayCountContext};
use finstack_quant_core::market_data::traits::Discounting;
use finstack_quant_core::types::CurveId;
use finstack_quant_core::{Error, Result};

/// Convert a date to signed ACT/365F model years.
///
/// # Arguments
///
/// * `as_of` - Valuation date defining time zero.
/// * `date` - Contractual event date; earlier dates yield negative times.
#[must_use]
pub fn model_time(as_of: Date, date: Date) -> f64 {
    (date - as_of).whole_days() as f64 / 365.0
}

/// Map ACT/365F model years to a curve's own date origin and day count.
/// Fractional days interpolate the destination clock between adjacent dates.
///
/// # Arguments
///
/// * `as_of` - Valuation date defining model time zero.
/// * `time` - Finite non-negative ACT/365F years from `as_of`.
/// * `curve_base_date` - Date defining time zero on the destination curve.
/// * `day_count` - Destination curve day count, independent of coupon accrual.
///
/// # Errors
///
/// Returns a validation error for invalid model times or unsupported day counts.
pub fn model_time_on_curve(
    as_of: Date,
    time: f64,
    curve_base_date: Date,
    day_count: DayCount,
) -> Result<f64> {
    let days = time * 365.0;
    if !days.is_finite() || days < 0.0 || days > (Date::MAX - as_of).whole_days() as f64 {
        return Err(Error::Validation(
            "model time must be finite, non-negative and within the date range".into(),
        ));
    }
    let date = as_of + time::Duration::days(days.floor() as i64);
    let curve_time =
        |date| day_count.signed_year_fraction(curve_base_date, date, DayCountContext::default());
    let mut mapped = curve_time(date)?;
    let fraction = days.fract();
    if fraction > 0.0 {
        let next = date
            .next_day()
            .ok_or_else(|| Error::Validation("model time exceeds date range".into()))?;
        mapped += fraction * (curve_time(next)? - mapped);
    }
    Ok(mapped)
}

/// Borrowed discount curve on ACT/365F model time, normalized at valuation date.
///
/// Invalid floating-point lookups yield NaN under the `Discounting::df`
/// contract; use `get_df` for diagnostic errors at checked boundaries.
pub struct ModelDiscountCurve<'a> {
    curve: &'a dyn Discounting,
    as_of: Date,
    df_as_of: f64,
}

impl<'a> ModelDiscountCurve<'a> {
    /// Normalize a curve to valuation date and expose it on the model clock.
    ///
    /// # Arguments
    ///
    /// * `curve` - Source curve, with its own base date and day count.
    /// * `as_of` - Valuation date; the returned curve has discount factor one here.
    ///
    /// # Errors
    ///
    /// Returns a validation error for an invalid date mapping or discount factor.
    pub fn new(curve: &'a dyn Discounting, as_of: Date) -> Result<Self> {
        let t = model_time_on_curve(as_of, 0.0, curve.base_date(), curve.day_count())?;
        let df_as_of = curve.df(t);
        if !df_as_of.is_finite() || df_as_of <= 0.0 {
            return Err(Error::Validation(format!(
                "invalid model discount factor at valuation date {as_of}: {df_as_of}"
            )));
        }
        Ok(Self {
            curve,
            as_of,
            df_as_of,
        })
    }

    /// Get the discount factor from valuation date to a model time.
    ///
    /// # Arguments
    ///
    /// * `time` - Finite non-negative ACT/365F years since valuation date.
    ///
    /// # Errors
    ///
    /// Returns a validation error for an invalid mapping or non-positive, non-finite factor.
    pub fn get_df(&self, time: f64) -> Result<f64> {
        let mapped = model_time_on_curve(
            self.as_of,
            time,
            self.curve.base_date(),
            self.curve.day_count(),
        )?;
        let df = self.curve.df(mapped) / self.df_as_of;
        if !df.is_finite() || df <= 0.0 {
            return Err(Error::Validation(format!(
                "invalid model discount factor at t={time}: {df}"
            )));
        }
        Ok(df)
    }
}

impl Discounting for ModelDiscountCurve<'_> {
    fn id(&self) -> &CurveId {
        self.curve.id()
    }
    fn base_date(&self) -> Date {
        self.as_of
    }
    fn df(&self, t: f64) -> f64 {
        self.get_df(t).unwrap_or(f64::NAN)
    }
}
