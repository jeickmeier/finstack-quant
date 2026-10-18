//! Pricing and metric helpers for interest-rate instruments.
//!
use crate::basis_swap::types::BasisSwap;
use finstack_quant_core::Result;
use finstack_quant_valuations_core::metrics::{MetricCalculator, MetricContext};

/// Calculator for the discounted accrual sum (annuity) of a basis swap leg.
///
/// The annuity represents the sum of discounted year fractions for a leg,
/// which is used in DV01 calculations and par spread computations.
///
/// See unit tests and `examples/` for usage.
pub(crate) struct AnnuityCalculator {
    /// Whether this calculator is for the primary leg (true) or reference leg (false).
    pub(crate) is_primary: bool,
}

impl AnnuityCalculator {
    /// Creates a calculator for the primary leg.
    pub(crate) const fn primary() -> Self {
        Self { is_primary: true }
    }

    /// Creates a calculator for the reference leg.
    pub(crate) const fn reference() -> Self {
        Self { is_primary: false }
    }
}

impl MetricCalculator for AnnuityCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let swap = context.instrument_as::<BasisSwap>()?;
        let curves = std::sync::Arc::clone(&context.curves);

        let leg = if self.is_primary {
            &swap.primary_leg
        } else {
            &swap.reference_leg
        };

        swap.annuity_for_leg(leg, curves.as_ref(), context.as_of)
    }
}
