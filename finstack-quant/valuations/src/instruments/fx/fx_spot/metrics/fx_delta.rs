//! FX Delta calculator for FX Spot.
//!
//! Computes FX delta (sensitivity to spot rate) analytically.
//! For a spot position $V = N \cdot S$, the delta $\frac{\partial V}{\partial S} = N$.
//! Returns the delta in Base currency units (which equals the notional).

use crate::instruments::fx::fx_spot::FxSpot;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_quant_core::Result;

/// FX Delta calculator for FX Spot.
///
/// Returns the analytical delta (Base Notional).
pub(crate) struct FxDeltaCalculator;

impl MetricCalculator for FxDeltaCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let fx_spot: &FxSpot = context.instrument_as()?;
        // Analytic Delta = Notional Amount (Base Currency)
        Ok(fx_spot.effective_notional().amount())
    }
}
