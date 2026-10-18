//! CDS risky annuity metric calculator.
//!
//! Returns the canonical risky annuity `Σ DF(t) × SP(t) × YearFrac` (sum over
//! coupon periods), independent of the par-spread denominator policy. The par
//! spread metric handles its own denominator choice via the CDS valuation
//! convention.

use crate::cds::pricing::CdsPricer;
use crate::cds::CreditDefaultSwap;
use finstack_quant_core::Result;
use finstack_quant_valuations_core::metrics::{MetricCalculator, MetricContext};

/// Risky annuity calculator for CDS
pub(crate) struct RiskyAnnuityCalculator;

impl MetricCalculator for RiskyAnnuityCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let cds: &CreditDefaultSwap = context.instrument_as()?;
        let disc = context
            .curves
            .get_discount(&cds.premium_leg.discount_curve_id)?;
        let surv = context
            .curves
            .get_hazard(&cds.protection_leg.credit_curve_id)?;
        CdsPricer::new().risky_annuity(cds, disc.as_ref(), surv.as_ref(), context.as_of)
    }
}
