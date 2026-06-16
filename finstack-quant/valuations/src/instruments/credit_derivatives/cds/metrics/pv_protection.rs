//! CDS protection leg PV metric calculator.
//!
//! Computes present value of the protection leg using the configured curves
//! and the engine's implementation. The measure is returned in currency units.

use crate::instruments::credit_derivatives::cds::pricer::CDSPricer;
use crate::instruments::credit_derivatives::cds::CreditDefaultSwap;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_quant_core::Result;

/// Protection leg PV calculator
pub(crate) struct ProtectionLegPvCalculator;

impl MetricCalculator for ProtectionLegPvCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let cds: &CreditDefaultSwap = context.instrument_as()?;
        let disc = context
            .curves
            .get_discount(&cds.premium.discount_curve_id)?;
        let surv = context.curves.get_hazard(&cds.protection.credit_curve_id)?;
        let pricer = CDSPricer::new();
        let pv = pricer.pv_protection_leg(cds, disc.as_ref(), surv.as_ref(), context.as_of)?;
        Ok(pv.amount())
    }
}
