//! Delta calculator for swaptions.
//!
//! Computes cash delta using Black or Normal model greeks with forward swap rate and
//! underlying swap annuity. Uses SABR-implied vol if parameters are set,
//! otherwise uses the volatility surface or an override from `PricingOverrides`.
//!
//! # Numerical Stability
//!
//! Although delta doesn't involve division by sqrt(T) (unlike gamma), the d1
//! calculation can become numerically unstable near expiry. We apply a
//! near-expiry threshold for consistency and to return intrinsic delta.

use crate::instruments::common_impl::parameters::OptionType;
use crate::instruments::rates::swaption::{Swaption, VolatilityModel};
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_core::Result;

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
        let inputs = match option.greek_inputs(&context.curves, context.as_of)? {
            Some(inputs) => inputs,
            None => return Ok(0.0), // Option expired
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

        // Black (lognormal) Greeks are undefined for non-positive forward or
        // strike; fall back to Bachelier (normal) for negative-rate regimes.
        let normal_by_model = matches!(option.vol_model, VolatilityModel::Normal);
        let normal_by_negative_rate = inputs.forward <= 0.0 || strike <= 0.0;
        let use_normal = normal_by_model || normal_by_negative_rate;
        let delta = if use_normal {
            use crate::models::volatility::normal::d_bachelier;
            // When the Normal model is the configured vol model, `inputs.sigma`
            // is already a normal vol. When the fallback is triggered purely by
            // a non-positive forward/strike, `inputs.sigma` is a LOGNORMAL vol
            // (from SABR or a lognormal surface) and must be converted before
            // the Bachelier greek, otherwise the d-value is mis-scaled.
            let normal_sigma = if normal_by_model {
                inputs.sigma
            } else {
                super::resolved_normal_sigma(
                    option,
                    inputs.forward,
                    strike,
                    inputs.sigma,
                    inputs.time_to_expiry,
                )
            };
            let d = d_bachelier(inputs.forward, strike, normal_sigma, inputs.time_to_expiry);
            match option.option_type {
                OptionType::Call => finstack_core::math::norm_cdf(d),
                OptionType::Put => -finstack_core::math::norm_cdf(-d),
            }
        } else {
            use crate::models::d1_black76;
            let d1 = d1_black76(inputs.forward, strike, inputs.sigma, inputs.time_to_expiry);
            match option.option_type {
                OptionType::Call => finstack_core::math::norm_cdf(d1),
                OptionType::Put => -finstack_core::math::norm_cdf(-d1),
            }
        };

        // Scale by notional and annuity for cash delta
        Ok(delta * option.notional.amount() * inputs.annuity)
    }
}
