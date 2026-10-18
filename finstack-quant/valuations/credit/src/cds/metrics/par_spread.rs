//! CDS par spread metric calculator.
//!
//! Computes the fixed spread in basis points that sets the CDS NPV to zero,
//! using the pricing engine's par-spread calculation. This is independent
//! of the instrument's current quoted spread.

use crate::cds::pricing::{CdsPricer, CdsPricerConfig};
use crate::cds::CreditDefaultSwap;
use finstack_quant_core::Result;
use finstack_quant_valuations_core::metrics::{MetricCalculator, MetricContext};

/// Par spread calculator for CDS
pub(crate) struct ParSpreadCalculator;

impl MetricCalculator for ParSpreadCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let cds: &CreditDefaultSwap = context.instrument_as()?;
        let disc = context
            .curves
            .get_discount(&cds.premium_leg.discount_curve_id)?;
        let surv = context
            .curves
            .get_hazard(&cds.protection_leg.credit_curve_id)?;
        let pricer = CdsPricer::with_config(CdsPricerConfig::from_cds(cds));
        pricer.par_spread(cds, disc.as_ref(), surv.as_ref(), context.as_of)
    }
}
