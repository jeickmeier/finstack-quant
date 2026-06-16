//! Correlation sensitivity calculator for quanto options.
//!
//! Bumps the equity-FX correlation by an absolute amount on each side and
//! returns a central finite difference per absolute correlation point. The
//! pair of bumps is shrunk symmetrically when the base correlation sits
//! within `bump_sizes::CORRELATION` of [-1, 1] so that the divisor matches
//! the actual width applied — avoiding the bias the asymmetric clamp would
//! otherwise introduce near the boundary.

use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::fx::quanto_option::QuantoOption;
use crate::metrics::{bump_sizes, MetricCalculator, MetricContext};
use finstack_quant_core::Result;

/// Correlation01 calculator for quanto options.
pub struct Correlation01Calculator;

impl MetricCalculator for Correlation01Calculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let option: &QuantoOption = context.instrument_as()?;
        let as_of = context.as_of;

        let t = option.day_count.year_fraction(
            as_of,
            option.expiry,
            finstack_quant_core::dates::DayCountContext::default(),
        )?;
        if t <= 0.0 {
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
        let half_bump = bump_sizes::CORRELATION
            .min(1.0 - rho)
            .min(1.0 + rho)
            .max(0.0);
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

        // Per absolute 1.0 correlation; multiply by bump_sizes::CORRELATION
        // outside if a per-1% number is wanted.
        Ok((pv_up - pv_down) / (2.0 * half_bump))
    }
}
