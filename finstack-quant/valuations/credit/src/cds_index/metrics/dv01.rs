//! CDS index DV01 with par-spread hazard re-bootstrap when possible.
//!
//! Same rate-bump-and-replay step as single-name CDS DV01
//! ([`rate_bumped_market_with_hazard_replay`]), applied to the index curve in
//! single-curve mode or to every live constituent curve in constituents mode.

use crate::cds::metrics::dv01::{rate_bumped_market_with_hazard_replay, HazardReplay};
use crate::cds::metrics::{deal_quote_override, market_doc_clause};
use crate::cds_index::{CdsIndex, IndexPricing};
use finstack_quant_core::Result;
use finstack_quant_valuations_core::metrics::sensitivities::config as sens_config;
use finstack_quant_valuations_core::metrics::sensitivities::cs01::sensitivity_central_diff;
use finstack_quant_valuations_core::metrics::{MetricCalculator, MetricContext};
use std::collections::BTreeSet;
use std::sync::Arc;

pub(crate) struct CdsIndexDv01Calculator;

impl MetricCalculator for CdsIndexDv01Calculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let idx = context.instrument_as::<CdsIndex>()?.clone();
        let bump_bp = sens_config::resolve(context)?.rate_bump_bp;
        if bump_bp.abs() <= 1e-10 {
            return Ok(0.0);
        }
        let synthetic = idx.to_synthetic_cds()?;
        let doc_clause = Some(market_doc_clause(&synthetic));
        let cds_valuation_convention = Some(synthetic.valuation_convention);

        let (curve_ids, quote_override) = match idx.pricing {
            IndexPricing::SingleCurve => (
                vec![idx.protection_leg.credit_curve_id.clone()],
                deal_quote_override(&synthetic),
            ),
            // The deal quote is an index-level spread; it does not describe
            // any single constituent curve, so constituents replay unchanged.
            IndexPricing::Constituents => (
                idx.constituents
                    .iter()
                    .filter(|con| !con.defaulted)
                    .map(|con| con.credit.credit_curve_id.clone())
                    .collect(),
                None,
            ),
        };
        let hazards = curve_ids
            .into_iter()
            .collect::<BTreeSet<_>>()
            .iter()
            .map(|curve_id| context.curves.get_hazard(curve_id.as_str()))
            .collect::<Result<Vec<_>>>()?;

        let reprice = |bp: f64| -> Result<f64> {
            let replays = hazards.iter().map(|hazard| HazardReplay {
                hazard: Arc::clone(hazard),
                doc_clause,
                cds_valuation_convention,
                deal_quote_override: quote_override,
            });
            let (bumped_market, _) = rate_bumped_market_with_hazard_replay(
                context,
                &idx.premium_leg.discount_curve_id,
                bp,
                replays,
            )?;
            context.reprice_raw(&bumped_market, context.as_of)
        };

        Ok(sensitivity_central_diff(
            reprice(bump_bp)?,
            reprice(-bump_bp)?,
            bump_bp,
        ))
    }
}
