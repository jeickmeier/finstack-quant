//! CDS risky PV01 metric calculator.
//!
//! Returns the canonical Risky PV01 = `Risky Annuity × Notional / 10000`.

use crate::instruments::credit_derivatives::cds::pricing::CdsPricer;
use crate::instruments::credit_derivatives::cds::CreditDefaultSwap;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_quant_core::Result;

/// Risky PV01 calculator for CDS
pub(crate) struct RiskyPv01Calculator;

impl MetricCalculator for RiskyPv01Calculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let cds: &CreditDefaultSwap = context.instrument_as()?;
        let disc = context
            .curves
            .get_discount(&cds.premium_leg.discount_curve_id)?;
        let surv = context
            .curves
            .get_hazard(&cds.protection_leg.credit_curve_id)?;
        CdsPricer::new().risky_pv01(cds, disc.as_ref(), surv.as_ref(), context.as_of)
    }
}
