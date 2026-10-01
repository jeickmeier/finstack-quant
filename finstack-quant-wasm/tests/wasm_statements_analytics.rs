//! wasm-bindgen-test suite for `api::statements_analytics`.
//!
//! Covers goal_seek, backtest_forecast, and pl_summary_report_text which use JsValue.

#![cfg(target_arch = "wasm32")]

use finstack_quant_statements::evaluator::StatementResult;
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
        JsValue::from(&model_json),
        JsValue::from("gross_profit"),
        JsValue::from("2024Q1"),
        JsValue::from(80_000.0),
        JsValue::from("revenue"),
        JsValue::from("2024Q1"),
        JsValue::from(true),
        Some(crate::js_object(&[50_000.0, 200_000.0])),
    )
    .unwrap();
    let obj: serde_json::Value = serde_wasm_bindgen::from_value(result).unwrap();
    let solved = obj["solved_value"].as_f64().unwrap();
    assert!(
        solved > 100_000.0,
        "revenue should increase to hit gross_profit=80k; got {solved}"
    );
    // The updated model is the `FinancialModelSpec` object, not a JSON string.
    assert!(obj["model"].is_object());
    assert!(obj.get("updated_model_json").is_none());
}

#[wasm_bindgen_test]
fn backtest_forecast_returns_metrics() {
    let actual = crate::js_object(&vec![100.0, 200.0, 300.0, 400.0]);
    let forecast = crate::js_object(&vec![110.0, 190.0, 310.0, 390.0]);
    let result = backtest_forecast(actual, forecast).unwrap();
    let obj: serde_json::Value = serde_wasm_bindgen::from_value(result).unwrap();
    assert!(obj["mae"].as_f64().unwrap() > 0.0);
    assert!(obj["mape"].as_f64().unwrap() > 0.0);
    assert!(obj["rmse"].as_f64().unwrap() > 0.0);
    assert_eq!(obj["n"].as_u64().unwrap(), 4);
}

#[wasm_bindgen_test]
fn generate_tornado_entries_returns_structured_array() {
    let result = run_sensitivity(JsValue::from(&test_model_json()), JsValue::from(r#"{"mode":"tornado","parameters":[{"node_id":"revenue","period_id":"2024Q1","base_value":100000.0,"perturbations":[90000.0,110000.0]}],"target_metrics":["revenue"]}"#))
        .unwrap();
    let result: serde_json::Value = serde_wasm_bindgen::from_value(result).unwrap();
    let entries = generate_tornado_entries(
        JsValue::from(&serde_json::to_string(&result).unwrap()),
        JsValue::from("revenue"),
        Some(JsValue::from("2024Q1".to_string())),
    )
    .unwrap();
    let entries: Vec<finstack_quant_statements_analytics::analysis::TornadoEntry> =
        serde_wasm_bindgen::from_value(entries).unwrap();

    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].parameter_id, "revenue");
}

#[wasm_bindgen_test]
fn compute_multiple_uses_canonical_company_metric_fields() {
    // A `null` metric is missing, as in Python `compute_multiple`.
    let metrics = std::collections::BTreeMap::from([
        ("enterprise_value".to_string(), Some(8_500.0)),
        ("ebitda".to_string(), Some(1_000.0)),
        ("custom_signal".to_string(), Some(3.0)),
        ("revenue".to_string(), None),
    ]);
    let metrics = crate::js_object(&metrics);
    let result = compute_multiple(metrics, JsValue::from("ev_ebitda"))
        .unwrap()
        .expect("ev_ebitda multiple is defined");
    let multiple: f64 = serde_wasm_bindgen::from_value(result).unwrap();
    assert!((multiple - 8.5).abs() < 1e-12);
}

#[wasm_bindgen_test]
fn scoring_accepts_one_optional_predictor_and_rejects_vectors() {
    use finstack_quant_statements_analytics::analysis::{CompanyMetrics, PeerSet, PeriodBasis};
    use serde_json::json;
    let mut subject = CompanyMetrics::new("subject");
    subject.oas_bp = Some(250.0);
    subject.leverage = Some(2.0);
    let peers = [1.0, 2.0, 3.0].map(|x| {
        let mut peer = CompanyMetrics::new(format!("peer-{x}"));
        peer.oas_bp = Some(x * 100.0);
        peer.leverage = Some(x);
        peer
    });
    let peers = PeerSet::new(subject, peers.to_vec(), PeriodBasis::Ltm);
    for predictor in [json!(null), json!({"named": "leverage"})] {
        let dimensions = json!([{
            "label": "spread", "y_extractor": {"named": "oas_bp"},
            "x_extractor": predictor, "weight": 1.0
        }]);
        let result = score_relative_value(
            crate::js_object(&peers),
            js_sys::JSON::parse(&dimensions.to_string()).expect("dimensions"),
        )
        .expect("supported predictor shape");
        let result: serde_json::Value = serde_wasm_bindgen::from_value(result).expect("score");
        assert!(result["composite_score"].as_f64().expect("score") > 0.0);
    }
    let dimensions = json!([{
        "label": "spread", "y_extractor": {"named": "oas_bp"},
        "x_extractors": [{"named": "leverage"}], "weight": 1.0
    }]);
    assert!(score_relative_value(
        crate::js_object(&peers),
        js_sys::JSON::parse(&dimensions.to_string()).expect("dimensions"),
    )
    .is_err());
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

    let value = run_checks(
        JsValue::from(&test_model_json()),
        JsValue::from(&spec.to_string()),
        None,
    )
    .unwrap();
    let report: serde_json::Value = serde_wasm_bindgen::from_value(value).unwrap();

    assert!(report.is_object());
    assert_eq!(report["results"][0]["check_id"], "revenue_positive");
    assert_eq!(report["summary"]["failed"], 0);
}

#[wasm_bindgen_test]
fn pl_summary_report_text_returns_text() {
    let results_json = evaluated_results_json();
    let line_items: JsValue = crate::js_object(&vec![
        "revenue".to_string(),
        "cogs".to_string(),
        "gross_profit".to_string(),
    ]);
    let periods: JsValue = crate::js_object(&vec!["2024Q1".to_string()]);
    let text = pl_summary_report_text(JsValue::from(&results_json), line_items, periods).unwrap();
    assert!(!text.is_empty());
}

#[wasm_bindgen_test]
fn regression_rejects_constant_predictor_and_unequal_lengths() {
    let x = crate::js_object(&vec![1.0, 1.0, 1.0]);
    let y = crate::js_object(&vec![2.0, 4.0, 6.0]);
    assert!(
        regression_fair_value(x, y.clone(), JsValue::from(2.0), JsValue::from(6.0))
            .unwrap()
            .is_none()
    );
    let x = crate::js_object(&vec![1.0, 2.0, 3.0, 4.0]);
    assert!(
        regression_fair_value(x, y, JsValue::from(2.0), JsValue::from(6.0))
            .unwrap()
            .is_none()
    );
}

#[wasm_bindgen_test]
fn monetary_goal_seek_returns_a_valid_updated_model() {
    use finstack_quant_core::{currency::Currency, dates::PeriodId};
    use finstack_quant_statements::{builder::ModelBuilder, types::AmountOrScalar};
    let period = PeriodId::annual(2025);
    let model = ModelBuilder::new("monetary-goal")
        .periods("2025..2025", None)
        .unwrap()
        .value(
            "revenue",
            &[(
                period,
                AmountOrScalar::amount(100.0, Currency::USD).unwrap(),
            )],
        )
        .compute("profit", "revenue * 0.5")
        .unwrap()
        .build()
        .unwrap();
    let json = serde_json::to_string(&model).unwrap();
    let result = goal_seek(
        JsValue::from(&json),
        JsValue::from("profit"),
        JsValue::from("2025"),
        JsValue::from(60.0),
        JsValue::from("revenue"),
        JsValue::from("2025"),
        JsValue::from(true),
        Some(crate::js_object(&[1.0, 200.0])),
    )
    .unwrap();
    let output: serde_json::Value = serde_wasm_bindgen::from_value(result).unwrap();
    let updated = ModelBuilder::from_spec(serde_json::from_value(output["model"].clone()).unwrap())
        .unwrap()
        .build()
        .unwrap();
    let results = finstack_quant_statements::evaluator::Evaluator::new()
        .evaluate(&updated)
        .unwrap();
    let revenue = results.get_money("revenue", &period).unwrap();
    assert_eq!(revenue.currency(), Currency::USD);
    assert!((revenue.amount() - 120.0).abs() < 1e-8);
}

fn evaluated_results() -> (String, String) {
    let model_json = test_model_json();
    let model: finstack_quant_statements::FinancialModelSpec =
        serde_json::from_str(&model_json).expect("parse");
    let mut evaluator = finstack_quant_statements::evaluator::Evaluator::new();
    let results = evaluator.evaluate(&model).expect("evaluate");
    let results_json = serde_json::to_string(&results).expect("serialize results");
    (model_json, results_json)
}

#[wasm_bindgen_test]
fn credit_assessment_report_accepts_minimal_results() {
    let results = StatementResult::default();
    let results_json = serde_json::to_string(&results).expect("serialize results");
    let text = credit_assessment_report_text(JsValue::from(&results_json), JsValue::from("2024"))
        .expect("report");
    assert!(text.contains("Credit Assessment"));
}

#[wasm_bindgen_test]
fn dependency_tree_returns_the_rust_tree_and_its_text() {
    let model_json = test_model_json();
    let tracer = JsDependencyTracer::new(JsValue::from(&model_json)).expect("tracer");
    let tree = tracer
        .dependency_tree(JsValue::from("gross_profit"))
        .expect("tree");
    let tree: finstack_quant_statements_analytics::analysis::DependencyTree =
        serde_wasm_bindgen::from_value(tree).expect("DependencyTree shape");
    assert_eq!(tree.node_id, "gross_profit");
    assert_eq!(tree.children.len(), 2);
    let text = tracer
        .dependency_tree_text(JsValue::from("gross_profit"))
        .expect("text");
    assert_eq!(
        text,
        finstack_quant_statements_analytics::analysis::render_tree_ascii(&tree)
    );
    assert!(text.contains("├── "), "{text}");
}

#[wasm_bindgen_test]
fn explain_formula_text_succeeds() {
    let (model_json, results_json) = evaluated_results();
    let explanation = explain_formula_text(
        JsValue::from(&model_json),
        JsValue::from(&results_json),
        JsValue::from("gross_profit"),
        JsValue::from("2024Q1"),
    )
    .expect("explain");
    assert!(!explanation.is_empty());
}

#[wasm_bindgen_test]
fn credit_assessment_report_with_data() {
    let (_, results_json) = evaluated_results();
    let text = credit_assessment_report_text(JsValue::from(&results_json), JsValue::from("2024Q1"))
        .expect("report");
    assert!(text.contains("Credit Assessment"));
}
