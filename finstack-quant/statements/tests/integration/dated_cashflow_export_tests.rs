use finstack_quant_core::dates::PeriodId;
use finstack_quant_statements::builder::ModelBuilder;
use finstack_quant_statements::evaluator::{
    node_to_dated_schedule, Evaluator, PeriodDateConvention,
};
use finstack_quant_statements::types::AmountOrScalar;

#[test]
fn exports_node_series_to_dated_schedule_using_period_end_minus_one_day() {
    let model = ModelBuilder::new("re_noi_bridge")
        .periods("2025Q1..Q2", None)
        .expect("periods should parse")
        .value(
            "noi",
            &[
                (PeriodId::quarter(2025, 1), AmountOrScalar::scalar(100.0)),
                (PeriodId::quarter(2025, 2), AmountOrScalar::scalar(110.0)),
            ],
        )
        .build()
        .expect("model should build");

    let mut eval = Evaluator::new();
    let results = eval.evaluate(&model).expect("evaluate should succeed");

    let sched = node_to_dated_schedule(&model, &results, "noi", PeriodDateConvention::End)
        .expect("schedule export should succeed");

    assert_eq!(sched.len(), 2);

    // Quarter periods are half-open; Q1 ends at 2025-04-01 exclusive, so End convention is 2025-03-31.
    assert_eq!(
        sched[0].0,
        finstack_quant_core::dates::Date::from_calendar_date(2025, time::Month::March, 31).unwrap()
    );
    assert_eq!(sched[0].1, 100.0);
}
