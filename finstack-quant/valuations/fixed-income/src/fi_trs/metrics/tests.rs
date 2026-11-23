use super::*;
use crate::fi_trs::FiIndexTotalReturnSwap;
use finstack_quant_core::{currency::Currency, market_data::context::MarketContext, money::Money};
use finstack_quant_valuations_core::metrics::{MetricCalculator, MetricContext};
use std::sync::Arc;
use time::macros::date;

#[test]
fn duration_risk_requires_a_duration_input() {
    let mut trs = FiIndexTotalReturnSwap::example().expect("trs");
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
