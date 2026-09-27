//! Hurdle01 calculator for PrivateMarketsFund.
//!
//! Computes Hurdle01 (hurdle rate sensitivity) using finite differences:
//! the LP position's PV change for a 1bp move in the hurdle IRR rates.
//!
//! # Formula
//! ```text
//! Hurdle01 = (PV(hurdle_rate + h) - PV(hurdle_rate - h)) / 2
//! ```
//! Where the FD bump h is 1bp (0.0001). The central difference is divided
//! by 2 (not 2h), so the result is the PV change *per 1bp move*, in
//! currency units, consistent with the workspace `Dv01` convention. Bumps
//! are not clamped at zero: the IRR solve accepts slightly negative hurdle
//! targets, so a 0% hurdle still gets a genuine central difference.
//!
//! # Note
//!
//! Hurdle rates appear in:
//! - `Tranche::PreferredIrr { hurdle_irr }` - preferred return hurdle
//! - `Tranche::PromoteTier { hurdle_irr, ... }` - promote tier hurdle
//!
//! Higher hurdle rates increase the LP preferred return, potentially reducing GP carry
//! and affecting LP valuation.

use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::equity::pe_fund::waterfall::{Tranche, WaterfallSpec};
use crate::instruments::equity::pe_fund::PrivateMarketsFund;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_quant_core::Result;

/// Standard hurdle bump: 1bp (0.0001)
const HURDLE_BUMP: f64 = 0.0001;

/// Return a copy of the spec with all hurdle IRR rates shifted by `delta`.
fn bumped_spec(spec: &WaterfallSpec, delta: f64) -> WaterfallSpec {
    let mut bumped = spec.clone();
    for tranche in &mut bumped.tranches {
        match tranche {
            Tranche::PreferredIrr { hurdle_irr } | Tranche::PromoteTier { hurdle_irr, .. } => {
                *hurdle_irr += delta
            }
            _ => {}
        }
    }
    bumped
}

/// Hurdle01 calculator for PrivateMarketsFund.
pub(crate) struct Hurdle01Calculator;

impl MetricCalculator for Hurdle01Calculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let fund: &PrivateMarketsFund = context.instrument_as()?;
        let as_of = context.as_of;

        let mut fund_up = fund.clone();
        fund_up.waterfall_spec = bumped_spec(&fund.waterfall_spec, HURDLE_BUMP);
        let pv_up = fund_up.value(context.curves.as_ref(), as_of)?.amount();

        let mut fund_down = fund.clone();
        fund_down.waterfall_spec = bumped_spec(&fund.waterfall_spec, -HURDLE_BUMP);
        let pv_down = fund_down.value(context.curves.as_ref(), as_of)?.amount();

        // Scenarios are ±1bp; return the PV change for one basis point.
        // Higher hurdle typically benefits LPs (more preferred return before GP carry)
        Ok((pv_up - pv_down) / 2.0)
    }
}
