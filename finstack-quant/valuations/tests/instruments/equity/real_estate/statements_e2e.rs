//! Pricing and metric helpers for equity instruments.
//!
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount, PeriodId};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::types::{CurveId, InstrumentId};
use finstack_quant_statements::builder::ModelBuilder;
use finstack_quant_statements::evaluator::{
    node_to_dated_schedule, Evaluator, PeriodDateConvention,
};
use finstack_quant_statements::types::AmountOrScalar;
use finstack_quant_statements_analytics::templates::RealEstateExtension;
use finstack_quant_valuations::instruments::equity::real_estate::{
    RealEstateAsset, RealEstateValuationMethod,
};
use finstack_quant_valuations::instruments::Instrument;

#[test]
fn e2e_statements_to_real_estate_asset_cashflows_prices() {
    let as_of = Date::from_calendar_date(2025, time::Month::January, 1).unwrap();

    // Build a tiny statement model with NOI and CapEx, then export to dated schedules.
    let model = ModelBuilder::new("re_e2e")
        .periods("2025Q1..Q2", None)
        .expect("periods should parse")
        .value(
            "rent",
            &[
                (PeriodId::quarter(2025, 1), AmountOrScalar::scalar(100.0)),
                (PeriodId::quarter(2025, 2), AmountOrScalar::scalar(110.0)),
            ],
        )
        .value(
            "taxes",
            &[
                (PeriodId::quarter(2025, 1), AmountOrScalar::scalar(20.0)),
                (PeriodId::quarter(2025, 2), AmountOrScalar::scalar(22.0)),
            ],
        )
        .value(
            "capex",
            &[
                (PeriodId::quarter(2025, 1), AmountOrScalar::scalar(5.0)),
                (PeriodId::quarter(2025, 2), AmountOrScalar::scalar(6.0)),
            ],
        )
        .add_noi_buildup("total_rev", &["rent"], "total_exp", &["taxes"], "noi")
        .expect("noi template")
        .build()
        .expect("build");

    let mut eval = Evaluator::new();
    let results = eval.evaluate(&model).expect("evaluate");

    let noi_sched = node_to_dated_schedule(&model, &results, "noi", PeriodDateConvention::End)
        .expect("noi sched");
    let capex_sched = node_to_dated_schedule(&model, &results, "capex", PeriodDateConvention::End)
        .expect("capex sched");

    // RealEstateAsset uses date-based schedules.
    let asset = RealEstateAsset::builder()
        .id(InstrumentId::new("RE-E2E"))
        .currency(Currency::USD)
        .valuation_date(as_of)
        .valuation_method(RealEstateValuationMethod::Dcf)
        .noi_schedule(noi_sched)
        .capex_schedule_opt(Some(capex_sched))
        .discount_rate_opt(Some(0.0)) // PV = sum flows
        .terminal_cap_rate_opt(None) // no terminal value
        .day_count(DayCount::Act365F)
        .discount_curve_id(CurveId::new("USD-OIS"))
        .build()
        .expect("asset build");

    let market = MarketContext::new();
    let pv = asset.value(&market, as_of).expect("pv").amount();

    // Unlevered flows = NOI - CapEx on the same dates.
    // Q1: 100-20 - 5 = 75
    // Q2: 110-22 - 6 = 82
    assert!((pv - (75.0 + 82.0)).abs() < 1e-10);
}
