//! Valuation-side resolution of closed-form Heston parameters.

use crate::instruments::common_impl::helpers::get_unitless_scalar_strict;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_models::volatility::heston::HestonParams;

/// Resolve all required Heston scalars from a valuation market context.
///
/// Reads the unitless scalars `HESTON_KAPPA`, `HESTON_THETA`, `HESTON_SIGMA_V`,
/// `HESTON_RHO` and `HESTON_V0` into the canonical [`HestonParams`].
/// Each scalar is required and must be unitless.
///
/// # Arguments
///
/// * `market` - Market context holding the five `HESTON_*` unitless scalars.
///
/// # Errors
///
/// Returns `Error::Validation` when a scalar is missing or carries a unit, or
/// when [`HestonParams::new`] rejects the parameter set.
pub fn heston_params_from_market_strict(
    market: &MarketContext,
) -> finstack_quant_core::Result<HestonParams> {
    let kappa = get_unitless_scalar_strict(market, "HESTON_KAPPA", "Heston")?;
    let theta = get_unitless_scalar_strict(market, "HESTON_THETA", "Heston")?;
    let vol_of_vol = get_unitless_scalar_strict(market, "HESTON_SIGMA_V", "Heston")?;
    let rho = get_unitless_scalar_strict(market, "HESTON_RHO", "Heston")?;
    let v0 = get_unitless_scalar_strict(market, "HESTON_V0", "Heston")?;
    HestonParams::new(v0, kappa, theta, vol_of_vol, rho)
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::market_data::scalars::MarketScalar;

    #[test]
    fn strict_resolution_rejects_missing_heston_scalars() {
        let error = heston_params_from_market_strict(&MarketContext::new())
            .expect_err("missing Heston scalars must fail");
        assert!(error.to_string().contains("HESTON_KAPPA"));
    }

    #[test]
    fn strict_resolution_names_a_partially_missing_scalar_set() {
        let market = MarketContext::new()
            .insert_price(
                "HESTON_KAPPA",
                MarketScalar::unitless(1.5).expect("valid scalar"),
            )
            .insert_price(
                "HESTON_THETA",
                MarketScalar::unitless(0.06).expect("valid scalar"),
            )
            .insert_price(
                "HESTON_SIGMA_V",
                MarketScalar::unitless(0.4).expect("valid scalar"),
            )
            .insert_price(
                "HESTON_RHO",
                MarketScalar::unitless(-0.5).expect("valid scalar"),
            );
        let error =
            heston_params_from_market_strict(&market).expect_err("missing HESTON_V0 must fail");
        assert!(error.to_string().contains("HESTON_V0"));
    }

    #[test]
    fn strict_resolution_preserves_complete_market_parameters() {
        let market = MarketContext::new()
            .insert_price(
                "HESTON_KAPPA",
                MarketScalar::unitless(1.5).expect("valid scalar"),
            )
            .insert_price(
                "HESTON_THETA",
                MarketScalar::unitless(0.06).expect("valid scalar"),
            )
            .insert_price(
                "HESTON_SIGMA_V",
                MarketScalar::unitless(0.4).expect("valid scalar"),
            )
            .insert_price(
                "HESTON_RHO",
                MarketScalar::unitless(-0.5).expect("valid scalar"),
            )
            .insert_price(
                "HESTON_V0",
                MarketScalar::unitless(0.05).expect("valid scalar"),
            );
        let params =
            heston_params_from_market_strict(&market).expect("complete Heston inputs must resolve");
        assert_eq!(params.kappa, 1.5);
        assert_eq!(params.theta, 0.06);
        assert_eq!(params.vol_of_vol, 0.4);
        assert_eq!(params.rho, -0.5);
        assert_eq!(params.v0, 0.05);
    }
}
