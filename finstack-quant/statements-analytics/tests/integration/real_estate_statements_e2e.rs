//! End-to-end test: statement model → dated schedules → real-estate asset pricing.
//!
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount, PeriodId};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::types::InstrumentId;
use finstack_quant_statements::builder::ModelBuilder;
use finstack_quant_statements::evaluator::{
    Evaluator, PeriodDateConvention, node_to_dated_schedule,
};
use finstack_quant_statements::types::AmountOrScalar;
use finstack_quant_statements_analytics::templates::real_estate;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::instruments::equity::real_estate::{
    RealEstateAsset, RealEstateValuationMethod,
};

#[test]
fn e2e_statements_to_real_estate_asset_cashflows_prices() {
    let as_of = Date::from_calendar_date(2025, time::Month::January, 1).unwrap();

    // Build a tiny statement model with NOI and CapEx, then export to dated schedules.
    let mut model_builder = ModelBuilder::new("re_e2e");
    model_builder
        .periods("2025Q1..Q2", None)
        .expect("periods should parse");
    model_builder.value(
        "rent",
        &[
            (
                PeriodId::quarter(2025, 1).expect("valid period fixture"),
                AmountOrScalar::scalar(100.0),
            ),
            (
                PeriodId::quarter(2025, 2).expect("valid period fixture"),
                AmountOrScalar::scalar(110.0),
            ),
        ],
    );
    model_builder.value(
        "taxes",
        &[
            (
                PeriodId::quarter(2025, 1).expect("valid period fixture"),
                AmountOrScalar::scalar(20.0),
            ),
            (
                PeriodId::quarter(2025, 2).expect("valid period fixture"),
                AmountOrScalar::scalar(22.0),
            ),
        ],
    );
    model_builder.value(
        "capex",
        &[
            (
                PeriodId::quarter(2025, 1).expect("valid period fixture"),
                AmountOrScalar::scalar(5.0),
            ),
            (
                PeriodId::quarter(2025, 2).expect("valid period fixture"),
                AmountOrScalar::scalar(6.0),
            ),
        ],
    );

    real_estate::add_noi_buildup(
        &mut model_builder,
        "total_rev",
        &["rent"],
        "total_exp",
        &["taxes"],
        "noi",
    )
    .expect("noi template");
    let model = model_builder.build().expect("build");

    let mut eval = Evaluator::new();
    let results = eval.evaluate(&model).expect("evaluate");

    // Both nodes are scalar magnitudes in every period; the asset negates
    // CapEx itself. A gap would fail here rather than shorten the schedule.
    let schedule = |node: &str| -> Vec<_> {
        node_to_dated_schedule(&results, node, PeriodDateConvention::End)
            .expect("dated schedule")
            .into_iter()
            .map(|row| (row.date, row.cell.numeric().expect("value in every period")))
            .collect()
    };
    let noi_sched = schedule("noi");
    let capex_sched = schedule("capex");

    // RealEstateAsset uses date-based schedules.
    let asset = RealEstateAsset::builder()
        .id(InstrumentId::new("RE-E2E"))
        .currency(Currency::USD)
        .valuation_method(RealEstateValuationMethod::Dcf)
        .noi_schedule(noi_sched)
        .capex_schedule_opt(Some(capex_sched))
        .discount_rate_opt(Some(0.0)) // PV = sum flows
        .terminal_cap_rate_opt(None) // no terminal value
        .day_count(DayCount::Act365F)
        .build()
        .expect("asset build");

    let market = MarketContext::new();
    let pv = asset.value(&market, as_of).expect("pv").amount();

    // Unlevered flows = NOI - CapEx on the same dates.
    // Q1: 100-20 - 5 = 75
    // Q2: 110-22 - 6 = 82
    assert!((pv - (75.0 + 82.0)).abs() < 1e-10);
}
