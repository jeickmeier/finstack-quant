//! Recovery01 calculator for CDS Option.
//!
//! Computes Recovery01 (recovery rate sensitivity) using finite differences.
//! Recovery01 measures the change in PV for a 1% (100bp) change in recovery rate.
//!
//! When the hazard curve carries its bootstrap par-spread quotes, Recovery01
//! reboots the hazard curve under the bumped recovery so quoted spreads remain
//! invariant. Curves without par quotes fall back to a frozen-curve local bump.
//! The finite-difference scheme and the bump bounds are the CDS ones
//! ([`crate::cds::metrics::recovery01`]).

use crate::cds::metrics::market_doc_clause;
use crate::cds::metrics::recovery01::{market_with_recovery, recovery01_fd, RecoveryReplayTerms};
use crate::cds_option::pricer::synthetic_underlying_cds;
use crate::cds_option::CdsOption;
use finstack_quant_core::Result;
use finstack_quant_valuations_core::instruments::Instrument;
use finstack_quant_valuations_core::metrics::{MetricCalculator, MetricContext};

/// Recovery01 calculator for CDS Option.
pub(crate) struct Recovery01Calculator;

impl MetricCalculator for Recovery01Calculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let option: &CdsOption = context.instrument_as()?;
        let as_of = context.as_of;
        let synthetic = synthetic_underlying_cds(option, as_of)?;
        let terms = RecoveryReplayTerms {
            discount_curve_id: option.discount_curve_id.clone(),
            doc_clause: Some(market_doc_clause(&synthetic)),
            cds_valuation_convention: Some(synthetic.valuation_convention),
            deal_quote_override: None,
        };
        recovery01_fd(
            option.recovery_rate,
            context.base_value.amount(),
            |recovery| {
                let mut bumped_option = option.clone();
                bumped_option.recovery_rate = recovery;
                let market = market_with_recovery(
                    context,
                    &context.curves,
                    option.credit_curve_id.as_str(),
                    recovery,
                    &terms,
                    "CDS option Recovery01",
                )?;
                Ok(bumped_option.value(&market, as_of)?.amount())
            },
        )
    }
}
