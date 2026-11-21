//! Resolve already-fitted HW1F short-rate parameters for valuation.
//!
//! Pricing accepts either a complete explicit `(kappa, sigma)` override, a
//! complete pair supplied by the pricer, or a complete pair written to the
//! market by a prior calibration step. It never reads a volatility surface,
//! performs a fit, or substitutes model defaults.

use crate::instruments::pricing_overrides::ModelConfig;
use finstack_quant_core::Result;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::MarketScalar;
use finstack_quant_models::rates::hull_white::{
    HullWhiteCalibrationParams, capfloor_hw1f_scalar_keys, hw1f_scalar_keys,
};

/// Parameter family that determines the market-scalar key convention.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hw1fParamFamily {
    /// Parameters fitted to a swaption volatility grid.
    Swaption,
    /// Parameters fitted to a cap/floor volatility strip.
    CapFloor,
}

impl Hw1fParamFamily {
    fn scalar_keys(self, curve_id: &str) -> (String, String) {
        match self {
            Self::Swaption => hw1f_scalar_keys(curve_id),
            Self::CapFloor => capfloor_hw1f_scalar_keys(curve_id),
        }
    }
}

fn scalar_as_unitless_positive_f64(scalar: &MarketScalar, key: &str, context: &str) -> Result<f64> {
    let Some(value) = scalar.get_unitless() else {
        return Err(finstack_quant_core::Error::Validation(format!(
            "{context}: calibrated HW1F scalar '{key}' must be unitless, got Money"
        )));
    };
    if value.is_finite() && value > 0.0 {
        Ok(value)
    } else {
        Err(finstack_quant_core::Error::Validation(format!(
            "{context}: calibrated HW1F scalar '{key}' must be positive and finite, got {value}"
        )))
    }
}

fn override_positive_f64(value: Option<f64>, key: &str) -> Result<Option<f64>> {
    match value {
        None => Ok(None),
        Some(value) if value.is_finite() && value > 0.0 => Ok(Some(value)),
        Some(value) => Err(finstack_quant_core::Error::Validation(format!(
            "{key} override must be positive and finite, got {value}"
        ))),
    }
}

/// Resolve a complete HW1F parameter pair without fitting during pricing.
///
/// Precedence: a complete `(hw1f_mean_reversion, hw1f_sigma)` pair on
/// `config` wins; a partial pair is an error. Otherwise `fallback` (a pair the
/// pricer was constructed with) is used when present; otherwise the complete
/// pre-fitted market scalar pair keyed by `family` and `curve_id`.
///
/// # Arguments
///
/// * `family` - Scalar-key convention (swaption grid or cap/floor strip fit).
/// * `curve_id` - Curve identifier used by the fitting step when storing scalars.
/// * `config` - Instrument model configuration carrying the optional
///   `hw1f_mean_reversion` (κ, inverse years) and `hw1f_sigma` (short-rate
///   absolute volatility, annual decimal) overrides.
/// * `fallback` - Optional complete pair supplied by the pricer, consulted
///   only when `config` sets neither override.
/// * `context` - Instrument or pricing-path label included in validation errors.
/// * `market` - Pre-calibrated market that may contain the complete scalar pair.
///
/// # Errors
///
/// Returns a validation error for invalid, partial, or missing parameters, or
/// a volatility schedule supplied to a scalar-only pricing path.
pub fn resolve_hw1f_params(
    family: Hw1fParamFamily,
    curve_id: &str,
    config: &ModelConfig,
    fallback: Option<HullWhiteCalibrationParams>,
    context: &str,
    market: &MarketContext,
) -> Result<HullWhiteCalibrationParams> {
    if config.hw1f_sigma_schedule.is_some() {
        return Err(finstack_quant_core::Error::Validation(format!(
            "{context}: this Hull-White pricing path requires scalar volatility; instrument_pricing_overrides.model_config.hw1f_sigma_schedule requires a schedule-capable engine"
        )));
    }
    let override_kappa = override_positive_f64(
        config.hw1f_mean_reversion,
        "instrument_pricing_overrides.model_config.hw1f_mean_reversion",
    )?;
    let override_sigma = override_positive_f64(
        config.hw1f_sigma,
        "instrument_pricing_overrides.model_config.hw1f_sigma",
    )?;
    match (override_kappa, override_sigma) {
        (Some(kappa), Some(sigma)) => return HullWhiteCalibrationParams::new(kappa, sigma),
        (None, None) => {}
        (kappa, sigma) => {
            return Err(finstack_quant_core::Error::Validation(format!(
                "{context}: partial HW1F override (instrument_pricing_overrides.model_config.hw1f_mean_reversion={kappa:?}, \
                 instrument_pricing_overrides.model_config.hw1f_sigma={sigma:?}); supply both positive finite parameters"
            )));
        }
    }
    if let Some(params) = fallback {
        return HullWhiteCalibrationParams::new(params.kappa, params.sigma);
    }

    let (kappa_key, sigma_key) = family.scalar_keys(curve_id);
    let kappa = market
        .get_price(&kappa_key)
        .ok()
        .map(|scalar| scalar_as_unitless_positive_f64(scalar, &kappa_key, context))
        .transpose()?;
    let sigma = market
        .get_price(&sigma_key)
        .ok()
        .map(|scalar| scalar_as_unitless_positive_f64(scalar, &sigma_key, context))
        .transpose()?;
    match (kappa, sigma) {
        (Some(kappa), Some(sigma)) => HullWhiteCalibrationParams::new(kappa, sigma),
        (None, None) => Err(finstack_quant_core::Error::Validation(format!(
            "{context}: missing HW1F parameters for curve '{curve_id}'; provide both \
             instrument_pricing_overrides.model_config.hw1f_mean_reversion and instrument_pricing_overrides.model_config.hw1f_sigma or pre-calibrate market scalars '{kappa_key}' and '{sigma_key}'"
        ))),
        (kappa, sigma) => Err(finstack_quant_core::Error::Validation(format!(
            "{context}: partial calibrated HW1F scalars for curve '{curve_id}' \
             ({kappa_key}={kappa:?}, {sigma_key}={sigma:?}); both must be present"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scalar_hw_resolver_does_not_ignore_a_volatility_schedule() {
        let config = ModelConfig {
            hw1f_mean_reversion: Some(0.05),
            hw1f_sigma: Some(0.01),
            hw1f_sigma_schedule: Some(
                finstack_quant_core::math::piecewise::PiecewiseConstantCurve::new(
                    vec![0.0, 1.0],
                    vec![0.01, 0.02],
                )
                .expect("schedule"),
            ),
            ..Default::default()
        };
        assert!(resolve(&config, None, &MarketContext::new()).is_err());
    }

    fn resolve(
        config: &ModelConfig,
        fallback: Option<HullWhiteCalibrationParams>,
        market: &MarketContext,
    ) -> Result<HullWhiteCalibrationParams> {
        resolve_hw1f_params(
            Hw1fParamFamily::Swaption,
            "USD-OIS",
            config,
            fallback,
            "test",
            market,
        )
    }

    #[test]
    fn complete_override_is_used() {
        let config = ModelConfig {
            hw1f_mean_reversion: Some(0.05),
            hw1f_sigma: Some(0.012),
            ..Default::default()
        };
        let params = resolve(&config, None, &MarketContext::new()).expect("complete override");
        assert_eq!(
            params,
            HullWhiteCalibrationParams::new(0.05, 0.012).expect("valid")
        );
    }

    #[test]
    fn missing_parameters_are_rejected() {
        let error = resolve(&ModelConfig::default(), None, &MarketContext::new())
            .expect_err("missing parameters");
        assert!(error.to_string().contains("missing HW1F parameters"));
    }

    #[test]
    fn partial_override_is_rejected() {
        let config = ModelConfig {
            hw1f_mean_reversion: Some(0.05),
            ..Default::default()
        };
        let error = resolve(&config, None, &MarketContext::new()).expect_err("partial override");
        assert!(error.to_string().contains("partial HW1F override"));
    }

    #[test]
    fn fallback_is_used_when_config_is_silent() {
        let fallback = HullWhiteCalibrationParams::new(0.02, 0.007).expect("valid");
        let params = resolve(
            &ModelConfig::default(),
            Some(fallback),
            &MarketContext::new(),
        )
        .expect("fallback pair");
        assert_eq!(params, fallback);
    }

    #[test]
    fn complete_market_pair_is_used() {
        let (kappa_key, sigma_key) = hw1f_scalar_keys("USD-OIS");
        let market = MarketContext::new()
            .insert_price(
                kappa_key,
                MarketScalar::unitless(0.04).expect("valid scalar"),
            )
            .insert_price(
                sigma_key,
                MarketScalar::unitless(0.009).expect("valid scalar"),
            );
        let params = resolve(&ModelConfig::default(), None, &market).expect("complete market pair");
        assert_eq!(
            params,
            HullWhiteCalibrationParams::new(0.04, 0.009).expect("valid")
        );
    }

    #[test]
    fn money_and_non_positive_market_scalars_are_rejected_explicitly() {
        use finstack_quant_core::currency::Currency;
        use finstack_quant_core::money::Money;

        let (kappa_key, sigma_key) = hw1f_scalar_keys("USD-OIS");
        let price_market = MarketContext::new()
            .insert_price(
                &kappa_key,
                MarketScalar::price(Money::from((1_i64, Currency::USD))),
            )
            .insert_price(&sigma_key, MarketScalar::unitless(0.01).expect("valid"));
        let price_err = resolve(&ModelConfig::default(), None, &price_market)
            .expect_err("Money scalar must be rejected");
        assert!(price_err.to_string().contains("must be unitless"));

        for bad in [0.0, -0.01] {
            let mut bad_market = MarketContext::new()
                .insert_price(&kappa_key, MarketScalar::unitless(0.03).expect("valid"));
            bad_market = bad_market.insert_price(
                &sigma_key,
                MarketScalar::unitless(bad).expect("finite unitless scalar"),
            );
            let err = resolve(&ModelConfig::default(), None, &bad_market)
                .expect_err("non-positive scalar must fail explicitly");
            assert!(err.to_string().contains("must be positive and finite"));
        }

        let bad_fallback = HullWhiteCalibrationParams {
            kappa: -0.05,
            sigma: 0.01,
        };
        let fallback_err = resolve(
            &ModelConfig::default(),
            Some(bad_fallback),
            &MarketContext::new(),
        )
        .expect_err("invalid fallback pair must fail");
        assert!(!fallback_err.to_string().is_empty());
    }
}
