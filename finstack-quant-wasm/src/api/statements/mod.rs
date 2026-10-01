//! WASM bindings for the `finstack-quant-statements` crate.
//!
//! Data crosses the boundary as JSON strings or plain objects; the stateful
//! pieces are classes:
//! - `Evaluator` evaluates a `FinancialModelSpec` (optionally with a market
//!   or under Monte Carlo) and can carry a check suite.
//! - `ModelBuilder` / `MixedNodeBuilder` assemble a model step by step, and
//!   `Registry` holds reusable metric definitions.
//!
//! Everything else is a function: spec validation (`validate*Json`), DSL
//! formula parsing, and the free-function twins of the Python result and
//! spec methods (`statementResult*`, `capitalStructureCashflows*`,
//! `forecastSpec*`, `adjustment*`, `normalize`).

mod builder;
mod handles;
mod results;
mod specs;

pub use builder::{JsMixedNodeBuilder, JsModelBuilder};
pub use handles::{JsEvaluator, JsRegistry};

use crate::utils::input::{js_opt_string, js_string, json_text};
use crate::utils::to_js_err;
use finstack_quant_statements::FinancialModelSpec;
use wasm_bindgen::prelude::*;

/// Validate a `FinancialModelSpec` JSON string.
///
/// Parses the input with the Rust `FinancialModelSpec::from_json` (schema
/// plus semantic validation, the same entry point every model-taking export
/// and the Python bindings use) and returns the canonical (re-serialized)
/// JSON.
///
/// # Errors
///
/// Rejects malformed or schema-incompatible `json`, an empty or invalid period
/// timeline, reserved node identifiers, incompatible node fields or value
/// types, invalid formulas or dimensions, an invalid waterfall, or failure to
/// serialize the normalized model.
/// @param json - Canonical JSON string defining the object to deserialize or normalize.
#[wasm_bindgen(js_name = validateFinancialModelJson)]
pub fn validate_financial_model_json(json: JsValue) -> Result<String, JsValue> {
    let json: &str = &json_text(&json, "json")?;
    let model = FinancialModelSpec::from_json(json).map_err(to_js_err)?;
    serde_json::to_string(&model).map_err(to_js_err)
}

/// Get the node identifiers from a model specification JSON.
///
/// Returns a JS array of node ID strings in declaration order. The model is
/// validated first (Rust `FinancialModelSpec::from_json`), so a model that
/// `validateFinancialModelJson` rejects is rejected here too.
///
/// # Errors
///
/// Rejects malformed or schema-incompatible `json`; an empty or invalid period
/// timeline, reserved node identifiers, incompatible node fields or value
/// types, invalid formulas, or an invalid capital structure; or failure to
/// serialize the node identifiers to JavaScript.
/// @param json - Canonical JSON string defining the object to deserialize or normalize.
#[wasm_bindgen(js_name = modelNodeIds)]
pub fn model_node_ids(json: JsValue) -> Result<JsValue, JsValue> {
    let json: &str = &json_text(&json, "json")?;
    let model = FinancialModelSpec::from_json(json).map_err(to_js_err)?;
    let ids: Vec<&str> = model.nodes.keys().map(|k| k.as_str()).collect();
    crate::utils::to_js_value(&ids)
}

/// Validate a `CheckSuiteSpec` JSON string.
///
/// Deserializes the spec, re-serializes to canonical form, and
/// returns the JSON string. Useful for client-side validation.
///
/// # Errors
///
/// Rejects malformed or schema-incompatible `json`, or failure to serialize
/// the decoded check-suite specification.
/// @param json - Canonical JSON string defining the object to deserialize or normalize.
#[wasm_bindgen(js_name = validateCheckSuiteSpecJson)]
pub fn validate_check_suite_spec_json(json: JsValue) -> Result<String, JsValue> {
    let json: &str = &json_text(&json, "json")?;
    let spec: finstack_quant_statements::checks::CheckSuiteSpec =
        serde_json::from_str(json).map_err(to_js_err)?;
    serde_json::to_string(&spec).map_err(to_js_err)
}

/// Validate a `CapitalStructureSpec` JSON string.
///
/// Deserializes the spec, runs the Rust `CapitalStructureSpec::validate`
/// (waterfall consistency, instrument compatibility with prepayment rungs,
/// and swap side), and returns the canonical JSON.
///
/// # Errors
///
/// Rejects malformed or schema-incompatible `json`; an invalid waterfall; a
/// bond, convertible, swap, cap/floor, or swaption instrument alongside a
/// prepayment rung; an interest-rate swap with side `Receive`; or failure to
/// serialize the validated capital-structure specification.
/// @param json - Canonical JSON string defining the object to deserialize or normalize.
#[wasm_bindgen(js_name = validateCapitalStructureSpecJson)]
pub fn validate_capital_structure_spec_json(json: JsValue) -> Result<String, JsValue> {
    let json: &str = &json_text(&json, "json")?;
    let spec: finstack_quant_statements::types::CapitalStructureSpec =
        serde_json::from_str(json).map_err(to_js_err)?;
    spec.validate().map_err(to_js_err)?;
    serde_json::to_string(&spec).map_err(to_js_err)
}

/// Validate a `WaterfallSpec` JSON string.
///
/// Performs both serde deserialization and the waterfall's internal
/// consistency check (for example rejecting `Sweep` ordered after `Equity`
/// when an ECF sweep is configured).
///
/// # Errors
///
/// Rejects malformed or schema-incompatible `json`; duplicate or inconsistent
/// payment priorities; incomplete available-cash priorities; invalid PIK,
/// payment-class, prepay-node, or ECF-sweep settings; or failure to serialize
/// the validated waterfall.
/// @param json - Canonical JSON string for a `WaterfallSpec`, including `priority_of_payments`, `available_cash_node`, optional `ecf_sweep`, `pik_toggle`, `payment_classes`, `mandatory_prepay_node`, and `voluntary_prepay_node`.
#[wasm_bindgen(js_name = validateWaterfallSpecJson)]
pub fn validate_waterfall_spec_json(json: JsValue) -> Result<String, JsValue> {
    let json: &str = &json_text(&json, "json")?;
    let spec: finstack_quant_statements::capital_structure::WaterfallSpec =
        serde_json::from_str(json).map_err(to_js_err)?;
    spec.validate().map_err(to_js_err)?;
    serde_json::to_string(&spec).map_err(to_js_err)
}

/// Validate an `EcfSweepSpec` JSON string.
///
/// Deserializes the spec, runs the Rust `EcfSweepSpec::validate`, and returns
/// the canonical JSON. The waterfall-level rule that a positive sweep needs a
/// prepayment priority is checked by `validateWaterfallSpecJson`.
///
/// # Errors
///
/// Rejects malformed or schema-incompatible `json`, a `sweep_percentage`
/// outside `[0.0, 1.0]`, or failure to serialize the validated ECF-sweep
/// specification.
/// @param json - Canonical JSON string defining the object to deserialize or normalize.
#[wasm_bindgen(js_name = validateEcfSweepSpecJson)]
pub fn validate_ecf_sweep_spec_json(json: JsValue) -> Result<String, JsValue> {
    let json: &str = &json_text(&json, "json")?;
    let spec: finstack_quant_statements::capital_structure::EcfSweepSpec =
        serde_json::from_str(json).map_err(to_js_err)?;
    spec.validate().map_err(to_js_err)?;
    serde_json::to_string(&spec).map_err(to_js_err)
}

/// Validate a `PikToggleSpec` JSON string.
///
/// Deserializes the spec, runs the Rust `PikToggleSpec::validate`, and returns
/// the canonical JSON.
///
/// # Errors
///
/// Rejects malformed or schema-incompatible `json`, a missing or empty
/// `target_instrument_ids` list, or failure to serialize the validated
/// PIK-toggle specification.
/// @param json - Canonical JSON string defining the object to deserialize or normalize.
#[wasm_bindgen(js_name = validatePikToggleSpecJson)]
pub fn validate_pik_toggle_spec_json(json: JsValue) -> Result<String, JsValue> {
    let json: &str = &json_text(&json, "json")?;
    let spec: finstack_quant_statements::capital_structure::PikToggleSpec =
        serde_json::from_str(json).map_err(to_js_err)?;
    spec.validate().map_err(to_js_err)?;
    serde_json::to_string(&spec).map_err(to_js_err)
}

/// Export one evaluated node as a dated schedule.
///
/// Free-function twin of Python `StatementResult.to_dated_schedule` (Rust
/// `evaluator::node_to_dated_schedule`): periods are taken in model timeline
/// order, periods without a value are skipped, and each period is dated by
/// `convention`.
/// @param model_json - The `FinancialModelSpec` that produced the result (its periods supply the dates).
/// @param result_json - The `StatementResult` returned by `Evaluator.evaluate` / `evaluateWithMarket` (object or JSON).
/// @param node_id - Node identifier to export.
/// @param convention - Optional `"end"` (default: the period's last inclusive day, `end - 1 day`, since periods are half-open `[start, end)`) or `"start"`.
/// @returns `[isoDate, value]` pairs in timeline order, in the node's own units.
///
/// # Errors
///
/// Throws with kind `not_found` if `nodeId` has no values in the result, and
/// kind `validation` if an input is malformed or `convention` is not
/// `"start"` / `"end"`.
#[wasm_bindgen(js_name = nodeToDatedSchedule)]
pub fn node_to_dated_schedule(
    model_json: JsValue,
    result_json: JsValue,
    node_id: JsValue,
    convention: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let model =
        FinancialModelSpec::from_json(&json_text(&model_json, "modelJson")?).map_err(to_js_err)?;
    let result: finstack_quant_statements::evaluator::StatementResult =
        serde_json::from_str(&json_text(&result_json, "resultJson")?).map_err(to_js_err)?;
    let node_id = js_string(&node_id, "nodeId")?;
    let convention = match js_opt_string(convention.as_ref(), "convention")? {
        Some(convention) => convention.parse().map_err(to_js_err)?,
        None => finstack_quant_statements::evaluator::PeriodDateConvention::default(),
    };
    let rows: Vec<(String, f64)> = finstack_quant_statements::evaluator::node_to_dated_schedule(
        &model, &result, &node_id, convention,
    )
    .map_err(to_js_err)?
    .into_iter()
    .map(|(date, value)| (crate::utils::date_to_iso(date), value))
    .collect();
    crate::utils::to_js_value(&rows)
}

/// Probability that a metric exceeds a threshold in any forecast period.
///
/// Free-function twin of Python `MonteCarloResults.breach_probability` (Rust
/// `MonteCarloResults::breach_probability`). Checks upside breaches only
/// (`value > threshold`); negate values and threshold for a downside test.
/// Per-path values come from the result's `path_data` table, so the
/// simulation must run with `include_path_data: true`.
/// @param results_json - `MonteCarloResults` returned by `Evaluator.evaluateMonteCarlo` (object or JSON).
/// @param metric - Node identifier to test.
/// @param threshold - Breach level in the metric's own units.
/// @returns Fraction of paths that breach in at least one forecast period, or `undefined` when the metric has no path data (including results run without `include_path_data`), there are no forecast periods, or the simulation is incomplete.
///
/// # Errors
///
/// Throws with kind `validation` if the results input is malformed, and kind
/// `invalid_type` if `threshold` is not a number.
#[wasm_bindgen(js_name = monteCarloBreachProbability)]
pub fn monte_carlo_breach_probability(
    results_json: JsValue,
    metric: JsValue,
    threshold: JsValue,
) -> Result<Option<f64>, JsValue> {
    let results: finstack_quant_statements::evaluator::MonteCarloResults =
        serde_json::from_str(&json_text(&results_json, "resultsJson")?).map_err(to_js_err)?;
    Ok(results.breach_probability(
        &js_string(&metric, "metric")?,
        crate::utils::input::js_f64(&threshold, "threshold")?,
    ))
}

/// Percentile time series of one metric across the forecast periods.
///
/// Free-function twin of Python `MonteCarloResults.percentile_by_period` (Rust
/// `MonteCarloResults::percentile_by_period`): looks up a percentile that the
/// simulation was configured to report.
/// @param results_json - `MonteCarloResults` returned by `Evaluator.evaluateMonteCarlo` (object or JSON).
/// @param metric - Node identifier to read.
/// @param percentile - Percentile as a fraction in `[0, 1]` (e.g. `0.95`); must be one of the configured percentiles.
/// @returns Object mapping period id (e.g. `"2025Q1"`) to the percentile value in the metric's own units, or `undefined` when the metric or percentile is not in the results.
///
/// # Errors
///
/// Throws with kind `validation` if the results input is malformed, and kind
/// `invalid_type` if `percentile` is not a number.
#[wasm_bindgen(js_name = monteCarloPercentileByPeriod)]
pub fn monte_carlo_percentile_by_period(
    results_json: JsValue,
    metric: JsValue,
    percentile: JsValue,
) -> Result<JsValue, JsValue> {
    let results: finstack_quant_statements::evaluator::MonteCarloResults =
        serde_json::from_str(&json_text(&results_json, "resultsJson")?).map_err(to_js_err)?;
    match results.percentile_by_period(
        &js_string(&metric, "metric")?,
        crate::utils::input::js_f64(&percentile, "percentile")?,
    ) {
        Some(series) => crate::utils::to_js_value(&series),
        None => Ok(JsValue::UNDEFINED),
    }
}

/// Export a statement result as a long-format table.
///
/// Free-function twin of Python `StatementResult.to_arrow_long`
/// (Rust `StatementResult::to_table_long`): one row per `(node, period)` in
/// the result's node and period declaration order.
/// @param result_json - The `StatementResult` returned by `Evaluator.evaluate` / `evaluateWithMarket` (object or JSON).
/// @returns `TableEnvelope` with columns `node_id`, `period_id`, `value`, `value_money`, `currency`, `value_type`; monetary nodes repeat their value in `value_money` and set `currency`, scalar nodes leave both null.
///
/// # Errors
///
/// Throws with kind `validation` if the result input is malformed or table
/// construction fails.
#[wasm_bindgen(js_name = statementResultToTableLong)]
pub fn statement_result_to_table_long(result_json: JsValue) -> Result<JsValue, JsValue> {
    let result: finstack_quant_statements::evaluator::StatementResult =
        serde_json::from_str(&json_text(&result_json, "resultJson")?).map_err(to_js_err)?;
    crate::utils::to_js_value(&result.to_table_long().map_err(to_js_err)?)
}

/// Export a statement result as a wide-format table.
///
/// Free-function twin of Python `StatementResult.to_arrow_wide`
/// (Rust `StatementResult::to_table_wide`): one row per period in
/// chronological order and one column per node in declaration order.
/// @param result_json - The `StatementResult` returned by `Evaluator.evaluate` / `evaluateWithMarket` (object or JSON).
/// @returns `TableEnvelope` with a `period_id` column followed by one value column per node; a node with no value in a period holds `NaN` (serialized as `null`), not zero.
///
/// # Errors
///
/// Throws with kind `validation` if the result input is malformed or table
/// construction fails.
#[wasm_bindgen(js_name = statementResultToTableWide)]
pub fn statement_result_to_table_wide(result_json: JsValue) -> Result<JsValue, JsValue> {
    let result: finstack_quant_statements::evaluator::StatementResult =
        serde_json::from_str(&json_text(&result_json, "resultJson")?).map_err(to_js_err)?;
    crate::utils::to_js_value(&result.to_table_wide().map_err(to_js_err)?)
}

/// Canonical content hash of a financial model.
///
/// Free-function twin of Python `FinancialModelSpec.content_hash` (Rust
/// `FinancialModelSpec::content_hash`): the model is loaded and validated
/// through `FinancialModelSpec::from_json`, then hashed over its canonical
/// JSON, so the hash does not depend on key order or number spelling of the
/// typed fields. Free-form `meta`/`params` maps keep their JSON spelling.
/// @param model_json - `FinancialModelSpec` (object or JSON).
/// @returns `"sha256:<hex>"` content hash.
///
/// # Errors
///
/// Throws with kind `validation` if the model is malformed or fails semantic
/// validation, or contains a non-finite number.
#[wasm_bindgen(js_name = financialModelContentHash)]
pub fn financial_model_content_hash(model_json: JsValue) -> Result<String, JsValue> {
    FinancialModelSpec::from_json(&json_text(&model_json, "modelJson")?)
        .map_err(to_js_err)?
        .content_hash()
        .map_err(to_js_err)
}

/// Parse a DSL formula and return its canonical source text.
///
/// The formula is parsed into the statements AST and rendered back through
/// the AST's `Display`: whitespace normalised, operators spaced, and
/// parentheses kept only where precedence requires them. Parsing the returned
/// text again yields the same AST, so it is a stable form for previewing,
/// diffing, or hashing formulas. Mirrors Python `parse_formula`.
///
/// # Errors
///
/// Rejects trailing tokens, malformed or incomplete syntax, or a formula that
/// exceeds the parser's nesting or term limits.
/// @param formula - Financial-model formula string to parse into its canonical expression representation.
/// @returns Canonical formula text, e.g. `"(revenue - cogs) / revenue"`.
#[wasm_bindgen(js_name = parseFormula)]
pub fn parse_formula(formula: JsValue) -> Result<String, JsValue> {
    let formula: &str = &js_string(&formula, "formula")?;
    let ast = finstack_quant_statements::dsl::parse_formula(formula).map_err(to_js_err)?;
    Ok(ast.to_string())
}

/// Parse and compile a DSL formula, throwing if either step fails.
///
/// Compilation lowers the AST onto the core expression engine and rejects
/// unsupported functions, wrong arities, and malformed capital-structure
/// references that a bare parse would accept. Returns `undefined` when the
/// formula is valid; an invalid formula throws a `FinstackError`, so
/// `if (parseAndCompile(f))` is not a validity check. Mirrors Python
/// `parse_and_compile`.
///
/// # Errors
///
/// Rejects any formula that cannot be parsed as one complete DSL expression or
/// compiled because it contains an unsupported component, function, or
/// operator form.
/// @param formula - Financial-model formula string to parse and validate without evaluation.
#[wasm_bindgen(js_name = parseAndCompile)]
pub fn parse_and_compile(formula: JsValue) -> Result<(), JsValue> {
    let formula: &str = &js_string(&formula, "formula")?;
    finstack_quant_statements::dsl::parse_and_compile(formula).map_err(to_js_err)?;
    Ok(())
}

#[cfg(test)]
mod tests {

    #[test]
    fn validate_financial_model_json_rejects_empty_periods() {
        // Test the Rust-level behavior natively (the previous cfg-gating made
        // this compile out of `cargo test` and never run under wasm either).
        let mut model = finstack_quant_statements::FinancialModelSpec::new("test", vec![]);
        assert!(
            model.validate_semantics().is_err(),
            "semantic validation should reject empty periods"
        );
    }

    #[test]
    fn validate_waterfall_spec_rejects_inverted_priority() {
        // Sweep after Equity is caught by WaterfallSpec::validate() once the
        // required cash node and cash-capping priorities are present.
        let bad = serde_json::json!({
            "priority_of_payments": ["fees", "interest", "amortization", "equity", "sweep"],
            "available_cash_node": "cash",
            "ecf_sweep": {
                "ebitda_node": "ebitda",
                "sweep_percentage": 0.5,
            },
        });
        let json = bad.to_string();
        let spec: finstack_quant_statements::capital_structure::WaterfallSpec =
            serde_json::from_str(&json).expect("parses");
        let err = spec
            .validate()
            .expect_err("equity before sweep should fail");
        let msg = err.to_string();
        assert!(
            msg.contains("Equity") && msg.contains("last entry"),
            "expected non-terminal equity error, got: {msg}"
        );
    }

    #[test]
    fn evaluate_model_runs_minimal_model() {
        use finstack_quant_statements::builder::ModelBuilder;
        use finstack_quant_statements::types::AmountOrScalar;
        let model = ModelBuilder::new("t")
            .periods("2025Q1..Q2", None)
            .expect("periods")
            .value(
                "revenue",
                &[
                    (
                        finstack_quant_core::dates::PeriodId::quarter(2025, 1)
                            .expect("valid period fixture"),
                        AmountOrScalar::scalar(100.0),
                    ),
                    (
                        finstack_quant_core::dates::PeriodId::quarter(2025, 2)
                            .expect("valid period fixture"),
                        AmountOrScalar::scalar(110.0),
                    ),
                ],
            )
            .compute("margin", "revenue * 0.4")
            .expect("compute")
            .build()
            .expect("build");
        // `Evaluator.evaluate` returns a `JsValue`, which cannot be constructed
        // off wasm32; exercise the evaluator it delegates to instead, and let
        // tests/facade/statements.test.mjs assert the JS object shape.
        let mut evaluator = finstack_quant_statements::evaluator::Evaluator::new();
        let result = evaluator.evaluate(&model).expect("evaluate should succeed");
        assert!(result.nodes.contains_key("revenue"));
        assert!(result.nodes.contains_key("margin"));
    }

    #[test]
    fn evaluate_monte_carlo_on_model() {
        use finstack_quant_statements::builder::ModelBuilder;
        use finstack_quant_statements::types::AmountOrScalar;

        let model = ModelBuilder::new("mc")
            .periods("2025Q1..Q2", None)
            .expect("periods")
            .value(
                "revenue",
                &[
                    (
                        finstack_quant_core::dates::PeriodId::quarter(2025, 1)
                            .expect("valid period fixture"),
                        AmountOrScalar::scalar(100.0),
                    ),
                    (
                        finstack_quant_core::dates::PeriodId::quarter(2025, 2)
                            .expect("valid period fixture"),
                        AmountOrScalar::scalar(110.0),
                    ),
                ],
            )
            .build()
            .expect("build");
        let config = finstack_quant_statements::evaluator::MonteCarloConfig::new(10, 42);

        // `Evaluator.evaluateMonteCarlo` returns a `JsValue` (unconstructible
        // off wasm32); assert the underlying engine and its serializable shape.
        let mut evaluator = finstack_quant_statements::evaluator::Evaluator::new();
        let results = evaluator
            .evaluate_monte_carlo(&model, &config)
            .expect("run Monte Carlo");
        let parsed = serde_json::to_value(&results).expect("results serialize");
        assert!(parsed.is_object());
    }

    #[test]
    fn parse_and_compile_rejects_invalid() {
        // Error path creates JsValue, which panics on native targets.
        // Test the underlying compile instead.
        assert!(finstack_quant_statements::dsl::parse_and_compile("revenue @").is_err());
    }

    // -- Boundary tests ------------------------------------------------
    // Error paths create JsValue, which panics on native targets.
    // Test the underlying serde deserialization instead.

    #[test]
    fn validate_rejects_invalid_json() {
        assert!(
            serde_json::from_str::<finstack_quant_statements::FinancialModelSpec>("not json")
                .is_err()
        );
    }

    #[test]
    fn validate_rejects_empty_string() {
        assert!(serde_json::from_str::<finstack_quant_statements::FinancialModelSpec>("").is_err());
    }
}
