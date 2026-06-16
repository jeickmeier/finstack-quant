//! wasm-bindgen-test suite for `api::statements_analytics`.
//!
//! Covers goal_seek, backtest_forecast, and pl_summary_report which use JsValue.

#![cfg(target_arch = "wasm32")]

use finstack_quant_wasm::api::statements_analytics::*;
use wasm_bindgen::JsValue;
use wasm_bindgen_test::*;

fn test_model_json() -> String {
    use finstack_quant_core::dates::PeriodId;
    use finstack_quant_statements::builder::ModelBuilder;
    use finstack_quant_statements::types::AmountOrScalar;

    let q1 = PeriodId::quarter(2024, 1);
    let q2 = PeriodId::quarter(2024, 2);
    let model = ModelBuilder::new("test_model")
        .periods("2024Q1..Q2", None)
        .unwrap()
        .value(
            "revenue",
            &[
                (q1, AmountOrScalar::scalar(100_000.0)),
                (q2, AmountOrScalar::scalar(110_000.0)),
            ],
        )
        .value(
            "cogs",
            &[
                (q1, AmountOrScalar::scalar(40_000.0)),
                (q2, AmountOrScalar::scalar(44_000.0)),
            ],
        )
        .compute("gross_profit", "revenue - cogs")
        .unwrap()
        .build()
        .unwrap();
    serde_json::to_string(&model).unwrap()
}

fn evaluated_results_json() -> String {
    let model_json = test_model_json();
    let model: finstack_quant_statements::FinancialModelSpec =
        serde_json::from_str(&model_json).unwrap();
    let mut evaluator = finstack_quant_statements::evaluator::Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();
    serde_json::to_string(&results).unwrap()
}

#[wasm_bindgen_test]
fn goal_seek_finds_revenue_for_target_gross_profit() {
    let model_json = test_model_json();
    let result = goal_seek(
        &model_json,
        "gross_profit",
        "2024Q1",
        80_000.0,
        "revenue",
        "2024Q1",
        true,
        Some(50_000.0),
        Some(200_000.0),
    )
    .unwrap();
    let obj: serde_json::Value = serde_wasm_bindgen::from_value(result).unwrap();
    let solved = obj["solved_value"].as_f64().unwrap();
    assert!(
        solved > 100_000.0,
        "revenue should increase to hit gross_profit=80k; got {solved}"
    );
    assert!(obj["updated_model_json"].as_str().is_some());
}

#[wasm_bindgen_test]
fn backtest_forecast_returns_metrics() {
    let actual = serde_wasm_bindgen::to_value(&vec![100.0, 200.0, 300.0, 400.0]).unwrap();
    let forecast = serde_wasm_bindgen::to_value(&vec![110.0, 190.0, 310.0, 390.0]).unwrap();
    let result = backtest_forecast(actual, forecast).unwrap();
    let obj: serde_json::Value = serde_wasm_bindgen::from_value(result).unwrap();
    assert!(obj["mae"].as_f64().unwrap() > 0.0);
    assert!(obj["mape"].as_f64().unwrap() > 0.0);
    assert!(obj["rmse"].as_f64().unwrap() > 0.0);
    assert_eq!(obj["n"].as_u64().unwrap(), 4);
}

#[wasm_bindgen_test]
fn pl_summary_report_returns_text() {
    let results_json = evaluated_results_json();
    let line_items: JsValue = serde_wasm_bindgen::to_value(&vec![
        "revenue".to_string(),
        "cogs".to_string(),
        "gross_profit".to_string(),
    ])
    .unwrap();
    let periods: JsValue = serde_wasm_bindgen::to_value(&vec!["2024Q1".to_string()]).unwrap();
    let text = pl_summary_report(&results_json, line_items, periods).unwrap();
    assert!(!text.is_empty());
}
