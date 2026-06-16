//! Fixed leg present value calculation for interest rate swaps.
//!
//! Computes the present value of all future fixed coupon payments by
//! discounting them using the discount curve specified in the fixed leg.
//!
//! # Calculation
//!
//! ```text
//! PV_fixed = Σ Notional × Fixed_Rate × α_i × DF(T_i)
//! ```
//!
//! where:
//! - `α_i` = accrual factor for period i (from day count convention)
//! - `DF(T_i)` = discount factor to payment date i
//! - Sum includes only future cashflows (T_i > as_of)
//!
//! # Use Cases
//!
//! - Computing individual leg PVs for risk analysis
//! - Decomposing swap value into fixed vs floating components
//! - Calibration and curve fitting workflows

use crate::instruments::InterestRateSwap;
use crate::metrics::{MetricCalculator, MetricContext};

/// Present value calculator for the fixed leg of an interest rate swap.
///
/// Discounts all future fixed coupon payments using the discount curve
/// specified in the swap's fixed leg specification.
pub(crate) struct FixedLegPvCalculator;

impl MetricCalculator for FixedLegPvCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let irs: &InterestRateSwap = context.instrument_as()?;
        let as_of = context.as_of;
        let disc = context.curves.get_discount(&irs.fixed.discount_curve_id)?;

        let pv = irs.pv_fixed_leg(&disc, as_of)?;
        Ok(pv)
    }
}
