//! Correlation sensitivity calculator for quanto options.
//!
//! Bumps the equity-FX correlation by an absolute amount on each side and
//! returns a central finite difference per absolute correlation point. The
//! pair of bumps is shrunk symmetrically when the base correlation sits
//! within `CORRELATION_BUMP` of [-1, 1] so that the divisor matches
//! the actual width applied — avoiding the bias the asymmetric clamp would
//! otherwise introduce near the boundary.

use crate::quanto_option::QuantoOption;
use finstack_quant_core::Result;
use finstack_quant_valuations_core::instruments::Instrument;
use finstack_quant_valuations_core::metrics::{MetricCalculator, MetricContext, CORRELATION_BUMP};

/// Correlation01 calculator for quanto options.
pub struct Correlation01Calculator;

impl MetricCalculator for Correlation01Calculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let option: &QuantoOption = context.instrument_as()?;
        let as_of = context.as_of;

        if crate::shared::greeks_expired(option.day_count, as_of, option.expiry)? {
            return Ok(0.0);
        }

        let rho = option.correlation;
        if !rho.is_finite() || !(-1.0..=1.0).contains(&rho) {
            return Err(finstack_quant_core::Error::Validation(format!(
                "QuantoOption {}: correlation must be in [-1, 1], got {rho}",
                option.id
            )));
        }

        // Symmetric, boundary-aware bump: shrink the half-width so both bumped
        // correlations remain within [-1, 1]. The denominator below matches.
        let half_bump = CORRELATION_BUMP.min(1.0 - rho).min(1.0 + rho).max(0.0);
        if half_bump <= 0.0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "QuantoOption {}: cannot bump correlation at boundary (rho={rho})",
                option.id
            )));
        }

        let mut option_up = option.clone();
        option_up.correlation = rho + half_bump;
        let pv_up = option_up.value(&context.curves, as_of)?.amount();

        let mut option_down = option.clone();
        option_down.correlation = rho - half_bump;
        let pv_down = option_down.value(&context.curves, as_of)?.amount();

        // Report the PV change for one correlation percentage point.  Near a
        // boundary the actual bump can be smaller, so recover the local
        // derivative and rescale it to the public one-point unit.
        Ok((pv_up - pv_down) / (2.0 * half_bump) * CORRELATION_BUMP)
    }
}
