//! CDS tranche DV01 with par-spread hazard re-bootstrap when possible.
//!
//! Same rate-bump-and-replay step as single-name CDS DV01
//! ([`rate_bumped_market_with_hazard_replay`]), applied to every credit curve
//! the tranche's credit index feeds into pricing (`try_insert_mut` auto-rebinds
//! dependent credit indices).

use super::cs01::active_hazards;
use crate::cds::metrics::dv01::{
    DiscountShock, HazardReplay, calculate_credit_dv01, rate_bumped_market_with_hazard_replay,
};
use crate::cds_tranche::CdsTranche;
use finstack_quant_core::Result;
use finstack_quant_valuations_core::metrics::{MetricCalculator, MetricContext};
use std::sync::Arc;

pub(crate) struct CdsTrancheDv01Calculator(pub(crate) bool);

impl MetricCalculator for CdsTrancheDv01Calculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let tranche = context.instrument_as::<CdsTranche>()?.clone();
        let index = context.curves.get_credit_index(&tranche.credit_index_id)?;
        let hazards = active_hazards(&index);

        let reprice = |context: &MetricContext, shock: DiscountShock| -> Result<f64> {
            // Tranche hazards come from the credit index, not from a CDS
            // contract, so there is no doc clause, valuation convention or
            // deal quote to assert against their recipes.
            let replays = hazards.iter().map(|hazard| HazardReplay {
                hazard: Arc::clone(hazard),
                doc_clause: None,
                cds_valuation_convention: None,
                deal_quote_override: None,
            });
            let bumped_market = rate_bumped_market_with_hazard_replay(
                context,
                &tranche.discount_curve_id,
                shock,
                replays,
            )?;
            context.reprice_raw(&bumped_market, context.as_of)
        };

        calculate_credit_dv01(context, &tranche.discount_curve_id, self.0, reprice)
    }
}
