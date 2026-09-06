//! SVI (Stochastic Volatility Inspired) surface calibration target.
//!
//! Calibrates a volatility surface using the SVI parameterization per expiry,
//! then interpolates across expiries to build a full grid surface.
//!
//! # Cross-expiry interpolation
//!
//! Per Gatheral (2004, 2013), linear interpolation of SVI parameters
//! (`a`, `b`, `ρ`, `m`, `σ`) between calibrated expiry slices is *not*
//! guaranteed to preserve the calendar-spread no-arbitrage constraint
//! `∂w(k, T)/∂T ≥ 0`. The arbitrage-safe recipe is to interpolate the
//! **total variance** `w(k, T) = σ²(k, T) · T` directly at each log-
//! moneyness `k` and recover `σ = √(w/T)`. That preserves calendar
//! monotonicity of `w` by linearity whenever the calibrated slices
//! themselves are calendar-monotone.
//!
//! After the grid is built, we hand it to
//! `validate_calendar_spread_with_forwards` (calendar monotonicity at fixed
//! forward log-moneyness) and enforce Durrleman's `g(k) ≥ 0` per calibrated
//! slice (with `validate_butterfly_call_convexity` as a grid-level
//! diagnostic) so any residual arbitrage surfaces as a structured
//! `Error::Validation` rather than propagating silently into pricing.

use crate::api::schema::SurfaceExtrapolationPolicy;
use crate::api::schema::SviSurfaceParams;
use crate::config::CalibrationConfig;
use crate::constants::OrderedF64;
use crate::quotes::market_quote::MarketQuote;
use crate::quotes::vol::VolQuote;
use crate::targets::util::{
    interpolate_total_variance, resolve_equity_forward_inputs, surface_quote_residuals,
};
use crate::validation::ValidationConfig;
use crate::CalibrationReport;
use finstack_quant_core::dates::{DayCount, DayCountContext};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::MarketScalar;
use finstack_quant_core::market_data::surfaces::VolSurface;
use finstack_quant_core::Result;
use std::collections::BTreeMap;

use crate::validation::surfaces::{
    validate_butterfly_call_convexity, validate_calendar_spread_with_forwards,
};

/// Target for SVI surface calibration from option volatility quotes.
pub(crate) struct SviSurfaceTarget;

impl SviSurfaceTarget {
    fn validate_positive_input(label: &str, value: f64) -> Result<()> {
        if !value.is_finite() || value <= 0.0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "{label} must be finite and positive; got {value}"
            )));
        }
        Ok(())
    }

    /// Calibrate an SVI volatility surface from option vol quotes.
    ///
    /// Groups quotes by expiry, calibrates SVI parameters per expiry slice,
    /// interpolates parameters for target expiries, and evaluates onto a grid.
    pub(crate) fn solve(
        params: &SviSurfaceParams,
        quotes: &[MarketQuote],
        context: &MarketContext,
        global_config: &CalibrationConfig,
    ) -> Result<(VolSurface, CalibrationReport)> {
        if params.target_expiries.is_empty() || params.target_strikes.is_empty() {
            return Err(finstack_quant_core::Error::Input(
                finstack_quant_core::InputError::TooFewPoints,
            ));
        }
        let option_quotes: Vec<&VolQuote> = quotes
            .iter()
            .filter_map(|quote| match quote {
                MarketQuote::Vol(vol_quote) => Some(vol_quote),
                _ => None,
            })
            .filter(|quote| match quote {
                VolQuote::OptionVol { underlying, .. } => {
                    underlying.as_str() == params.underlying_ticker.as_str()
                }
                VolQuote::SwaptionVol { .. } | VolQuote::CapFloorVol { .. } => false,
            })
            .collect();

        if option_quotes.is_empty() {
            return Err(finstack_quant_core::Error::Input(
                finstack_quant_core::InputError::TooFewPoints,
            ));
        }

        let spot = if let Some(spot) = params.spot_override {
            spot
        } else {
            match context.get_price(&params.underlying_ticker)? {
                MarketScalar::Price(money) => money.amount(),
                MarketScalar::Unitless(value) => *value,
            }
        };
        Self::validate_positive_input("SVI spot", spot)?;

        let discount = params
            .discount_curve_id
            .as_ref()
            .map(|curve_id| context.get_discount(curve_id))
            .transpose()?;

        let zero_rate_yield = if discount.is_none() {
            let value = if let Some(value) = params.dividend_yield_override {
                value
            } else {
                let key = format!("{}-DIVYIELD", params.underlying_ticker);
                match context.get_price(&key) {
                    Ok(MarketScalar::Unitless(value)) => *value,
                    Ok(MarketScalar::Price(_)) => {
                        return Err(finstack_quant_core::Error::Validation(format!(
                            "equity carry scalar '{key}' must be unitless"
                        )))
                    }
                    Err(_) => {
                        return Err(finstack_quant_core::Error::Input(
                            finstack_quant_core::InputError::NotFound {
                                id: format!(
                                "explicit dividend yield for equity '{}' without a discount curve",
                                params.underlying_ticker
                            ),
                            },
                        ))
                    }
                }
            };
            if !value.is_finite() {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "SVI dividend yield must be finite, got {value}"
                )));
            }
            Some(value)
        } else {
            None
        };
        let forward_inputs = discount
            .as_ref()
            .map(|discount| {
                resolve_equity_forward_inputs(
                    &params.underlying_ticker,
                    params.base_date,
                    spot,
                    params.dividend_yield_override,
                    discount.as_ref(),
                    context,
                )
            })
            .transpose()?;
        let forward_fn = |t: f64| {
            if let (Some(discount), Some(inputs)) = (&discount, &forward_inputs) {
                inputs.forward(discount.as_ref(), t)
            } else {
                let dividend_yield = zero_rate_yield.ok_or_else(|| {
                    finstack_quant_core::Error::Validation(
                        "SVI equity carry resolution is incomplete".to_string(),
                    )
                })?;
                let forward = spot * (-dividend_yield * t).exp();
                Self::validate_positive_input("SVI forward", forward)?;
                Ok(forward)
            }
        };

        let mut quotes_by_expiry: BTreeMap<OrderedF64, Vec<(f64, f64)>> = BTreeMap::new();
        for quote in option_quotes {
            if let VolQuote::OptionVol {
                expiry,
                strike,
                vol,
                ..
            } = quote
            {
                let t = DayCount::Act365F.year_fraction(
                    params.base_date,
                    *expiry,
                    DayCountContext::default(),
                )?;
                Self::validate_positive_input("SVI quote strike", *strike)?;
                Self::validate_positive_input("SVI quote vol", *vol)?;
                if t > 0.0 {
                    quotes_by_expiry
                        .entry(t.into())
                        .or_default()
                        .push((*strike, *vol));
                }
            }
        }

        if quotes_by_expiry.is_empty() {
            return Err(finstack_quant_core::Error::Input(
                finstack_quant_core::InputError::TooFewPoints,
            ));
        }

        let mut params_by_expiry = BTreeMap::new();

        for (&expiry_key, expiry_quotes) in &quotes_by_expiry {
            if expiry_quotes.len() < 5 {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "SVI surface calibration requires at least 5 strikes per expiry; got {} at t={:.6}",
                    expiry_quotes.len(),
                    expiry_key.into_inner()
                )));
            }

            let expiry = expiry_key.into_inner();
            let forward = forward_fn(expiry)?;
            Self::validate_positive_input("SVI forward", forward)?;
            let strikes: Vec<f64> = expiry_quotes.iter().map(|(strike, _)| *strike).collect();
            let vols: Vec<f64> = expiry_quotes.iter().map(|(_, vol)| *vol).collect();

            let svi_params = finstack_quant_models::volatility::svi::calibrate_svi(
                &strikes, &vols, forward, expiry,
            )?;

            params_by_expiry.insert(expiry_key, svi_params);
        }

        let mut grid =
            Vec::with_capacity(params.target_expiries.len() * params.target_strikes.len());
        for &target_expiry in &params.target_expiries {
            let forward = forward_fn(target_expiry)?;
            Self::validate_positive_input("SVI forward", forward)?;
            for &target_strike in &params.target_strikes {
                let vol = interpolate_svi_vol(
                    target_expiry,
                    target_strike,
                    &forward_fn,
                    &params_by_expiry,
                )?;
                if !vol.is_finite() || vol <= 0.0 {
                    return Err(finstack_quant_core::Error::Validation(format!(
                        "SVI surface produced invalid implied vol at t={target_expiry:.6}, strike={target_strike:.6}",
                    )));
                }
                grid.push(vol);
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

        // Calendar and per-slice butterfly checks are strict; interpolated
        // grid call convexity is advisory.
        let target_forwards: Vec<f64> = params
            .target_expiries
            .iter()
            .map(|&t| forward_fn(t))
            .collect::<Result<Vec<_>>>()?;
        let calendar_cfg = ValidationConfig {
            lenient_arbitrage: false,
            ..ValidationConfig::default()
        };
        let butterfly_cfg = ValidationConfig {
            lenient_arbitrage: true,
            ..ValidationConfig::default()
        };
        let calendar_arbitrage =
            validate_calendar_spread_with_forwards(&surface, &calendar_cfg, &target_forwards)
                .err()
                .map(|e| format!("SVI calendar-spread arbitrage: {e}"));

        // Scan each calibrated slice with a one-unit wing margin.
        let mut slice_butterfly_arbitrage: Option<String> = None;
        for (&expiry_key, svi_params) in &params_by_expiry {
            let expiry = expiry_key.into_inner();
            let forward = forward_fn(expiry)?;
            let k_lo =
                (params.target_strikes.first().copied().unwrap_or(forward) / forward).ln() - 1.0;
            let k_hi =
                (params.target_strikes.last().copied().unwrap_or(forward) / forward).ln() + 1.0;
            if let Some((k, g)) = svi_params.butterfly_violation(k_lo, k_hi, 201, 1e-10) {
                slice_butterfly_arbitrage = Some(format!(
                    "SVI butterfly arbitrage: Durrleman g(k)={g:.3e} < 0 at k={k:.4}, \
                     T={expiry:.4}y (negative implied density)"
                ));
                break;
            }
        }

        let butterfly_warning =
            validate_butterfly_call_convexity(&surface, &butterfly_cfg, &target_forwards)
                .err()
                .map(|e| format!("SVI butterfly-spread arbitrage: {e}"));

        let mut report = CalibrationReport::for_type_with_tolerance(
            "svi_surface",
            residuals,
            params_by_expiry.len(),
            global_config.vol_surface.validation_tolerance,
        )
        .with_model_version(crate::versions::SVI_SURFACE);
        report.update_solver_config(global_config.solver.clone());

        let strict_failures: Vec<String> = [calendar_arbitrage, slice_butterfly_arbitrage]
            .into_iter()
            .flatten()
            .collect();
        if !strict_failures.is_empty() {
            report.validation_passed = false;
            let mut detail = strict_failures.join("; ");
            if let Some(bf) = &butterfly_warning {
                detail = format!("{detail}; (advisory) {bf}");
            }
            report.validation_error = Some(detail.clone());
            report.success = false;
            report.convergence_reason =
                format!("SVI surface calibration failed validation: {detail}");
        } else if let Some(bf) = butterfly_warning {
            report.validation_error = Some(format!("(advisory) {bf}"));
        }

        Ok((surface, report))
    }
}

/// Evaluate one calibrated SVI slice without allowing NaN sentinel residuals.
fn evaluate_svi_model_vol(
    params: &finstack_quant_models::volatility::svi::SviParams,
    expiry: f64,
    strike: f64,
    forward: f64,
) -> Result<f64> {
    let log_moneyness = (strike / forward).ln();
    params.implied_vol(log_moneyness, expiry).map_err(|source| {
        finstack_quant_core::Error::Calibration {
            message: format!(
                "SVI implied-vol evaluation failed: expiry={expiry:.12}, strike={strike:.12}, \
                 forward={forward:.12}, log_moneyness={log_moneyness:.12}, \
                 params={{a:{:.12},b:{:.12},rho:{:.12},m:{:.12},sigma:{:.12}}}, cause={source}",
                params.a, params.b, params.rho, params.m, params.sigma
            ),
            category: "svi_evaluation".to_string(),
        }
    })
}

/// Interpolate total variance at a fixed absolute strike, evaluating each SVI
/// slice in its own forward-moneyness coordinate. Extrapolation always holds
/// the nearest calibrated slice's total variance flat (`Clamp`).
fn interpolate_svi_vol(
    target_expiry: f64,
    target_strike: f64,
    forward_fn: &impl Fn(f64) -> Result<f64>,
    params_by_expiry: &BTreeMap<OrderedF64, finstack_quant_models::volatility::svi::SviParams>,
) -> Result<f64> {
    interpolate_total_variance(
        "SVI",
        target_expiry,
        target_strike,
        params_by_expiry,
        |slice_expiry, slice: &finstack_quant_models::volatility::svi::SviParams| {
            let forward = forward_fn(slice_expiry)?;
            if !forward.is_finite() || forward <= 0.0 {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "SVI interpolation: non-positive forward {forward} at T={slice_expiry:.6}"
                )));
            }
            let vol = evaluate_svi_model_vol(slice, slice_expiry, target_strike, forward)?;
            Ok(vol * vol * slice_expiry)
        },
        SurfaceExtrapolationPolicy::Clamp,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::schema::SviSurfaceParams;
    use crate::quotes::ids::QuoteId;
    use crate::quotes::market_quote::MarketQuote;
    use crate::quotes::vol::VolQuote;
    use finstack_quant_core::dates::Date;
    use finstack_quant_core::types::UnderlyingId;
    use finstack_quant_valuations::instruments::OptionType;
    use time::Month;

    fn base_date() -> Date {
        Date::from_calendar_date(2025, Month::January, 1).expect("valid date")
    }

    fn base_params() -> SviSurfaceParams {
        SviSurfaceParams {
            vol_surface_id: "SPX-SVI".to_string(),
            base_date: base_date(),
            underlying_ticker: "SPX".to_string(),
            discount_curve_id: None,
            target_expiries: vec![0.5],
            target_strikes: vec![80.0, 90.0, 100.0, 110.0, 120.0],
            spot_override: Some(100.0),
            dividend_yield_override: Some(0.0),
        }
    }

    #[test]
    fn svi_evaluation_failure_retains_slice_context_without_nan() {
        let params = finstack_quant_models::volatility::svi::SviParams {
            a: -1.0,
            b: 0.0,
            rho: 0.0,
            m: 0.0,
            sigma: 0.1,
        };
        let error = evaluate_svi_model_vol(&params, 1.0, 95.0, 100.0)
            .expect_err("negative total variance must fail");
        let finstack_quant_core::Error::Calibration { message, category } = error else {
            unreachable!("invalid SVI variance should return a Calibration error");
        };
        assert_eq!(category, "svi_evaluation");
        assert!(message.contains("expiry=1.000000000000"));
        assert!(message.contains("strike=95.000000000000"));
        assert!(message.contains("forward=100.000000000000"));
        assert!(message.contains("params={a:-1.000000000000"));
    }

    fn sample_quotes() -> Vec<MarketQuote> {
        let expiry = Date::from_calendar_date(2025, Month::July, 2).expect("valid expiry");
        [
            (80.0, 0.30),
            (90.0, 0.24),
            (100.0, 0.20),
            (110.0, 0.22),
            (120.0, 0.27),
        ]
        .into_iter()
        .map(|(strike, vol)| {
            MarketQuote::Vol(VolQuote::OptionVol {
                id: QuoteId::new(format!("SPX-VOL-20250702-{strike}")),
                underlying: UnderlyingId::new("SPX"),
                expiry,
                strike,
                vol,
                option_type: OptionType::Call,
            })
        })
        .collect()
    }

    #[test]
    fn solve_rejects_non_positive_quote_strike() {
        let mut quotes = sample_quotes();
        quotes[0] = MarketQuote::Vol(VolQuote::OptionVol {
            id: QuoteId::new("SPX-VOL-20250702-0"),
            underlying: UnderlyingId::new("SPX"),
            expiry: Date::from_calendar_date(2025, Month::July, 2).expect("valid expiry"),
            strike: 0.0,
            vol: 0.30,
            option_type: OptionType::Call,
        });

        let err = SviSurfaceTarget::solve(
            &base_params(),
            &quotes,
            &MarketContext::new(),
            &CalibrationConfig::default(),
        )
        .expect_err("non-positive quote strikes should fail");
        assert!(err.to_string().to_lowercase().contains("strike"));
    }

    /// `interpolate_svi_vol` must do Gatheral total-variance
    /// interpolation, not parameter-space linear interpolation. Two slices
    /// at T1=0.5, T2=1.0 with identical ATM total variance (w=0.04) must
    /// yield σ(k=0, T=0.75) consistent with `√(w/T) = √(0.04/0.75)`, not
    /// the parameter-averaged value which would differ whenever `a`, `b`,
    /// or `σ` of the slices disagree.
    #[test]
    fn interpolate_svi_vol_matches_gatheral_total_variance() {
        use finstack_quant_models::volatility::svi::SviParams;

        // Two distinct SVI slices with w_ATM = 0.04 each:
        //   Slice A at T=0.5: σ_ATM = √(0.04 / 0.5) ≈ 0.2828
        //   Slice B at T=1.0: σ_ATM = √(0.04 / 1.0) = 0.2
        // a=0.04, b=0 collapses each slice's total variance to `a` (flat).
        // The two slices agree on w(k=0) = 0.04 but have wildly different
        // implied-vol numbers, precisely the case where parameter-linear
        // interpolation would produce the wrong answer.
        let slice_a = SviParams {
            a: 0.04,
            b: 0.0,
            rho: 0.0,
            m: 0.0,
            sigma: 0.10,
        };
        let slice_b = SviParams {
            a: 0.04,
            b: 0.0,
            rho: 0.0,
            m: 0.0,
            sigma: 0.10,
        };
        let mut by_expiry: BTreeMap<OrderedF64, SviParams> = BTreeMap::new();
        by_expiry.insert(OrderedF64::from(0.5), slice_a);
        by_expiry.insert(OrderedF64::from(1.0), slice_b);

        // Flat forward F = 100 ⇒ ATM strike K = 100 maps to k = 0 on
        // every slice, so the per-slice forward-moneyness recomputation is
        // a no-op here and the assertion still pins the Gatheral value.
        let forward_fn = |_t: f64| Ok(100.0);
        let vol =
            interpolate_svi_vol(0.75, 100.0, &forward_fn, &by_expiry).expect("interpolation ok");
        let expected = (0.04_f64 / 0.75).sqrt();
        assert!(
            (vol - expected).abs() < 1e-9,
            "σ(T=0.75) = {vol:.9}, expected √(w/T) = {expected:.9}"
        );
    }

    /// Calendar-spread no-arbitrage (`∂w/∂T ≥ 0` at a fixed *absolute*
    /// strike) must hold for the cross-expiry interpolation even when the
    /// discount curve is non-flat and the smile genuinely depends on `k`
    /// (`b > 0`).
    ///
    /// # The defect this guards (M11)
    ///
    /// Each SVI slice is fitted in its OWN forward-moneyness coordinate
    /// `kᵢ = ln(K / F(Tᵢ))`. The pre-fix `interpolate_svi_vol` evaluated
    /// every bracketing slice at a SINGLE `k` derived from the target
    /// expiry's forward, so with a non-flat curve (`F(T₁) ≠ F(T₂) ≠ …`)
    /// the two slices were compared at *different absolute strikes*. The
    /// resulting interpolated total variance can DECREASE in `T` at a fixed
    /// absolute strike — a calendar-spread arbitrage.
    ///
    /// `single_k_total_variance` below replicates that buggy recipe and the
    /// test first asserts it genuinely produces a violation (so the fixture
    /// truly exercises the bug), then asserts the production
    /// `interpolate_svi_vol` — which recomputes `kᵢ` per slice — is
    /// calendar-monotone at every strike. With the old `b = 0` fixture the
    /// smile was flat in `k`, so the single-`k` mismatch was invisible;
    /// `b > 0` and a non-flat curve are both required.
    #[test]
    fn interpolate_svi_vol_preserves_calendar_monotonicity() {
        use finstack_quant_models::volatility::svi::SviParams;

        // Three SVI slices, each calibrated in its own forward-moneyness.
        // b > 0 ⇒ total variance genuinely depends on k. Parameters are
        // arbitrage-valid (SviParams::validate) and the slices are
        // calendar-monotone at every fixed absolute strike on the test grid.
        let t1 = 0.5_f64;
        let t2 = 1.5_f64;
        let t3 = 3.0_f64;
        let slice_1 = SviParams {
            a: 0.015,
            b: 0.50,
            rho: 0.6,
            m: 0.20,
            sigma: 0.10,
        };
        let slice_2 = SviParams {
            a: 0.060,
            b: 0.525,
            rho: 0.6,
            m: 0.08,
            sigma: 0.10,
        };
        let slice_3 = SviParams {
            a: 0.180,
            b: 0.55,
            rho: 0.6,
            m: 0.0,
            sigma: 0.10,
        };
        slice_1.validate().expect("slice 1 arbitrage-valid");
        slice_2.validate().expect("slice 2 arbitrage-valid");
        slice_3.validate().expect("slice 3 arbitrage-valid");

        let mut by_expiry: BTreeMap<OrderedF64, SviParams> = BTreeMap::new();
        by_expiry.insert(OrderedF64::from(t1), slice_1);
        by_expiry.insert(OrderedF64::from(t2), slice_2);
        by_expiry.insert(OrderedF64::from(t3), slice_3);

        // Non-flat discount curve: zero rate rises with T, so the forward
        // F(T) = S·exp(r(T)·T) is genuinely T-dependent and F(T₁) ≠ F(T₂).
        let spot = 100.0_f64;
        let forward_value = |t: f64| spot * ((0.01 + 0.09 * t) * t).exp();
        let forward_fn = |t: f64| Ok(forward_value(t));
        assert!(
            (forward_value(t1) - forward_value(t2)).abs() > 1.0,
            "test fixture must use a non-flat forward curve"
        );

        // Pre-fix recipe: BOTH bracketing slices evaluated at one `k`
        // computed from the *target* expiry's forward.
        let single_k_total_variance = |t: f64, strike: f64| -> f64 {
            let k_target = (strike / forward_value(t)).ln();
            let mut lower = (t1, slice_1);
            let mut upper = (t3, slice_3);
            for (&ek, &p) in &by_expiry {
                let e = ek.into_inner();
                if e <= t {
                    lower = (e, p);
                }
                if e >= t {
                    upper = (e, p);
                    break;
                }
            }
            if (upper.0 - lower.0).abs() < f64::EPSILON {
                return lower.1.total_variance(k_target);
            }
            let w_l = lower.1.total_variance(k_target);
            let w_u = upper.1.total_variance(k_target);
            let tau = (t - lower.0) / (upper.0 - lower.0);
            w_l + tau * (w_u - w_l)
        };

        // ITM / ATM / OTM grid (relative to the front forward ≈ 102.8).
        let strikes = [80.0_f64, 95.0, 103.0, 115.0, 135.0];
        let n_steps = 50;

        // (a) The buggy single-`k` recipe must genuinely violate calendar
        // monotonicity off-ATM — otherwise the fixture would not exercise
        // the defect and the test below would be vacuous.
        let mut buggy_violation = None;
        for &strike in &strikes {
            let mut prev_w = f64::NEG_INFINITY;
            let mut prev_t = t1;
            for i in 0..=n_steps {
                let t = t1 + (t3 - t1) * (i as f64) / (n_steps as f64);
                let w = single_k_total_variance(t, strike);
                if prev_w.is_finite() && w < prev_w - 1e-9 && buggy_violation.is_none() {
                    buggy_violation = Some((strike, prev_t, prev_w, t, w));
                }
                prev_w = w;
                prev_t = t;
            }
        }
        let (vk, vt0, vw0, vt1, vw1) = buggy_violation.expect(
            "single-k SVI interpolation must produce a calendar-arbitrage \
             violation for this non-flat-curve fixture",
        );
        println!(
            "single-k recipe calendar arbitrage: strike={vk:.1} \
             w(T={vt0:.4})={vw0:.6} -> w(T={vt1:.4})={vw1:.6} (decrease {:.3e})",
            vw0 - vw1
        );

        // (b) The production interpolation recomputes kᵢ per slice and must
        // therefore stay calendar-monotone at every fixed absolute strike.
        for &strike in &strikes {
            let mut prev_w = f64::NEG_INFINITY;
            let mut prev_t = t1;
            for i in 0..=n_steps {
                let t = t1 + (t3 - t1) * (i as f64) / (n_steps as f64);
                let vol = interpolate_svi_vol(t, strike, &forward_fn, &by_expiry)
                    .expect("interpolation ok");
                let w = vol * vol * t;
                assert!(
                    w >= prev_w - 1e-9,
                    "calendar-spread arbitrage in interpolate_svi_vol at \
                     strike={strike:.1}: w(T={prev_t:.4})={prev_w:.6} > \
                     w(T={t:.4})={w:.6}"
                );
                prev_w = w;
                prev_t = t;
            }
        }
    }

    /// Item 9: `report.success` must reflect `report.validation_passed`. An SVI surface
    /// that fails the post-fit calendar-spread / butterfly arbitrage checks must not be
    /// reported as a successful calibration.
    ///
    /// The fixture supplies two expiry slices where the longer-dated slice (T≈0.75y)
    /// has uniformly *lower* implied vols than the shorter one (T≈0.25y). Each slice
    /// fits arbitrage-free in isolation (so `calibrate_svi` succeeds per slice), but the
    /// assembled surface has total variance that *decreases* with expiry — a calendar
    /// arbitrage. Pre-fix the surface validators ran in lenient mode (only warned, so
    /// `validation_passed` stayed `true`) and `success` was hard-coded `true`; post-fix
    /// the strict validators flag the violation and `success` follows `validation_passed`
    /// to `false`. `solve` must still return `Ok` (a failed *report*, not an aborted
    /// calibration).
    #[test]
    fn item9_arbitrageable_svi_surface_is_not_reported_successful() {
        let e1 = Date::from_calendar_date(2025, Month::April, 1).expect("e1");
        let e2 = Date::from_calendar_date(2026, Month::January, 1).expect("e2");
        let smile_short = [
            (80.0, 0.55),
            (90.0, 0.45),
            (100.0, 0.40),
            (110.0, 0.44),
            (120.0, 0.52),
        ];
        // Longer-dated vols are far lower -> σ²·T decreases with T -> calendar arbitrage.
        let smile_long = [
            (80.0, 0.16),
            (90.0, 0.13),
            (100.0, 0.11),
            (110.0, 0.13),
            (120.0, 0.15),
        ];

        let mut quotes = Vec::new();
        for (strike, vol) in smile_short {
            quotes.push(MarketQuote::Vol(VolQuote::OptionVol {
                id: QuoteId::new(format!("E1-{strike}")),
                underlying: UnderlyingId::new("SPX"),
                expiry: e1,
                strike,
                vol,
                option_type: OptionType::Call,
            }));
        }
        for (strike, vol) in smile_long {
            quotes.push(MarketQuote::Vol(VolQuote::OptionVol {
                id: QuoteId::new(format!("E2-{strike}")),
                underlying: UnderlyingId::new("SPX"),
                expiry: e2,
                strike,
                vol,
                option_type: OptionType::Call,
            }));
        }

        let mut p = base_params();
        p.target_expiries = vec![90.0 / 365.0, 1.0];

        let (_surface, report) = SviSurfaceTarget::solve(
            &p,
            &quotes,
            &MarketContext::new(),
            &CalibrationConfig::default(),
        )
        .expect("solve must still return Ok — a failed report, not an aborted calibration");

        assert!(
            !report.validation_passed,
            "an arbitrageable SVI surface must fail validation"
        );
        assert!(
            !report.success,
            "Item 9: `success` must reflect `validation_passed` — an arbitrageable \
             surface must NOT report success (validation_error: {:?})",
            report.validation_error,
        );
        assert!(
            report
                .validation_error
                .as_deref()
                .is_some_and(|e| e.contains("arbitrage")),
            "validation_error should describe the arbitrage violation: {:?}",
            report.validation_error,
        );
    }

    #[test]
    fn solve_rejects_non_positive_spot_override() {
        let mut params = base_params();
        params.spot_override = Some(0.0);

        let err = SviSurfaceTarget::solve(
            &params,
            &sample_quotes(),
            &MarketContext::new(),
            &CalibrationConfig::default(),
        )
        .expect_err("non-positive spot override should fail");
        assert!(err.to_string().to_lowercase().contains("spot"));
    }
}
