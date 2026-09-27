//! Underlying CDS spread DV01 metric for `CdsOption`.
//!
//! Reports the Bloomberg CDSW-style Spread DV01 of the synthetic underlying CDS
//! linked from a CDS option, not the option's own CS01. This follows the CDSW
//! screen convention: apply a +1bp parallel bump to the quoted CDS par-spread
//! curve, re-bootstrap the hazard curve, and revalue the underlying CDS.

use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::credit_derivatives::cds_option::pricer::synthetic_underlying_cds;
use crate::instruments::credit_derivatives::cds_option::CdsOption;
use crate::metrics::{MetricCalculator, MetricContext};
use crate::recalibration::{HazardRecalibrationAction, HazardRecalibrationRequest, QuoteBump};
use finstack_quant_core::Result;

/// Spread DV01 calculator for the option's synthetic underlying CDS.
pub(crate) struct UnderlyingSpreadDv01Calculator;

impl MetricCalculator for UnderlyingSpreadDv01Calculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let option: &CdsOption = context.instrument_as()?;
        let cds = synthetic_underlying_cds(option, context.as_of)?;
        let hazard = context.curves.get_hazard(&option.credit_curve_id)?;
        crate::metrics::sensitivities::cs01::require_hazard_replay(
            hazard.as_ref(),
            "CDS option underlying spread DV01",
        )?;
        let bumped_hazard = context.rebuild_hazard_curve(
            HazardRecalibrationRequest {
                hazard: std::sync::Arc::clone(&hazard),
                source_market: std::sync::Arc::clone(&context.curves),
                target_market: std::sync::Arc::clone(&context.curves),
                discount_curve_id: option.discount_curve_id.clone(),
                doc_clause: None,
                cds_valuation_convention: None,
                deal_quote_override: None,
                action: HazardRecalibrationAction::SpreadBump(QuoteBump::ParallelBp(1.0)),
            },
            "cs01",
        )?;
        let bumped_market = context
            .curves
            .as_ref()
            .clone()
            .insert(bumped_hazard.as_ref().clone());
        let base_pv = cds.value(&context.curves, context.as_of)?.amount();
        let bumped_pv = cds.value(&bumped_market, context.as_of)?.amount();
        Ok(bumped_pv - base_pv)
    }
}
