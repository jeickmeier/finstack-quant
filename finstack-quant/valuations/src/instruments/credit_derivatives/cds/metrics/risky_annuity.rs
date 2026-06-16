//! CDS risky annuity metric calculator.
//!
//! Returns the canonical risky annuity `Σ DF(t) × SP(t) × YearFrac` (sum over
//! coupon periods), independent of the par-spread denominator policy. The par
//! spread metric handles its own denominator choice via the CDS valuation
//! convention.

use crate::instruments::credit_derivatives::cds::pricer::CDSPricer;
use crate::instruments::credit_derivatives::cds::CreditDefaultSwap;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_quant_core::Result;

/// Risky annuity calculator for CDS
pub(crate) struct RiskyAnnuityCalculator;

impl MetricCalculator for RiskyAnnuityCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let cds: &CreditDefaultSwap = context.instrument_as()?;
        let disc = context
            .curves
            .get_discount(&cds.premium.discount_curve_id)?;
        let surv = context.curves.get_hazard(&cds.protection.credit_curve_id)?;
        CDSPricer::new().risky_annuity(cds, disc.as_ref(), surv.as_ref(), context.as_of)
    }
}
