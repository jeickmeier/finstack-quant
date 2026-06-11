//! Carry01 calculator for PrivateMarketsFund.
//!
//! Computes Carry01 (GP carry sensitivity) using finite differences.
//! Carry01 is the derivative dPV/ds of LP value with respect to the GP share
//! (per unit share), consistent with the workspace Dv01 convention.
//!
//! # Formula
//! ```text
//! Carry01 = (PV(GP_share + h) - PV(GP_share - h)) / (2h)
//! ```
//! Where the FD bump h is 1bp (0.0001); dividing by the bump yields the
//! derivative, not a per-1bp PV change.
//!
//! # Note
//! GP carry is determined by the waterfall allocation (promote tiers, catch-up).
//! This metric bumps the GP share in all promote tiers and catch-up tranches
//! and measures the impact on LP valuation (lower carry = higher LP value).

use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::equity::pe_fund::PrivateMarketsFund;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_core::Result;

/// Standard carry bump: 1bp (0.0001 = 0.01%)
const CARRY_BUMP: f64 = 0.0001;

/// Carry01 calculator for PrivateMarketsFund.
pub(crate) struct Carry01Calculator;

impl MetricCalculator for Carry01Calculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let fund: &PrivateMarketsFund = context.instrument_as()?;
        let as_of = context.as_of;

        use crate::instruments::equity::pe_fund::waterfall::Tranche;

        // Bump GP share up in all promote tiers and catch-up tranches
        let mut spec_up = fund.waterfall_spec.clone();
        for tranche in &mut spec_up.tranches {
            match tranche {
                Tranche::CatchUp { gp_share } => {
                    *gp_share = (*gp_share + CARRY_BUMP).clamp(0.0, 1.0);
                }
                Tranche::PromoteTier {
                    lp_share, gp_share, ..
                } => {
                    // Adjust GP share and renormalize LP share to sum to 1.0
                    let new_gp = (*gp_share + CARRY_BUMP).clamp(0.0, 1.0);
                    let new_lp = (1.0 - new_gp).max(0.0);
                    *gp_share = new_gp;
                    *lp_share = new_lp;
                }
                _ => {
                    // ReturnOfCapital, PreferredIrr don't affect carry directly
                }
            }
        }

        let mut fund_up = fund.clone();
        fund_up.waterfall_spec = spec_up;
        let pv_up = fund_up.value(context.curves.as_ref(), as_of)?.amount();

        // Bump GP share down in all promote tiers and catch-up tranches
        let mut spec_down = fund.waterfall_spec.clone();
        for tranche in &mut spec_down.tranches {
            match tranche {
                Tranche::CatchUp { gp_share } => {
                    *gp_share = (*gp_share - CARRY_BUMP).clamp(0.0, 1.0);
                }
                Tranche::PromoteTier {
                    lp_share, gp_share, ..
                } => {
                    let new_gp = (*gp_share - CARRY_BUMP).clamp(0.0, 1.0);
                    let new_lp = (1.0 - new_gp).max(0.0);
                    *gp_share = new_gp;
                    *lp_share = new_lp;
                }
                _ => {}
            }
        }

        let mut fund_down = fund.clone();
        fund_down.waterfall_spec = spec_down;
        let pv_down = fund_down.value(context.curves.as_ref(), as_of)?.amount();

        // Carry01 = (PV_up - PV_down) / (2 * bump_size)
        // Higher GP carry means less for LPs, so PV_up < PV_down typically
        // Result is the derivative dPV/ds per unit GP share (Dv01 convention)
        let carry01 = (pv_up - pv_down) / (2.0 * CARRY_BUMP);

        Ok(carry01)
    }
}
