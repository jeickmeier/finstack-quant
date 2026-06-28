//! Shared rough-Heston market scalar lookup.
//!
//! Both `RoughHestonFourier` and `MonteCarloRoughHeston` pricers source the
//! same set of `ROUGH_HESTON_*` market scalars and fall back to the same
//! defaults. This module is the single source of truth so the two pricers
//! cannot drift.

use crate::instruments::common_impl::helpers::{get_unitless_scalar, get_unitless_scalar_strict};
use finstack_quant_core::market_data::context::MarketContext;

/// Default rough-Heston parameters used when no market scalar is supplied.
#[allow(dead_code)]
pub(crate) mod rough_heston_defaults {
    /// Default initial variance (v₀).
    pub const V0: f64 = 0.04;
    /// Default mean reversion speed of variance (κ).
    pub const KAPPA: f64 = 2.0;
    /// Default long-run variance level (θ).
    pub const THETA: f64 = 0.04;
    /// Default vol-of-vol (σᵥ).
    pub const SIGMA_V: f64 = 0.3;
    /// Default spot/variance correlation (ρ); negative for equity (leverage effect).
    pub const RHO: f64 = -0.7;
    /// Default Hurst exponent (H); rough volatility uses H ≪ 0.5.
    pub const HURST: f64 = 0.1;
}

/// Bundle of rough-Heston parameters resolved from a market context.
#[derive(Debug, Clone, Copy)]
pub struct RoughHestonScalars {
    /// Initial variance.
    pub v0: f64,
    /// Mean reversion speed of variance.
    pub kappa: f64,
    /// Long-run variance level.
    pub theta: f64,
    /// Vol-of-vol.
    pub sigma_v: f64,
    /// Spot/variance correlation.
    pub rho: f64,
    /// Hurst exponent.
    pub hurst: f64,
}

impl RoughHestonScalars {
    /// Read rough-Heston scalars from the market, falling back to
    /// [`rough_heston_defaults`] for any missing value. No validation is done
    /// here; downstream constructors (`RoughHestonFourierParams::new`,
    /// `RoughHestonParams::new`, `HurstExponent::new`) enforce numerical
    /// invariants.
    #[allow(dead_code)]
    pub(crate) fn from_market(market: &MarketContext) -> Self {
        Self {
            v0: get_unitless_scalar(market, "ROUGH_HESTON_V0", rough_heston_defaults::V0),
            kappa: get_unitless_scalar(market, "ROUGH_HESTON_KAPPA", rough_heston_defaults::KAPPA),
            theta: get_unitless_scalar(market, "ROUGH_HESTON_THETA", rough_heston_defaults::THETA),
            sigma_v: get_unitless_scalar(
                market,
                "ROUGH_HESTON_SIGMA_V",
                rough_heston_defaults::SIGMA_V,
            ),
            rho: get_unitless_scalar(market, "ROUGH_HESTON_RHO", rough_heston_defaults::RHO),
            hurst: get_unitless_scalar(market, "ROUGH_HESTON_HURST", rough_heston_defaults::HURST),
        }
    }

    /// Strict variant of [`Self::from_market`] that errors when any
    /// `ROUGH_HESTON_*` scalar is missing or is not unitless.
    ///
    /// Production rough-Heston pricers use this form so missing model
    /// calibration cannot silently fall back to representative defaults.
    pub fn from_market_strict(market: &MarketContext) -> finstack_quant_core::Result<Self> {
        Ok(Self {
            v0: get_unitless_scalar_strict(market, "ROUGH_HESTON_V0", "rough Heston")?,
            kappa: get_unitless_scalar_strict(market, "ROUGH_HESTON_KAPPA", "rough Heston")?,
            theta: get_unitless_scalar_strict(market, "ROUGH_HESTON_THETA", "rough Heston")?,
            sigma_v: get_unitless_scalar_strict(market, "ROUGH_HESTON_SIGMA_V", "rough Heston")?,
            rho: get_unitless_scalar_strict(market, "ROUGH_HESTON_RHO", "rough Heston")?,
            hurst: get_unitless_scalar_strict(market, "ROUGH_HESTON_HURST", "rough Heston")?,
        })
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use finstack_quant_core::market_data::scalars::MarketScalar;

    #[test]
    fn from_market_uses_defaults_when_market_is_empty() {
        let market = MarketContext::new();
        let s = RoughHestonScalars::from_market(&market);
        assert_eq!(s.v0, rough_heston_defaults::V0);
        assert_eq!(s.kappa, rough_heston_defaults::KAPPA);
        assert_eq!(s.theta, rough_heston_defaults::THETA);
        assert_eq!(s.sigma_v, rough_heston_defaults::SIGMA_V);
        assert_eq!(s.rho, rough_heston_defaults::RHO);
        assert_eq!(s.hurst, rough_heston_defaults::HURST);
    }

    #[test]
    fn from_market_overrides_defaults_with_market_scalars() {
        let market = MarketContext::new()
            .insert_price("ROUGH_HESTON_HURST", MarketScalar::Unitless(0.05))
            .insert_price("ROUGH_HESTON_KAPPA", MarketScalar::Unitless(1.5));
        let s = RoughHestonScalars::from_market(&market);
        assert_eq!(s.hurst, 0.05);
        assert_eq!(s.kappa, 1.5);
        assert_eq!(s.theta, rough_heston_defaults::THETA); // untouched
    }

    #[test]
    fn from_market_strict_errors_when_any_scalar_is_missing() {
        let market = MarketContext::new();
        let err = RoughHestonScalars::from_market_strict(&market)
            .expect_err("strict resolver must reject missing rough-Heston scalars");
        let msg = err.to_string();
        assert!(
            msg.contains("ROUGH_HESTON_V0") && msg.contains("rough Heston"),
            "unexpected error message: {msg}"
        );
    }

    #[test]
    fn from_market_strict_succeeds_when_full_config_present() {
        let market = MarketContext::new()
            .insert_price("ROUGH_HESTON_V0", MarketScalar::Unitless(0.05))
            .insert_price("ROUGH_HESTON_KAPPA", MarketScalar::Unitless(1.5))
            .insert_price("ROUGH_HESTON_THETA", MarketScalar::Unitless(0.06))
            .insert_price("ROUGH_HESTON_SIGMA_V", MarketScalar::Unitless(0.4))
            .insert_price("ROUGH_HESTON_RHO", MarketScalar::Unitless(-0.5))
            .insert_price("ROUGH_HESTON_HURST", MarketScalar::Unitless(0.08));

        let s = RoughHestonScalars::from_market_strict(&market).expect("strict config");
        assert_eq!(s.v0, 0.05);
        assert_eq!(s.kappa, 1.5);
        assert_eq!(s.theta, 0.06);
        assert_eq!(s.sigma_v, 0.4);
        assert_eq!(s.rho, -0.5);
        assert_eq!(s.hurst, 0.08);
    }
}
