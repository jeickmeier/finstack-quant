//! Covenant compliance and breach forecasting over evaluated statement results.
//!
//! A trailing ratio such as LTM leverage is non-finite until four quarters of
//! history exist; a covenant window that opens later must keep those early
//! periods evaluable, while a covenant that does read the ratio must fail.
use finstack_quant_core::dates::{Date, PeriodId, Tenor};
use finstack_quant_core::{Error, InputError};
use finstack_quant_covenants::{
    Covenant, CovenantEngine, CovenantForecastConfig, CovenantSpec, CovenantType, CovenantWindow,
    DatedMetrics,
};
use finstack_quant_statements::builder::ModelBuilder;
use finstack_quant_statements::evaluator::StatementResult;
use finstack_quant_statements::types::FinancialModelSpec;
use finstack_quant_statements_analytics::analysis::{evaluate_covenants, forecast_breaches};
use indexmap::IndexMap;
use time::macros::date;

fn quarters() -> [PeriodId; 4] {
    [1, 2, 3, 4].map(|q| PeriodId::quarter(2024, q).unwrap())
}

fn model() -> FinancialModelSpec {
    let mut builder = ModelBuilder::new("borrower");
    builder.periods("2024Q1..Q4", None).unwrap();
    builder.value_scalar("placeholder", &[]);
    builder.build().unwrap()
}

fn results(leverage: [f64; 4], coverage: [f64; 4]) -> StatementResult {
    let mut results = StatementResult {
        periods: Some(model().periods),
        ..StatementResult::default()
    };
    // A `NaN` fixture value is a cell that is present without a value.
    for (node, values) in [("leverage", leverage), ("interest_coverage", coverage)] {
        for (period, value) in quarters().into_iter().zip(values) {
            let cell = finstack_quant_statements::evaluator::ResultCell::scalar(value)
                .unwrap_or_else(|_| {
                    finstack_quant_statements::evaluator::ResultCell::unavailable(
                        finstack_quant_statements::evaluator::UnavailableReason::MissingInput,
                    )
                });
            results.set_cell(node, period, cell);
        }
    }
    results
}

fn leverage_spec() -> CovenantSpec {
    CovenantSpec::with_metric(
        Covenant::new(
            CovenantType::MaxDebtToEbitda { threshold: 3.5 },
            Tenor::quarterly(),
            "max_debt_to_ebitda",
        ),
        "leverage",
    )
}

fn coverage_spec() -> CovenantSpec {
    CovenantSpec::with_metric(
        Covenant::new(
            CovenantType::MinInterestCoverage { threshold: 3.0 },
            Tenor::quarterly(),
            "min_interest_coverage",
        ),
        "interest_coverage",
    )
}

/// Coverage tested throughout; leverage added by a window from `start`.
fn engine(start: Date) -> CovenantEngine {
    let mut engine = CovenantEngine::new();
    engine.add_spec(coverage_spec());
    engine.add_window(CovenantWindow {
        start,
        end: date!(2024 - 12 - 31),
        covenants: vec![leverage_spec(), coverage_spec()],
    });
    engine
}

const COVERAGE: [f64; 4] = [4.5, 4.9, 5.1, 6.0];

#[test]
fn window_keeps_early_non_finite_leverage_out_of_the_evaluation() {
    let results = results([f64::NAN, f64::NAN, 2.2, 3.9], COVERAGE);
    let series = evaluate_covenants(&engine(date!(2024 - 07 - 01)), &results).unwrap();

    let dates: Vec<Date> = series.iter().map(|row| row.as_of).collect();
    assert_eq!(
        dates,
        [
            date!(2024 - 03 - 31),
            date!(2024 - 06 - 30),
            date!(2024 - 09 - 30),
            date!(2024 - 12 - 31)
        ]
    );
    let keys = |i: usize| series[i].reports.keys().cloned().collect::<Vec<_>>();
    assert_eq!(keys(0), ["min_interest_coverage"]);
    assert_eq!(keys(1), ["min_interest_coverage"]);
    assert_eq!(keys(2), ["max_debt_to_ebitda", "min_interest_coverage"]);
    assert_eq!(
        series[0].reports["min_interest_coverage"].actual_value,
        Some(4.5)
    );
    assert!(series[2].reports["max_debt_to_ebitda"].passed);
    assert!(!series[3].reports["max_debt_to_ebitda"].passed);

    // The breach forecast reads the same periods under the same window rule.
    let breaches = forecast_breaches(
        &engine(date!(2024 - 07 - 01)),
        &results,
        CovenantForecastConfig::default(),
    )
    .unwrap();
    assert_eq!(breaches.len(), 1);
    assert_eq!(breaches[0].covenant_id, "max_debt_to_ebitda");
    assert_eq!(breaches[0].breach_date, date!(2024 - 12 - 31));
}

#[test]
fn a_required_non_finite_metric_fails_the_evaluation() {
    // The window opens in 2024Q2, where leverage is still NaN.
    let results = results([f64::NAN, f64::NAN, 2.2, 3.9], COVERAGE);
    let err = evaluate_covenants(&engine(date!(2024 - 04 - 01)), &results).unwrap_err();
    assert!(matches!(err, Error::Validation(_)), "{err:?}");
    assert!(err.to_string().contains("'leverage'"), "{err}");
    assert!(err.to_string().contains("2024Q2"), "{err}");
    // The forecast fails on the same input.
    assert!(
        forecast_breaches(
            &engine(date!(2024 - 04 - 01)),
            &results,
            CovenantForecastConfig::default()
        )
        .is_err()
    );
}

#[test]
fn a_missing_required_metric_is_not_found() {
    let mut results = results([1.0, 1.0, 2.2, 3.9], COVERAGE);
    results
        .cells
        .get_mut("leverage")
        .unwrap()
        .shift_remove(&quarters()[3]);
    let err = evaluate_covenants(&engine(date!(2024 - 07 - 01)), &results).unwrap_err();
    assert!(
        matches!(&err, Error::Input(InputError::NotFound { id }) if id.contains("leverage") && id.contains("2024Q4")),
        "{err:?}"
    );
}

#[test]
fn only_the_result_periods_are_tested() {
    let mut results = results([1.0, 1.0, 2.2, 3.9], COVERAGE);
    results
        .set_scalar(
            "interest_coverage",
            PeriodId::quarter(2025, 1).unwrap(),
            6.1,
        )
        .unwrap();
    let series = evaluate_covenants(&engine(date!(2024 - 07 - 01)), &results).unwrap();
    assert_eq!(series.len(), 4);
    assert_eq!(series[3].as_of, date!(2024 - 12 - 31));
}

#[test]
fn finite_results_match_the_engine_series_over_period_end_rows() {
    let leverage = [3.0, 3.6, 2.2, 3.9];
    let results = results(leverage, COVERAGE);
    let engine = engine(date!(2024 - 07 - 01));
    let ends = [
        date!(2024 - 03 - 31),
        date!(2024 - 06 - 30),
        date!(2024 - 09 - 30),
        date!(2024 - 12 - 31),
    ];
    let rows: Vec<DatedMetrics> = (0..4)
        .map(|i| DatedMetrics {
            date: ends[i],
            metrics: IndexMap::from([
                ("leverage".to_string(), leverage[i]),
                ("interest_coverage".to_string(), COVERAGE[i]),
            ]),
        })
        .collect();

    assert_eq!(
        evaluate_covenants(&engine, &results).unwrap(),
        engine.evaluate_series(&rows).unwrap()
    );
}

#[test]
fn empty_results_evaluate_to_an_empty_series() {
    let series =
        evaluate_covenants(&engine(date!(2024 - 07 - 01)), &StatementResult::default()).unwrap();
    assert!(series.is_empty());
}
