//! FX Delta calculator for quanto options.
//!
//! Computes FX delta (FX rate sensitivity) using a central finite difference
//! against a relative SPOT bump. Quanto options have payoffs dependent on an
//! equity in one currency but settled in another; FX delta measures sensitivity
//! to changes in the FX exchange rate between those currencies.

use crate::instruments::fx::quanto_option::QuantoOption;
use crate::metrics::{bump_sizes, central_diff_scalar_relative, MetricCalculator, MetricContext};
use finstack_quant_core::Result;

/// FX Delta calculator for quanto options.
pub struct FxDeltaCalculator;

impl MetricCalculator for FxDeltaCalculator {
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

        let fx_rate_id = option.fx_rate_id.as_ref().ok_or_else(|| {
            finstack_quant_core::Error::Validation(format!(
                "QuantoOption {}: fx_rate_id is required to compute FX Delta",
                option.id
            ))
        })?;

        central_diff_scalar_relative(
            option,
            context.curves.as_ref(),
            as_of,
            fx_rate_id,
            bump_sizes::SPOT,
        )
    }
}
