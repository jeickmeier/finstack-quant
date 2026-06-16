//! Trivial metric calculators for CDS Index that delegate to the index pricer.
//!
//! Each calculator here is a 3-line forward to a method on `CDSIndex`. Kept
//! together to reduce per-metric file overhead. Metrics with non-trivial
//! logic (CS01, expected loss, jump-to-default, recovery01) live in their
//! own files.

use crate::instruments::credit_derivatives::cds_index::CDSIndex;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_quant_core::Result;

/// Par spread calculator for CDS Index (basis points that zero the NPV).
pub(crate) struct ParSpreadCalculator;

impl MetricCalculator for ParSpreadCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let idx: &CDSIndex = context.instrument_as()?;
        idx.par_spread(&context.curves, context.as_of)
    }
}

/// Risky PV01 calculator for CDS Index.
pub(crate) struct RiskyPv01Calculator;

impl MetricCalculator for RiskyPv01Calculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let idx: &CDSIndex = context.instrument_as()?;
        idx.risky_pv01(&context.curves, context.as_of)
    }
}

/// Premium leg PV calculator for CDS Index.
pub(crate) struct PremiumLegPvCalculator;

impl MetricCalculator for PremiumLegPvCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let idx: &CDSIndex = context.instrument_as()?;
        Ok(idx.pv_premium_leg(&context.curves, context.as_of)?.amount())
    }
}

/// Protection leg PV calculator for CDS Index.
pub(crate) struct ProtectionLegPvCalculator;

impl MetricCalculator for ProtectionLegPvCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let idx: &CDSIndex = context.instrument_as()?;
        Ok(idx
            .pv_protection_leg(&context.curves, context.as_of)?
            .amount())
    }
}
