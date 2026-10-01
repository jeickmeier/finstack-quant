//! Delta calculator for swaptions.
//!
//! Computes cash delta using Black or Normal model greeks with forward swap rate and
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
//! The annuity is taken from `greek_inputs` as a constant. For physically
//! settled swaptions the annuity is a separate numeraire and this is exact
//! (Black-76 in the annuity measure). For **cash-settled ParYield**
//! settlement, the cash annuity `A(F)` is itself a function of the forward
//! swap rate, so the true delta carries an extra `A'(F)·V/A` term that this
//! analytic calculator drops (the "frozen annuity" approximation, standard
//! but increasingly inaccurate for long tails / high vol). Use
//! bump-and-revalue (e.g. `Dv01`) where the A'(F) contribution matters.

use crate::instruments::common_impl::parameters::OptionType;
use crate::instruments::common_impl::vol_resolution::ResolvedVolatility;
use crate::instruments::rates::swaption::Swaption;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_quant_core::Result;
use finstack_quant_models::closed_form::{
    bachelier_delta_call, bachelier_delta_put, black_delta_call, black_delta_put,
};
use finstack_quant_models::volatility::VolatilityConvention;

/// Delta calculator for swaptions
pub(crate) struct DeltaCalculator;

impl MetricCalculator for DeltaCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let option: &Swaption = context.instrument_as()?;
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
        let delta = match (inputs.convention, option.option_type) {
            (VolatilityConvention::Normal, OptionType::Call) => {
                bachelier_delta_call(forward, strike, inputs.sigma, inputs.time_to_expiry)
            }
            (VolatilityConvention::Normal, OptionType::Put) => {
                bachelier_delta_put(forward, strike, inputs.sigma, inputs.time_to_expiry)
            }
            (_, OptionType::Call) => {
                black_delta_call(forward, strike, inputs.sigma, inputs.time_to_expiry)
            }
            (_, OptionType::Put) => {
                black_delta_put(forward, strike, inputs.sigma, inputs.time_to_expiry)
            }
        };

        // Scale by notional and annuity for cash delta
        Ok(delta * option.notional.amount() * inputs.annuity)
    }
}
