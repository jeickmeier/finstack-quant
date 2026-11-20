//! CDS option discount risk with replay of calibrated underlying hazards.

use crate::cds::metrics::dv01::{
    DiscountShock, HazardReplay, calculate_credit_dv01, rate_bumped_market_with_hazard_replay,
};
use crate::cds_option::CdsOption;
use finstack_quant_core::Result;
use finstack_quant_valuations_core::metrics::{MetricCalculator, MetricContext};
use std::sync::Arc;

pub(crate) struct CdsOptionDv01Calculator(pub(crate) bool);

impl MetricCalculator for CdsOptionDv01Calculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let option = context.instrument_as::<CdsOption>()?;
        let discount_id = option.discount_curve_id.clone();
        let hazard = context.curves.get_hazard(&option.credit_curve_id)?;
        let reprice = |context: &MetricContext, shock: DiscountShock| -> Result<f64> {
            let replay = HazardReplay {
                hazard: Arc::clone(&hazard),
                doc_clause: None,
                cds_valuation_convention: None,
                deal_quote_override: None,
            };
            let (market, _) =
                rate_bumped_market_with_hazard_replay(context, &discount_id, shock, [replay])?;
            context.reprice_raw(&market, context.as_of)
        };
        calculate_credit_dv01(context, &discount_id, self.0, reprice)
    }
}
