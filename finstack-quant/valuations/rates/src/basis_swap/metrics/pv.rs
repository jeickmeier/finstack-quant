//! Pricing and metric helpers for interest-rate instruments.
//!
use crate::basis_swap::types::BasisSwap;
use finstack_quant_core::Result;
use finstack_quant_valuations_core::metrics::{MetricCalculator, MetricContext};

/// Calculator for the present value of a basis swap leg.
///
/// This calculator computes the present value of either the primary or reference
/// leg of a basis swap, including the spread and forward rate components.
///
/// See unit tests and `examples/` for usage.
pub(crate) struct PvCalculator {
    /// Whether this calculator is for the primary leg (true) or reference leg (false).
    pub(crate) is_primary: bool,
}

impl PvCalculator {
    /// Creates a calculator for the primary leg.
    pub(crate) const fn primary() -> Self {
        Self { is_primary: true }
    }

    /// Creates a calculator for the reference leg.
    pub(crate) const fn reference() -> Self {
        Self { is_primary: false }
    }
}

impl MetricCalculator for PvCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let swap = context.instrument_as::<BasisSwap>()?;
        let curves = std::sync::Arc::clone(&context.curves);
        let as_of = context.as_of;

        let leg = if self.is_primary {
            &swap.primary_leg
        } else {
            &swap.reference_leg
        };

        let pv = swap.pv_float_leg(leg, curves.as_ref(), as_of)?;
        Ok(pv.amount())
    }
}
