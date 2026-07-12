//! FX Delta calculator for FX Spot.
//!
//! Computes FX delta (sensitivity to spot rate) analytically.
//! For a spot position $V = N \cdot S$, the delta $\frac{\partial V}{\partial S} = N$.
//! Returns the cash P&L for a 1% relative FX move, matching `MetricId::FxDelta`.

use crate::instruments::fx::fx_spot::FxSpot;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_quant_core::Result;

/// FX Delta calculator for FX Spot.
///
/// Returns the analytical cash P&L for a 1% relative spot move.
pub(crate) struct FxDeltaCalculator;

impl MetricCalculator for FxDeltaCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let fx_spot: &FxSpot = context.instrument_as()?;
        let spot = if let Some(spot) = fx_spot.spot_rate {
            spot
        } else {
            let matrix = context.curves.fx().ok_or_else(|| {
                finstack_quant_core::Error::Validation(
                    "FxSpot FxDelta requires an FX matrix when spot_rate is unset".to_string(),
                )
            })?;
            matrix
                .rate(finstack_quant_core::money::fx::FxQuery::new(
                    fx_spot.base_currency,
                    fx_spot.quote_currency,
                    context.as_of,
                ))?
                .rate
        };
        if !spot.is_finite() || spot <= 0.0 {
            return Err(finstack_quant_core::Error::Validation(
                "FxSpot FxDelta requires a positive finite spot rate".to_string(),
            ));
        }

        // V = N_base * S_quote/base, so a 1% relative move changes PV by
        // N_base * S * 0.01.
        Ok(fx_spot.effective_notional().amount() * spot * 0.01)
    }
}
