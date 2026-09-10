//! Bond floor (investment value) calculator for convertible bonds.
//!
//! The bond floor is the present value of the convertible's cash flows (coupons
//! and principal redemption) discounted at the appropriate rate, ignoring the
//! conversion option entirely. It represents the "straight bond" value -- what
//! the instrument would be worth if it had no equity conversion feature.
//!
//! The cash component uses the main tree's discrete recovery blend of
//! forward discount factors: `df_cash = df_credit * (1 - R) + df_risk_free * R`.
//! This retains the canonical grid's discounting and coupon timing exactly.
//!
//! # Use Cases
//!
//! - Assessing downside protection (bond floor vs market price)
//! - Computing the "equity option value" = CB price - bond floor
//! - Monitoring busted convertibles (trading near bond floor)

use crate::instruments::fixed_income::convertible::ConvertibleBond;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_quant_core::Result;

pub(crate) struct BondFloorCalculator;

impl MetricCalculator for BondFloorCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let bond: &ConvertibleBond = context.instrument_as()?;
        crate::instruments::fixed_income::convertible::pricing::price_bond_floor(
            bond,
            &context.curves,
            context.as_of,
        )
    }
}
