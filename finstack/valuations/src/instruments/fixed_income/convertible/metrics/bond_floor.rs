//! Bond floor (investment value) calculator for convertible bonds.
//!
//! The bond floor is the present value of the convertible's cash flows (coupons
//! and principal redemption) discounted at the appropriate rate, ignoring the
//! conversion option entirely. It represents the "straight bond" value -- what
//! the instrument would be worth if it had no equity conversion feature.
//!
//! When a credit curve is set, discounting uses the risky rate. Otherwise,
//! the risk-free discount curve is used.
//!
//! # Use Cases
//!
//! - Assessing downside protection (bond floor vs market price)
//! - Computing the "equity option value" = CB price - bond floor
//! - Monitoring busted convertibles (trading near bond floor)

use crate::instruments::common_impl::pricing::time::relative_df_discount_curve;
use crate::instruments::fixed_income::convertible::ConvertibleBond;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_core::Result;

pub(crate) struct BondFloorCalculator;

impl MetricCalculator for BondFloorCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let bond: &ConvertibleBond = context.instrument_as()?;
        let as_of = context.as_of;

        if as_of >= bond.maturity {
            return Ok(0.0);
        }

        let schedule =
            crate::instruments::fixed_income::convertible::pricer::build_convertible_schedule(
                bond,
            )?;

        let curve_id = bond
            .credit_curve_id
            .as_ref()
            .unwrap_or(&bond.discount_curve_id);
        let curve = context.curves.get_discount(curve_id.as_str())?;

        // Date-based DFs on the curve's own axis: correct when the curve base
        // date differs from as_of, and a day-count failure is an error rather
        // than a silent t=0 (which would discount nothing).
        let mut pv = 0.0;

        for cf in schedule.coupons() {
            if cf.date <= as_of {
                continue;
            }
            let df = relative_df_discount_curve(curve.as_ref(), as_of, cf.date)?;
            pv += cf.amount.amount() * df;
        }

        // Add principal redemption at maturity
        let df_mat = relative_df_discount_curve(curve.as_ref(), as_of, bond.maturity)?;
        pv += bond.notional.amount() * df_mat;

        Ok(pv)
    }
}
