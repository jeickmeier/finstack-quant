//! Delta calculator for swaptions.
//!
//! Computes cash delta using Black or Normal model greeks with forward swap rate and
//! underlying swap annuity. Uses SABR-implied vol if parameters are set,
//! otherwise uses the volatility surface or an override from `InstrumentPricingOverrides`.
//!
//! # Numerical Stability
//!
//! Although delta doesn't involve division by sqrt(T) (unlike gamma), the d1
//! calculation can become numerically unstable near expiry. We apply a
//! near-expiry threshold for consistency and to return intrinsic delta.
//!
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

/// Minimum time to expiry (in years) for Black/Normal model delta.
///
/// Below this threshold, return intrinsic delta (1 for ITM call, -1 for ITM put,
/// 0 for OTM) for consistency with gamma/vega behavior near expiry.
const EXPIRY_THRESHOLD: f64 = 1.0 / 252.0;

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

        // Near-expiry guard: return intrinsic delta when within ~1 business day of expiry.
        // This avoids d1 instability and is economically meaningful (binary ITM/OTM).
        if inputs.time_to_expiry < EXPIRY_THRESHOLD {
            let intrinsic_delta = match option.option_type {
                OptionType::Call => {
                    if inputs.forward > strike {
                        1.0
                    } else {
                        0.0
                    }
                }
                OptionType::Put => {
                    if inputs.forward < strike {
                        -1.0
                    } else {
                        0.0
                    }
                }
            };
            return Ok(intrinsic_delta * option.notional.amount() * inputs.annuity);
        }

        let (forward, strike) = ResolvedVolatility {
            sigma: inputs.sigma,
            convention: inputs.volatility_convention,
        }
        .model_rates(inputs.forward, strike)?;
        let delta = match (inputs.volatility_convention, option.option_type) {
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
