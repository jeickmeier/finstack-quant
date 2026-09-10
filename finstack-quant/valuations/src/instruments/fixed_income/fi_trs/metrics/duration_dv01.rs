//! Duration-based DV01 calculator for fixed income index TRS.

use crate::instruments::common_impl::parameters::trs_common::TrsSide;
use crate::instruments::fixed_income::fi_trs::FIIndexTotalReturnSwap;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_quant_core::Result;

/// Calculates duration-based DV01 for fixed income index TRS.
///
/// Measures the dollar value change for a 1 basis point yield shift:
///
/// ```text
/// DurationDv01 = Notional × Duration × 0.0001
/// ```
///
/// This is a yield sensitivity metric (not an index-level delta). For equity TRS,
/// use `IndexDelta` which measures `dV/dS` per unit of index level change.
///
/// # Sign Convention
///
/// Returns **negative** for `ReceiveTotalReturn` and **positive** for
/// `PayTotalReturn`. A total-return receiver is economically long the reference
/// bond index, so its signed rate sensitivity is negative: the position loses
/// value when yields rise. This matches the ISDA SIMM IR-delta convention (see
/// the `Marginable::simm_sensitivities` implementation for this instrument) and
/// the workspace CS01 convention (long credit → negative to spread widening).
///
/// Note: the underlying FI-TRS pricer is carry/income-only — it has no price
/// mark-to-market term, so `dV/dy` of the carry model itself would not produce
/// this sign. The convention here is chosen for economic consistency with the
/// rest of the workspace, not derived from the carry model.
///
/// # Errors
///
/// Requires `duration_id` and its finite unitless market scalar
/// in years. Missing inputs or a monetary scalar return an error.
pub(crate) struct DurationDv01Calculator;

impl MetricCalculator for DurationDv01Calculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let trs: &FIIndexTotalReturnSwap = context.instrument_as()?;

        let duration = trs.index_duration(context.curves.as_ref())?;

        // DV01 = Notional × Duration × 1bp
        let dv01 = trs.notional.amount() * duration * 0.0001;

        // Long the reference bond (receive TR) loses when yields rise → negative.
        Ok(match trs.side {
            TrsSide::ReceiveTotalReturn => -dv01,
            TrsSide::PayTotalReturn => dv01,
        })
    }
}

#[cfg(test)]
mod production_mortgage_audit {
    use super::*;
    use finstack_quant_core::{
        currency::Currency, market_data::context::MarketContext, money::Money,
    };
    use std::sync::Arc;
    use time::macros::date;

    #[test]
    fn duration_risk_requires_a_duration_input() {
        let mut trs = FIIndexTotalReturnSwap::example().expect("trs");
        trs.underlying.duration_id = None;
        let mut context = MetricContext::new(
            Arc::new(trs),
            Arc::new(MarketContext::new()),
            date!(2024 - 01 - 15),
            Money::from((0_i64, Currency::USD)),
            MetricContext::default_config(),
        );
        assert!(
            DurationDv01Calculator.calculate(&mut context).is_err(),
            "missing duration must not fabricate five years"
        );
    }
}
