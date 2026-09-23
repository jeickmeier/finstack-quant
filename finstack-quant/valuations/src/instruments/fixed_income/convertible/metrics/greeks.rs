//! Greeks metrics for `ConvertibleBond`.
//!
//! Delta, Gamma, Vega and Rho come from one tree-based
//! [`ConvertibleBond::greeks`] run per metric context: the first requested
//! Greek computes the full set and seeds the others into `context.computed`,
//! so the registry does not reprice the lattice for each of them.

use crate::metrics::{MetricCalculator, MetricContext, MetricId};
use finstack_quant_core::Result;

use crate::instruments::fixed_income::convertible::types::ConvertibleBond;

/// Compute every tree Greek once, seed the siblings, and return `requested`.
fn tree_greek(context: &mut MetricContext, requested: MetricId) -> Result<f64> {
    let bond = context.instrument_as::<ConvertibleBond>()?;
    let greeks = bond.greeks(&context.curves, None, None, context.as_of)?;
    let values = [
        (MetricId::Delta, greeks.delta),
        (MetricId::Gamma, greeks.gamma),
        (MetricId::Vega, greeks.vega),
        (MetricId::Rho, greeks.rho),
    ];
    let mut result = None;
    for (id, value) in values {
        if id == requested {
            result = Some(value);
        } else {
            context.computed.entry(id).or_insert(value);
        }
    }
    result.ok_or_else(|| finstack_quant_core::Error::internal("unsupported convertible Greek"))
}

pub(crate) struct DeltaCalculator;
impl MetricCalculator for DeltaCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        tree_greek(context, MetricId::Delta)
    }
}

pub(crate) struct GammaCalculator;
impl MetricCalculator for GammaCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        tree_greek(context, MetricId::Gamma)
    }
}

pub(crate) struct VegaCalculator;
impl MetricCalculator for VegaCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        tree_greek(context, MetricId::Vega)
    }
}

pub(crate) struct RhoCalculator;
impl MetricCalculator for RhoCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        tree_greek(context, MetricId::Rho)
    }
}

// Theta calculator is implemented in `metrics/theta.rs` to share a generic implementation.
