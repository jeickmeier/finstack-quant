//! Repo interest amount metric.
//!
//! Computes the accrued interest between start and maturity using the
//! instrument's day count and effective rate.

use crate::metrics::{MetricCalculator, MetricContext};
use finstack_quant_core::Result;

/// Calculate repo interest amount.
pub(crate) struct RepoInterestCalculator;

impl MetricCalculator for RepoInterestCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let repo = context.instrument_as::<crate::instruments::rates::repo::Repo>()?;
        let interest = repo.interest_amount()?;
        Ok(interest.amount())
    }
}
