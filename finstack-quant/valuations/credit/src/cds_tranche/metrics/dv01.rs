//! CDS tranche DV01 with par-spread hazard re-bootstrap when possible.
//!
//! Same rate-bump-and-replay step as single-name CDS DV01
//! ([`rate_bumped_market_with_hazard_replay`]), applied to every credit curve
//! the tranche's credit index feeds into pricing. Rebuilt curves are swapped
//! into the credit index as well as the market, since the tranche pricer reads
//! hazards through the index.

use super::cs01::{active_hazards, replace_hazard};
use crate::cds::metrics::dv01::{rate_bumped_market_with_hazard_replay, HazardReplay};
use crate::cds_tranche::CdsTranche;
use finstack_quant_core::Result;
use finstack_quant_valuations_core::metrics::sensitivities::config as sens_config;
use finstack_quant_valuations_core::metrics::sensitivities::cs01::sensitivity_central_diff;
use finstack_quant_valuations_core::metrics::{MetricCalculator, MetricContext};
use std::sync::Arc;

pub(crate) struct CdsTrancheDv01Calculator;

impl MetricCalculator for CdsTrancheDv01Calculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let tranche = context.instrument_as::<CdsTranche>()?.clone();
        let bump_bp = sens_config::resolve(context)?.rate_bump_bp;
        if bump_bp.abs() <= 1e-10 {
            return Ok(0.0);
        }
        let index = context.curves.get_credit_index(&tranche.credit_index_id)?;
        let hazards = active_hazards(&index);

        let reprice = |bp: f64| -> Result<f64> {
            // Tranche hazards come from the credit index, not from a CDS
            // contract, so there is no doc clause, valuation convention or
            // deal quote to assert against their recipes.
            let replays = hazards.iter().map(|hazard| HazardReplay {
                hazard: Arc::clone(hazard),
                doc_clause: None,
                cds_valuation_convention: None,
                deal_quote_override: None,
            });
            let (mut bumped_market, rebuilt) = rate_bumped_market_with_hazard_replay(
                context,
                &tranche.discount_curve_id,
                bp,
                replays,
            )?;
            if !rebuilt.is_empty() {
                let mut bumped_index = index.as_ref().clone();
                for hazard in rebuilt {
                    bumped_index = replace_hazard(&bumped_index, hazard)?;
                }
                bumped_market = bumped_market
                    .insert_credit_index(tranche.credit_index_id.as_str(), bumped_index)?;
            }
            context.reprice_raw(&bumped_market, context.as_of)
        };

        Ok(sensitivity_central_diff(
            reprice(bump_bp)?,
            reprice(-bump_bp)?,
            bump_bp,
        ))
    }
}
