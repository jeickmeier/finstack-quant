//! Calibration target construction and shared input validation.
//!
use crate::api::schema::SurfaceExtrapolationPolicy;
use crate::api::schema::VolSurfaceParams;
use crate::config::CalibrationConfig;
use crate::quotes::market_quote::MarketQuote;
use crate::quotes::vol::VolQuote;
use crate::targets::util::{
    interpolate_total_variance, resolve_equity_forward_inputs, surface_quote_residuals,
};
use crate::validation::ValidationConfig;
use crate::CalibrationReport;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::MarketScalar;
use finstack_quant_core::market_data::surfaces::VolSurface;
use finstack_quant_core::Result;
use finstack_quant_models::{SabrCalibrator, SabrModel, SabrParameters, SabrShift};
use std::collections::BTreeMap;

use crate::constants::OrderedF64;
use crate::validation::surfaces::{
    validate_butterfly_call_convexity, validate_calendar_spread_with_forwards,
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

        // Equity surfaces store Black vols; β≈0 produces normal vols.
        const EQUITY_BETA_MIN: f64 = 1e-4;
        if params.beta < EQUITY_BETA_MIN {
            return Err(finstack_quant_core::Error::Validation(format!(
                "Equity vol surface calibration requires beta >= {EQUITY_BETA_MIN} \
                 (Black/lognormal surface); beta={} would produce normal (Bachelier) \
                 vols mislabeled as lognormal",
                params.beta
            )));
        }

        // Use only option quotes for the requested underlying.
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
        let time_day_count = finstack_quant_core::dates::DayCount::Act365F;

        for q in &vol_quotes {
            if let VolQuote::OptionVol { expiry, .. } = q {
                let t = time_day_count.year_fraction(
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

        let disc_id = params
            .discount_curve_id
            .clone()
            .ok_or(finstack_quant_core::Error::Input(
                finstack_quant_core::InputError::Invalid, // Should specify discount curve
            ))?;
        let discount = context.get_discount(&disc_id)?;

        let forward_inputs = resolve_equity_forward_inputs(
            &params.underlying_ticker,
            params.base_date,
            spot,
            params.dividend_yield_override,
            discount.as_ref(),
            context,
        )?;
        let forward_fn = |t: f64| forward_inputs.forward(discount.as_ref(), t);

        let mut sabr_params_by_expiry: BTreeMap<OrderedF64, SabrParameters> = BTreeMap::new();
        let mut sabr_winning_starts = Vec::new();
        let mut sabr_winning_iterations = Vec::new();
        let mut sabr_residual_evaluations = Vec::new();
        let mut sabr_bound_hits = Vec::new();
        let mut total_iterations = 0;

        for (t_key, expiry_quotes) in &quotes_by_expiry {
            let t = t_key.into_inner();
            let f = forward_fn(t)?;

            let mut strikes = Vec::new();
            let mut vols = Vec::new();

            for q in expiry_quotes {
                if let VolQuote::OptionVol { strike, vol, .. } = q {
                    strikes.push(*strike);
                    vols.push(*vol);
                }
            }

            if strikes.len() < 3 {
                return Err(finstack_quant_core::Error::Calibration {
                    message: format!(
                        "SABR calibration failed at t={t:.6}: need at least 3 strikes, got {}",
                        strikes.len()
                    ),
                    category: "vol_surface".to_string(),
                });
            }

            let min_quote = vols.iter().copied().fold(f64::INFINITY, f64::min);
            let sabr_calibrator = SabrCalibrator::new()
                .with_tolerance(config.vol_surface.validation_tolerance / min_quote)
                .with_max_iterations(config.solver.max_iterations());
            let outcome = sabr_calibrator
                .with_shift(SabrShift::Auto)
                .calibrate_with_diagnostics(f, &strikes, &vols, t, params.beta)
                .map_err(|e| finstack_quant_core::Error::Calibration {
                    message: format!("SABR calibration failed at t={t:.6}: {e}"),
                    category: "vol_surface".to_string(),
                })?;
            total_iterations += outcome.total_iterations;
            sabr_winning_starts.push(format!(
                "T={t:.6}:alpha={:.8},nu={:.8},rho={:.8}",
                outcome.winning_start[0], outcome.winning_start[1], outcome.winning_start[2]
            ));
            sabr_winning_iterations.push(format!("T={t:.6}:{}", outcome.winning_iterations));
            sabr_residual_evaluations.push(format!("T={t:.6}:{}", outcome.residual_evaluations));
            if !outcome.parameters_at_bounds.is_empty() {
                sabr_bound_hits.push(format!(
                    "T={t:.6}:{}",
                    outcome.parameters_at_bounds.join("|")
                ));
            }
            let p = outcome.parameters;
            sabr_params_by_expiry.insert(*t_key, p);
        }

        if sabr_params_by_expiry.is_empty() {
            return Err(finstack_quant_core::Error::Calibration {
                message: "SABR calibration failed: no quoted expiries with t > 0".to_string(),
                category: "vol_surface".to_string(),
            });
        }

        let mut grid = Vec::new();

        for &t in &params.target_expiries {
            for &k in &params.target_strikes {
                let v = Self::interpolate_total_variance_vol(
                    t,
                    k,
                    &forward_fn,
                    &sabr_params_by_expiry,
                    params.expiry_extrapolation,
                )?;
                grid.push(v);
            }
        }

        let surface = VolSurface::from_grid(
            &params.vol_surface_id,
            &params.target_expiries,
            &params.target_strikes,
            &grid,
        )?;

        let residuals = surface_quote_residuals(
            &surface,
            quotes,
            &params.underlying_ticker,
            params.base_date,
        )?;

        // Forward-aware arbitrage violations fail the calibration.
        let validation_cfg = ValidationConfig {
            lenient_arbitrage: false,
            ..ValidationConfig::default()
        };
        let target_forwards: Vec<f64> = params
            .target_expiries
            .iter()
            .map(|&t| forward_fn(t))
            .collect::<Result<Vec<_>>>()?;
        let calendar_warning =
            validate_calendar_spread_with_forwards(&surface, &validation_cfg, &target_forwards)
                .err()
                .map(|e| format!("SABR calendar-spread arbitrage: {e}"));
        let butterfly_warning =
            validate_butterfly_call_convexity(&surface, &validation_cfg, &target_forwards)
                .err()
                .map(|e| format!("SABR butterfly-spread arbitrage: {e}"));

        let calibrated_expiries: Vec<String> = sabr_params_by_expiry
            .keys()
            .map(|k| format!("{:.6}", k.into_inner()))
            .collect();

        let mut report = CalibrationReport::for_type_with_tolerance(
            "vol_surface",
            residuals,
            total_iterations,
            config.vol_surface.validation_tolerance,
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
        if !calibrated_expiries.is_empty() {
            report.update_metadata("calibrated_expiries", calibrated_expiries.join(","));
        }

        report.update_solver_config(config.solver.clone());
        report.update_metadata("sabr_winning_starts", sabr_winning_starts.join(";"));
        report.update_metadata("sabr_winning_iterations", sabr_winning_iterations.join(";"));
        report.update_metadata(
            "sabr_residual_evaluations",
            sabr_residual_evaluations.join(";"),
        );
        report.update_metadata("sabr_parameters_at_bounds", sabr_bound_hits.join(";"));

        let warnings: Vec<String> = [calendar_warning, butterfly_warning]
            .into_iter()
            .flatten()
            .collect();
        if !warnings.is_empty() {
            let detail = warnings.join("; ");
            report = report.with_validation_result(false, Some(detail));
        }

        Ok((surface, report))
    }

    /// Fill one grid cell by interpolating total variance `w = σ²T` in expiry.
    ///
    /// Each neighbouring SABR slice is evaluated at the **absolute** target
    /// strike with that slice's own forward and expiry. Linear interpolation
    /// of `w` in `T` is calendar-safe whenever the calibrated slices themselves
    /// are calendar-monotone. Extrapolation holds the nearest slice's `w` flat
    /// (`Clamp`) or rejects the target (`Error`).
    fn interpolate_total_variance_vol(
        target_expiry: f64,
        target_strike: f64,
        forward_fn: &impl Fn(f64) -> Result<f64>,
        params: &BTreeMap<OrderedF64, SabrParameters>,
        extrapolation: SurfaceExtrapolationPolicy,
    ) -> Result<f64> {
        let slice_total_variance = |slice_expiry: f64,
                                    slice_params: &SabrParameters|
         -> Result<f64> {
            let forward = forward_fn(slice_expiry)?;
            let sigma = SabrModel::new(slice_params.clone())
                .implied_volatility(forward, target_strike, slice_expiry)
                .map_err(|e| finstack_quant_core::Error::Calibration {
                    message: format!(
                        "Failed to compute SABR implied vol at t={slice_expiry:.6}, k={target_strike:.6}: {e}"
                    ),
                    category: "vol_surface".to_string(),
                })?;
            let w = sigma * sigma * slice_expiry;
            if !w.is_finite() || w < 0.0 {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "SABR negative total variance at K={target_strike:.4}, T={slice_expiry:.6}: w={w:.6}"
                )));
            }
            Ok(w)
        };

        interpolate_total_variance(
            "SABR",
            target_expiry,
            target_strike,
            params,
            slice_total_variance,
            extrapolation,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::api::schema::VolSurfaceModel;
    use crate::quotes::ids::QuoteId;
    use finstack_quant_core::dates::{Date, DateExt};
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_models::SabrParameters;
    use finstack_quant_valuations::instruments::OptionType;
    use time::Month;

    fn params(alpha: f64, beta: f64, nu: f64, rho: f64, shift: f64) -> SabrParameters {
        SabrParameters {
            alpha,
            beta,
            nu,
            rho,
            shift: Some(shift),
        }
    }

    fn slice_vol(p: &SabrParameters, forward: f64, strike: f64, expiry: f64) -> f64 {
        SabrModel::new(p.clone())
            .implied_volatility(forward, strike, expiry)
            .expect("SABR vol")
    }

    fn slice_total_variance(p: &SabrParameters, forward: f64, strike: f64, expiry: f64) -> f64 {
        let sigma = slice_vol(p, forward, strike, expiry);
        sigma * sigma * expiry
    }

    #[test]
    fn interpolate_total_variance_out_of_bounds_errors_by_default() {
        let mut map = BTreeMap::new();
        map.insert(OrderedF64(1.0), params(0.10, 0.5, 0.30, -0.20, 0.01));
        map.insert(OrderedF64(2.0), params(0.20, 0.5, 0.40, -0.10, 0.01));
        let forward_fn = |_t: f64| Ok(100.0);

        let err = VolSurfaceTarget::interpolate_total_variance_vol(
            0.5,
            100.0,
            &forward_fn,
            &map,
            SurfaceExtrapolationPolicy::Error,
        )
        .expect_err("out-of-bounds should error");
        assert!(err.to_string().contains("out of bounds"));
    }

    #[test]
    fn interpolate_total_variance_out_of_bounds_clamps_when_configured() {
        let p1 = params(0.10, 0.5, 0.30, -0.20, 0.01);
        let p2 = params(0.20, 0.5, 0.40, -0.10, 0.01);
        let mut map = BTreeMap::new();
        map.insert(OrderedF64(1.0), p1.clone());
        map.insert(OrderedF64(2.0), p2.clone());
        let forward_fn = |_t: f64| Ok(100.0);
        let strike = 100.0;

        let left = VolSurfaceTarget::interpolate_total_variance_vol(
            0.5,
            strike,
            &forward_fn,
            &map,
            SurfaceExtrapolationPolicy::Clamp,
        )
        .expect("clamp-left");
        let w1 = slice_total_variance(&p1, 100.0, strike, 1.0);
        assert!(
            (left - (w1 / 0.5).sqrt()).abs() < 1e-12,
            "left clamp should hold the front-slice total variance"
        );

        let right = VolSurfaceTarget::interpolate_total_variance_vol(
            3.0,
            strike,
            &forward_fn,
            &map,
            SurfaceExtrapolationPolicy::Clamp,
        )
        .expect("clamp-right");
        let w2 = slice_total_variance(&p2, 100.0, strike, 2.0);
        assert!(
            (right - (w2 / 3.0).sqrt()).abs() < 1e-12,
            "right clamp should hold the back-slice total variance"
        );
    }

    #[test]
    fn interpolate_total_variance_matches_calibrated_knots() {
        let p1 = params(0.10, 0.5, 0.30, -0.20, 0.01);
        let p2 = params(0.20, 0.5, 0.50, 0.10, 0.01);
        let mut map = BTreeMap::new();
        map.insert(OrderedF64(1.0), p1.clone());
        map.insert(OrderedF64(2.0), p2.clone());
        let forward_value = |t: f64| 100.0 * ((0.01 + 0.09 * t) * t).exp();
        let forward_fn = |t: f64| Ok(forward_value(t));
        let strike = 95.0;

        let at_t1 = VolSurfaceTarget::interpolate_total_variance_vol(
            1.0,
            strike,
            &forward_fn,
            &map,
            SurfaceExtrapolationPolicy::Error,
        )
        .expect("knot t=1");
        let expected_t1 = slice_vol(&p1, forward_value(1.0), strike, 1.0);
        assert!(
            (at_t1 - expected_t1).abs() < 1e-12,
            "exact match at t=1: got {at_t1}, expected {expected_t1}"
        );

        let at_t2 = VolSurfaceTarget::interpolate_total_variance_vol(
            2.0,
            strike,
            &forward_fn,
            &map,
            SurfaceExtrapolationPolicy::Error,
        )
        .expect("knot t=2");
        let expected_t2 = slice_vol(&p2, forward_value(2.0), strike, 2.0);
        assert!(
            (at_t2 - expected_t2).abs() < 1e-12,
            "exact match at t=2: got {at_t2}, expected {expected_t2}"
        );
    }

    /// Linear interpolation of `w = σ²T` at a fixed absolute strike must stay
    /// calendar-monotone when the calibrated slices themselves are, even if
    /// the forward is non-flat (`F(T₁) ≠ F(T₂)`).
    #[test]
    fn interpolate_total_variance_preserves_calendar_monotonicity() {
        let t1 = 0.5_f64;
        let t2 = 1.5_f64;
        let t3 = 3.0_f64;
        // β = 1 keeps ATM vol ≈ α (independent of F), so a non-flat forward
        // cannot hide a calendar violation behind CEV backbone drift.
        let p1 = params(0.25, 1.0, 0.20, -0.20, 0.01);
        let p2 = params(0.22, 1.0, 0.18, -0.15, 0.01);
        let p3 = params(0.20, 1.0, 0.16, -0.10, 0.01);
        let mut map = BTreeMap::new();
        map.insert(OrderedF64(t1), p1.clone());
        map.insert(OrderedF64(t2), p2.clone());
        map.insert(OrderedF64(t3), p3.clone());

        let spot = 100.0_f64;
        let forward_value = |t: f64| spot * ((0.01 + 0.09 * t) * t).exp();
        let forward_fn = |t: f64| Ok(forward_value(t));
        assert!(
            (forward_value(t1) - forward_value(t2)).abs() > 1.0,
            "test fixture must use a non-flat forward curve"
        );

        let strikes = [80.0_f64, 95.0, 103.0, 115.0, 135.0];
        for &strike in &strikes {
            let w1 = slice_total_variance(&p1, forward_value(t1), strike, t1);
            let w2 = slice_total_variance(&p2, forward_value(t2), strike, t2);
            let w3 = slice_total_variance(&p3, forward_value(t3), strike, t3);
            assert!(
                w1 <= w2 + 1e-12 && w2 <= w3 + 1e-12,
                "fixture slices must themselves be calendar-monotone at K={strike}: \
                 w({t1})={w1}, w({t2})={w2}, w({t3})={w3}"
            );
        }

        let n_steps = 50;
        for &strike in &strikes {
            let mut prev_w = f64::NEG_INFINITY;
            let mut prev_t = t1;
            for i in 0..=n_steps {
                let t = t1 + (t3 - t1) * (i as f64) / (n_steps as f64);
                let vol = VolSurfaceTarget::interpolate_total_variance_vol(
                    t,
                    strike,
                    &forward_fn,
                    &map,
                    SurfaceExtrapolationPolicy::Error,
                )
                .expect("interpolation ok");
                let w = vol * vol * t;
                assert!(
                    w >= prev_w - 1e-9,
                    "calendar-spread arbitrage in interpolate_total_variance_vol at \
                     strike={strike:.1}: w(T={prev_t:.4})={prev_w:.6} > \
                     w(T={t:.4})={w:.6}"
                );
                prev_w = w;
                prev_t = t;
            }
        }
    }

    fn date(year: i32, month: Month, day: u8) -> Date {
        Date::from_calendar_date(year, month, day).expect("valid date")
    }

    #[test]
    fn vol_surface_params_reject_noncanonical_model() {
        let base_date = date(2025, Month::January, 2);
        let params = VolSurfaceParams {
            vol_surface_id: "SPX-VOL".to_string(),
            base_date,
            underlying_ticker: "SPX".to_string(),
            model: VolSurfaceModel::Sabr,
            discount_curve_id: None,
            beta: 0.5,
            target_expiries: vec![0.5],
            target_strikes: vec![90.0, 100.0, 110.0],
            spot_override: Some(100.0),
            dividend_yield_override: Some(0.0),
            expiry_extrapolation: SurfaceExtrapolationPolicy::Clamp,
        };

        let mut json = serde_json::to_value(params).expect("serialize params");
        json["model"] = serde_json::Value::String("SABR".to_string());
        assert!(serde_json::from_value::<VolSurfaceParams>(json).is_err());
    }

    #[test]
    fn vol_surface_fails_when_any_expiry_cannot_calibrate() {
        let base_date = date(2025, Month::January, 2);
        let disc = DiscountCurve::builder("USD-OIS")
            .base_date(base_date)
            .day_count(finstack_quant_core::dates::DayCount::Act365F)
            .knots([(0.0, 1.0), (10.0, 0.80)])
            .build()
            .expect("discount curve");
        let ctx = MarketContext::new().insert(disc);

        let params = VolSurfaceParams {
            vol_surface_id: "SPX-VOL".to_string(),
            base_date,
            underlying_ticker: "SPX".to_string(),
            model: VolSurfaceModel::Sabr,
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
            }),
            MarketQuote::Vol(VolQuote::OptionVol {
                id: QuoteId::new("SPX-VOL-1Y-100"),
                underlying: "SPX".to_string().into(),
                expiry: expiry_1y,
                strike: 100.0,
                vol: 0.19,
                option_type: OptionType::Call,
            }),
            MarketQuote::Vol(VolQuote::OptionVol {
                id: QuoteId::new("SPX-VOL-1Y-110"),
                underlying: "SPX".to_string().into(),
                expiry: expiry_1y,
                strike: 110.0,

                vol: 0.18,
                option_type: OptionType::Call,
            }),
            MarketQuote::Vol(VolQuote::OptionVol {
                id: QuoteId::new("SPX-VOL-2Y-0"),
                underlying: "SPX".to_string().into(),
                expiry: expiry_2y,
                strike: 0.0,
                vol: 0.20,
                option_type: OptionType::Call,
            }),
            MarketQuote::Vol(VolQuote::OptionVol {
                id: QuoteId::new("SPX-VOL-2Y-100"),
                underlying: "SPX".to_string().into(),
                expiry: expiry_2y,
                strike: 100.0,
                vol: 0.19,
                option_type: OptionType::Call,
            }),
            MarketQuote::Vol(VolQuote::OptionVol {
                id: QuoteId::new("SPX-VOL-2Y-110"),
                underlying: "SPX".to_string().into(),
                expiry: expiry_2y,
                strike: 110.0,
                vol: 0.18,
                option_type: OptionType::Call,
            }),
        ];

        let config = CalibrationConfig::default();
        let err = VolSurfaceTarget::solve(&params, &quotes, &ctx, &config)
            .expect_err("any failed expiry must abort the surface");
        let msg = err.to_string();
        assert!(
            msg.contains("SABR") || msg.contains("strike") || msg.contains("t="),
            "error should identify the failed expiry: {msg}"
        );
    }
}
