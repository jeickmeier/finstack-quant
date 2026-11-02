//! Recovery01 calculator for CDS.
//!
//! PV change per +1% (0.01 absolute) recovery-rate move, by the finite
//! differences and hazard recalibration of
//! [`finstack_quant_valuations_core::metrics::sensitivities::recovery01`]. A
//! recovery change affects both the protection leg (LGD = 1 - recovery) and the
//! premium leg (accrued on default settlement); this metric captures the full
//! direct sensitivity across both legs.

use super::{deal_quote_override, market_doc_clause};
use crate::cds::CreditDefaultSwap;
use finstack_quant_core::Result;
use finstack_quant_valuations_core::instruments::Instrument;
use finstack_quant_valuations_core::metrics::sensitivities::recovery01::{
    market_with_recovery, recovery01_fd, RecoveryReplayTerms,
};
use finstack_quant_valuations_core::metrics::{MetricCalculator, MetricContext};

/// Recovery01 calculator for CDS.
pub(crate) struct Recovery01Calculator;

impl MetricCalculator for Recovery01Calculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let cds: &CreditDefaultSwap = context.instrument_as()?;
        let as_of = context.as_of;
        let terms = RecoveryReplayTerms {
            discount_curve_id: cds.premium_leg.discount_curve_id.clone(),
            doc_clause: Some(market_doc_clause(cds)),
            cds_valuation_convention: Some(cds.valuation_convention),
            deal_quote_override: deal_quote_override(cds),
        };
        recovery01_fd(
            cds.protection_leg.recovery_rate,
            context.base_value.amount(),
            |recovery| {
                let mut bumped_cds = cds.clone();
                bumped_cds.protection_leg.recovery_rate = recovery;
                let market = market_with_recovery(
                    context,
                    &context.curves,
                    cds.protection_leg.credit_curve_id.as_str(),
                    recovery,
                    &terms,
                    "CDS Recovery01",
                )?;
                Ok(bumped_cds.value(&market, as_of)?.amount())
            },
        )
    }
}
