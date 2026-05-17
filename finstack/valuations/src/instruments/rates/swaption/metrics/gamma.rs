//! Gamma calculator for swaptions.
//!
//! Computes cash gamma using Black or Normal model gamma with forward swap rate and
//! underlying swap annuity. Uses SABR-implied vol if parameters are set,
//! otherwise uses the volatility surface or an override from `PricingOverrides`.
//!
//! # Numerical Stability
//!
//! Gamma involves division by `sqrt(T)` which approaches infinity as expiry approaches.
//! This module applies a near-expiry threshold (`EXPIRY_THRESHOLD`) to return zero
//! for options within ~1 business day of expiry, where gamma is mathematically
//! undefined for vanilla options.

use crate::instruments::rates::swaption::{Swaption, VolatilityModel};
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_core::Result;

/// Minimum time to expiry (in years) for valid gamma calculation.
///
/// Below this threshold, gamma is numerically unstable (division by near-zero sqrt(T))
/// and economically meaningless for vanilla options. Set to ~1 business day.
const EXPIRY_THRESHOLD: f64 = 1.0 / 252.0;

/// Gamma calculator for swaptions
pub(crate) struct GammaCalculator;

impl MetricCalculator for GammaCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let option: &Swaption = context.instrument_as()?;
        let strike = option.strike_f64()?;

        // Use consolidated helper to get pre-computed inputs
        let inputs = match option.greek_inputs(&context.curves, context.as_of)? {
            Some(inputs) => inputs,
            None => return Ok(0.0), // Option expired
        };

        if inputs.sigma <= 0.0 {
            return Ok(0.0);
        }

        // Near-expiry guard: gamma is undefined/infinite as T -> 0.
        // For vanilla options, return 0 when within ~1 business day of expiry.
        if inputs.time_to_expiry < EXPIRY_THRESHOLD {
            return Ok(0.0);
        }

        // Black (lognormal) Greeks are undefined for non-positive forward or
        // strike; fall back to Bachelier (normal) for negative-rate regimes.
        let use_normal = matches!(option.vol_model, VolatilityModel::Normal)
            || inputs.forward <= 0.0
            || strike <= 0.0;
        let gamma = if use_normal {
            use crate::instruments::common_impl::models::volatility::normal::d_bachelier;
            let d = d_bachelier(inputs.forward, strike, inputs.sigma, inputs.time_to_expiry);
            finstack_core::math::norm_pdf(d) / (inputs.sigma * inputs.time_to_expiry.sqrt())
        } else {
            use crate::instruments::common_impl::models::d1_black76;
            let d1 = d1_black76(inputs.forward, strike, inputs.sigma, inputs.time_to_expiry);
            finstack_core::math::norm_pdf(d1)
                / (inputs.forward * inputs.sigma * inputs.time_to_expiry.sqrt())
        };

        // Scale by notional and annuity for cash gamma
        Ok(gamma * option.notional.amount() * inputs.annuity)
    }
}
