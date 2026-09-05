//! wasm-bindgen-test suite for `api::statements_analytics`.
//!
//! Covers goal_seek, backtest_forecast, and pl_summary_report_text which use JsValue.

#![cfg(target_arch = "wasm32")]

use finstack_quant_wasm::api::statements_analytics::*;
use wasm_bindgen::JsValue;
use wasm_bindgen_test::*;

fn test_model_json() -> String {
    use finstack_quant_core::dates::PeriodId;
    use finstack_quant_statements::builder::ModelBuilder;
    use finstack_quant_statements::types::AmountOrScalar;

    let q1 = PeriodId::quarter(2024, 1).expect("valid period fixture");
    let q2 = PeriodId::quarter(2024, 2).expect("valid period fixture");
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
fn generate_tornado_entries_returns_structured_array() {
    let result = run_sensitivity(&test_model_json(), r#"{"mode":"tornado","parameters":[{"node_id":"revenue","period_id":"2024Q1","base_value":100000.0,"perturbations":[90000.0,110000.0]}],"target_metrics":["revenue"]}"#)
        .unwrap();
    let result: serde_json::Value = serde_wasm_bindgen::from_value(result).unwrap();
    let entries = generate_tornado_entries(
        &serde_json::to_string(&result).unwrap(),
        "revenue",
        Some("2024Q1".to_string()),
    )
    .unwrap();
    let entries: Vec<finstack_quant_statements_analytics::analysis::TornadoEntry> =
        serde_wasm_bindgen::from_value(entries).unwrap();

    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].parameter_id, "revenue");
}

#[wasm_bindgen_test]
fn compute_multiple_uses_canonical_company_metric_fields() {
    let metrics = std::collections::BTreeMap::from([
        ("enterprise_value".to_string(), 8_500.0),
        ("ebitda".to_string(), 1_000.0),
        ("custom_signal".to_string(), 3.0),
    ]);
    let metrics = serde_wasm_bindgen::to_value(&metrics).unwrap();
    let result = compute_multiple(metrics, "ev_ebitda")
        .unwrap()
        .expect("ev_ebitda multiple is defined");
    let multiple: f64 = serde_wasm_bindgen::from_value(result).unwrap();
    assert!((multiple - 8.5).abs() < 1e-12);
}

#[wasm_bindgen_test]
fn run_checks_returns_structured_report() {
    let spec = serde_json::json!({
        "name": "formula suite",
        "builtin_checks": [],
        "formula_checks": [{
            "id": "revenue_positive",
            "name": "Revenue must be positive",
            "category": "internal_consistency",
            "severity": "error",
            "formula": "revenue > 0",
            "message_template": "Revenue not positive in {period}",
            "tolerance": null
        }]
    });

    let value = run_checks(&test_model_json(), &spec.to_string(), None).unwrap();
    let report: serde_json::Value = serde_wasm_bindgen::from_value(value).unwrap();

    assert!(report.is_object());
    assert_eq!(report["results"][0]["check_id"], "revenue_positive");
    assert_eq!(report["summary"]["failed"], 0);
}

#[wasm_bindgen_test]
fn pl_summary_report_text_returns_text() {
    let results_json = evaluated_results_json();
    let line_items: JsValue = serde_wasm_bindgen::to_value(&vec![
        "revenue".to_string(),
        "cogs".to_string(),
        "gross_profit".to_string(),
    ])
    .unwrap();
    let periods: JsValue = serde_wasm_bindgen::to_value(&vec!["2024Q1".to_string()]).unwrap();
    let text = pl_summary_report_text(&results_json, line_items, periods).unwrap();
    assert!(!text.is_empty());
}
