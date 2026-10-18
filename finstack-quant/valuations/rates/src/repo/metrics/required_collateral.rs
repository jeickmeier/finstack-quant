//! Required collateral metric for `Repo`.
//!
//! Computes the required collateral value including haircut for a repo.

use finstack_quant_core::Result;
use finstack_quant_valuations_core::metrics::{MetricCalculator, MetricContext};

/// Calculate required collateral value including haircut.
pub(crate) struct RequiredCollateralCalculator;

impl MetricCalculator for RequiredCollateralCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let repo = context.instrument_as::<crate::repo::Repo>()?;
        let required_value = repo.required_collateral_value()?;
        Ok(required_value.amount())
    }
}
