//! WASM bindings for the `finstack-statements-analytics` crate.
//!
//! Exposes financial statement analysis functions that accept and return
//! JSON strings, suitable for consumption from JavaScript/TypeScript.

mod comps;

pub use comps::{
    compute_multiple, peer_stats, percentile_rank, regression_fair_value, score_relative_value,
    z_score,
};

use crate::utils::{to_js_err, to_js_value};
use wasm_bindgen::prelude::*;

/// Run a sensitivity analysis on a financial model.
///
/// Accepts JSON strings for the model spec and sensitivity configuration,
/// evaluates all perturbation scenarios, and returns JSON results.
#[wasm_bindgen(js_name = runSensitivity)]
pub fn run_sensitivity(model_json: &str, config_json: &str) -> Result<String, JsValue> {
    let model: finstack_statements::FinancialModelSpec =
        serde_json::from_str(model_json).map_err(to_js_err)?;

    let config: finstack_statements_analytics::analysis::SensitivityConfig =
        serde_json::from_str(config_json).map_err(to_js_err)?;

    let analyzer = finstack_statements_analytics::analysis::SensitivityAnalyzer::new(&model);
    let result = analyzer.run(&config).map_err(to_js_err)?;

    serde_json::to_string(&result).map_err(to_js_err)
}

/// Run a variance analysis comparing two evaluated statement results.
///
/// Returns JSON-serialized variance report.
#[wasm_bindgen(js_name = runVariance)]
pub fn run_variance(
    base_json: &str,
    comparison_json: &str,
    config_json: &str,
) -> Result<String, JsValue> {
    let base: finstack_statements::evaluator::StatementResult =
        serde_json::from_str(base_json).map_err(to_js_err)?;

    let comparison: finstack_statements::evaluator::StatementResult =
        serde_json::from_str(comparison_json).map_err(to_js_err)?;

    let config: finstack_statements_analytics::analysis::VarianceConfig =
        serde_json::from_str(config_json).map_err(to_js_err)?;

    let analyzer =
        finstack_statements_analytics::analysis::VarianceAnalyzer::new(&base, &comparison);
    let report = analyzer.compute(&config).map_err(to_js_err)?;

    serde_json::to_string(&report).map_err(to_js_err)
}

/// Evaluate all scenarios in a scenario set against a base model.
///
/// Returns a JSON object mapping scenario names to their statement results.
#[wasm_bindgen(js_name = evaluateScenarioSet)]
pub fn evaluate_scenario_set(model_json: &str, scenario_set_json: &str) -> Result<String, JsValue> {
    let model: finstack_statements::FinancialModelSpec =
        serde_json::from_str(model_json).map_err(to_js_err)?;

    let scenario_set: finstack_statements_analytics::analysis::ScenarioSet =
        serde_json::from_str(scenario_set_json).map_err(to_js_err)?;

    let results = scenario_set.evaluate_all(&model).map_err(to_js_err)?;

    let map: indexmap::IndexMap<&String, &finstack_statements::evaluator::StatementResult> =
        results.scenarios.iter().collect();
    serde_json::to_string(&map).map_err(to_js_err)
}

/// Compute forecast accuracy metrics (MAE, MAPE, RMSE).
///
/// Takes two float arrays (actual, forecast) and returns a JSON object
/// with keys `mae`, `mape`, `rmse`, `n`.
#[wasm_bindgen(js_name = backtestForecast)]
pub fn backtest_forecast(actual: JsValue, forecast: JsValue) -> Result<JsValue, JsValue> {
    let actual_vec: Vec<f64> = serde_wasm_bindgen::from_value(actual).map_err(to_js_err)?;
    let forecast_vec: Vec<f64> = serde_wasm_bindgen::from_value(forecast).map_err(to_js_err)?;

    let metrics =
        finstack_statements_analytics::analysis::backtest_forecast(&actual_vec, &forecast_vec)
            .map_err(to_js_err)?;

    let result = serde_json::json!({
        "mae": metrics.mae,
        "mape": metrics.mape,
        "rmse": metrics.rmse,
        "n": metrics.n,
    });
    to_js_value(&result)
}

/// Generate tornado chart entries for a sensitivity result.
#[wasm_bindgen(js_name = generateTornadoEntries)]
pub fn generate_tornado_entries(
    result_json: &str,
    metric_node: &str,
    period: Option<String>,
) -> Result<String, JsValue> {
    let result: finstack_statements_analytics::analysis::SensitivityResult =
        serde_json::from_str(result_json).map_err(to_js_err)?;
    let period_id: Option<finstack_core::dates::PeriodId> =
        period.map(|p| p.parse().map_err(to_js_err)).transpose()?;
    let entries = finstack_statements_analytics::analysis::generate_tornado_entries(
        &result,
        metric_node,
        period_id,
    );
    serde_json::to_string(&entries).map_err(to_js_err)
}

/// Run Monte Carlo simulation on a financial model (JSON in/out).
#[wasm_bindgen(js_name = runMonteCarlo)]
pub fn run_monte_carlo(model_json: &str, config_json: &str) -> Result<String, JsValue> {
    let model: finstack_statements::FinancialModelSpec =
        serde_json::from_str(model_json).map_err(to_js_err)?;
    let config: finstack_statements::evaluator::MonteCarloConfig =
        serde_json::from_str(config_json).map_err(to_js_err)?;
    let mut evaluator = finstack_statements::evaluator::Evaluator::new();
    let results = evaluator
        .evaluate_monte_carlo(&model, &config)
        .map_err(to_js_err)?;
    serde_json::to_string(&results).map_err(to_js_err)
}

/// Find the driver value that makes a target node reach a target value.
#[wasm_bindgen(js_name = goalSeek)]
#[allow(clippy::too_many_arguments)]
pub fn goal_seek(
    model_json: &str,
    target_node: &str,
    target_period: &str,
    target_value: f64,
    driver_node: &str,
    driver_period: &str,
    update_model: bool,
    bounds_lo: Option<f64>,
    bounds_hi: Option<f64>,
) -> Result<JsValue, JsValue> {
    let mut model: finstack_statements::FinancialModelSpec =
        serde_json::from_str(model_json).map_err(to_js_err)?;
    let tp: finstack_core::dates::PeriodId = target_period.parse().map_err(to_js_err)?;
    let dp: finstack_core::dates::PeriodId = driver_period.parse().map_err(to_js_err)?;
    let bounds = goal_seek_bounds(bounds_lo, bounds_hi).map_err(|e| JsValue::from_str(&e))?;

    let result = finstack_statements_analytics::analysis::goal_seek(
        &mut model,
        target_node,
        tp,
        target_value,
        driver_node,
        dp,
        update_model,
        bounds,
    )
    .map_err(to_js_err)?;

    // Only re-serialize the (potentially mutated) model when the caller
    // asked for the update; otherwise `model` is unchanged and the JSON is
    // wasted work + a confusing `updated_model_json` on non-updating calls.
    let out = if update_model {
        let updated_json = serde_json::to_string_pretty(&model).map_err(to_js_err)?;
        serde_json::json!({
            "solved_value": result,
            "updated_model_json": updated_json,
        })
    } else {
        serde_json::json!({ "solved_value": result })
    };
    to_js_value(&out)
}

/// Validate that goal-seek bounds are either both present or both absent.
///
/// Kept JsValue-free so the rejection logic is unit-testable on native
/// targets (constructing a `JsValue` aborts off-wasm32).
fn goal_seek_bounds(
    bounds_lo: Option<f64>,
    bounds_hi: Option<f64>,
) -> Result<Option<(f64, f64)>, String> {
    match (bounds_lo, bounds_hi) {
        (Some(lo), Some(hi)) => Ok(Some((lo, hi))),
        (None, None) => Ok(None),
        _ => Err(
            "goalSeek: bounds_lo and bounds_hi must be provided together \
             (got exactly one bound)"
                .to_string(),
        ),
    }
}

/// Trace dependencies for a node and return ASCII tree.
#[wasm_bindgen(js_name = traceDependencies)]
pub fn trace_dependencies(model_json: &str, node_id: &str) -> Result<String, JsValue> {
    let model: finstack_statements::FinancialModelSpec =
        serde_json::from_str(model_json).map_err(to_js_err)?;
    let graph =
        finstack_statements::evaluator::DependencyGraph::from_model(&model).map_err(to_js_err)?;
    let tracer = finstack_statements_analytics::analysis::DependencyTracer::new(&model, &graph);
    let tree = tracer.dependency_tree(node_id).map_err(to_js_err)?;
    Ok(finstack_statements_analytics::analysis::render_tree_ascii(
        &tree,
    ))
}

/// Explain a formula for a specific node and period (JSON in/out).
#[wasm_bindgen(js_name = explainFormula)]
pub fn explain_formula(
    model_json: &str,
    results_json: &str,
    node_id: &str,
    period: &str,
) -> Result<String, JsValue> {
    let model: finstack_statements::FinancialModelSpec =
        serde_json::from_str(model_json).map_err(to_js_err)?;
    let results: finstack_statements::evaluator::StatementResult =
        serde_json::from_str(results_json).map_err(to_js_err)?;
    let pid: finstack_core::dates::PeriodId = period.parse().map_err(to_js_err)?;
    let explainer =
        finstack_statements_analytics::analysis::FormulaExplainer::new(&model, &results);
    let explanation = explainer.explain(node_id, &pid).map_err(to_js_err)?;
    Ok(explanation.to_string_detailed())
}

/// Generate a P&L summary report as formatted text.
#[wasm_bindgen(js_name = plSummaryReport)]
pub fn pl_summary_report(
    results_json: &str,
    line_items: JsValue,
    periods: JsValue,
) -> Result<String, JsValue> {
    use finstack_statements_analytics::analysis::Report;

    let results: finstack_statements::evaluator::StatementResult =
        serde_json::from_str(results_json).map_err(to_js_err)?;
    let items: Vec<String> = serde_wasm_bindgen::from_value(line_items).map_err(to_js_err)?;
    let period_strs: Vec<String> = serde_wasm_bindgen::from_value(periods).map_err(to_js_err)?;
    let period_ids: Vec<finstack_core::dates::PeriodId> = period_strs
        .iter()
        .map(|p| p.parse().map_err(to_js_err))
        .collect::<Result<Vec<_>, _>>()?;
    let report =
        finstack_statements_analytics::analysis::PLSummaryReport::new(&results, items, period_ids);
    Ok(report.to_string())
}

/// Generate a credit assessment report as formatted text.
#[wasm_bindgen(js_name = creditAssessmentReport)]
pub fn credit_assessment_report(results_json: &str, as_of: &str) -> Result<String, JsValue> {
    use finstack_statements_analytics::analysis::Report;

    let results: finstack_statements::evaluator::StatementResult =
        serde_json::from_str(results_json).map_err(to_js_err)?;
    let period: finstack_core::dates::PeriodId = as_of.parse().map_err(to_js_err)?;
    let report =
        finstack_statements_analytics::analysis::CreditAssessmentReport::new(&results, period);
    Ok(report.to_string())
}

/// Run checks from a suite spec against a model (JSON in/out).
///
/// Evaluates the model, resolves the suite spec into runnable checks
/// (built-in **and** user-defined formula checks), and returns a JSON
/// check report.
#[wasm_bindgen(js_name = runChecks)]
pub fn run_checks(model_json: &str, suite_spec_json: &str) -> Result<String, JsValue> {
    let model: finstack_statements::FinancialModelSpec =
        serde_json::from_str(model_json).map_err(to_js_err)?;
    let spec: finstack_statements::checks::CheckSuiteSpec =
        serde_json::from_str(suite_spec_json).map_err(to_js_err)?;
    let suite =
        finstack_statements_analytics::analysis::resolve_check_suite(&spec).map_err(to_js_err)?;
    let mut evaluator = finstack_statements::evaluator::Evaluator::new();
    let results = evaluator.evaluate(&model).map_err(to_js_err)?;
    let report = suite.run(&model, &results).map_err(to_js_err)?;
    serde_json::to_string(&report).map_err(to_js_err)
}

/// Run three-statement checks using node mappings.
///
/// Accepts a model and a mapping JSON, builds the appropriate check
/// suite, evaluates the model, runs the checks, and returns the report.
#[wasm_bindgen(js_name = runThreeStatementChecks)]
pub fn run_three_statement_checks(model_json: &str, mapping_json: &str) -> Result<String, JsValue> {
    let model: finstack_statements::FinancialModelSpec =
        serde_json::from_str(model_json).map_err(to_js_err)?;
    let mapping: finstack_statements_analytics::analysis::ThreeStatementMapping =
        serde_json::from_str(mapping_json).map_err(to_js_err)?;
    let suite = finstack_statements_analytics::analysis::three_statement_checks(mapping);
    let mut evaluator = finstack_statements::evaluator::Evaluator::new();
    let results = evaluator.evaluate(&model).map_err(to_js_err)?;
    let report = suite.run(&model, &results).map_err(to_js_err)?;
    serde_json::to_string(&report).map_err(to_js_err)
}

/// Run credit underwriting checks using credit-specific mappings.
#[wasm_bindgen(js_name = runCreditUnderwritingChecks)]
pub fn run_credit_underwriting_checks(
    model_json: &str,
    mapping_json: &str,
) -> Result<String, JsValue> {
    let model: finstack_statements::FinancialModelSpec =
        serde_json::from_str(model_json).map_err(to_js_err)?;
    let mapping: finstack_statements_analytics::analysis::CreditMapping =
        serde_json::from_str(mapping_json).map_err(to_js_err)?;
    let suite = finstack_statements_analytics::analysis::credit_underwriting_checks(mapping);
    let mut evaluator = finstack_statements::evaluator::Evaluator::new();
    let results = evaluator.evaluate(&model).map_err(to_js_err)?;
    let report = suite.run(&model, &results).map_err(to_js_err)?;
    serde_json::to_string(&report).map_err(to_js_err)
}

/// Render a check report as plain text.
#[wasm_bindgen(js_name = renderCheckReportText)]
pub fn render_check_report_text(report_json: &str) -> Result<String, JsValue> {
    let report: finstack_statements::checks::CheckReport =
        serde_json::from_str(report_json).map_err(to_js_err)?;
    Ok(finstack_statements_analytics::analysis::CheckReportRenderer::render_text(&report))
}

/// Render a check report as HTML.
#[wasm_bindgen(js_name = renderCheckReportHtml)]
pub fn render_check_report_html(report_json: &str) -> Result<String, JsValue> {
    let report: finstack_statements::checks::CheckReport =
        serde_json::from_str(report_json).map_err(to_js_err)?;
    Ok(finstack_statements_analytics::analysis::CheckReportRenderer::render_html(&report))
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_core::dates::PeriodId;
    use finstack_statements::builder::ModelBuilder;
    use finstack_statements::evaluator::StatementResult;
    use finstack_statements::types::AmountOrScalar;

    fn test_model_json() -> String {
        let q1 = PeriodId::quarter(2024, 1);
        let model = ModelBuilder::new("test_model")
            .periods("2024Q1..Q2", None)
            .expect("periods")
            .value(
                "revenue",
                &[
                    (q1, AmountOrScalar::scalar(100_000.0)),
                    (
                        PeriodId::quarter(2024, 2),
                        AmountOrScalar::scalar(110_000.0),
                    ),
                ],
            )
            .value(
                "cogs",
                &[
                    (q1, AmountOrScalar::scalar(40_000.0)),
                    (PeriodId::quarter(2024, 2), AmountOrScalar::scalar(44_000.0)),
                ],
            )
            .compute("gross_profit", "revenue - cogs")
            .expect("compute")
            .build()
            .expect("build");
        serde_json::to_string(&model).expect("serialize")
    }

    fn evaluated_results() -> (String, String) {
        let model_json = test_model_json();
        let model: finstack_statements::FinancialModelSpec =
            serde_json::from_str(&model_json).expect("parse");
        let mut evaluator = finstack_statements::evaluator::Evaluator::new();
        let results = evaluator.evaluate(&model).expect("evaluate");
        let results_json = serde_json::to_string(&results).expect("serialize results");
        (model_json, results_json)
    }

    #[test]
    fn credit_assessment_report_accepts_minimal_results() {
        let results = StatementResult::default();
        let results_json = serde_json::to_string(&results).expect("serialize results");
        let text = credit_assessment_report(&results_json, "2024").expect("report");
        assert!(text.contains("Credit Assessment"));
    }

    #[test]
    fn trace_dependencies_renders_for_simple_model() {
        let model_json = test_model_json();
        let tree = trace_dependencies(&model_json, "gross_profit").expect("trace");
        assert!(!tree.is_empty());
        assert!(tree.contains("revenue") || tree.contains("gross_profit"));
    }

    #[test]
    fn explain_formula_succeeds() {
        let (model_json, results_json) = evaluated_results();
        let explanation =
            explain_formula(&model_json, &results_json, "gross_profit", "2024Q1").expect("explain");
        assert!(!explanation.is_empty());
    }

    #[test]
    fn credit_assessment_report_with_data() {
        let (_, results_json) = evaluated_results();
        let text = credit_assessment_report(&results_json, "2024Q1").expect("report");
        assert!(text.contains("Credit Assessment"));
    }

    #[test]
    fn run_sensitivity_diagonal() {
        let model_json = test_model_json();
        let config = finstack_statements_analytics::analysis::SensitivityConfig {
            mode: finstack_statements_analytics::analysis::SensitivityMode::Diagonal,
            parameters: vec![finstack_statements_analytics::analysis::ParameterSpec {
                node_id: "revenue".to_string(),
                period_id: PeriodId::quarter(2024, 1),
                base_value: 100_000.0,
                perturbations: vec![-0.1, 0.0, 0.1],
            }],
            target_metrics: vec!["gross_profit".to_string()],
        };
        let config_json = serde_json::to_string(&config).expect("config");
        let result = run_sensitivity(&model_json, &config_json).expect("sensitivity");
        let parsed: serde_json::Value = serde_json::from_str(&result).expect("parse");
        assert!(parsed.is_object() || parsed.is_array());
    }

    #[test]
    fn generate_tornado_from_sensitivity() {
        let model_json = test_model_json();
        let config = finstack_statements_analytics::analysis::SensitivityConfig {
            mode: finstack_statements_analytics::analysis::SensitivityMode::Tornado,
            parameters: vec![finstack_statements_analytics::analysis::ParameterSpec {
                node_id: "revenue".to_string(),
                period_id: PeriodId::quarter(2024, 1),
                base_value: 100_000.0,
                perturbations: vec![-0.1, 0.1],
            }],
            target_metrics: vec!["gross_profit".to_string()],
        };
        let config_json = serde_json::to_string(&config).expect("config");
        let result_str = run_sensitivity(&model_json, &config_json).expect("sensitivity");
        let entries = generate_tornado_entries(&result_str, "gross_profit", None).expect("tornado");
        let parsed: serde_json::Value = serde_json::from_str(&entries).expect("parse");
        assert!(parsed.is_array());
    }

    #[test]
    fn run_variance_between_two_results() {
        let (model_json, _) = evaluated_results();
        let model: finstack_statements::FinancialModelSpec =
            serde_json::from_str(&model_json).expect("parse model");
        let mut evaluator = finstack_statements::evaluator::Evaluator::new();
        let base = evaluator.evaluate(&model).expect("eval base");
        let comparison = evaluator.evaluate(&model).expect("eval comparison");
        let base_json = serde_json::to_string(&base).expect("ser base");
        let comparison_json = serde_json::to_string(&comparison).expect("ser comparison");
        let config = finstack_statements_analytics::analysis::VarianceConfig {
            baseline_label: "base".to_string(),
            comparison_label: "comp".to_string(),
            metrics: vec!["revenue".to_string(), "gross_profit".to_string()],
            periods: vec![PeriodId::quarter(2024, 1)],
        };
        let config_json = serde_json::to_string(&config).expect("ser config");
        let result = run_variance(&base_json, &comparison_json, &config_json).expect("variance");
        let parsed: serde_json::Value = serde_json::from_str(&result).expect("parse");
        assert!(parsed.is_object());
    }

    #[test]
    fn evaluate_scenario_set_with_override() {
        let model_json = test_model_json();
        let mut overrides = indexmap::IndexMap::new();
        overrides.insert("revenue".to_string(), 200_000.0);
        let scenario_set = finstack_statements_analytics::analysis::ScenarioSet {
            scenarios: indexmap::indexmap! {
                "upside".to_string() => finstack_statements_analytics::analysis::ScenarioDefinition {
                    model_id: None,
                    parent: None,
                    overrides,
                },
            },
        };
        let scenario_set_json = serde_json::to_string(&scenario_set).expect("ser");
        let result = evaluate_scenario_set(&model_json, &scenario_set_json).expect("eval");
        let parsed: serde_json::Value = serde_json::from_str(&result).expect("parse");
        assert!(parsed.is_object());
        assert!(parsed.get("upside").is_some());
    }

    #[test]
    fn run_checks_includes_formula_checks() {
        let model_json = test_model_json();
        let spec_json = serde_json::json!({
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
        })
        .to_string();
        let report_json = run_checks(&model_json, &spec_json).expect("run checks");
        let report: serde_json::Value = serde_json::from_str(&report_json).expect("parse report");
        // The formula check must appear in the report's executed results,
        // not be silently dropped.
        assert!(
            report_json.contains("revenue_positive"),
            "formula check missing from report: {report_json}"
        );
        assert!(report.is_object());
    }

    #[test]
    fn goal_seek_rejects_half_specified_bounds() {
        let msg = goal_seek_bounds(Some(0.0), None).expect_err("half-specified bounds must error");
        assert!(msg.contains("bounds_lo and bounds_hi"), "got: {msg}");
        let msg = goal_seek_bounds(None, Some(1.0)).expect_err("half-specified bounds must error");
        assert!(msg.contains("bounds_lo and bounds_hi"), "got: {msg}");
        assert_eq!(
            goal_seek_bounds(Some(0.0), Some(1.0)).expect("both bounds valid"),
            Some((0.0, 1.0))
        );
        assert_eq!(goal_seek_bounds(None, None).expect("no bounds valid"), None);
    }

    #[test]
    fn run_monte_carlo_on_model() {
        let model_json = test_model_json();
        let config = finstack_statements::evaluator::MonteCarloConfig::new(10, 42);
        let config_json = serde_json::to_string(&config).expect("ser config");
        let result = run_monte_carlo(&model_json, &config_json).expect("mc");
        let parsed: serde_json::Value = serde_json::from_str(&result).expect("parse");
        assert!(parsed.is_object());
    }
}
