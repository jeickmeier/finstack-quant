//! Delta calculator for commodity swaps.
//!
//! Delta measures the sensitivity of the swap's NPV to changes in the floating price.
//! For each payment period:
//!
//! Delta contribution = sign × Q × DF(as_of → payment_date)
//!
//! Uses `df_between_dates(as_of, payment_date)` for base-date-safe discounting.

use crate::instruments::commodity::commodity_swap::CommoditySwap;
use crate::instruments::common_impl::parameters::legs::PayReceive;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_core::Result;

/// Delta calculator for commodity swaps (per 1.0 unit of floating price).
///
/// Uses `df_between_dates(as_of, payment_date)` for base-date-safe discounting,
/// consistent with the NPV calculation in `CommoditySwap::floating_leg_pv()`.
pub(crate) struct DeltaCalculator;

impl MetricCalculator for DeltaCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let swap: &CommoditySwap = context.instrument_as()?;
        let disc = context
            .curves
            .get_discount(swap.discount_curve_id.as_str())?;
        let schedule = swap.payment_schedule(context.as_of)?;
        let sign = match swap.side {
            PayReceive::Pay => 1.0,
            PayReceive::Receive => -1.0,
        };

        let mut delta = 0.0;
        for payment_date in schedule {
            if payment_date < context.as_of {
                continue;
            }
            // Use df_between_dates for base-date-safe discounting (consistent with npv())
            let df = disc.df_between_dates(context.as_of, payment_date)?;
            delta += sign * swap.quantity * df;
        }

        Ok(delta)
    }
}
