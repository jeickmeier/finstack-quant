//! Accrued interest and clean price calculators for convertible bonds.
//!
//! Provides dirty/clean price decomposition essential for market quote reconciliation.
//! - **Accrued interest**: Pro-rata coupon since last payment date.
//! - **Clean price**: Dirty price (PV) minus accrued interest.

use crate::instruments::fixed_income::convertible::{calculate_accrued_interest, ConvertibleBond};
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_quant_core::Result;

/// Accrued interest calculator for convertible bonds.
pub(crate) struct AccruedInterestCalculator;

impl MetricCalculator for AccruedInterestCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let bond: &ConvertibleBond = context.instrument_as()?;
        calculate_accrued_interest(bond, context.as_of)
    }
}

/// Clean price calculator: `dirty_price - accrued_interest`.
pub(crate) struct CleanPriceCalculator;

impl MetricCalculator for CleanPriceCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let bond: &ConvertibleBond = context.instrument_as()?;
        let dirty_price = context.base_value.amount();
        let accrued = calculate_accrued_interest(bond, context.as_of)?;
        Ok(dirty_price - accrued)
    }
}
