//! Collateral coverage ratio metric for `Repo`.
//!
//! Computes `market_value / required_value` using pre-computed metrics.

use crate::metrics::{MetricCalculator, MetricContext, MetricId};
use finstack_quant_core::Result;

/// Calculate collateral coverage ratio (market value / required value).
pub(crate) struct CollateralCoverageCalculator;

impl MetricCalculator for CollateralCoverageCalculator {
    fn dependencies(&self) -> &[MetricId] {
        &[MetricId::CollateralValue, MetricId::RequiredCollateral]
    }

    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        // Dependencies are declared above, so missing entries indicate a
        // registry/dependency-resolution bug — propagate instead of silently
        // substituting defaults that would fabricate a coverage ratio.
        let collateral_value = context
            .computed
            .get(&MetricId::CollateralValue)
            .copied()
            .ok_or_else(|| {
                finstack_quant_core::Error::Validation(
                    "CollateralCoverage requires CollateralValue to be computed first".to_string(),
                )
            })?;
        let required_value = context
            .computed
            .get(&MetricId::RequiredCollateral)
            .copied()
            .ok_or_else(|| {
                finstack_quant_core::Error::Validation(
                    "CollateralCoverage requires RequiredCollateral to be computed first"
                        .to_string(),
                )
            })?;

        if required_value == 0.0 {
            return Err(finstack_quant_core::Error::Validation(
                "CollateralCoverage is undefined: required collateral is zero".to_string(),
            ));
        }

        Ok(collateral_value / required_value)
    }
}
