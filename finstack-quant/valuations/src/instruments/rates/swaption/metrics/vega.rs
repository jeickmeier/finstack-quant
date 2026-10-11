//! Vega calculator for swaptions.
//!
//! Computes cash vega using Black or Normal model vega with forward swap rate and
//! underlying swap annuity. Uses SABR-implied vol if parameters are set,
//! otherwise uses the volatility surface or an override from `InstrumentPricingOverrides`.
//!
//! # Output Convention
//!
//! **Vega is expressed per 1% absolute volatility change (0.01 in decimal terms).**
//!
//! This means:
//! - If the swaption has vega = 50,000, then a 1% increase in volatility
//!   (e.g., from 20% to 21%) increases the option value by 50,000.
//! - For lognormal (Black) vol: 1% = 0.01 absolute change in σ_lognormal
//! - For normal (Bachelier) vol: 1% = 0.01 absolute change in σ_normal
//!
//! # Scaling Detail
//!
//! The raw vega formula gives sensitivity per unit vol change. We divide by 100
//! (`VOL_PCT_SCALE`) to express per 1% change, making the output more intuitive
//! for risk reports where volatility is often quoted in percentage terms.
//!
//! # Expiry convention
//!
//! Positive time retains the analytic model Greek, however short the horizon.
//! Expired contracts return zero. At zero volatility the shared model kernel
//! uses call delta one at the exact ATM kink (put delta zero), and zero gamma
//! and vega; these are finite reporting conventions at a nondifferentiable point.

use crate::instruments::common_impl::vol_resolution::ResolvedVolatility;
use crate::instruments::rates::swaption::Swaption;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_quant_core::Result;
use finstack_quant_models::closed_form::{bachelier_vega, black_vega};
use finstack_quant_models::volatility::VolatilityConvention;

/// Vega calculator for swaptions
pub(crate) struct VegaCalculator;

impl MetricCalculator for VegaCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let option: &Swaption = context.instrument_as()?;
        if context.pricing_model() == Some(crate::pricer::ModelKey::HullWhite1F) {
            return Err(finstack_quant_core::Error::Validation(
                "HullWhite1F swaption pricing consumes explicit short-rate parameters; use hw_sigma_vega, not Black surface vega".into(),
            ));
        }
        let strike = option.strike_f64()?;

        // Use consolidated helper to get pre-computed inputs
        let Some(inputs) = option.greek_inputs(&context.curves, context.as_of)? else {
            return Ok(0.0); // Option expired
        };

        let (forward, strike) = ResolvedVolatility {
            sigma: inputs.sigma,
            convention: inputs.convention,
        }
        .model_rates(inputs.forward, strike)?;
        let vega_raw = match inputs.convention {
            VolatilityConvention::Normal => {
                bachelier_vega(forward, strike, inputs.sigma, inputs.time_to_expiry)
            }
            _ => black_vega(forward, strike, inputs.sigma, inputs.time_to_expiry),
        };
        let vega = vega_raw / super::config::VOL_PCT_SCALE;
        // Scale by notional and annuity for cash vega
        Ok(vega * option.notional.amount() * inputs.annuity)
    }
}

/// Short-rate sigma sensitivity of the selected European HW model, per 0.01.
pub(crate) struct HwSigmaVegaCalculator;

impl MetricCalculator for HwSigmaVegaCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        use crate::instruments::rates::hw1f::{
            resolve_hw1f_params, Hw1fParamFamily, HW_SIGMA_BUMP,
        };
        if context.pricing_model() != Some(crate::pricer::ModelKey::HullWhite1F) {
            return Err(finstack_quant_core::Error::Validation(
                "hw_sigma_vega requires HullWhite1F pricing".into(),
            ));
        }
        let option: &Swaption = context.instrument_as()?;
        let params = resolve_hw1f_params(
            Hw1fParamFamily::Swaption,
            option.get_discount_curve_id().as_str(),
            &option.instrument_pricing_overrides.model_config,
            None,
            "swaption sigma vega",
            &context.curves,
        )?;
        let h = HW_SIGMA_BUMP.min(params.sigma * 0.5);
        if h <= 0.0 {
            return Err(finstack_quant_core::Error::Validation(
                "hw_sigma_vega requires positive sigma".into(),
            ));
        }
        let price = |shift: f64| {
            let mut bumped = option.clone();
            bumped
                .instrument_pricing_overrides
                .model_config
                .hw1f_mean_reversion = Some(params.kappa);
            bumped.instrument_pricing_overrides.model_config.hw1f_sigma =
                Some(params.sigma + shift);
            context.reprice_instrument_raw(&bumped, &context.curves, context.as_of)
        };
        Ok((price(h)? - price(-h)?) / (2.0 * h) * 0.01)
    }
}
