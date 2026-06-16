//! Financing annuity calculator for fixed income index TRS.

use crate::instruments::fixed_income::fi_trs::FIIndexTotalReturnSwap;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_quant_core::Result;

/// Calculates the financing annuity for a fixed income index TRS.
///
/// The financing annuity is the sum of discounted year fractions over all payment periods,
/// multiplied by the notional amount. This represents the present value of a 1 basis point
/// spread over the floating rate.
pub(crate) struct FinancingAnnuityCalculator;

impl MetricCalculator for FinancingAnnuityCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let trs: &FIIndexTotalReturnSwap = context.instrument_as()?;
        trs.financing_annuity(context.curves.as_ref(), context.as_of)
    }
}
