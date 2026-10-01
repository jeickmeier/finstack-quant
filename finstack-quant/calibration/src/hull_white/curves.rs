//! Curve-input entry points for the direct Hull-White calibrators.
//!
//! The solvers in this module's siblings take discount-factor closures on the
//! ACT/365F model clock. Hosts hold calibrated [`DiscountCurve`] objects, so
//! these functions own the curve-to-closure adaptation
//! ([`ModelDiscountCurve`]) once for every caller.

use super::{
    bootstrap_hull_white_sigma_schedule_to_cap_floors, calibrate_hull_white_to_cap_floors,
    calibrate_hull_white_to_swaptions, CapFloorCalibrationConfig, CapFloorQuote,
    HullWhiteCalibrationParams, HullWhiteParams, PiecewiseSigmaCalibrationConfig, SwapFrequency,
    SwaptionQuote,
};
use crate::CalibrationReport;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::Result;
use finstack_quant_models::rates::clock::ModelDiscountCurve;

/// Run `solve` with discount-factor closures for the discount and projection curves.
///
/// Both curves are normalized at the discount curve's base date and read on
/// the ACT/365F model clock; a lookup that fails yields `NaN`, which the
/// solvers reject as an invalid discount factor.
fn with_model_curves<T>(
    discount: &DiscountCurve,
    forward: Option<&DiscountCurve>,
    solve: impl FnOnce(&(dyn Fn(f64) -> f64 + Sync), &(dyn Fn(f64) -> f64 + Sync)) -> Result<T>,
) -> Result<T> {
    let as_of = discount.base_date();
    let model_discount = ModelDiscountCurve::new(discount, as_of)?;
    let model_forward = ModelDiscountCurve::new(forward.unwrap_or(discount), as_of)?;
    let discount_df = |t: f64| model_discount.get_df(t).unwrap_or(f64::NAN);
    let forward_df = |t: f64| model_forward.get_df(t).unwrap_or(f64::NAN);
    solve(&discount_df, &forward_df)
}

/// Calibrate scalar Hull-White `(κ, σ)` to ATM swaption quotes on a discount curve.
///
/// Curve-input form of [`calibrate_hull_white_to_swaptions`] with the
/// synthetic constant-period fixed-leg schedule (no contractual schedules).
///
/// # Arguments
///
/// * `discount` - Discount curve the swap annuities and forward swap rates are
///   read from. Its base date is the valuation date; times are ACT/365F years
///   from that date.
/// * `quotes` - At least two ATM swaption quotes (expiry and tenor in years,
///   volatility as a decimal: absolute rate volatility for normal quotes,
///   Black volatility for lognormal quotes).
/// * `frequency` - Fixed-leg payment frequency of the underlying swaps.
/// * `initial_guess` - Optional solver seed for `(κ, σ)`; `None` uses the
///   built-in starting point.
/// * `fit_tolerance` - Required positive maximum implied-quote error: decimal
///   rate volatility for normal quotes, relative volatility for Black quotes.
///
/// # Errors
///
/// Returns a validation error for a non-positive `fit_tolerance`, fewer than
/// two quotes, an invalid quote or an unusable discount curve, and a
/// calibration error when the solver does not converge.
pub fn calibrate_hull_white_to_swaptions_from_curve(
    discount: &DiscountCurve,
    quotes: &[SwaptionQuote],
    frequency: SwapFrequency,
    initial_guess: Option<HullWhiteCalibrationParams>,
    fit_tolerance: f64,
) -> Result<(HullWhiteCalibrationParams, CalibrationReport)> {
    with_model_curves(discount, None, |df, _| {
        calibrate_hull_white_to_swaptions(df, quotes, frequency, None, initial_guess, fit_tolerance)
    })
}

/// Calibrate scalar Hull-White `(κ, σ)` to cap/floor quotes on market curves.
///
/// Curve-input form of [`calibrate_hull_white_to_cap_floors`].
///
/// # Arguments
///
/// * `discount` - Discounting curve; its base date is the valuation date and
///   times are ACT/365F years from that date.
/// * `forward` - Curve projecting the caplet forwards from ratios of its
///   discount factors, normalized at the discount curve's base date. `None`
///   projects on `discount` (single-curve).
/// * `quotes` - Normal-vol cap or floor quotes (maturity in years, strike and
///   volatility as decimals). A single quote requires `config.fixed_kappa`.
/// * `config` - Fit tolerance in normal-vol units, caplet payment frequency,
///   optional fixed mean reversion and optional solver seed.
///
/// # Errors
///
/// Returns a validation error when no quotes are supplied, a single quote is
/// given without `config.fixed_kappa`, the tolerance is not positive or a
/// curve is unusable, and a calibration error when the solver does not converge.
pub fn calibrate_hull_white_to_cap_floors_from_curves(
    discount: &DiscountCurve,
    forward: Option<&DiscountCurve>,
    quotes: &[CapFloorQuote],
    config: CapFloorCalibrationConfig,
) -> Result<(HullWhiteCalibrationParams, CalibrationReport)> {
    with_model_curves(discount, forward, |discount_df, forward_df| {
        calibrate_hull_white_to_cap_floors(discount_df, forward_df, quotes, config)
    })
}

/// Bootstrap a piecewise-constant Hull-White sigma schedule to cap/floor quotes on market curves.
///
/// Curve-input form of [`bootstrap_hull_white_sigma_schedule_to_cap_floors`].
///
/// # Arguments
///
/// * `discount` - Discounting curve; its base date is the valuation date and
///   times are ACT/365F years from that date.
/// * `forward` - Curve projecting the caplet forwards, normalized at the
///   discount curve's base date. `None` projects on `discount`.
/// * `quotes` - Normal-vol cap or floor quotes with distinct maturities in
///   years; each maturity adds one constant-sigma interval.
/// * `config` - Fixed mean reversion, sigma search bracket (absolute rate
///   volatility), fit tolerance and caplet payment frequency.
///
/// # Errors
///
/// Returns a validation error when no quotes are supplied, maturities repeat,
/// the configuration bounds are invalid or a curve is unusable, and a
/// calibration error when an interval's sigma cannot be bracketed or solved.
pub fn bootstrap_hull_white_sigma_schedule_to_cap_floors_from_curves(
    discount: &DiscountCurve,
    forward: Option<&DiscountCurve>,
    quotes: &[CapFloorQuote],
    config: PiecewiseSigmaCalibrationConfig,
) -> Result<(HullWhiteParams, CalibrationReport)> {
    with_model_curves(discount, forward, |discount_df, forward_df| {
        bootstrap_hull_white_sigma_schedule_to_cap_floors(discount_df, forward_df, quotes, config)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::dates::{Date, DayCount};
    use time::Month;

    fn flat_curve(id: &str, rate: f64) -> DiscountCurve {
        let base = Date::from_calendar_date(2026, Month::January, 2).expect("date");
        let knots: Vec<(f64, f64)> = [0.0, 0.5, 1.0, 2.0, 5.0, 10.0, 20.0, 30.0]
            .iter()
            .map(|&t: &f64| (t, (-rate * t).exp()))
            .collect();
        DiscountCurve::builder(id)
            .base_date(base)
            .day_count(DayCount::Act365F)
            .knots(knots)
            .build()
            .expect("curve")
    }

    fn cap_quotes() -> Vec<CapFloorQuote> {
        [(1.0, 0.0070), (2.0, 0.0075), (5.0, 0.0080)]
            .iter()
            .map(|&(maturity, vol)| {
                CapFloorQuote::try_new(maturity, 0.03, vol, true, true).expect("quote")
            })
            .collect()
    }

    #[test]
    fn curve_form_matches_the_closure_form() {
        let curve = flat_curve("USD-OIS", 0.03);
        let config = PiecewiseSigmaCalibrationConfig {
            fit_tolerance: 1e-6,
            fixed_kappa: 0.03,
            sigma_min: 1e-5,
            sigma_max: 0.1,
            frequency: SwapFrequency::SemiAnnual,
        };
        let (from_curve, _) = bootstrap_hull_white_sigma_schedule_to_cap_floors_from_curves(
            &curve,
            None,
            &cap_quotes(),
            config,
        )
        .expect("curve form");
        let model = ModelDiscountCurve::new(&curve, curve.base_date()).expect("model curve");
        let df = |t: f64| model.get_df(t).unwrap_or(f64::NAN);
        let (from_closure, _) =
            bootstrap_hull_white_sigma_schedule_to_cap_floors(&df, &df, &cap_quotes(), config)
                .expect("closure form");
        assert_eq!(from_curve, from_closure);
    }

    #[test]
    fn omitted_forward_projects_on_the_discount_curve() {
        let curve = flat_curve("USD-OIS", 0.03);
        let config = CapFloorCalibrationConfig {
            fit_tolerance: 1e-4,
            frequency: SwapFrequency::SemiAnnual,
            fixed_kappa: Some(0.03),
            initial_guess: None,
        };
        let quote = [cap_quotes()[1]];
        let (single, _) =
            calibrate_hull_white_to_cap_floors_from_curves(&curve, None, &quote, config)
                .expect("single curve");
        let (explicit, _) =
            calibrate_hull_white_to_cap_floors_from_curves(&curve, Some(&curve), &quote, config)
                .expect("explicit forward");
        assert_eq!(single, explicit);
    }

    #[test]
    fn swaption_curve_form_requires_two_quotes() {
        let curve = flat_curve("USD-OIS", 0.03);
        let quote = [SwaptionQuote::try_new(1.0, 5.0, 0.0065, true).expect("quote")];
        let error = calibrate_hull_white_to_swaptions_from_curve(
            &curve,
            &quote,
            SwapFrequency::SemiAnnual,
            None,
            1e-4,
        )
        .expect_err("one quote cannot fit two parameters");
        assert!(error.to_string().contains("at least 2 swaption quotes"));
    }

    #[test]
    fn swap_frequency_parses_its_wire_labels() {
        for frequency in [
            SwapFrequency::Annual,
            SwapFrequency::SemiAnnual,
            SwapFrequency::Quarterly,
        ] {
            assert_eq!(
                frequency.to_string().parse::<SwapFrequency>().ok(),
                Some(frequency)
            );
        }
        assert!("monthly".parse::<SwapFrequency>().is_err());
    }

    #[test]
    fn configs_deserialize_with_the_default_frequency() {
        let scalar: CapFloorCalibrationConfig =
            serde_json::from_str(r#"{"fit_tolerance": 1e-4}"#).expect("scalar config");
        assert_eq!(scalar.frequency, SwapFrequency::SemiAnnual);
        assert!(scalar.fixed_kappa.is_none() && scalar.initial_guess.is_none());
        let piecewise: PiecewiseSigmaCalibrationConfig = serde_json::from_str(
            r#"{"fit_tolerance": 1e-4, "fixed_kappa": 0.03, "sigma_min": 1e-4, "sigma_max": 0.1}"#,
        )
        .expect("piecewise config");
        assert_eq!(piecewise.frequency, SwapFrequency::SemiAnnual);
        assert!(serde_json::from_str::<CapFloorCalibrationConfig>(
            r#"{"fit_tolerance": 1e-4, "kappa": 0.03}"#
        )
        .is_err());
    }
}
