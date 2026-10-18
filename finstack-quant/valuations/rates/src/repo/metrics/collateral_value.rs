//! Collateral value metric for `Repo`.
//!
//! Computes the current market value of the collateral backing the repo
//! using prices from `MarketContext`.

use finstack_quant_core::Result;
use finstack_quant_valuations_core::metrics::{MetricCalculator, MetricContext};

/// Calculate the market value of collateral.
pub(crate) struct CollateralValueCalculator;

impl MetricCalculator for CollateralValueCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let repo = context.instrument_as::<crate::repo::Repo>()?;
        let collateral_value = repo.collateral.market_value(&context.curves)?;
        Ok(collateral_value.amount())
    }
}
