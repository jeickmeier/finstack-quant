//! CDS premium leg PV metric calculator.
//!
//! Computes present value of the premium leg using discount and hazard curves
//! via the engine. The value is returned in currency units.

use crate::cds::pricing::CdsPricer;
use crate::cds::CreditDefaultSwap;
use finstack_quant_core::Result;
use finstack_quant_valuations_core::metrics::{MetricCalculator, MetricContext};

/// Premium leg PV calculator
pub(crate) struct PremiumLegPvCalculator;

impl MetricCalculator for PremiumLegPvCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let cds: &CreditDefaultSwap = context.instrument_as()?;
        let disc = context
            .curves
            .get_discount(&cds.premium_leg.discount_curve_id)?;
        let surv = context
            .curves
            .get_hazard(&cds.protection_leg.credit_curve_id)?;
        let pricer = CdsPricer::new();
        let pv = pricer.pv_premium_leg(cds, disc.as_ref(), surv.as_ref(), context.as_of)?;
        Ok(pv.amount())
    }
}
