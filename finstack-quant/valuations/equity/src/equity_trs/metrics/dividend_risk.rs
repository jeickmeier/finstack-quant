//! Dividend risk calculator for equity TRS.
//!
//! Dividend01 is the change in PV for a 1bp (0.0001) move in dividend yield;
//! see [`crate::dividend01`] for the finite-difference
//! contract. For equity TRS, dividend yield affects the forward price of the
//! underlying equity, which impacts the total return leg value.
//!
//! The dividend-yield scalar is accepted as either `Unitless` or `Price`
//! (matching the equity-option calculator); earlier revisions rejected a
//! `Price`-typed scalar here.

use crate::dividend01::dividend01_central_diff;
use crate::equity_trs::EquityTotalReturnSwap;
use finstack_quant_core::Result;
use finstack_quant_valuations_core::metrics::{MetricCalculator, MetricContext};

/// Dividend risk (Dividend01) calculator for equity TRS.
pub(crate) struct Dividend01Calculator;

impl MetricCalculator for Dividend01Calculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let trs: &EquityTotalReturnSwap = context.instrument_as()?;
        dividend01_central_diff(trs, trs.underlying.div_yield_id.as_ref(), context)
    }
}
