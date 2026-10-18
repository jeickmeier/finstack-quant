//! CDS protection leg PV metric calculator.
//!
//! Computes present value of the protection leg using the configured curves
//! and the engine's implementation. The measure is returned in currency units.

use crate::cds::pricing::CdsPricer;
use crate::cds::CreditDefaultSwap;
use finstack_quant_core::Result;
use finstack_quant_valuations_core::metrics::{MetricCalculator, MetricContext};

/// Protection leg PV calculator
pub(crate) struct ProtectionLegPvCalculator;

impl MetricCalculator for ProtectionLegPvCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let cds: &CreditDefaultSwap = context.instrument_as()?;
        let disc = context
            .curves
            .get_discount(&cds.premium_leg.discount_curve_id)?;
        let surv = context
            .curves
            .get_hazard(&cds.protection_leg.credit_curve_id)?;
        let pricer = CdsPricer::new();
        let pv = pricer.pv_protection_leg(cds, disc.as_ref(), surv.as_ref(), context.as_of)?;
        Ok(pv.amount())
    }
}
