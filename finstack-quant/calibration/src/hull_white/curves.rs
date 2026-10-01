//! Curve-based Hull-White calibration with canonical model-time mapping.

use super::{
    bootstrap_hull_white_sigma_schedule_to_cap_floors_with_fn,
    calibrate_hull_white_to_cap_floors_with_fn, calibrate_hull_white_to_swaptions_with_fn,
    CapFloorCalibrationConfig, CapFloorQuote, HullWhiteCalibrationParams, HullWhiteParams,
    PiecewiseSigmaCalibrationConfig, SwapFrequency, SwaptionQuote,
};
use crate::CalibrationReport;
use finstack_quant_core::market_data::{traits::Discounting, DiscountCurve};
use finstack_quant_core::Result;
use finstack_quant_models::rates::clock::ModelDiscountCurve;

fn model_curves<'a>(
    discount: &'a DiscountCurve,
    forward: Option<&'a DiscountCurve>,
) -> Result<(ModelDiscountCurve<'a>, ModelDiscountCurve<'a>)> {
    let as_of = discount.base_date();
    Ok((
        ModelDiscountCurve::new(discount, as_of)?,
        ModelDiscountCurve::new(forward.unwrap_or(discount), as_of)?,
    ))
}

/// Fit scalar Hull-White parameters to ATM swaption quotes using a dated curve.
///
/// The curve is mapped from its day count to ACT/365F model years, normalized
/// at its base date. Invalid factor lookups follow the model curve's `NaN`
/// discounting contract and are rejected by the calibrator.
///
/// # Arguments
///
/// * `discount` - Discount curve defining valuation date and discount factors.
/// * `quotes` - ATM normal or Black-volatility quotes; expiry and tenor are
///   ACT/365F years from the discount curve's base date.
/// * `frequency` - Fixed-leg frequency used to construct synthetic swap schedules.
/// * `initial_guess` - Optional positive mean-reversion and short-rate volatility
///   seed; `None` uses the calibrator's defaults.
/// * `fit_tolerance` - Positive maximum absolute reconstructed quote error;
///   decimal rate volatility for normal quotes, relative volatility for Black quotes.
///
/// # Errors
///
/// Returns an error for invalid curve normalization, quotes, or numerical fitting.
/// A fit outside `fit_tolerance` returns a report with `success = false`.
pub fn calibrate_hull_white_to_swaptions(
    discount: &DiscountCurve,
    quotes: &[SwaptionQuote],
    frequency: SwapFrequency,
    initial_guess: Option<HullWhiteCalibrationParams>,
    fit_tolerance: f64,
) -> Result<(HullWhiteCalibrationParams, CalibrationReport)> {
    let model_discount = ModelDiscountCurve::new(discount, discount.base_date())?;
    let df = |time| model_discount.df(time);
    calibrate_hull_white_to_swaptions_with_fn(
        &df,
        quotes,
        frequency,
        None,
        initial_guess,
        fit_tolerance,
    )
}

/// Fit scalar Hull-White parameters to normal cap/floor quotes using dated curves.
///
/// Both curves are normalized at `discount.base_date()` and mapped from their
/// own day counts to ACT/365F model years. Invalid factor lookups follow the
/// model curve's `NaN` discounting contract and are rejected by the calibrator.
///
/// # Arguments
///
/// * `discount` - Discount curve defining valuation date and caplet discounting.
/// * `quotes` - Normal/Bachelier cap or floor quotes; maturities are ACT/365F
///   years from the discount curve's base date and volatilities are decimal rate units.
/// * `config` - Fit tolerance, payment frequency, and optional fixed mean reversion
///   or initial parameters. A single quote requires fixed mean reversion.
/// * `forward` - Optional projection curve with its own base date and day count;
///   `None` projects forwards using `discount`.
///
/// # Errors
///
/// Returns an error for invalid curve normalization, quotes, or numerical fitting.
/// A fit outside the configured tolerance returns a report with `success = false`.
pub fn calibrate_hull_white_to_cap_floors(
    discount: &DiscountCurve,
    quotes: &[CapFloorQuote],
    config: CapFloorCalibrationConfig,
    forward: Option<&DiscountCurve>,
) -> Result<(HullWhiteCalibrationParams, CalibrationReport)> {
    let (model_discount, model_forward) = model_curves(discount, forward)?;
    let discount_df = |time| model_discount.df(time);
    let forward_df = |time| model_forward.df(time);
    calibrate_hull_white_to_cap_floors_with_fn(&discount_df, &forward_df, quotes, None, config)
}

/// Bootstrap piecewise Hull-White volatility from normal cap/floor quotes and curves.
///
/// Both curves use ACT/365F model time normalized at `discount.base_date()`.
/// Invalid factor lookups follow the model curve's `NaN` discounting contract
/// and are rejected by the calibrator.
///
/// # Arguments
///
/// * `discount` - Discount curve defining valuation date and caplet discounting.
/// * `quotes` - Normal/Bachelier cap or floor quotes with distinct maturities in
///   ACT/365F years from the discount curve's base date; volatility is in decimal rate units.
/// * `config` - Fixed mean reversion, positive sigma brackets, quote-fit tolerance,
///   and frequency used while solving successive volatility intervals.
/// * `forward` - Optional projection curve mapped from its own base date and day count;
///   `None` projects forwards using `discount`.
///
/// # Errors
///
/// Returns an error for invalid curves, quotes, settings, or an unsolved sigma interval.
/// A fit outside the configured tolerance returns a report with `success = false`.
pub fn bootstrap_hull_white_sigma_schedule_to_cap_floors(
    discount: &DiscountCurve,
    quotes: &[CapFloorQuote],
    config: PiecewiseSigmaCalibrationConfig,
    forward: Option<&DiscountCurve>,
) -> Result<(HullWhiteParams, CalibrationReport)> {
    let (model_discount, model_forward) = model_curves(discount, forward)?;
    let discount_df = |time| model_discount.df(time);
    let forward_df = |time| model_forward.df(time);
    bootstrap_hull_white_sigma_schedule_to_cap_floors_with_fn(
        &discount_df,
        &forward_df,
        quotes,
        None,
        config,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::dates::{Date, DayCount};
    use finstack_quant_core::math::interp::InterpStyle;
    use time::Month;

    #[test]
    fn curve_adapter_preserves_distinct_dates_and_day_counts() {
        let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let forward_base = Date::from_calendar_date(2024, Month::January, 1).expect("valid date");
        let discount = DiscountCurve::builder("DISCOUNT")
            .base_date(as_of)
            .day_count(DayCount::Act360)
            .knots([(0.0, 1.0), (20.0, (-0.03_f64 * 20.0).exp())])
            .interp(InterpStyle::LogLinear)
            .build()
            .expect("valid curve");
        let forward = DiscountCurve::builder("FORWARD")
            .base_date(forward_base)
            .day_count(DayCount::Act365F)
            .knots([(0.0, 1.0), (20.0, (-0.04_f64 * 20.0).exp())])
            .interp(InterpStyle::LogLinear)
            .build()
            .expect("valid curve");
        let quotes = [CapFloorQuote::try_new(5.0, 0.04, 0.009, true, true).expect("valid quote")];
        let config = CapFloorCalibrationConfig {
            fit_tolerance: 1e-5,
            frequency: SwapFrequency::Quarterly,
            fixed_kappa: Some(0.03),
            initial_guess: None,
        };
        let discount_df = |time: f64| (-0.03 * time * 365.0 / 360.0).exp();
        let forward_df = |time: f64| (-0.04 * time).exp();
        let (expected, _) = calibrate_hull_white_to_cap_floors_with_fn(
            &discount_df,
            &forward_df,
            &quotes,
            None,
            config,
        )
        .expect("reference fit");
        let (actual, report) =
            calibrate_hull_white_to_cap_floors(&discount, &quotes, config, Some(&forward))
                .expect("curve fit");
        assert!(report.success);
        assert!((actual.sigma - expected.sigma).abs() < 1e-12);
    }
}
