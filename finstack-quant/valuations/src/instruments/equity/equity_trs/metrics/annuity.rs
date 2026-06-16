//! Financing annuity calculator for equity TRS.

use crate::instruments::equity::equity_trs::EquityTotalReturnSwap;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_quant_core::Result;

/// Calculates the financing annuity for an equity TRS.
///
/// The financing annuity is the sum of discounted year fractions over all payment periods,
/// multiplied by the notional amount. This represents the present value of a 1 basis point
/// spread over the floating rate.
pub(crate) struct FinancingAnnuityCalculator;

impl MetricCalculator for FinancingAnnuityCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let trs: &EquityTotalReturnSwap = context.instrument_as()?;
        trs.financing_annuity(context.curves.as_ref(), context.as_of)
    }
}
