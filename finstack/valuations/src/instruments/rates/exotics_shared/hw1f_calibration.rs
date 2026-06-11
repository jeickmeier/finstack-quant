//! Resolve HW1F short-rate parameters (κ, σ) for exotic rate products.
//!
//! Precedence:
//! 1. Explicit overrides in `PricingOverrides.model_config` (keys `hw1f_kappa`, `hw1f_sigma`).
//! 2. Pricing-time calibration from a volatility surface, when supplied.
//! 3. Pre-calibrated parameters read from the `MarketContext` scalar store.
//!    A prior Hull-White calibration step (`StepParams::HullWhite` /
//!    `StepParams::CapFloorHullWhite`) writes solved κ/σ as named scalars under
//!    the keys produced by
//!    [`hw1f_scalar_keys`](crate::calibration::hull_white::hw1f_scalar_keys) /
//!    [`capfloor_hw1f_scalar_keys`](crate::calibration::hull_white::capfloor_hw1f_scalar_keys).
//!    When both scalars are present and valid for the request's `curve_id`,
//!    the resolver returns those market-consistent parameters.
//! 4. `HullWhiteParams::default()` when neither overrides nor calibrated market
//!    scalars are available, with a `tracing::warn!` log.
//!
//! The resolver returns the winning [`Hw1fParamSource`] alongside the
//! parameters so callers can stamp provenance, and rejects *partial* inputs
//! (exactly one of κ/σ supplied as an override or found as a calibrated
//! scalar) instead of silently falling through.

use crate::calibration::hull_white::{
    calibrate_hull_white_to_swaptions, capfloor_hw1f_scalar_keys,
    hw1f_caplet_forward_rate_normal_vol, hw1f_scalar_keys, HullWhiteParams, SwapFrequency,
    SwaptionQuote,
};
use finstack_core::market_data::context::MarketContext;
use finstack_core::market_data::scalars::MarketScalar;
use finstack_core::Result;

/// Where the resolved HW1F parameters came from.
///
/// Returned alongside the parameters by [`resolve_hw1f_params`] so pricers can
/// stamp the provenance (`hw1f_param_source`) into logs/diagnostics instead of
/// silently proceeding on uncalibrated defaults.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hw1fParamSource {
    /// Both κ and σ supplied via `PricingOverrides.model_config`.
    Override,
    /// Calibrated at pricing time from a volatility surface.
    CalibratedSurface,
    /// Pre-calibrated κ/σ read from the `MarketContext` scalar store.
    MarketScalars,
    /// Neither overrides, surface, nor scalars were available: the pricer's
    /// constructor fallback or `HullWhiteParams::default()` was used.
    DefaultFallback,
}

impl Hw1fParamSource {
    /// Stable string form for logging / metadata stamping (`hw1f_param_source`).
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Override => "override",
            Self::CalibratedSurface => "calibrated_surface",
            Self::MarketScalars => "market_scalars",
            Self::DefaultFallback => "default_fallback",
        }
    }
}

/// Which calibration flavour populated the [`MarketContext`] scalars.
///
/// Swaption-calibrated and cap/floor-calibrated HW1F parameters live under
/// distinct scalar-key conventions, so the resolver must know which set of
/// keys to read for a given instrument.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hw1fCalibrationFlavor {
    /// Parameters calibrated to a swaption vol grid (`{curve}_HW1F_*`).
    Swaption,
    /// Parameters calibrated to a cap/floor vol strip (`{curve}_CAPFLOOR_HW1F_*`).
    CapFloor,
}

/// One caplet/floorlet observation used to infer a short-rate σ from a normal-vol surface.
#[derive(Debug, Clone, Copy)]
pub struct Hw1fCapletSurfacePoint {
    /// Time from valuation date to option fixing, in years.
    pub t_fix: f64,
    /// Accrual year fraction for the underlying rate period.
    pub accrual: f64,
    /// Surface strike coordinate to read.
    pub strike: f64,
    /// Non-negative aggregation weight, typically annuity × normal vega.
    pub weight: f64,
}

/// Optional pricing-time vol-surface calibration input for HW1F products.
pub enum Hw1fSurfaceCalibration<'a> {
    /// Infer σ from caplet normal vols on a cap/floor surface.
    CapFloor {
        /// Vol surface identifier.
        surface_id: &'a str,
        /// Caplet observations to sample from the surface.
        points: &'a [Hw1fCapletSurfacePoint],
    },
    /// Calibrate κ/σ from an ATM swaption surface.
    Swaption {
        /// Swaption vol surface identifier.
        surface_id: &'a str,
        /// Ignore surface expiries beyond this horizon, when supplied.
        max_expiry: Option<f64>,
        /// Underlying swap fixed-leg frequency for calibration.
        frequency: SwapFrequency,
    },
}

impl Hw1fCalibrationFlavor {
    /// Scalar-store keys `(kappa, sigma)` for this flavour and curve id.
    #[must_use]
    fn scalar_keys(self, curve_id: &str) -> (String, String) {
        match self {
            Self::Swaption => hw1f_scalar_keys(curve_id),
            Self::CapFloor => capfloor_hw1f_scalar_keys(curve_id),
        }
    }
}

/// Input for HW1F parameter resolution.
pub struct Hw1fResolveRequest<'a> {
    /// Discount/forward curve id under which a prior calibration step keyed
    /// its solved κ/σ scalars. Used to look up calibrated parameters from the
    /// [`MarketContext`].
    pub curve_id: &'a str,
    /// Which calibration flavour to read scalars for (swaption vs cap/floor).
    pub flavor: Hw1fCalibrationFlavor,
    /// Optional pricing-override JSON blob (from `PricingOverrides.model_config`).
    pub overrides: Option<&'a serde_json::Value>,
    /// Optional pricing-time surface input used to infer market-consistent HW1F params.
    pub surface: Option<Hw1fSurfaceCalibration<'a>>,
    /// Optional fallback used by pricers with explicit constructor-level defaults.
    pub fallback: Option<HullWhiteParams>,
    /// Context label for logs/warns (e.g., "TARN TARN-USD-5Y").
    pub context: &'a str,
}

/// Read a positive, finite `f64` from a [`MarketScalar`].
fn scalar_as_positive_f64(scalar: &MarketScalar) -> Option<f64> {
    let value = match scalar {
        MarketScalar::Unitless(v) => *v,
        MarketScalar::Price(m) => m.amount(),
    };
    (value.is_finite() && value > 0.0).then_some(value)
}

fn override_positive_f64(
    obj: Option<&serde_json::Map<String, serde_json::Value>>,
    key: &str,
) -> Result<Option<f64>> {
    let Some(value) = obj.and_then(|o| o.get(key)).and_then(|x| x.as_f64()) else {
        return Ok(None);
    };
    if value.is_finite() && value > 0.0 {
        Ok(Some(value))
    } else {
        Err(finstack_core::Error::Validation(format!(
            "{key} override must be positive and finite, got {value}"
        )))
    }
}

fn resolve_from_surface(
    req: &Hw1fResolveRequest<'_>,
    market: &MarketContext,
    kappa_hint: f64,
) -> Result<Option<HullWhiteParams>> {
    let Some(surface_calibration) = req.surface.as_ref() else {
        return Ok(None);
    };
    match surface_calibration {
        Hw1fSurfaceCalibration::CapFloor { surface_id, points } => {
            resolve_capfloor_surface_params(market, surface_id, points, kappa_hint)
        }
        Hw1fSurfaceCalibration::Swaption {
            surface_id,
            max_expiry,
            frequency,
        } => resolve_swaption_surface_params(
            market,
            req.curve_id,
            surface_id,
            *max_expiry,
            *frequency,
        ),
    }
}

fn resolve_capfloor_surface_params(
    market: &MarketContext,
    surface_id: &str,
    points: &[Hw1fCapletSurfacePoint],
    kappa: f64,
) -> Result<Option<HullWhiteParams>> {
    let surface = match market.get_surface(surface_id) {
        Ok(surface) => surface,
        Err(_) => return Ok(None),
    };
    let mut weighted_sigma = 0.0;
    let mut total_weight = 0.0;
    for point in points {
        let factor = hw1f_caplet_forward_rate_normal_vol(kappa, 1.0, point.t_fix, point.accrual);
        let normal_vol = surface.value_clamped(point.t_fix, point.strike);
        if factor > 0.0 && normal_vol.is_finite() && normal_vol > 0.0 {
            let weight = point.weight.max(0.0);
            if weight > 0.0 {
                weighted_sigma += (normal_vol / factor) * weight;
                total_weight += weight;
            }
        }
    }
    if total_weight <= 0.0 {
        return Ok(None);
    }
    HullWhiteParams::new(kappa, weighted_sigma / total_weight).map(Some)
}

fn resolve_swaption_surface_params(
    market: &MarketContext,
    curve_id: &str,
    surface_id: &str,
    max_expiry: Option<f64>,
    frequency: SwapFrequency,
) -> Result<Option<HullWhiteParams>> {
    let surface = match market.get_surface(surface_id) {
        Ok(surface) => surface,
        Err(_) => return Ok(None),
    };
    // The swaption calibration interprets the grid as expiry × swap-tenor with
    // NORMAL (Bachelier) vols (`is_normal_vol: true` below). A surface tagged
    // as strike-axis or Black-quoted would be silently misread, so enforce the
    // contract instead of guessing.
    surface.require_secondary_axis(finstack_core::market_data::surfaces::VolSurfaceAxis::Tenor)?;
    surface.require_quote_type(finstack_core::market_data::surfaces::VolQuoteType::Normal)?;
    let discount = match market.get_discount(curve_id) {
        Ok(discount) => discount,
        Err(_) => return Ok(None),
    };
    let mut quotes = Vec::new();
    for &expiry in surface.expiries() {
        if expiry <= 0.0 || max_expiry.is_some_and(|limit| expiry > limit) {
            continue;
        }
        for &tenor in surface.strikes() {
            if tenor <= 0.0 {
                continue;
            }
            let vol = surface.value_clamped(expiry, tenor);
            if vol.is_finite() && vol > 0.0 {
                quotes.push(SwaptionQuote {
                    expiry,
                    tenor,
                    volatility: vol,
                    is_normal_vol: true,
                });
            }
        }
    }
    if quotes.len() < 2 {
        return Ok(None);
    }
    let df = |t: f64| discount.df(t);
    let (params, _report) = calibrate_hull_white_to_swaptions(&df, &quotes, frequency, None)?;
    Ok(Some(params))
}

/// Resolve HW1F parameters following the documented precedence.
///
/// Returns the parameters together with their [`Hw1fParamSource`] so callers
/// can stamp the provenance (`hw1f_param_source`).
///
/// Never returns an error for the "no overrides + no calibrated scalars" case;
/// instead emits a `tracing::warn!` and returns `HullWhiteParams::default()`
/// tagged [`Hw1fParamSource::DefaultFallback`].
///
/// Errors when:
/// - overrides are malformed (non-positive / non-finite values), or
/// - a *partial* override pair is supplied (exactly one of `hw1f_kappa` /
///   `hw1f_sigma`), or
/// - a *partial* calibrated scalar pair is found in the `MarketContext` and no
///   higher-precedence source resolves. Partial inputs almost certainly
///   indicate a wiring bug; silently discarding half a parameter set and
///   proceeding on defaults hides it.
///
/// `market` is consulted for pre-calibrated κ/σ scalars (precedence step 2)
/// when no explicit overrides are supplied.
pub fn resolve_hw1f_params(
    req: &Hw1fResolveRequest<'_>,
    market: &MarketContext,
) -> Result<(HullWhiteParams, Hw1fParamSource)> {
    let override_obj = req.overrides.and_then(|v| v.as_object());
    let override_kappa = override_positive_f64(override_obj, "hw1f_kappa")?;
    let override_sigma = override_positive_f64(override_obj, "hw1f_sigma")?;
    match (override_kappa, override_sigma) {
        (Some(k), Some(s)) => {
            return HullWhiteParams::new(k, s).map(|p| (p, Hw1fParamSource::Override));
        }
        (None, None) => {}
        // Partial override: exactly one of κ/σ supplied. Reject instead of
        // silently discarding the supplied value and falling through.
        (k, s) => {
            return Err(finstack_core::Error::Validation(format!(
                "{}: partial HW1F override (hw1f_kappa={k:?}, hw1f_sigma={s:?}); \
                 supply both hw1f_kappa and hw1f_sigma or neither",
                req.context
            )));
        }
    }

    // Pre-calibrated parameters from the MarketContext scalar store.
    let (kappa_key, sigma_key) = req.flavor.scalar_keys(req.curve_id);
    let kappa = market
        .get_price(&kappa_key)
        .ok()
        .and_then(scalar_as_positive_f64);
    let sigma = market
        .get_price(&sigma_key)
        .ok()
        .and_then(scalar_as_positive_f64);

    // (2) Pricing-time vol-surface calibration. This keeps scenario/attribution
    // vol shocks flowing through the same `VolSurface` channel as vanilla options.
    let defaults = req.fallback.unwrap_or_default();
    let kappa_hint = override_kappa.or(kappa).unwrap_or(defaults.kappa);
    if let Some(surface_params) = resolve_from_surface(req, market, kappa_hint)? {
        tracing::debug!(
            target = "finstack.exotic_rates",
            context = req.context,
            curve_id = req.curve_id,
            kappa = surface_params.kappa,
            sigma = surface_params.sigma,
            hw1f_param_source = Hw1fParamSource::CalibratedSurface.as_str(),
            "resolved HW1F parameters from volatility surface"
        );
        return Ok((surface_params, Hw1fParamSource::CalibratedSurface));
    }

    // (3) Pre-calibrated parameters from the MarketContext scalar store.
    match (kappa, sigma) {
        (Some(k), Some(s)) => {
            tracing::debug!(
                target = "finstack.exotic_rates",
                context = req.context,
                curve_id = req.curve_id,
                kappa = k,
                sigma = s,
                hw1f_param_source = Hw1fParamSource::MarketScalars.as_str(),
                "resolved HW1F parameters from calibrated MarketContext scalars"
            );
            return HullWhiteParams::new(k, s).map(|p| (p, Hw1fParamSource::MarketScalars));
        }
        (None, None) => {}
        // Partial calibrated pair: exactly one of the κ/σ scalars exists under
        // this flavour's keys. A prior calibration step wrote half a parameter
        // set — reject instead of silently proceeding on defaults.
        (k, s) => {
            return Err(finstack_core::Error::Validation(format!(
                "{}: partial calibrated HW1F scalars for curve '{}' \
                 ({kappa_key}={k:?}, {sigma_key}={s:?}); both must be present",
                req.context, req.curve_id
            )));
        }
    }

    // (4) Genuine fallback: no overrides, no surface, no calibrated scalars.
    tracing::warn!(
        target = "finstack.exotic_rates",
        context = req.context,
        kappa = defaults.kappa,
        sigma = defaults.sigma,
        hw1f_param_source = Hw1fParamSource::DefaultFallback.as_str(),
        "no HW1F overrides, volatility surface, or calibrated market scalars found; using fallback parameters"
    );
    Ok((defaults, Hw1fParamSource::DefaultFallback))
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_core::market_data::context::MarketContext;
    use finstack_core::market_data::scalars::MarketScalar;
    use finstack_core::market_data::surfaces::VolSurface;
    use serde_json::json;

    fn empty_market() -> MarketContext {
        MarketContext::new()
    }

    fn req<'a>(
        curve_id: &'a str,
        flavor: Hw1fCalibrationFlavor,
        overrides: Option<&'a serde_json::Value>,
    ) -> Hw1fResolveRequest<'a> {
        Hw1fResolveRequest {
            curve_id,
            flavor,
            overrides,
            surface: None,
            fallback: None,
            context: "test",
        }
    }

    #[test]
    fn overrides_are_used_when_present() {
        let overrides = json!({ "hw1f_kappa": 0.05, "hw1f_sigma": 0.012 });
        let (params, source) = resolve_hw1f_params(
            &req("USD-OIS", Hw1fCalibrationFlavor::Swaption, Some(&overrides)),
            &empty_market(),
        )
        .expect("ok");
        assert!((params.kappa - 0.05).abs() < 1e-12);
        assert!((params.sigma - 0.012).abs() < 1e-12);
        assert_eq!(source, Hw1fParamSource::Override);
    }

    #[test]
    fn defaults_when_nothing_provided() {
        let (params, source) = resolve_hw1f_params(
            &req("USD-OIS", Hw1fCalibrationFlavor::Swaption, None),
            &empty_market(),
        )
        .expect("ok");
        let default = HullWhiteParams::default();
        assert!((params.kappa - default.kappa).abs() < 1e-12);
        assert!((params.sigma - default.sigma).abs() < 1e-12);
        assert_eq!(source, Hw1fParamSource::DefaultFallback);
    }

    #[test]
    fn negative_kappa_errors() {
        let overrides = json!({ "hw1f_kappa": -0.05, "hw1f_sigma": 0.01 });
        let err = resolve_hw1f_params(
            &req("USD-OIS", Hw1fCalibrationFlavor::Swaption, Some(&overrides)),
            &empty_market(),
        )
        .expect_err("should error");
        assert!(format!("{err}").contains("hw1f_kappa"));
    }

    #[test]
    fn zero_sigma_errors() {
        // Note: JSON does not support NaN/Inf (serde_json drops them to Null), so
        // the `is_finite` branch is unreachable via JSON input. The positivity
        // check is exercised here with `sigma = 0.0`, which must still error.
        let overrides = json!({ "hw1f_kappa": 0.03, "hw1f_sigma": 0.0 });
        let err = resolve_hw1f_params(
            &req("USD-OIS", Hw1fCalibrationFlavor::Swaption, Some(&overrides)),
            &empty_market(),
        )
        .expect_err("should error");
        assert!(format!("{err}").contains("hw1f_sigma"));
    }

    #[test]
    fn partial_override_is_rejected() {
        let overrides = json!({ "hw1f_kappa": 0.07 });
        let err = resolve_hw1f_params(
            &req("USD-OIS", Hw1fCalibrationFlavor::Swaption, Some(&overrides)),
            &empty_market(),
        )
        .expect_err("partial override must be a hard error");
        let msg = format!("{err}");
        assert!(
            msg.contains("partial HW1F override"),
            "unexpected error: {msg}"
        );
    }

    #[test]
    fn calibrated_swaption_scalars_are_used() {
        let (kappa_key, sigma_key) = hw1f_scalar_keys("USD-OIS");
        let market = empty_market()
            .insert_price(&kappa_key, MarketScalar::Unitless(0.08))
            .insert_price(&sigma_key, MarketScalar::Unitless(0.015));
        let (params, source) = resolve_hw1f_params(
            &req("USD-OIS", Hw1fCalibrationFlavor::Swaption, None),
            &market,
        )
        .expect("ok");
        assert!((params.kappa - 0.08).abs() < 1e-12);
        assert!((params.sigma - 0.015).abs() < 1e-12);
        assert_eq!(source, Hw1fParamSource::MarketScalars);
    }

    #[test]
    fn calibrated_capfloor_scalars_are_used() {
        let (kappa_key, sigma_key) = capfloor_hw1f_scalar_keys("USD-OIS");
        let market = empty_market()
            .insert_price(&kappa_key, MarketScalar::Unitless(0.06))
            .insert_price(&sigma_key, MarketScalar::Unitless(0.009));
        let (params, source) = resolve_hw1f_params(
            &req("USD-OIS", Hw1fCalibrationFlavor::CapFloor, None),
            &market,
        )
        .expect("ok");
        assert!((params.kappa - 0.06).abs() < 1e-12);
        assert!((params.sigma - 0.009).abs() < 1e-12);
        assert_eq!(source, Hw1fParamSource::MarketScalars);
    }

    #[test]
    fn capfloor_surface_calibration_wins_over_calibrated_scalars() {
        let kappa = 0.05;
        let target_sigma = 0.02;
        let t_fix = 1.0;
        let accrual = 0.25;
        let strike = 0.04;
        let normal_vol = hw1f_caplet_forward_rate_normal_vol(kappa, target_sigma, t_fix, accrual);
        let surface = VolSurface::builder("USD-CAP-VOL")
            .expiries(&[t_fix])
            .strikes(&[strike])
            .row(&[normal_vol])
            .build()
            .expect("surface");
        let (kappa_key, sigma_key) = capfloor_hw1f_scalar_keys("USD-OIS");
        let market = empty_market()
            .insert_surface(surface)
            .insert_price(&kappa_key, MarketScalar::Unitless(kappa))
            .insert_price(&sigma_key, MarketScalar::Unitless(0.009));
        let points = [Hw1fCapletSurfacePoint {
            t_fix,
            accrual,
            strike,
            weight: 1.0,
        }];
        let request = Hw1fResolveRequest {
            curve_id: "USD-OIS",
            flavor: Hw1fCalibrationFlavor::CapFloor,
            overrides: None,
            surface: Some(Hw1fSurfaceCalibration::CapFloor {
                surface_id: "USD-CAP-VOL",
                points: &points,
            }),
            fallback: None,
            context: "surface-test",
        };

        let (params, source) = resolve_hw1f_params(&request, &market).expect("params");

        assert!((params.kappa - kappa).abs() < 1e-12);
        assert!((params.sigma - target_sigma).abs() < 1e-12);
        assert_eq!(source, Hw1fParamSource::CalibratedSurface);
    }

    #[test]
    fn capfloor_surface_shock_scales_resolved_sigma() {
        let kappa = 0.03;
        let base_sigma = 0.01;
        let t_fix = 2.0;
        let accrual = 0.5;
        let strike = 0.035;
        let base_normal_vol =
            hw1f_caplet_forward_rate_normal_vol(kappa, base_sigma, t_fix, accrual);
        let bumped_surface = VolSurface::builder("USD-CAP-VOL")
            .expiries(&[t_fix])
            .strikes(&[strike])
            .row(&[base_normal_vol * 1.25])
            .build()
            .expect("surface");
        let market = empty_market().insert_surface(bumped_surface);
        let points = [Hw1fCapletSurfacePoint {
            t_fix,
            accrual,
            strike,
            weight: 1.0,
        }];
        let request = Hw1fResolveRequest {
            curve_id: "USD-OIS",
            flavor: Hw1fCalibrationFlavor::CapFloor,
            overrides: None,
            surface: Some(Hw1fSurfaceCalibration::CapFloor {
                surface_id: "USD-CAP-VOL",
                points: &points,
            }),
            fallback: Some(HullWhiteParams::new(kappa, base_sigma).expect("fallback")),
            context: "surface-test",
        };

        let (params, source) = resolve_hw1f_params(&request, &market).expect("params");

        assert!((params.sigma - base_sigma * 1.25).abs() < 1e-12);
        assert_eq!(source, Hw1fParamSource::CalibratedSurface);
    }

    #[test]
    fn overrides_win_over_calibrated_scalars() {
        let (kappa_key, sigma_key) = hw1f_scalar_keys("USD-OIS");
        let market = empty_market()
            .insert_price(&kappa_key, MarketScalar::Unitless(0.08))
            .insert_price(&sigma_key, MarketScalar::Unitless(0.015));
        let overrides = json!({ "hw1f_kappa": 0.04, "hw1f_sigma": 0.011 });
        let (params, source) = resolve_hw1f_params(
            &req("USD-OIS", Hw1fCalibrationFlavor::Swaption, Some(&overrides)),
            &market,
        )
        .expect("ok");
        assert!((params.kappa - 0.04).abs() < 1e-12);
        assert!((params.sigma - 0.011).abs() < 1e-12);
        assert_eq!(source, Hw1fParamSource::Override);
    }

    #[test]
    fn flavor_keys_do_not_cross_over() {
        // Swaption-keyed scalars must NOT satisfy a cap/floor request.
        let (kappa_key, sigma_key) = hw1f_scalar_keys("USD-OIS");
        let market = empty_market()
            .insert_price(&kappa_key, MarketScalar::Unitless(0.08))
            .insert_price(&sigma_key, MarketScalar::Unitless(0.015));
        let (params, source) = resolve_hw1f_params(
            &req("USD-OIS", Hw1fCalibrationFlavor::CapFloor, None),
            &market,
        )
        .expect("ok");
        let default = HullWhiteParams::default();
        assert!((params.kappa - default.kappa).abs() < 1e-12);
        assert!((params.sigma - default.sigma).abs() < 1e-12);
        assert_eq!(source, Hw1fParamSource::DefaultFallback);
    }

    #[test]
    fn partial_calibrated_scalars_are_rejected() {
        let (kappa_key, _sigma_key) = hw1f_scalar_keys("USD-OIS");
        let market = empty_market().insert_price(&kappa_key, MarketScalar::Unitless(0.08));
        let err = resolve_hw1f_params(
            &req("USD-OIS", Hw1fCalibrationFlavor::Swaption, None),
            &market,
        )
        .expect_err("partial calibrated scalar pair must be a hard error");
        let msg = format!("{err}");
        assert!(
            msg.contains("partial calibrated HW1F scalars"),
            "unexpected error: {msg}"
        );
    }
}
