//! WASM bindings for the `finstack-quant-statements` crate.
//!
//! Exposes JSON-in / JSON-out functions for:
//! - `FinancialModelSpec` validation and node enumeration
//! - `CheckSuiteSpec`, `WaterfallSpec`, `EcfSweepSpec`, `PikToggleSpec`,
//!   `CapitalStructureSpec` validation
//! - DSL formula parsing and validation
//! - Full `Evaluator` execution, including Monte Carlo paths
//!
//! The evaluator runs a fresh `Evaluator::new()` per call; WASM clients
//! hold no live handles. Capital-structure models are configured by
//! embedding the spec directly in the `FinancialModelSpec` JSON — there is
//! no separate builder surface on this side because JS assembles JSON
//! natively.

use crate::utils::to_js_err;
use wasm_bindgen::prelude::*;

/// Deserialize a `FinancialModelSpec` JSON string and run semantic validation.
///
/// Every WASM entry point that ingests a model routes through this helper so
/// structurally invalid specs (empty periods, invalid node ids, bad formulas)
/// are rejected identically here and in the typed Python `from_json` path —
/// otherwise the same input would diverge between the two bindings.
///
/// # Errors
///
/// Rejects malformed or schema-incompatible `json` and any semantic-validation
/// failure, with the same error shaping as the other statements entry points.
pub(crate) fn parse_validated_model(
    json: &str,
) -> Result<finstack_quant_statements::FinancialModelSpec, JsValue> {
    let mut model: finstack_quant_statements::FinancialModelSpec =
        serde_json::from_str(json).map_err(to_js_err)?;
    model.validate_semantics().map_err(to_js_err)?;
    Ok(model)
}

/// Validate a `FinancialModelSpec` JSON string.
///
/// Deserializes the input against the model schema, runs semantic validation,
/// and returns the canonical (re-serialized) JSON.
///
/// # Errors
///
/// Rejects malformed or schema-incompatible `json`, an empty or invalid period
/// timeline, reserved node identifiers, incompatible node fields or value
/// types, invalid formulas or dimensions, an invalid waterfall, or failure to
/// serialize the normalized model.
/// @param json - Canonical JSON string defining the object to deserialize or normalize.
#[wasm_bindgen(js_name = validateFinancialModelJson)]
pub fn validate_financial_model_json(json: &str) -> Result<String, JsValue> {
    let model = parse_validated_model(json)?;
    serde_json::to_string(&model).map_err(to_js_err)
}

/// Get the node identifiers from a model specification JSON.
///
/// Returns a JS array of node ID strings in declaration order.
///
/// # Errors
///
/// Rejects malformed or schema-incompatible `json`, or if the node identifiers
/// cannot be serialized to JavaScript.
/// @param json - Canonical JSON string defining the object to deserialize or normalize.
#[wasm_bindgen(js_name = modelNodeIds)]
pub fn model_node_ids(json: &str) -> Result<JsValue, JsValue> {
    let model: finstack_quant_statements::FinancialModelSpec =
        serde_json::from_str(json).map_err(to_js_err)?;
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
pub fn validate_check_suite_spec_json(json: &str) -> Result<String, JsValue> {
    let spec: finstack_quant_statements::checks::CheckSuiteSpec =
        serde_json::from_str(json).map_err(to_js_err)?;
    serde_json::to_string(&spec).map_err(to_js_err)
}

/// Validate a `CapitalStructureSpec` JSON string.
///
/// # Errors
///
/// Rejects malformed or schema-incompatible `json`, or failure to serialize
/// the decoded capital-structure specification.
/// @param json - Canonical JSON string defining the object to deserialize or normalize.
#[wasm_bindgen(js_name = validateCapitalStructureSpecJson)]
pub fn validate_capital_structure_spec_json(json: &str) -> Result<String, JsValue> {
    let spec: finstack_quant_statements::types::CapitalStructureSpec =
        serde_json::from_str(json).map_err(to_js_err)?;
    serde_json::to_string(&spec).map_err(to_js_err)
}

/// Validate a `WaterfallSpec` JSON string.
///
/// Performs both serde deserialization and the waterfall's internal
/// consistency check (for example rejecting `Sweep` ordered after `Equity`
/// when an ECF sweep is configured).
///
/// # Arguments
///
/// * `json` - Canonical JSON string for a `WaterfallSpec`, including
///   `priority_of_payments`, `available_cash_node`, optional `ecf_sweep`,
///   `pik_toggle`, `payment_classes`, `mandatory_prepay_node`, and
///   `voluntary_prepay_node`.
///
/// # Errors
///
/// Rejects malformed or schema-incompatible `json`; duplicate or inconsistent
/// payment priorities; incomplete available-cash priorities; invalid PIK,
/// payment-class, prepay-node, or ECF-sweep settings; or failure to serialize
/// the validated waterfall.
/// @param json - Canonical JSON string defining the object to deserialize or normalize.
#[wasm_bindgen(js_name = validateWaterfallSpecJson)]
pub fn validate_waterfall_spec_json(json: &str) -> Result<String, JsValue> {
    let spec: finstack_quant_statements::capital_structure::WaterfallSpec =
        serde_json::from_str(json).map_err(to_js_err)?;
    spec.validate().map_err(to_js_err)?;
    serde_json::to_string(&spec).map_err(to_js_err)
}

/// Validate an `EcfSweepSpec` JSON string.
///
/// # Errors
///
/// Rejects malformed or schema-incompatible `json`, or failure to serialize
/// the decoded ECF-sweep specification.
/// @param json - Canonical JSON string defining the object to deserialize or normalize.
#[wasm_bindgen(js_name = validateEcfSweepSpecJson)]
pub fn validate_ecf_sweep_spec_json(json: &str) -> Result<String, JsValue> {
    let spec: finstack_quant_statements::capital_structure::EcfSweepSpec =
        serde_json::from_str(json).map_err(to_js_err)?;
    serde_json::to_string(&spec).map_err(to_js_err)
}

/// Validate a `PikToggleSpec` JSON string.
///
/// # Errors
///
/// Rejects malformed or schema-incompatible `json`, or failure to serialize
/// the decoded PIK-toggle specification.
/// @param json - Canonical JSON string defining the object to deserialize or normalize.
#[wasm_bindgen(js_name = validatePikToggleSpecJson)]
pub fn validate_pik_toggle_spec_json(json: &str) -> Result<String, JsValue> {
    let spec: finstack_quant_statements::capital_structure::PikToggleSpec =
        serde_json::from_str(json).map_err(to_js_err)?;
    serde_json::to_string(&spec).map_err(to_js_err)
}

/// Evaluate a `FinancialModelSpec` and return the `StatementResult`.
///
/// Returns a structured JavaScript object (the Python binding returns a typed
/// `StatementResult` from the same Rust evaluator).
///
/// # Errors
///
/// Rejects malformed `model_json`, model semantic failures, invalid formula or
/// dependency graphs, missing evaluation inputs, unsupported capital-structure
/// requirements, or failure to serialize the statement result to JavaScript.
/// @param model_json - JSON-serialized FinancialModelSpec to evaluate across its statement periods.
#[wasm_bindgen(js_name = evaluateModel)]
pub fn evaluate_model(model_json: &str) -> Result<JsValue, JsValue> {
    let model = parse_validated_model(model_json)?;
    let mut evaluator = finstack_quant_statements::evaluator::Evaluator::new();
    let result = evaluator.evaluate(&model).map_err(to_js_err)?;
    crate::utils::to_js_value(&result)
}

/// Evaluate a `FinancialModelSpec` against a `MarketContext` as of a given date.
///
/// Required for capital-structure-aware models. The `as_of` argument is an
/// ISO 8601 date string (e.g. `"2025-01-15"`). Returns a structured
/// JavaScript object, matching [`evaluate_model`].
///
/// # Errors
///
/// Rejects malformed model or market JSON, model semantic failures, an invalid
/// ISO `as_of` date, invalid formulas or dependencies, missing market data, or
/// failure to serialize the statement result to JavaScript.
/// @param model_json - JSON-serialized FinancialModelSpec to evaluate across its statement periods.
/// @param market_json - Canonical market-context JSON supplying curves, quotes, and FX data.
/// @param as_of - ISO-8601 valuation date used to resolve date-dependent market data.
#[wasm_bindgen(js_name = evaluateModelWithMarket)]
pub fn evaluate_model_with_market(
    model_json: &str,
    market_json: &str,
    as_of: &str,
) -> Result<JsValue, JsValue> {
    let model = parse_validated_model(model_json)?;
    let market: finstack_quant_core::market_data::context::MarketContext =
        serde_json::from_str(market_json).map_err(to_js_err)?;
    // Use the shared ISO date parser for a consistent `YYYY-MM-DD` grammar and
    // error message across all wasm namespaces.
    let date = crate::utils::parse_iso_date(as_of)?;
    let mut evaluator = finstack_quant_statements::evaluator::Evaluator::new();
    let result = evaluator
        .evaluate_with_market(&model, &market, date)
        .map_err(to_js_err)?;
    crate::utils::to_js_value(&result)
}

/// Run Monte Carlo simulation on a financial model.
///
/// Takes JSON inputs and returns a structured JavaScript object (the Python
/// binding returns a typed `MonteCarloResults` from the same Rust engine).
///
/// # Errors
///
/// Rejects malformed model or configuration JSON, model semantic failures,
/// zero simulation paths, a model containing capital structure, model
/// compilation or dependency failures, any path-evaluation failure, or failure
/// to serialize the results to JavaScript.
/// @param model_json - Financial-model specification JSON.
/// @param config_json - Monte Carlo configuration JSON.
#[wasm_bindgen(js_name = runMonteCarlo)]
pub fn run_monte_carlo(model_json: &str, config_json: &str) -> Result<JsValue, JsValue> {
    let model = parse_validated_model(model_json)?;
    let config: finstack_quant_statements::evaluator::MonteCarloConfig =
        serde_json::from_str(config_json).map_err(to_js_err)?;
    let mut evaluator = finstack_quant_statements::evaluator::Evaluator::new();
    let results = evaluator
        .evaluate_monte_carlo(&model, &config)
        .map_err(to_js_err)?;
    crate::utils::to_js_value(&results)
}

/// Parse a DSL formula and return a human-readable rendering of its AST.
///
/// Useful for previewing expression structure in UI tooling before
/// committing a formula to a model. The returned string is a debug rendering,
/// **not** JSON: the canonical `StmtExpr` AST deliberately does not implement
/// `serde::Serialize`, so there is no structured wire form to return. Treat
/// the output as display text and do not parse it.
///
/// # Errors
///
/// Rejects trailing tokens, malformed or incomplete syntax, or a formula that
/// exceeds the parser's nesting or term limits.
/// @param formula - Financial-model formula string to parse into its canonical expression representation.
#[wasm_bindgen(js_name = parseFormulaText)]
pub fn parse_formula_text(formula: &str) -> Result<String, JsValue> {
    let ast = finstack_quant_statements::dsl::parse_formula(formula).map_err(to_js_err)?;
    Ok(format!("{ast:?}"))
}

/// Validate that a DSL formula parses and compiles successfully.
///
/// Returns `undefined` when the formula is valid; throws a `FinstackError`
/// otherwise. This mirrors the Python `validate_formula` API, which returns
/// `None` — an invalid formula raises rather than returning a falsy value, so
/// `if (validateFormula(f))` is not a validity check.
///
/// # Errors
///
/// Rejects any formula that cannot be parsed as one complete DSL expression or
/// compiled because it contains an unsupported component, function, or
/// operator form.
/// @param formula - Financial-model formula string to parse and validate without evaluation.
#[wasm_bindgen(js_name = validateFormula)]
pub fn validate_formula(formula: &str) -> Result<(), JsValue> {
    finstack_quant_statements::dsl::parse_and_compile(formula).map_err(to_js_err)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_financial_model_json_accepts_valid_model() {
        let periods = finstack_quant_core::dates::build_periods("2025Q1..Q1", None)
            .expect("valid periods")
            .periods;
        let model = finstack_quant_statements::FinancialModelSpec::new("test", periods);
        let json = serde_json::to_string(&model).expect("model should serialize to JSON");
        let out = validate_financial_model_json(&json)
            .expect("validate_financial_model_json should accept valid model");
        let round_trip =
            serde_json::from_str::<finstack_quant_statements::FinancialModelSpec>(&out)
                .expect("validated JSON should deserialize");
        assert_eq!(round_trip.id, "test");
        assert!(round_trip.nodes.is_empty());
    }

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
    fn validate_check_suite_spec_roundtrip() {
        let spec = finstack_quant_statements::checks::CheckSuiteSpec {
            name: "test".to_string(),
            description: None,
            builtin_checks: vec![],
            formula_checks: vec![],
            config: finstack_quant_statements::checks::CheckConfig::default(),
        };
        let json = serde_json::to_string(&spec).expect("serialize");
        let out = validate_check_suite_spec_json(&json).expect("should accept valid spec");
        let rt = serde_json::from_str::<finstack_quant_statements::checks::CheckSuiteSpec>(&out)
            .expect("should roundtrip");
        assert_eq!(rt.name, "test");
    }

    #[test]
    fn validate_waterfall_spec_accepts_minimal_spec() {
        let spec = finstack_quant_statements::capital_structure::WaterfallSpec {
            priority_of_payments: vec![
                finstack_quant_statements::capital_structure::PaymentPriority::Fees,
                finstack_quant_statements::capital_structure::PaymentPriority::Interest,
                finstack_quant_statements::capital_structure::PaymentPriority::Amortization,
            ],
            available_cash_node: "cash".into(),
            ecf_sweep: None,
            pik_toggle: None,
            ..Default::default()
        };
        let json = serde_json::to_string(&spec).expect("serialize");
        let out = validate_waterfall_spec_json(&json).expect("should accept default spec");
        assert!(out.contains("priority_of_payments"));
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
        // `evaluate_model` now returns a `JsValue`, which cannot be constructed
        // off wasm32; exercise the evaluator it delegates to instead, and let
        // tests/facade/statements.test.mjs assert the JS object shape.
        let mut evaluator = finstack_quant_statements::evaluator::Evaluator::new();
        let result = evaluator.evaluate(&model).expect("evaluate should succeed");
        assert!(result.nodes.contains_key("revenue"));
        assert!(result.nodes.contains_key("margin"));
    }

    #[test]
    fn run_monte_carlo_on_model() {
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

        // `run_monte_carlo` now returns a `JsValue` (unconstructible off
        // wasm32); assert the underlying engine and its serializable shape.
        let mut evaluator = finstack_quant_statements::evaluator::Evaluator::new();
        let results = evaluator
            .evaluate_monte_carlo(&model, &config)
            .expect("run Monte Carlo");
        let parsed = serde_json::to_value(&results).expect("results serialize");
        assert!(parsed.is_object());
    }

    #[test]
    fn parse_formula_returns_ast_debug() {
        let out = parse_formula_text("revenue - cogs").expect("parse_formula_text should succeed");
        // Debug format contains "BinOp"/"NodeRef" markers
        assert!(!out.is_empty());
    }

    #[test]
    fn validate_formula_accepts_valid() {
        validate_formula("revenue * 0.5").expect("should accept valid formula");
    }

    #[test]
    fn validate_formula_rejects_invalid() {
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
