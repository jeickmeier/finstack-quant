use crate::calibration::api::schema::SurfaceExtrapolationPolicy;
use crate::calibration::api::schema::VolSurfaceParams;
use crate::calibration::config::CalibrationConfig;
use crate::calibration::validation::ValidationConfig;
use crate::calibration::CalibrationReport;
use crate::market::quotes::market_quote::MarketQuote;
use crate::market::quotes::vol::VolQuote;
use crate::models::{SABRCalibrator, SABRModel, SABRParameters};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::MarketScalar;
use finstack_quant_core::market_data::surfaces::VolSurface;
use finstack_quant_core::Result;
use std::collections::BTreeMap;

use crate::calibration::constants::OrderedF64;
use crate::calibration::validation::surfaces::{
    validate_butterfly_spread, validate_calendar_spread,
};

/// Bootstrapper for calibrating option volatility surfaces.
///
/// Calibrates volatility surfaces from option quotes using the SABR model.
/// Groups quotes by expiry, calibrates SABR parameters per expiry, and builds
/// a volatility surface grid.
pub(crate) struct VolSurfaceTarget;

impl VolSurfaceTarget {
    /// Calibrates an option volatility surface from market quotes.
    ///
    /// Groups option quotes by expiry, calibrates SABR parameters for each
    /// expiry, and constructs a volatility surface grid.
    ///
    /// # Arguments
    ///
    /// * `params` - Parameters defining the volatility surface structure
    /// * `quotes` - Market quotes containing option volatility quotes
    /// * `context` - Market context containing spot prices, discount curves, and dividend yields
    /// * `config` - Calibration configuration settings
    ///
    /// # Returns
    ///
    /// A tuple containing the calibrated volatility surface and calibration report.
    ///
    /// # Errors
    ///
    /// Returns an error if insufficient quotes are provided or calibration fails.
    /// Returns an error if insufficient quotes are provided or calibration fails.
    pub(crate) fn solve(
        params: &VolSurfaceParams,
        quotes: &[MarketQuote],
        context: &MarketContext,
        config: &CalibrationConfig,
    ) -> Result<(VolSurface, CalibrationReport)> {
        if params.target_expiries.is_empty() {
            return Err(finstack_quant_core::Error::Validation(
                "VolSurfaceParams.target_expiries must not be empty".to_string(),
            ));
        }
        if params.target_strikes.len() < 3 {
            return Err(finstack_quant_core::Error::Validation(
                "VolSurfaceParams.target_strikes must contain at least three points".to_string(),
            ));
        }

        let model = params.model.trim().to_ascii_lowercase();
        if model != "sabr" {
            return Err(finstack_quant_core::Error::Validation(format!(
                "VolSurface model '{}' is not supported (currently supported: 'sabr')",
                params.model
            )));
        }

        // Filter quotes. Only ingest `OptionVol` quotes whose underlying
        // matches `params.underlying_ticker` — without this filter the SABR
        // surface would pool quotes from unrelated underlyings into the same
        // expiry bucket (mirrors the SVI target's ticker filter).
        let vol_quotes: Vec<&VolQuote> = quotes
            .iter()
            .filter_map(|q| match q {
                MarketQuote::Vol(vq) => Some(vq),
                _ => None,
            })
            .filter(|vq| match vq {
                VolQuote::OptionVol { underlying, .. } => {
                    underlying.as_str() == params.underlying_ticker.as_str()
                }
                VolQuote::SwaptionVol { .. } | VolQuote::CapFloorVol { .. } => false,
            })
            .collect();

        if vol_quotes.is_empty() {
            return Err(finstack_quant_core::Error::Input(
                finstack_quant_core::InputError::TooFewPoints,
            ));
        }

        // Group by expiry (year fraction)
        let mut quotes_by_expiry: BTreeMap<OrderedF64, Vec<&VolQuote>> = BTreeMap::new();
        // We need day count for time conversion. Default to Act365F for vol surfaces if not specified.
        let time_dc = finstack_quant_core::dates::DayCount::Act365F;

        for q in &vol_quotes {
            if let VolQuote::OptionVol { expiry, .. } = q {
                let t = time_dc.year_fraction(
                    params.base_date,
                    *expiry,
                    finstack_quant_core::dates::DayCountContext::default(),
                )?;
                if t > 0.0 {
                    quotes_by_expiry.entry(t.into()).or_default().push(q);
                }
            }
        }

        // Forward function
        // Need spot and dividend yield
        let spot = if let Some(s) = params.spot_override {
            s
        } else {
            let scalar = context.get_price(&params.underlying_ticker).map_err(|_| {
                finstack_quant_core::Error::Input(finstack_quant_core::InputError::NotFound {
                    id: params.underlying_ticker.clone(),
                })
            })?;
            match scalar {
                MarketScalar::Price(m) => m.amount(),
                MarketScalar::Unitless(v) => *v,
            }
        };

        // Resolve discount curve
        let disc_id = params
            .discount_curve_id
            .clone()
            .ok_or(finstack_quant_core::Error::Input(
                finstack_quant_core::InputError::Invalid, // Should specify discount curve
            ))?;
        let discount = context.get_discount(&disc_id)?;

        // Dividend yield
        let div_yield = if let Some(d) = params.dividend_yield_override {
            d
        } else {
            // Try looking up underlying-DIVYIELD
            let key = format!("{}-DIVYIELD", params.underlying_ticker);
            if let Ok(MarketScalar::Unitless(v)) = context.get_price(&key) {
                *v
            } else {
                0.0
            }
        };

        let forward_fn = |t: f64| -> f64 {
            let r = discount.zero(t); // Continuously compounded zero rate
            spot * ((r - div_yield) * t).exp()
        };

        // Calibrate SABR per expiry.
        //
        // Note: `config.solver` is the Brent *root-finding* configuration for
        // the 1D bootstrap (price-space tolerance, default 1e-12). That is not
        // a meaningful tolerance for the vega-weighted SSE objective minimized
        // by the SABR Levenberg-Marquardt calibrator: reusing it previously
        // "worked" only because the LM solver silently returned its best
        // iterate on MaxIterations.
        // `core::math::solver_multi::minimize` errors loudly on
        // non-convergence, so the SABR fit uses the calibrator's attainable
        // production defaults (tolerance 1e-4 on the SSE, 2000 iterations).
        let sabr_calibrator = SABRCalibrator::new();

        let mut sabr_params_by_expiry: BTreeMap<OrderedF64, SABRParameters> = BTreeMap::new();
        let mut residuals = BTreeMap::new();
        let mut expiry_errors: BTreeMap<OrderedF64, String> = BTreeMap::new();
        let mut total_iterations = 0;

        for (t_key, expiry_quotes) in &quotes_by_expiry {
            let t = t_key.into_inner();
            let f = forward_fn(t);

            let mut strikes = Vec::new();
            let mut vols = Vec::new();

            for q in expiry_quotes {
                if let VolQuote::OptionVol { strike, vol, .. } = q {
                    strikes.push(*strike);
                    vols.push(*vol);
                }
            }

            if strikes.len() < 3 {
                expiry_errors.insert(
                    *t_key,
                    format!(
                        "Need at least 3 strikes to calibrate SABR; got {}",
                        strikes.len()
                    ),
                );
                continue;
            }

            match sabr_calibrator.calibrate_auto_shift(f, &strikes, &vols, t, params.beta) {
                Ok(p) => {
                    // Residuals
                    let model = SABRModel::new(p.clone());
                    let mut bucket_residuals: Vec<(String, f64)> =
                        Vec::with_capacity(strikes.len());
                    let mut bucket_error: Option<String> = None;
                    for (i, k) in strikes.iter().enumerate() {
                        match model.implied_volatility(f, *k, t) {
                            Ok(model_vol) => {
                                let res = (model_vol - vols[i]).abs();
                                bucket_residuals
                                    .push((format!("opt_vol_t{:.2}_k{:.2}_i{}", t, k, i), res));
                            }
                            Err(e) => {
                                bucket_error = Some(format!(
                                    "SABR implied vol failed at t={:.6}, strike={:.6}: {}",
                                    t, k, e
                                ));
                                break;
                            }
                        }
                    }
                    if let Some(err) = bucket_error {
                        expiry_errors.insert(*t_key, err);
                        continue;
                    }

                    sabr_params_by_expiry.insert(*t_key, p);
                    for (k, v) in bucket_residuals {
                        residuals.insert(k, v);
                    }
                    total_iterations += 1;
                }
                Err(e) => {
                    expiry_errors.insert(*t_key, e.to_string());
                }
            }
        }

        // Build grid
        // Use params.target_expiries and target_strikes
        let mut grid = Vec::new();

        for &t in &params.target_expiries {
            let f = forward_fn(t);
            // Interpolate SABR params for t
            // For brevity, using nearest valid calibration or linear if possible.
            // Let's implement simple linear interpolation of params.

            let p =
                Self::interpolate_params(t, &sabr_params_by_expiry, params.expiry_extrapolation)?;
            let model = SABRModel::new(p);

            for &k in &params.target_strikes {
                let v = model.implied_volatility(f, k, t).map_err(|e| {
                    finstack_quant_core::Error::Calibration {
                        message: format!(
                            "Failed to compute SABR implied vol at t={:.6}, k={:.6}: {}",
                            t, k, e
                        ),
                        category: "vol_surface".to_string(),
                    }
                })?;
                grid.push(v);
            }
        }

        let surface = VolSurface::from_grid(
            &params.surface_id,
            &params.target_expiries,
            &params.target_strikes,
            &grid,
        )?;

        // Sanity-check the produced grid for calendar-spread and butterfly
        // arbitrage. Mirrors the SVI surface target: the validators operate
        // on a generic vol grid so they are reusable for SABR-calibrated
        // surfaces. `lenient_arbitrage = true` keeps a borderline surface
        // from hard-failing the calibration pipeline; any violation is
        // surfaced as a diagnostic on the report so strict-mode callers can
        // promote it to an error.
        let validation_cfg = ValidationConfig {
            lenient_arbitrage: true,
            ..ValidationConfig::default()
        };
        let calendar_warning = validate_calendar_spread(&surface, &validation_cfg)
            .err()
            .map(|e| format!("SABR calendar-spread arbitrage: {e}"));
        let butterfly_warning = validate_butterfly_spread(&surface, &validation_cfg)
            .err()
            .map(|e| format!("SABR butterfly-spread arbitrage: {e}"));

        let calibrated_expiries: Vec<String> = sabr_params_by_expiry
            .keys()
            .map(|k| format!("{:.6}", k.into_inner()))
            .collect();
        let failed_examples: Vec<String> = expiry_errors
            .iter()
            .take(5)
            .map(|(t, e)| format!("t={:.6}: {}", t.into_inner(), e))
            .collect();

        let mut report = CalibrationReport::for_type_with_tolerance(
            "vol_surface",
            residuals,
            total_iterations,
            config.solver.tolerance(),
        );
        report.update_metadata(
            "expiry_extrapolation_policy",
            match params.expiry_extrapolation {
                SurfaceExtrapolationPolicy::Error => "error",
                SurfaceExtrapolationPolicy::Clamp => "clamp",
            },
        );
        report.update_metadata(
            "calibrated_expiry_count",
            sabr_params_by_expiry.len().to_string(),
        );
        report.update_metadata("failed_expiry_count", expiry_errors.len().to_string());
        if !calibrated_expiries.is_empty() {
            report.update_metadata("calibrated_expiries", calibrated_expiries.join(","));
        }
        if !failed_examples.is_empty() {
            report.update_metadata("failed_expiry_examples", failed_examples.join(" | "));
        }

        report.update_solver_config(config.solver.clone());

        // Surface arbitrage warnings on the report so callers can detect them.
        let warnings: Vec<String> = [calendar_warning, butterfly_warning]
            .into_iter()
            .flatten()
            .collect();
        if !warnings.is_empty() {
            report.validation_passed = false;
            report.validation_error = Some(warnings.join("; "));
        }

        Ok((surface, report))
    }

    /// Interpolate SABR parameters across the 1D expiry axis.
    #[allow(clippy::expect_used)] // Infallible after is_empty check
    fn interpolate_params(
        t: f64,
        params: &BTreeMap<OrderedF64, SABRParameters>,
        extrapolation: SurfaceExtrapolationPolicy,
    ) -> Result<SABRParameters> {
        let min_key =
            params
                .keys()
                .next()
                .ok_or_else(|| finstack_quant_core::Error::Calibration {
                    message: "No calibrated SABR parameters".to_string(),
                    category: "vol_surface".to_string(),
                })?;
        let max_key =
            params
                .keys()
                .next_back()
                .ok_or_else(|| finstack_quant_core::Error::Calibration {
                    message: "No calibrated SABR parameters".to_string(),
                    category: "vol_surface".to_string(),
                })?;
        let min_t = min_key.into_inner();
        let max_t = max_key.into_inner();

        if extrapolation == SurfaceExtrapolationPolicy::Error {
            // Require targets to be within the calibrated expiry range.
            if t < min_t || t > max_t {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "Target expiry t={:.6} is out of bounds for calibrated expiries [{:.6}, {:.6}]. \
Set params.expiry_extrapolation='clamp' to allow flat extrapolation.",
                    t, min_t, max_t
                )));
            }
        }

        // Find neighbors
        let mut before = None;
        let mut after = None;

        for (&kt, p) in params {
            let kt_f = kt.into_inner();
            if kt_f <= t {
                before = Some((kt_f, p));
            }
            if kt_f >= t && after.is_none() {
                after = Some((kt_f, p));
            }
        }

        match (before, after) {
            (Some((t1, p1)), Some((t2, p2))) if (t2 - t1).abs() > 1e-12 => {
                let w = (t - t1) / (t2 - t1);
                let w = w.clamp(0.0, 1.0);

                // Log-space interpolation for α and ν preserves positivity and
                // is consistent with the swaption vol target implementation.
                let log_alpha1 = p1.alpha.max(1e-16).ln();
                let log_alpha2 = p2.alpha.max(1e-16).ln();
                let log_nu1 = p1.nu.max(1e-16).ln();
                let log_nu2 = p2.nu.max(1e-16).ln();

                let alpha = (log_alpha1 * (1.0 - w) + log_alpha2 * w).exp();
                let nu = (log_nu1 * (1.0 - w) + log_nu2 * w).exp();
                let rho = (p1.rho * (1.0 - w) + p2.rho * w).clamp(-0.999, 0.999);

                Ok(SABRParameters {
                    alpha,
                    beta: p1.beta,
                    nu,
                    rho,
                    shift: p1.shift,
                })
            }
            (Some((_, p)), _) | (_, Some((_, p))) => Ok(p.clone()),
            _ => unreachable!(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::instruments::OptionType;
    use crate::market::conventions::ids::OptionConventionId;
    use crate::market::quotes::ids::QuoteId;
    use crate::models::SABRParameters;
    use finstack_quant_core::dates::{Date, DateExt};
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use time::Month;

    fn params(alpha: f64, beta: f64, nu: f64, rho: f64, shift: f64) -> SABRParameters {
        SABRParameters {
            alpha,
            beta,
            nu,
            rho,
            shift: Some(shift),
        }
    }

    #[test]
    fn interpolate_params_out_of_bounds_errors_by_default() {
        let mut map = BTreeMap::new();
        map.insert(OrderedF64(1.0), params(0.10, 0.5, 0.30, -0.20, 0.01));
        map.insert(OrderedF64(2.0), params(0.20, 0.5, 0.40, -0.10, 0.01));

        let err =
            VolSurfaceTarget::interpolate_params(0.5, &map, SurfaceExtrapolationPolicy::Error)
                .expect_err("out-of-bounds should error");
        assert!(err.to_string().contains("out of bounds"));
    }

    #[test]
    fn interpolate_params_out_of_bounds_clamps_when_configured() {
        let mut map = BTreeMap::new();
        let p1 = params(0.10, 0.5, 0.30, -0.20, 0.01);
        let p2 = params(0.20, 0.5, 0.40, -0.10, 0.01);
        map.insert(OrderedF64(1.0), p1.clone());
        map.insert(OrderedF64(2.0), p2.clone());

        let left =
            VolSurfaceTarget::interpolate_params(0.5, &map, SurfaceExtrapolationPolicy::Clamp)
                .expect("clamp-left");
        assert_eq!(left.alpha, p1.alpha);

        let right =
            VolSurfaceTarget::interpolate_params(3.0, &map, SurfaceExtrapolationPolicy::Clamp)
                .expect("clamp-right");
        assert_eq!(right.alpha, p2.alpha);
    }

    #[test]
    fn interpolate_params_log_space_interpolates_in_range() {
        let mut map = BTreeMap::new();
        map.insert(OrderedF64(1.0), params(0.10, 0.5, 0.30, -0.20, 0.01));
        map.insert(OrderedF64(2.0), params(0.20, 0.5, 0.50, 0.10, 0.01));

        let mid =
            VolSurfaceTarget::interpolate_params(1.5, &map, SurfaceExtrapolationPolicy::Error)
                .expect("in-range");

        // α and ν use log-space: geometric mean at midpoint
        let expected_alpha = (0.10_f64 * 0.20).sqrt(); // ≈ 0.1414
        let expected_nu = (0.30_f64 * 0.50).sqrt(); // ≈ 0.3873
        assert!(
            (mid.alpha - expected_alpha).abs() < 1e-12,
            "alpha: expected {expected_alpha}, got {}",
            mid.alpha
        );
        assert!(
            (mid.nu - expected_nu).abs() < 1e-12,
            "nu: expected {expected_nu}, got {}",
            mid.nu
        );
        // ρ is still linear
        assert!((mid.rho - (-0.05)).abs() < 1e-12);
    }

    fn date(year: i32, month: Month, day: u8) -> Date {
        Date::from_calendar_date(year, month, day).expect("valid date")
    }

    #[test]
    fn vol_surface_rejects_non_sabr_model() {
        let base_date = date(2025, Month::January, 2);
        let params = VolSurfaceParams {
            surface_id: "SPX-VOL".to_string(),
            base_date,
            underlying_ticker: "SPX".to_string(),
            model: "black".to_string(),
            discount_curve_id: None,
            beta: 0.5,
            target_expiries: vec![0.5],
            target_strikes: vec![90.0, 100.0, 110.0],
            spot_override: Some(100.0),
            dividend_yield_override: Some(0.0),
            expiry_extrapolation: SurfaceExtrapolationPolicy::Clamp,
        };

        let err = VolSurfaceTarget::solve(
            &params,
            &[],
            &MarketContext::new(),
            &CalibrationConfig::default(),
        )
        .expect_err("unsupported model should error");
        assert!(err.to_string().contains("not supported"));
    }

    #[test]
    fn vol_surface_marks_implied_vol_failures_as_failed_expiries() {
        let base_date = date(2025, Month::January, 2);
        let disc = DiscountCurve::builder("USD-OIS")
            .base_date(base_date)
            .day_count(finstack_quant_core::dates::DayCount::Act365F)
            .knots([(0.0, 1.0), (10.0, 0.80)])
            .build()
            .expect("discount curve");
        let ctx = MarketContext::new().insert(disc);

        let params = VolSurfaceParams {
            surface_id: "SPX-VOL".to_string(),
            base_date,
            underlying_ticker: "SPX".to_string(),
            model: "SABR".to_string(),
            discount_curve_id: Some("USD-OIS".into()),
            beta: 0.5,
            target_expiries: vec![1.0, 2.0],
            target_strikes: vec![90.0, 100.0, 110.0],
            spot_override: Some(100.0),
            dividend_yield_override: Some(0.0),
            // Allow building the surface even if an expiry bucket fails.
            expiry_extrapolation: SurfaceExtrapolationPolicy::Clamp,
        };

        let expiry_1y = base_date.add_months(12);
        let expiry_2y = base_date.add_months(24);

        // One valid expiry (all strikes > 0), one invalid expiry (strike=0 triggers SABR error).
        let quotes = vec![
            MarketQuote::Vol(VolQuote::OptionVol {
                id: QuoteId::new("SPX-VOL-1Y-90"),
                underlying: "SPX".to_string().into(),
                expiry: expiry_1y,
                strike: 90.0,
                vol: 0.20,
                option_type: OptionType::Call,

                convention: OptionConventionId::new("USD-Option"),
            }),
            MarketQuote::Vol(VolQuote::OptionVol {
                id: QuoteId::new("SPX-VOL-1Y-100"),
                underlying: "SPX".to_string().into(),
                expiry: expiry_1y,
                strike: 100.0,
                vol: 0.19,
                option_type: OptionType::Call,

                convention: OptionConventionId::new("USD-Option"),
            }),
            MarketQuote::Vol(VolQuote::OptionVol {
                id: QuoteId::new("SPX-VOL-1Y-110"),
                underlying: "SPX".to_string().into(),
                expiry: expiry_1y,
                strike: 110.0,

                vol: 0.18,
                option_type: OptionType::Call,
                convention: OptionConventionId::new("USD-Option"),
            }),
            MarketQuote::Vol(VolQuote::OptionVol {
                id: QuoteId::new("SPX-VOL-2Y-0"),
                underlying: "SPX".to_string().into(),
                expiry: expiry_2y,
                strike: 0.0,
                vol: 0.20,
                option_type: OptionType::Call,

                convention: OptionConventionId::new("USD-Option"),
            }),
            MarketQuote::Vol(VolQuote::OptionVol {
                id: QuoteId::new("SPX-VOL-2Y-100"),
                underlying: "SPX".to_string().into(),
                expiry: expiry_2y,
                strike: 100.0,
                vol: 0.19,
                option_type: OptionType::Call,

                convention: OptionConventionId::new("USD-Option"),
            }),
            MarketQuote::Vol(VolQuote::OptionVol {
                id: QuoteId::new("SPX-VOL-2Y-110"),
                underlying: "SPX".to_string().into(),
                expiry: expiry_2y,
                strike: 110.0,
                vol: 0.18,
                option_type: OptionType::Call,

                convention: OptionConventionId::new("USD-Option"),
            }),
        ];

        // Review 2026-06-09 (core math): SABR calibration now fails loudly on
        // non-convergence. The SABR LM fit uses the calibrator's production
        // defaults (1e-4 SSE tolerance, 2000 iterations) rather than the
        // envelope's Brent root-finding config, so the 1Y bucket calibrates
        // (its vega-weighted SSE stagnates around 2e-5) and only the strike=0
        // expiry fails.
        let config = CalibrationConfig::default();
        let (_surface, report) =
            VolSurfaceTarget::solve(&params, &quotes, &ctx, &config).expect("calibrate");

        assert_eq!(
            report
                .metadata
                .get("failed_expiry_count")
                .map(|s| s.as_str()),
            Some("1")
        );
        assert!(
            report
                .metadata
                .get("failed_expiry_examples")
                .is_some_and(|s| s.contains("strike=0.000000")),
            "missing or unexpected failed_expiry_examples: {:?}",
            report.metadata.get("failed_expiry_examples")
        );
    }
}
