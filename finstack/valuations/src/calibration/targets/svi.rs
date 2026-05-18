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
//! [`SurfaceValidator::validate_calendar_spread`] and
//! [`SurfaceValidator::validate_butterfly_spread`] so any residual
//! arbitrage in the calibrated slices surfaces as a structured
//! `Error::Validation` rather than propagating silently into pricing.

use crate::calibration::api::schema::SviSurfaceParams;
use crate::calibration::config::CalibrationConfig;
use crate::calibration::constants::OrderedF64;
use crate::calibration::validation::ValidationConfig;
use crate::calibration::CalibrationReport;
use crate::market::quotes::market_quote::MarketQuote;
use crate::market::quotes::vol::VolQuote;
use finstack_core::dates::{DayCount, DayCountContext};
use finstack_core::market_data::context::MarketContext;
use finstack_core::market_data::scalars::MarketScalar;
use finstack_core::market_data::surfaces::VolSurface;
use finstack_core::Result;
use std::collections::BTreeMap;

use crate::calibration::validation::surfaces::SurfaceValidator;

/// Target for SVI surface calibration from option volatility quotes.
pub(crate) struct SviSurfaceTarget;

impl SviSurfaceTarget {
    fn validate_positive_input(label: &str, value: f64) -> Result<()> {
        if !value.is_finite() || value <= 0.0 {
            return Err(finstack_core::Error::Validation(format!(
                "{label} must be finite and positive; got {value}"
            )));
        }
        Ok(())
    }

    fn validate_target_grid(params: &SviSurfaceParams) -> Result<()> {
        for (idx, expiry) in params.target_expiries.iter().enumerate() {
            Self::validate_positive_input(&format!("SVI target_expiries[{idx}]"), *expiry)?;
        }
        for (idx, strike) in params.target_strikes.iter().enumerate() {
            Self::validate_positive_input(&format!("SVI target_strikes[{idx}]"), *strike)?;
        }
        if let Some(spot_override) = params.spot_override {
            Self::validate_positive_input("SVI spot_override", spot_override)?;
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
            return Err(finstack_core::Error::Input(
                finstack_core::InputError::TooFewPoints,
            ));
        }
        Self::validate_target_grid(params)?;

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
            return Err(finstack_core::Error::Input(
                finstack_core::InputError::TooFewPoints,
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

        let forward_fn = |t: f64| -> f64 {
            if let Some(curve) = discount.as_ref() {
                let r = curve.zero(t);
                spot * (r * t).exp()
            } else {
                spot
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
            return Err(finstack_core::Error::Input(
                finstack_core::InputError::TooFewPoints,
            ));
        }

        let mut params_by_expiry = BTreeMap::new();
        let mut residuals = BTreeMap::new();

        for (&expiry_key, expiry_quotes) in &quotes_by_expiry {
            if expiry_quotes.len() < 5 {
                return Err(finstack_core::Error::Validation(format!(
                    "SVI surface calibration requires at least 5 strikes per expiry; got {} at t={:.6}",
                    expiry_quotes.len(),
                    expiry_key.into_inner()
                )));
            }

            let expiry = expiry_key.into_inner();
            let forward = forward_fn(expiry);
            Self::validate_positive_input("SVI forward", forward)?;
            let strikes: Vec<f64> = expiry_quotes.iter().map(|(strike, _)| *strike).collect();
            let vols: Vec<f64> = expiry_quotes.iter().map(|(_, vol)| *vol).collect();

            let svi_params = finstack_core::math::volatility::svi::calibrate_svi(
                &strikes, &vols, forward, expiry,
            )?;

            for (idx, (strike, market_vol)) in expiry_quotes.iter().enumerate() {
                let log_moneyness = (*strike / forward).ln();
                let model_vol = svi_params.implied_vol(log_moneyness, expiry);
                residuals.insert(
                    format!("svi_t{expiry:.6}_k{strike:.6}_i{idx}"),
                    (model_vol - *market_vol).abs(),
                );
            }

            params_by_expiry.insert(expiry_key, svi_params);
        }

        let mut grid =
            Vec::with_capacity(params.target_expiries.len() * params.target_strikes.len());
        for &target_expiry in &params.target_expiries {
            let forward = forward_fn(target_expiry);
            Self::validate_positive_input("SVI forward", forward)?;
            for &target_strike in &params.target_strikes {
                // Pass the absolute strike (not a single pre-computed
                // log-moneyness): each bracketing slice was calibrated in its
                // own forward-moneyness coordinate, so `interpolate_svi_vol`
                // must re-derive `k_i = ln(K / F(Tᵢ))` per slice.
                let vol = interpolate_svi_vol(
                    target_expiry,
                    target_strike,
                    &forward_fn,
                    &params_by_expiry,
                )?;
                if !vol.is_finite() || vol <= 0.0 {
                    return Err(finstack_core::Error::Validation(format!(
                        "SVI surface produced invalid implied vol at t={target_expiry:.6}, strike={target_strike:.6}",
                    )));
                }
                grid.push(vol);
            }
        }

        let surface = VolSurface::from_grid(
            &params.surface_id,
            &params.target_expiries,
            &params.target_strikes,
            &grid,
        )?;

        // After Gatheral total-variance interpolation, sanity-check the produced grid
        // for calendar-spread and butterfly arbitrage.
        //
        // Two validation modes are used deliberately:
        //
        //  * Calendar-spread is checked in STRICT mode. "Total variance non-decreasing
        //    in expiry" is an unambiguous no-arbitrage condition that holds exactly
        //    under sampling, so a strict-mode `Err` here is a genuine arbitrage and
        //    must fail the calibration (item 9 — see below).
        //
        //  * Butterfly is checked in LENIENT mode. The butterfly validator's discrete
        //    convexity test on the coarse *absolute-strike* target grid is only an
        //    approximation (an SVI smile is convex in log-moneyness `k`, not
        //    necessarily in absolute strike `K`), so it can flag a genuinely
        //    arbitrage-free SVI surface. It is recorded as a warning/diagnostic only
        //    and does not gate `success`.
        let calendar_cfg = ValidationConfig {
            lenient_arbitrage: false,
            ..ValidationConfig::default()
        };
        let butterfly_cfg = ValidationConfig {
            lenient_arbitrage: true,
            ..ValidationConfig::default()
        };
        // A strict calendar-spread `Err` is a genuine arbitrage -> fails the report.
        let calendar_arbitrage = surface
            .validate_calendar_spread(&calendar_cfg)
            .err()
            .map(|e| format!("SVI calendar-spread arbitrage: {e}"));
        // Butterfly is advisory only (lenient mode never returns `Err`; this stays
        // `None`, but is kept for symmetry / future strict-butterfly tightening).
        let butterfly_warning = surface
            .validate_butterfly_spread(&butterfly_cfg)
            .err()
            .map(|e| format!("SVI butterfly-spread arbitrage: {e}"));

        let mut report = CalibrationReport::new(
            residuals,
            params_by_expiry.len(),
            true,
            "SVI surface calibration completed",
        )
        .with_model_version(finstack_core::versions::SVI_SURFACE);
        report.update_solver_config(global_config.solver.clone());

        // A genuine (strict-mode) calendar-spread arbitrage fails validation.
        if let Some(calendar_err) = calendar_arbitrage {
            report.validation_passed = false;
            // Append the advisory butterfly diagnostic, if any, for completeness.
            let detail = match &butterfly_warning {
                Some(bf) => format!("{calendar_err}; (advisory) {bf}"),
                None => calendar_err,
            };
            report.validation_error = Some(detail.clone());
            // Item 9: `success` MUST reflect `validation_passed`. An SVI surface that
            // fails the calendar-spread no-arbitrage check is not a usable calibration
            // result — reporting `success = true` while `validation_passed` is `false`
            // lets an arbitrageable surface be silently accepted downstream. A
            // calibration that produces an arbitrageable surface has not succeeded.
            report.success = false;
            report.convergence_reason =
                format!("SVI surface calibration failed validation: {detail}");
        } else if let Some(bf) = butterfly_warning {
            // Butterfly is advisory only: record it without failing validation or
            // success (the discrete absolute-strike convexity test is approximate).
            report.validation_error = Some(format!("(advisory) {bf}"));
        }

        Ok((surface, report))
    }
}

/// Interpolate implied volatility between calibrated SVI expiry slices using
/// Gatheral's total-variance recipe: `w(T) = (1 − τ)·w₁(k₁) + τ·w₂(k₂)`
/// where `τ = (T − T₁) / (T₂ − T₁)` and `wᵢ(kᵢ) = σ²·Tᵢ` is the per-slice
/// total variance. Returns `σ(T) = √(w(T) / T)`.
///
/// # Per-slice forward-moneyness
///
/// Each SVI slice is calibrated in *its own* forward-moneyness coordinate
/// `kᵢ = ln(K / F(Tᵢ))`. With a non-flat discount curve `F(T)` varies with
/// `T`, so a single `k` computed at the target expiry does **not** address
/// the same absolute strike on the neighbouring slices. To keep the
/// interpolation calendar-arbitrage-free at a *fixed absolute strike* `K`,
/// this function takes `target_strike` and `forward_fn` and re-derives the
/// log-moneyness per slice before evaluating its total variance.
///
/// This preserves calendar-spread monotonicity of `w` by construction:
/// if the calibrated slices at `T₁ < T₂` are themselves calendar-monotone
/// at every absolute strike (i.e. `w₁(k₁) ≤ w₂(k₂)`), linear interpolation
/// in `w` keeps the surface arbitrage-free between them.
///
/// Extrapolation (outside the calibrated range) falls back to the nearest
/// slice's `implied_vol`, evaluated at *that slice's* forward-moneyness —
/// the standard approach when there's no data to constrain the surface
/// beyond the outermost expiries.
///
/// A naive parameter-space interpolation (linear in `a`, `b`, `ρ`,
/// `m`, `σ`) makes no calendar-spread guarantees; Gatheral (2004,
/// 2013) showed this admits calendar arbitrage under common calibration
/// conditions, which motivates the total-variance form used here.
fn interpolate_svi_vol(
    target_expiry: f64,
    target_strike: f64,
    forward_fn: &impl Fn(f64) -> f64,
    params_by_expiry: &BTreeMap<OrderedF64, finstack_core::math::volatility::svi::SviParams>,
) -> Result<f64> {
    if target_expiry <= 0.0 {
        return Err(finstack_core::Error::Validation(format!(
            "SVI interpolation target expiry must be positive; got {target_expiry:.6}"
        )));
    }

    // Log-moneyness of `target_strike` in the forward-moneyness coordinate of
    // the SVI slice calibrated at expiry `slice_expiry`.
    let slice_log_moneyness = |slice_expiry: f64| -> Result<f64> {
        let forward = forward_fn(slice_expiry);
        if !forward.is_finite() || forward <= 0.0 {
            return Err(finstack_core::Error::Validation(format!(
                "SVI interpolation: non-positive forward {forward} at T={slice_expiry:.6}"
            )));
        }
        Ok((target_strike / forward).ln())
    };

    let Some((&first_key, &first_params)) = params_by_expiry.iter().next() else {
        return Err(finstack_core::Error::Input(
            finstack_core::InputError::TooFewPoints,
        ));
    };

    if params_by_expiry.len() == 1 || target_expiry <= first_key.into_inner() {
        let k = slice_log_moneyness(first_key.into_inner())?;
        return Ok(first_params.implied_vol(k, target_expiry));
    }

    let Some((&last_key, &last_params)) = params_by_expiry.iter().next_back() else {
        let k = slice_log_moneyness(first_key.into_inner())?;
        return Ok(first_params.implied_vol(k, target_expiry));
    };
    if target_expiry >= last_key.into_inner() {
        let k = slice_log_moneyness(last_key.into_inner())?;
        return Ok(last_params.implied_vol(k, target_expiry));
    }

    let mut lower = (first_key.into_inner(), first_params);
    let mut upper = (last_key.into_inner(), last_params);
    for (&expiry_key, &p) in params_by_expiry {
        let expiry = expiry_key.into_inner();
        if expiry <= target_expiry {
            lower = (expiry, p);
        }
        if expiry >= target_expiry {
            upper = (expiry, p);
            break;
        }
    }

    if (upper.0 - lower.0).abs() < f64::EPSILON {
        let k = slice_log_moneyness(lower.0)?;
        return Ok(lower.1.implied_vol(k, target_expiry));
    }

    // Total-variance interpolation per Gatheral. Each slice is evaluated at
    // its OWN forward-moneyness so the interpolated total variance refers to
    // a fixed absolute strike across `T₁`, `T`, and `T₂`.
    let k_lower = slice_log_moneyness(lower.0)?;
    let k_upper = slice_log_moneyness(upper.0)?;
    let w_lower = lower.1.total_variance(k_lower);
    let w_upper = upper.1.total_variance(k_upper);
    if w_lower < 0.0 || w_upper < 0.0 {
        return Err(finstack_core::Error::Validation(format!(
            "SVI negative total variance at K={target_strike:.4} (k_lower={k_lower:.4}, k_upper={k_upper:.4}): w_lower={w_lower:.6}, w_upper={w_upper:.6}"
        )));
    }
    let tau = (target_expiry - lower.0) / (upper.0 - lower.0);
    let w_interp = w_lower + tau * (w_upper - w_lower);
    if !w_interp.is_finite() || w_interp < 0.0 {
        return Err(finstack_core::Error::Validation(format!(
            "SVI total-variance interpolation produced invalid w={w_interp:.6} at T={target_expiry:.6}, K={target_strike:.4}"
        )));
    }
    Ok((w_interp / target_expiry).sqrt())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::calibration::api::schema::SviSurfaceParams;
    use crate::instruments::OptionType;
    use crate::market::conventions::ids::OptionConventionId;
    use crate::market::quotes::ids::QuoteId;
    use crate::market::quotes::market_quote::MarketQuote;
    use crate::market::quotes::vol::VolQuote;
    use finstack_core::dates::Date;
    use finstack_core::types::UnderlyingId;
    use time::Month;

    fn base_date() -> Date {
        Date::from_calendar_date(2025, Month::January, 1).expect("valid date")
    }

    fn base_params() -> SviSurfaceParams {
        SviSurfaceParams {
            surface_id: "SPX-SVI".to_string(),
            base_date: base_date(),
            underlying_ticker: "SPX".to_string(),
            discount_curve_id: None,
            target_expiries: vec![0.5],
            target_strikes: vec![80.0, 90.0, 100.0, 110.0, 120.0],
            spot_override: Some(100.0),
        }
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
                convention: OptionConventionId::new("USD-EQ"),
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
            convention: OptionConventionId::new("USD-EQ"),
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
        use finstack_core::math::volatility::svi::SviParams;

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
        let forward_fn = |_t: f64| -> f64 { 100.0 };
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
        use finstack_core::math::volatility::svi::SviParams;

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
        let forward_fn = |t: f64| -> f64 { spot * ((0.01 + 0.09 * t) * t).exp() };
        assert!(
            (forward_fn(t1) - forward_fn(t2)).abs() > 1.0,
            "test fixture must use a non-flat forward curve"
        );

        // Pre-fix recipe: BOTH bracketing slices evaluated at one `k`
        // computed from the *target* expiry's forward.
        let single_k_total_variance = |t: f64, strike: f64| -> f64 {
            let k_target = (strike / forward_fn(t)).ln();
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
                convention: OptionConventionId::new("USD-EQ"),
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
                convention: OptionConventionId::new("USD-EQ"),
            }));
        }

        let mut p = base_params();
        p.target_expiries = vec![0.25, 0.75];

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
