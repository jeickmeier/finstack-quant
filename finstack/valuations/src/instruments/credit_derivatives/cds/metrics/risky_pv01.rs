//! CDS risky PV01 metric calculator.
//!
//! Returns the canonical Risky PV01 = `Risky Annuity × Notional / 10000`.
//! When the instrument carries a deal quote (`pricing_overrides.cds_quote_bp`)
//! and is priced clean, the calculator delegates to the CS01 path so the
//! reported PV01 is consistent with the deal-quote-based hazard rebootstrap.

use crate::instruments::credit_derivatives::cds::pricer::CDSPricer;
use crate::instruments::credit_derivatives::cds::CreditDefaultSwap;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_core::Result;

/// Risky PV01 calculator for CDS
pub(crate) struct RiskyPv01Calculator;

impl MetricCalculator for RiskyPv01Calculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let use_deal_quote = {
            let cds: &CreditDefaultSwap = context.instrument_as()?;
            cds.uses_clean_price() && cds.pricing_overrides.market_quotes.cds_quote_bp.is_some()
        };
        if use_deal_quote {
            return crate::metrics::sensitivities::cs01::CreditParallelCs01::<
                CreditDefaultSwap,
            >::default()
            .calculate(context);
        }

        let cds: &CreditDefaultSwap = context.instrument_as()?;
        let disc = context
            .curves
            .get_discount(&cds.premium.discount_curve_id)?;
        let surv = context.curves.get_hazard(&cds.protection.credit_curve_id)?;
        CDSPricer::new().risky_pv01(cds, disc.as_ref(), surv.as_ref(), context.as_of)
    }
}
