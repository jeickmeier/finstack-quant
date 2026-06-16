//! Funding risk metric for `Repo`.
//!
//! Sensitivity of the repo PV to a +1bp change in the instrument's repo rate
//! parameter, reported as `ΔPV = PV(rate + 1bp) − PV(rate)` — the workspace
//! dPV/dy convention shared with Dv01. From the cash lender's perspective a
//! higher repo rate raises the repurchase payment, so FundingRisk is
//! typically **positive** for a lender (and negative for a reverse repo).

use crate::instruments::common_impl::traits::Instrument;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_quant_core::Result;
use rust_decimal::Decimal;

/// Calculate funding risk (repo rate sensitivity).
pub(crate) struct FundingRiskCalculator;

impl MetricCalculator for FundingRiskCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        const ONE_BP: f64 = 1e-4; // 1 basis point as decimal
        let repo = context.instrument_as::<crate::instruments::rates::repo::Repo>()?;
        let base_pv = repo.value(&context.curves, context.as_of)?.amount();
        let mut repo_bumped = repo.clone();
        repo_bumped.repo_rate += Decimal::try_from(ONE_BP)
            .map_err(|_| finstack_quant_core::InputError::ConversionOverflow)?;
        let bumped_pv = repo_bumped.value(&context.curves, context.as_of)?.amount();
        // dPV/dy convention: ΔPV for a +1bp bump (NOT the negated change).
        Ok(bumped_pv - base_pv)
    }
}
