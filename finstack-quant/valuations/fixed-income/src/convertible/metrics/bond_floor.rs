//! Bond floor (investment value) calculator for convertible bonds.
//!
//! The bond floor is the present value of the convertible's cash flows (coupons
//! and principal redemption) discounted at the appropriate rate, ignoring the
//! conversion option entirely. It represents the "straight bond" value -- what
//! the instrument would be worth if it had no equity conversion feature.
//!
//! The floor uses the main tree's survival-weighted debt payments plus explicit
//! recovery of outstanding principal, including PIK and prior amortization.
//! Payment-date corrections preserve contractual cashflow discounting.
//!
//! # Use Cases
//!
//! - Assessing downside protection (bond floor vs market price)
//! - Computing the "equity option value" = CB price - bond floor
//! - Monitoring busted convertibles (trading near bond floor)

use crate::convertible::ConvertibleBond;
use finstack_quant_core::Result;
use finstack_quant_valuations_core::metrics::{MetricCalculator, MetricContext};

pub(crate) struct BondFloorCalculator;

impl MetricCalculator for BondFloorCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let bond: &ConvertibleBond = context.instrument_as()?;
        crate::convertible::pricing::price_bond_floor(bond, &context.curves, context.as_of)
    }
}
