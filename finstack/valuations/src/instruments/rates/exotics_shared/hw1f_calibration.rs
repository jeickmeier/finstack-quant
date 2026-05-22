//! Resolve HW1F short-rate parameters (κ, σ) for exotic rate products.
//!
//! Precedence:
//! 1. Explicit overrides in `PricingOverrides.model_config` (keys `hw1f_kappa`, `hw1f_sigma`).
//! 2. Pre-calibrated parameters read from the `MarketContext` scalar store.
//!    A prior Hull-White calibration step (`StepParams::HullWhite` /
//!    `StepParams::CapFloorHullWhite`) writes solved κ/σ as named scalars under
//!    the keys produced by
//!    [`hw1f_scalar_keys`](crate::calibration::hull_white::hw1f_scalar_keys) /
//!    [`capfloor_hw1f_scalar_keys`](crate::calibration::hull_white::capfloor_hw1f_scalar_keys).
//!    When both scalars are present and valid for the request's `curve_id`,
//!    the resolver returns those market-consistent parameters.
//! 3. `HullWhiteParams::default()` when neither overrides nor calibrated market
//!    scalars are available, with a `tracing::warn!` log.

use crate::calibration::hull_white::{
    capfloor_hw1f_scalar_keys, hw1f_scalar_keys, HullWhiteParams,
};
use finstack_core::market_data::context::MarketContext;
use finstack_core::market_data::scalars::MarketScalar;
use finstack_core::Result;

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

/// Resolve HW1F parameters following the documented precedence.
///
/// Never returns an error for the "no overrides + no calibrated scalars" case;
/// instead emits a `tracing::warn!` and returns `HullWhiteParams::default()`.
/// An error is only returned when overrides are malformed.
///
/// `market` is consulted for pre-calibrated κ/σ scalars (precedence step 2)
/// when no explicit overrides are supplied.
pub fn resolve_hw1f_params(
    req: &Hw1fResolveRequest<'_>,
    market: &MarketContext,
) -> Result<HullWhiteParams> {
    // (1) Explicit pricing overrides.
    if let Some(obj) = req.overrides.and_then(|v| v.as_object()) {
        let kappa = obj.get("hw1f_kappa").and_then(|x| x.as_f64());
        let sigma = obj.get("hw1f_sigma").and_then(|x| x.as_f64());
        if let (Some(k), Some(s)) = (kappa, sigma) {
            if !k.is_finite() || k <= 0.0 {
                return Err(finstack_core::Error::Validation(format!(
                    "hw1f_kappa override must be positive and finite, got {k}"
                )));
            }
            if !s.is_finite() || s <= 0.0 {
                return Err(finstack_core::Error::Validation(format!(
                    "hw1f_sigma override must be positive and finite, got {s}"
                )));
            }
            return HullWhiteParams::new(k, s);
        }
    }

    // (2) Pre-calibrated parameters from the MarketContext scalar store.
    let (kappa_key, sigma_key) = req.flavor.scalar_keys(req.curve_id);
    let kappa = market
        .get_price(&kappa_key)
        .ok()
        .and_then(scalar_as_positive_f64);
    let sigma = market
        .get_price(&sigma_key)
        .ok()
        .and_then(scalar_as_positive_f64);
    if let (Some(k), Some(s)) = (kappa, sigma) {
        tracing::debug!(
            target = "finstack.exotic_rates",
            context = req.context,
            curve_id = req.curve_id,
            kappa = k,
            sigma = s,
            "resolved HW1F parameters from calibrated MarketContext scalars"
        );
        return HullWhiteParams::new(k, s);
    }

    // (3) Genuine fallback: no overrides, no calibrated scalars.
    let defaults = HullWhiteParams::default();
    tracing::warn!(
        target = "finstack.exotic_rates",
        context = req.context,
        kappa = defaults.kappa,
        sigma = defaults.sigma,
        "no HW1F overrides or calibrated market scalars found; using HullWhiteParams::default()"
    );
    Ok(defaults)
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_core::market_data::context::MarketContext;
    use finstack_core::market_data::scalars::MarketScalar;
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
            context: "test",
        }
    }

    #[test]
    fn overrides_are_used_when_present() {
        let overrides = json!({ "hw1f_kappa": 0.05, "hw1f_sigma": 0.012 });
        let params = resolve_hw1f_params(
            &req("USD-OIS", Hw1fCalibrationFlavor::Swaption, Some(&overrides)),
            &empty_market(),
        )
        .expect("ok");
        assert!((params.kappa - 0.05).abs() < 1e-12);
        assert!((params.sigma - 0.012).abs() < 1e-12);
    }

    #[test]
    fn defaults_when_nothing_provided() {
        let params = resolve_hw1f_params(
            &req("USD-OIS", Hw1fCalibrationFlavor::Swaption, None),
            &empty_market(),
        )
        .expect("ok");
        let default = HullWhiteParams::default();
        assert!((params.kappa - default.kappa).abs() < 1e-12);
        assert!((params.sigma - default.sigma).abs() < 1e-12);
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
    fn partial_override_falls_through_to_default() {
        let overrides = json!({ "hw1f_kappa": 0.07 });
        let params = resolve_hw1f_params(
            &req("USD-OIS", Hw1fCalibrationFlavor::Swaption, Some(&overrides)),
            &empty_market(),
        )
        .expect("ok");
        let default = HullWhiteParams::default();
        assert!((params.kappa - default.kappa).abs() < 1e-12);
        assert!((params.sigma - default.sigma).abs() < 1e-12);
    }

    #[test]
    fn calibrated_swaption_scalars_are_used() {
        let (kappa_key, sigma_key) = hw1f_scalar_keys("USD-OIS");
        let market = empty_market()
            .insert_price(&kappa_key, MarketScalar::Unitless(0.08))
            .insert_price(&sigma_key, MarketScalar::Unitless(0.015));
        let params = resolve_hw1f_params(
            &req("USD-OIS", Hw1fCalibrationFlavor::Swaption, None),
            &market,
        )
        .expect("ok");
        assert!((params.kappa - 0.08).abs() < 1e-12);
        assert!((params.sigma - 0.015).abs() < 1e-12);
    }

    #[test]
    fn calibrated_capfloor_scalars_are_used() {
        let (kappa_key, sigma_key) = capfloor_hw1f_scalar_keys("USD-OIS");
        let market = empty_market()
            .insert_price(&kappa_key, MarketScalar::Unitless(0.06))
            .insert_price(&sigma_key, MarketScalar::Unitless(0.009));
        let params = resolve_hw1f_params(
            &req("USD-OIS", Hw1fCalibrationFlavor::CapFloor, None),
            &market,
        )
        .expect("ok");
        assert!((params.kappa - 0.06).abs() < 1e-12);
        assert!((params.sigma - 0.009).abs() < 1e-12);
    }

    #[test]
    fn overrides_win_over_calibrated_scalars() {
        let (kappa_key, sigma_key) = hw1f_scalar_keys("USD-OIS");
        let market = empty_market()
            .insert_price(&kappa_key, MarketScalar::Unitless(0.08))
            .insert_price(&sigma_key, MarketScalar::Unitless(0.015));
        let overrides = json!({ "hw1f_kappa": 0.04, "hw1f_sigma": 0.011 });
        let params = resolve_hw1f_params(
            &req("USD-OIS", Hw1fCalibrationFlavor::Swaption, Some(&overrides)),
            &market,
        )
        .expect("ok");
        assert!((params.kappa - 0.04).abs() < 1e-12);
        assert!((params.sigma - 0.011).abs() < 1e-12);
    }

    #[test]
    fn flavor_keys_do_not_cross_over() {
        // Swaption-keyed scalars must NOT satisfy a cap/floor request.
        let (kappa_key, sigma_key) = hw1f_scalar_keys("USD-OIS");
        let market = empty_market()
            .insert_price(&kappa_key, MarketScalar::Unitless(0.08))
            .insert_price(&sigma_key, MarketScalar::Unitless(0.015));
        let params = resolve_hw1f_params(
            &req("USD-OIS", Hw1fCalibrationFlavor::CapFloor, None),
            &market,
        )
        .expect("ok");
        let default = HullWhiteParams::default();
        assert!((params.kappa - default.kappa).abs() < 1e-12);
        assert!((params.sigma - default.sigma).abs() < 1e-12);
    }

    #[test]
    fn partial_calibrated_scalars_fall_through_to_default() {
        let (kappa_key, _sigma_key) = hw1f_scalar_keys("USD-OIS");
        let market = empty_market().insert_price(&kappa_key, MarketScalar::Unitless(0.08));
        let params = resolve_hw1f_params(
            &req("USD-OIS", Hw1fCalibrationFlavor::Swaption, None),
            &market,
        )
        .expect("ok");
        let default = HullWhiteParams::default();
        assert!((params.kappa - default.kappa).abs() < 1e-12);
        assert!((params.sigma - default.sigma).abs() < 1e-12);
    }
}
