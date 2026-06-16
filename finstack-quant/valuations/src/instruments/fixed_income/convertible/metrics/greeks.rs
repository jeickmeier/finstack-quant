//! Greeks metrics for `ConvertibleBond`.
//!
//! Provides Delta, Gamma, Vega, Rho, Theta using the tree-based greeks from
//! the pricing engine.

use crate::metrics::{MetricCalculator, MetricContext};
use finstack_quant_core::Result;

use crate::instruments::fixed_income::convertible::types::ConvertibleBond;

pub(crate) struct DeltaCalculator;
impl MetricCalculator for DeltaCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let bond = context.instrument_as::<ConvertibleBond>()?;
        bond.delta(&context.curves, context.as_of)
    }
}

pub(crate) struct GammaCalculator;
impl MetricCalculator for GammaCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let bond = context.instrument_as::<ConvertibleBond>()?;
        bond.gamma(&context.curves, context.as_of)
    }
}

pub(crate) struct VegaCalculator;
impl MetricCalculator for VegaCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let bond = context.instrument_as::<ConvertibleBond>()?;
        bond.vega(&context.curves, context.as_of)
    }
}

pub(crate) struct RhoCalculator;
impl MetricCalculator for RhoCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let bond = context.instrument_as::<ConvertibleBond>()?;
        bond.rho(&context.curves, context.as_of)
    }
}

// Theta calculator is implemented in `metrics/theta.rs` to share a generic implementation.
