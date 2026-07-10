//! Vega calculator for swaptions.
//!
//! Computes cash vega using Black or Normal model vega with forward swap rate and
//! underlying swap annuity. Uses SABR-implied vol if parameters are set,
//! otherwise uses the volatility surface or an override from `PricingOverrides`.
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
//! # Numerical Stability
//!
//! Although vega involves `sqrt(T)` which approaches zero at expiry (making vega
//! approach zero naturally), we apply a near-expiry threshold for consistency
//! with other Greeks and to avoid potential numerical issues with d1 calculation.

use crate::instruments::rates::swaption::{Swaption, VolatilityModel};
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_quant_core::Result;

/// Minimum time to expiry (in years) for valid vega calculation.
///
/// Below this threshold, vega is economically negligible and d1/d calculations
/// may become numerically unstable. Set to ~1 business day for consistency with gamma.
const EXPIRY_THRESHOLD: f64 = 1.0 / 252.0;

/// Vega calculator for swaptions
pub(crate) struct VegaCalculator;

impl MetricCalculator for VegaCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let option: &Swaption = context.instrument_as()?;
        let strike = option.strike_f64()?;

        // Use consolidated helper to get pre-computed inputs
        let Some(inputs) = option.greek_inputs(&context.curves, context.as_of)? else {
            return Ok(0.0); // Option expired
        };

        // Near-expiry guard: vega approaches zero as T -> 0, but d1 calculation
        // may become unstable. Return 0 when within ~1 business day of expiry.
        if inputs.time_to_expiry < EXPIRY_THRESHOLD {
            return Ok(0.0);
        }

        // Black (lognormal) Greeks are undefined for non-positive forward or
        // strike; fall back to Bachelier (normal) for negative-rate regimes.
        let normal_by_model = matches!(option.vol_model, VolatilityModel::Normal);
        let normal_by_negative_rate = inputs.forward <= 0.0 || strike <= 0.0;
        let use_normal = normal_by_model || normal_by_negative_rate;
        let (vega_raw, quote_axis_jacobian) = if use_normal {
            use crate::models::volatility::normal::d_bachelier;
            // For the negative-rate fallback `inputs.sigma` is a lognormal vol;
            // convert it to a normal vol so the Bachelier d-value — and hence
            // the vega — is correctly scaled. (Vega here measures sensitivity
            // to the normal vol on this path, consistent with the Bachelier
            // pricer the fallback uses.)
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
            let jacobian = if normal_by_model {
                1.0
            } else {
                let bump = (inputs.sigma.abs() * 1.0e-5).max(1.0e-7);
                let lower = (inputs.sigma - bump).max(0.0);
                let upper = inputs.sigma + bump;
                let normal_upper = super::resolved_normal_sigma(
                    option,
                    inputs.forward,
                    strike,
                    upper,
                    inputs.time_to_expiry,
                );
                let normal_lower = super::resolved_normal_sigma(
                    option,
                    inputs.forward,
                    strike,
                    lower,
                    inputs.time_to_expiry,
                );
                (normal_upper - normal_lower) / (upper - lower)
            };
            (
                finstack_quant_core::math::norm_pdf(d) * inputs.time_to_expiry.sqrt(),
                jacobian,
            )
        } else {
            use crate::models::d1_black76;
            let d1 = d1_black76(inputs.forward, strike, inputs.sigma, inputs.time_to_expiry);
            (
                inputs.forward
                    * finstack_quant_core::math::norm_pdf(d1)
                    * inputs.time_to_expiry.sqrt(),
                1.0,
            )
        };

        let vega = vega_raw * quote_axis_jacobian / super::config::VOL_PCT_SCALE;
        // Scale by notional and annuity for cash vega
        Ok(vega * option.notional.amount() * inputs.annuity)
    }
}
