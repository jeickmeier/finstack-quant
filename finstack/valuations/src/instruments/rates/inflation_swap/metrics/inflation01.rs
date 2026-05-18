//! Inflation01 (inflation rate sensitivity) metric for `InflationSwap`.
//!
//! # Finite-Difference Method
//!
//! Inflation01 is computed by **central finite differences** on the inflation
//! curve — the same approach the YoY path (`YoYInflation01Calculator`) uses,
//! so the zero-coupon and YoY metrics are mutually consistent and both agree
//! with a bumped-curve DV01:
//!
//! ```text
//! Inflation01 ≈ [PV(+1bp) − PV(−1bp)] / (2 · 1bp)
//! ```
//!
//! # Why not the closed form
//!
//! The previous analytical approximation
//!
//! ```text
//! Inflation01 ≈ N · I(T)/I(0) · DF(T) · T · 1bp
//! ```
//!
//! had a **lag mismatch**: the maturity time `T` in the `dPV/dπ` factor was
//! computed to the *lagged* maturity (`maturity − indexation lag`, ACT/365F)
//! while the discount factor `DF(T)` used the *unlagged* maturity on the
//! discount curve's own day count. The two `T`s referred to different dates,
//! so the analytic sensitivity disagreed with the bumped-curve DV01. It also
//! silently assumed continuous compounding `exp(π·T)` whereas inflation curves
//! may compound discretely. Finite differences re-use the instrument's actual
//! `value()` (lag, day counts, compounding all consistent) and so carry no
//! such mismatch.
//!
//! # Sign Convention
//!
//! - **PayFixed**: positive Inflation01 (benefits from higher inflation).
//! - **ReceiveFixed**: negative Inflation01 (loses from higher inflation).
//!
//! The bumped-curve `value()` already carries the leg signs, so the finite
//! difference reproduces the correct sign without an explicit branch.

use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::rates::inflation_swap::InflationSwap;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_core::market_data::bumps::{BumpSpec, MarketBump};
use finstack_core::Result;

/// Standard inflation curve bump: 1bp (0.0001 in decimal).
const INFLATION_BUMP_BP: f64 = 0.0001;

/// Calculates Inflation01 (1bp inflation rate sensitivity) for zero-coupon
/// inflation swaps via central finite differences on the inflation curve.
pub(crate) struct Inflation01Calculator;

impl MetricCalculator for Inflation01Calculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let swap: &InflationSwap = context.instrument_as()?;
        let as_of = context.as_of;

        // Bump the inflation curve up by 1bp and reprice.
        let bump_up = BumpSpec::inflation_shift_pct(INFLATION_BUMP_BP * 100.0);
        let curves_up = context.curves.as_ref().bump([MarketBump::Curve {
            id: swap.inflation_index_id.clone(),
            spec: bump_up,
        }])?;
        let pv_up = swap.value(&curves_up, as_of)?.amount();

        // Bump the inflation curve down by 1bp and reprice.
        let bump_down = BumpSpec::inflation_shift_pct(-INFLATION_BUMP_BP * 100.0);
        let curves_down = context.curves.as_ref().bump([MarketBump::Curve {
            id: swap.inflation_index_id.clone(),
            spec: bump_down,
        }])?;
        let pv_down = swap.value(&curves_down, as_of)?.amount();

        // Central difference: dPV/dπ scaled to a 1bp move.
        Ok((pv_up - pv_down) / (2.0 * INFLATION_BUMP_BP))
    }
}
