//! Carry01 calculator for PrivateMarketsFund.
//!
//! Computes Carry01 (GP carry sensitivity) using finite differences:
//! the LP position's PV change for a 1bp move in the GP share.
//!
//! # Formula
//! ```text
//! Carry01 = (PV(GP_share + h) - PV(GP_share - h)) / 2
//! ```
//! Where the FD bump h is 1bp (0.0001). The central difference is divided
//! by 2 (not 2h), so the result is the PV change *per 1bp move*, in
//! currency units, consistent with the workspace `Dv01` convention. When a
//! GP share sits at a boundary (0 or 1) so one direction cannot be bumped,
//! a one-sided difference against the unbumped base value is used instead
//! of silently halving the sensitivity with a clamped no-op bump.
//!
//! # Note
//! GP carry is determined by the waterfall allocation (promote tiers, catch-up).
//! This metric bumps the GP share in all promote tiers and catch-up tranches
//! and measures the impact on LP valuation (lower carry = higher LP value).

use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::equity::pe_fund::waterfall::{PeFundWaterfallSpec, PeFundWaterfallTranche};
use crate::instruments::equity::pe_fund::PrivateMarketsFund;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_quant_core::Result;

/// Standard carry bump: 1bp (0.0001 = 0.01%)
const CARRY_BUMP: f64 = 0.0001;

/// Return a copy of the spec with GP shares in catch-up and promote tranches
/// shifted by `delta` (promote LP shares renormalized to sum to 1.0).
fn bumped_spec(spec: &PeFundWaterfallSpec, delta: f64) -> PeFundWaterfallSpec {
    let mut bumped = spec.clone();
    for tranche in &mut bumped.tranches {
        match tranche {
            PeFundWaterfallTranche::CatchUp { gp_share } => {
                *gp_share += delta;
            }
            PeFundWaterfallTranche::PromoteTier {
                lp_share, gp_share, ..
            } => {
                *gp_share += delta;
                *lp_share = 1.0 - *gp_share;
            }
            _ => {
                // ReturnOfCapital, PreferredIrr don't affect carry directly
            }
        }
    }
    bumped
}

/// Whether every carry-bearing GP share in the spec can absorb `delta`
/// without leaving `[0, 1]`.
fn can_bump(spec: &PeFundWaterfallSpec, delta: f64) -> bool {
    spec.tranches.iter().all(|t| match t {
        PeFundWaterfallTranche::CatchUp { gp_share }
        | PeFundWaterfallTranche::PromoteTier { gp_share, .. } => {
            (0.0..=1.0).contains(&(gp_share + delta))
        }
        _ => true,
    })
}

/// Carry01 calculator for PrivateMarketsFund.
pub(crate) struct Carry01Calculator;

impl MetricCalculator for Carry01Calculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let fund: &PrivateMarketsFund = context.instrument_as()?;
        let as_of = context.as_of;

        let pv_for = |spec: PeFundWaterfallSpec| -> Result<f64> {
            let mut bumped_fund = fund.clone();
            bumped_fund.waterfall_spec = spec;
            Ok(bumped_fund.value(context.curves.as_ref(), as_of)?.amount())
        };

        let can_up = can_bump(&fund.waterfall_spec, CARRY_BUMP);
        let can_down = can_bump(&fund.waterfall_spec, -CARRY_BUMP);

        // Higher GP carry means less for LPs, so PV_up < PV_down typically.
        // Return the PV change for a +1bp GP share move.
        let carry01 = match (can_up, can_down) {
            (true, true) => {
                let pv_up = pv_for(bumped_spec(&fund.waterfall_spec, CARRY_BUMP))?;
                let pv_down = pv_for(bumped_spec(&fund.waterfall_spec, -CARRY_BUMP))?;
                (pv_up - pv_down) / 2.0
            }
            (true, false) => {
                let pv_up = pv_for(bumped_spec(&fund.waterfall_spec, CARRY_BUMP))?;
                pv_up - context.base_value.amount()
            }
            (false, true) => {
                let pv_down = pv_for(bumped_spec(&fund.waterfall_spec, -CARRY_BUMP))?;
                context.base_value.amount() - pv_down
            }
            // Shares pinned at opposite boundaries across tranches: no
            // consistent bump direction exists.
            (false, false) => 0.0,
        };

        Ok(carry01)
    }
}
