//! Delta calculator for equity spot positions.
//!
//! Returns the effective share exposure (shares).

use crate::instruments::equity::Equity;
use crate::metrics::{MetricCalculator, MetricContext, MetricId};
use finstack_quant_core::Result;

/// Delta calculator for equity spot.
pub(crate) struct DeltaCalculator;

impl MetricCalculator for DeltaCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let equity: &Equity = context.instrument_as()?;
        let delta = equity.shares.unwrap_or(1.0);

        context.computed.insert(
            MetricId::custom(format!("delta::{}", equity.ticker.as_str())),
            delta,
        );

        Ok(delta)
    }
}
