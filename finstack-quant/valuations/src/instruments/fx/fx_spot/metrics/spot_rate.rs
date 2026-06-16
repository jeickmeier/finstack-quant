//! Spot rate metric for `FxSpot`.
//!
//! Computes the realized spot rate as `quote_amount / base_amount`, where the
//! `quote_amount` is the instrument PV in quote currency and `base_amount` is
//! the effective base notional.

use crate::instruments::fx::fx_spot::FxSpot;
use crate::metrics::{MetricCalculator, MetricContext};

/// Epsilon for floating-point near-zero comparisons.
/// This threshold prevents division by extremely small numbers that would
/// produce numerically unstable results.
const EPSILON: f64 = 1e-15;

/// Calculates the FX spot rate as `quote_amount / base_amount`.
///
/// Returns an error when the base notional is near-zero because the realized
/// spot rate is undefined in that case.
pub struct SpotRateCalculator;

impl MetricCalculator for SpotRateCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let fx: &FxSpot = context.instrument_as()?;
        let base_amt = fx.effective_notional().amount();

        if base_amt.abs() < EPSILON {
            return Err(finstack_quant_core::Error::Validation(format!(
                "FxSpot spot_rate is undefined for near-zero base notional ({base_amt})"
            )));
        }

        Ok(context.base_value.amount() / base_amt)
    }
}
