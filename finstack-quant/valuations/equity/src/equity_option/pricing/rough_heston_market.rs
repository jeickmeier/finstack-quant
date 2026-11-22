//! Shared rough-Heston market scalar lookup.
//!
//! Both `RoughHestonFourier` and `MonteCarloRoughHeston` pricers source the
//! same required `ROUGH_HESTON_*` market scalars through this module.

use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_valuations_core::instruments::pricing::market_inputs::get_unitless_scalar_strict;

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
    pub vol_of_vol: f64,
    /// Spot/variance correlation.
    pub rho: f64,
    /// Hurst exponent.
    pub hurst: f64,
}

impl RoughHestonScalars {
    /// Resolve every required `ROUGH_HESTON_*` scalar from the market.
    ///
    /// Missing or non-unitless values are rejected so model calibration cannot
    /// silently fall back to representative parameters.
    pub fn from_market_strict(market: &MarketContext) -> finstack_quant_core::Result<Self> {
        Ok(Self {
            v0: get_unitless_scalar_strict(market, "ROUGH_HESTON_V0", "rough Heston")?,
            kappa: get_unitless_scalar_strict(market, "ROUGH_HESTON_KAPPA", "rough Heston")?,
            theta: get_unitless_scalar_strict(market, "ROUGH_HESTON_THETA", "rough Heston")?,
            vol_of_vol: get_unitless_scalar_strict(market, "ROUGH_HESTON_SIGMA_V", "rough Heston")?,
            rho: get_unitless_scalar_strict(market, "ROUGH_HESTON_RHO", "rough Heston")?,
            hurst: get_unitless_scalar_strict(market, "ROUGH_HESTON_HURST", "rough Heston")?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::market_data::scalars::MarketScalar;

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
            .insert_price(
                "ROUGH_HESTON_V0",
                MarketScalar::unitless(0.05).expect("valid scalar"),
            )
            .insert_price(
                "ROUGH_HESTON_KAPPA",
                MarketScalar::unitless(1.5).expect("valid scalar"),
            )
            .insert_price(
                "ROUGH_HESTON_THETA",
                MarketScalar::unitless(0.06).expect("valid scalar"),
            )
            .insert_price(
                "ROUGH_HESTON_SIGMA_V",
                MarketScalar::unitless(0.4).expect("valid scalar"),
            )
            .insert_price(
                "ROUGH_HESTON_RHO",
                MarketScalar::unitless(-0.5).expect("valid scalar"),
            )
            .insert_price(
                "ROUGH_HESTON_HURST",
                MarketScalar::unitless(0.08).expect("valid scalar"),
            );

        let s = RoughHestonScalars::from_market_strict(&market).expect("strict config");
        assert_eq!(s.v0, 0.05);
        assert_eq!(s.kappa, 1.5);
        assert_eq!(s.theta, 0.06);
        assert_eq!(s.vol_of_vol, 0.4);
        assert_eq!(s.rho, -0.5);
        assert_eq!(s.hurst, 0.08);
    }
}
