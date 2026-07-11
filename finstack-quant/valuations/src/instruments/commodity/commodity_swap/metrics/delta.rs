//! Delta calculator for commodity swaps.
//!
//! Delta measures the sensitivity of the swap's NPV to changes in the floating price.
//! For each payment period:
//!
//! Delta contribution = sign × Q × DF(as_of → payment_date)
//!
//! Uses `df_between_dates(as_of, payment_date)` for base-date-safe discounting.

use crate::instruments::commodity::commodity_swap::CommoditySwap;
use crate::instruments::common_impl::traits::Instrument;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_quant_core::market_data::bumps::{
    BumpMode, BumpSpec, BumpType, BumpUnits, MarketBump,
};
use finstack_quant_core::types::CurveId;
use finstack_quant_core::Result;

/// Delta calculator for commodity swaps (per 1.0 unit of floating price).
///
/// Uses `df_between_dates(as_of, payment_date)` for base-date-safe discounting,
/// consistent with the NPV calculation in `CommoditySwap::floating_leg_pv()`.
pub(crate) struct DeltaCalculator;

impl MetricCalculator for DeltaCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let swap: &CommoditySwap = context.instrument_as()?;
        let curve_id = CurveId::new(swap.floating_index_id.as_str());
        let bump = |value| MarketBump::Curve {
            id: curve_id.clone(),
            spec: BumpSpec {
                bump_type: BumpType::Parallel,
                mode: BumpMode::Additive,
                units: BumpUnits::Fraction,
                value,
            },
        };
        let up = context.curves.bump([bump(1.0)])?;
        let down = context.curves.bump([bump(-1.0)])?;
        let pv_up = swap.value(&up, context.as_of)?.amount();
        let pv_down = swap.value(&down, context.as_of)?.amount();
        Ok((pv_up - pv_down) / 2.0)
    }
}
