//! Financing annuity calculator for equity TRS.

use crate::equity_trs::EquityTotalReturnSwap;
use finstack_quant_core::Result;
use finstack_quant_valuations_core::metrics::{MetricCalculator, MetricContext};

/// Calculates the financing annuity for an equity TRS.
///
/// The annuity is the discounted notional-weighted accrual sum used by the
/// par-spread solve. Multiplying it by `0.0001` gives the PV of one basis point
/// of financing spread.
pub(crate) struct FinancingAnnuityCalculator;

impl MetricCalculator for FinancingAnnuityCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let trs: &EquityTotalReturnSwap = context.instrument_as()?;
        trs.financing_annuity(context.curves.as_ref(), context.as_of)
    }
}
