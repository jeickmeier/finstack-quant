//! Gamma calculator for swaptions.
//!
//! Computes cash gamma using Black or Normal model gamma with forward swap rate and
//! underlying swap annuity. Uses SABR-implied vol if parameters are set,
//! otherwise uses the volatility surface or an override from `InstrumentPricingOverrides`.
//!
//! # Expiry convention
//!
//! Positive time retains the analytic model Greek, however short the horizon.
//! Expired contracts return zero. At zero volatility the shared model kernel
//! uses call delta one at the exact ATM kink (put delta zero), and zero gamma
//! and vega; these are finite reporting conventions at a nondifferentiable point.

//! # Cash-settled (ParYield) limitation — frozen annuity
//!
//! As with delta, the annuity is treated as a constant. For cash-settled
//! ParYield swaptions the cash annuity `A(F)` depends on the forward swap
//! rate, so the true gamma carries additional `A'(F)` / `A''(F)` terms that
//! this analytic calculator drops (frozen-annuity approximation). Use
//! bump-and-revalue greeks where those terms matter.

use crate::instruments::common_impl::vol_resolution::ResolvedVolatility;
use crate::instruments::rates::swaption::Swaption;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_quant_core::Result;
use finstack_quant_models::closed_form::{bachelier_gamma, black_gamma};
use finstack_quant_models::volatility::VolatilityConvention;

/// Gamma calculator for swaptions
pub(crate) struct GammaCalculator;

impl MetricCalculator for GammaCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let option: &Swaption = context.instrument_as()?;
        let strike = option.strike_f64()?;

        // Use consolidated helper to get pre-computed inputs
        let Some(inputs) = option.greek_inputs(&context.curves, context.as_of)? else {
            return Ok(0.0);
        };

        let (forward, strike) = ResolvedVolatility {
            sigma: inputs.sigma,
            convention: inputs.convention,
        }
        .model_rates(inputs.forward, strike)?;
        let gamma = match inputs.convention {
            VolatilityConvention::Normal => {
                bachelier_gamma(forward, strike, inputs.sigma, inputs.time_to_expiry)
            }
            _ => black_gamma(forward, strike, inputs.sigma, inputs.time_to_expiry),
        };

        // Scale by notional and annuity for cash gamma
        Ok(gamma * option.notional.amount() * inputs.annuity)
    }
}
