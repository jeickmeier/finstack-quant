//! Inverse spot rate metric for `FxSpot`.
//!
//! Computes the inverse of the realized spot rate. Errors when inversion
//! would be numerically unstable rather than silently returning 0.

use crate::instruments::fx::fx_spot::FxSpot;
use crate::metrics::{MetricCalculator, MetricContext};

/// Threshold below which an inversion is considered ill-conditioned.
const INVERSION_FLOOR: f64 = 1e-12;

/// Calculates the inverse of the spot rate (base per quote).
///
/// # Errors
///
/// Returns a `Validation` error when:
/// - Base notional magnitude is below `INVERSION_FLOOR`
/// - Realized spot magnitude is below `INVERSION_FLOOR`
/// - Either input is non-finite
///
/// A silent zero would mask configuration errors (zero notional, zero spot)
/// in calling code that assumes the inverse is meaningful.
pub struct InverseRateCalculator;

impl MetricCalculator for InverseRateCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let fx: &FxSpot = context.instrument_as()?;
        let base_amt = fx.effective_notional().amount();
        let pv = context.base_value.amount();

        if !base_amt.is_finite() || base_amt.abs() < INVERSION_FLOOR {
            return Err(finstack_quant_core::Error::Validation(format!(
                "FxSpot {}: cannot compute inverse rate, base notional is non-finite \
                 or below {INVERSION_FLOOR:.0e} (got {base_amt})",
                fx.id
            )));
        }

        let spot = pv / base_amt;
        if !spot.is_finite() || spot.abs() < INVERSION_FLOOR {
            return Err(finstack_quant_core::Error::Validation(format!(
                "FxSpot {}: cannot compute inverse rate, realized spot is non-finite \
                 or below {INVERSION_FLOOR:.0e} (got {spot})",
                fx.id
            )));
        }
        Ok(1.0 / spot)
    }
}
