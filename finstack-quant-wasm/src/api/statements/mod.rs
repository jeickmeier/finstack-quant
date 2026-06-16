//! WASM bindings for the `finstack-quant-statements` crate.
//!
//! Exposes JSON-in / JSON-out functions for:
//! - `FinancialModelSpec` validation and node enumeration
//! - `CheckSuiteSpec`, `WaterfallSpec`, `EcfSweepSpec`, `PikToggleSpec`,
//!   `CapitalStructureSpec` validation
//! - DSL formula parsing and validation
//! - Full `Evaluator` execution (`evaluate`, `evaluate_with_market`)
//!
//! The evaluator runs a fresh `Evaluator::new()` per call; WASM clients
//! hold no live handles. Capital-structure models are configured by
//! embedding the spec directly in the `FinancialModelSpec` JSON — there is
//! no separate builder surface on this side because JS assembles JSON
//! natively.

use crate::utils::to_js_err;
use wasm_bindgen::prelude::*;

// ---------------------------------------------------------------------------
// Validators
// ---------------------------------------------------------------------------

/// Validate a `FinancialModelSpec` JSON string.
///
/// Deserializes the input against the model schema, runs semantic validation,
/// and returns the canonical (re-serialized) JSON.
#[wasm_bindgen(js_name = validateFinancialModelJson)]
pub fn validate_financial_model_json(json: &str) -> Result<String, JsValue> {
    let mut model: finstack_quant_statements::FinancialModelSpec =
        serde_json::from_str(json).map_err(to_js_err)?;
    model.validate_semantics().map_err(to_js_err)?;
    serde_json::to_string(&model).map_err(to_js_err)
}

/// Get the node identifiers from a model specification JSON.
///
/// Returns a JS array of node ID strings in declaration order.
#[wasm_bindgen(js_name = modelNodeIds)]
pub fn model_node_ids(json: &str) -> Result<JsValue, JsValue> {
    let model: finstack_quant_statements::FinancialModelSpec =
        serde_json::from_str(json).map_err(to_js_err)?;
    let ids: Vec<&str> = model.nodes.keys().map(|k| k.as_str()).collect();
    serde_wasm_bindgen::to_value(&ids).map_err(to_js_err)
}

/// Validate a `CheckSuiteSpec` JSON string.
///
/// Deserializes the spec, re-serializes to canonical form, and
/// returns the JSON string. Useful for client-side validation.
#[wasm_bindgen(js_name = validateCheckSuiteSpec)]
pub fn validate_check_suite_spec(json: &str) -> Result<String, JsValue> {
    let spec: finstack_quant_statements::checks::CheckSuiteSpec =
        serde_json::from_str(json).map_err(to_js_err)?;
    serde_json::to_string(&spec).map_err(to_js_err)
}

/// Validate a `CapitalStructureSpec` JSON string.
#[wasm_bindgen(js_name = validateCapitalStructureSpec)]
pub fn validate_capital_structure_spec(json: &str) -> Result<String, JsValue> {
    let spec: finstack_quant_statements::types::CapitalStructureSpec =
        serde_json::from_str(json).map_err(to_js_err)?;
    serde_json::to_string(&spec).map_err(to_js_err)
}

/// Validate a `WaterfallSpec` JSON string.
///
/// Performs both serde deserialization and the waterfall's internal
/// consistency check (for example rejecting `Sweep` ordered after `Equity`
/// when an ECF sweep is configured).
#[wasm_bindgen(js_name = validateWaterfallSpec)]
pub fn validate_waterfall_spec(json: &str) -> Result<String, JsValue> {
    let spec: finstack_quant_statements::capital_structure::WaterfallSpec =
        serde_json::from_str(json).map_err(to_js_err)?;
    spec.validate().map_err(to_js_err)?;
    serde_json::to_string(&spec).map_err(to_js_err)
}

/// Validate an `EcfSweepSpec` JSON string.
#[wasm_bindgen(js_name = validateEcfSweepSpec)]
pub fn validate_ecf_sweep_spec(json: &str) -> Result<String, JsValue> {
    let spec: finstack_quant_statements::capital_structure::EcfSweepSpec =
        serde_json::from_str(json).map_err(to_js_err)?;
    serde_json::to_string(&spec).map_err(to_js_err)
}

/// Validate a `PikToggleSpec` JSON string.
#[wasm_bindgen(js_name = validatePikToggleSpec)]
pub fn validate_pik_toggle_spec(json: &str) -> Result<String, JsValue> {
    let spec: finstack_quant_statements::capital_structure::PikToggleSpec =
        serde_json::from_str(json).map_err(to_js_err)?;
    serde_json::to_string(&spec).map_err(to_js_err)
}

// ---------------------------------------------------------------------------
// Evaluator
// ---------------------------------------------------------------------------

/// Evaluate a `FinancialModelSpec` and return the `StatementResult` JSON.
#[wasm_bindgen(js_name = evaluateModel)]
pub fn evaluate_model(model_json: &str) -> Result<String, JsValue> {
    let model: finstack_quant_statements::FinancialModelSpec =
        serde_json::from_str(model_json).map_err(to_js_err)?;
    let mut evaluator = finstack_quant_statements::evaluator::Evaluator::new();
    let result = evaluator.evaluate(&model).map_err(to_js_err)?;
    serde_json::to_string(&result).map_err(to_js_err)
}

/// Evaluate a `FinancialModelSpec` against a `MarketContext` as of a given date.
///
/// Required for capital-structure-aware models. The `as_of` argument is an
/// ISO 8601 date string (e.g. `"2025-01-15"`).
#[wasm_bindgen(js_name = evaluateModelWithMarket)]
pub fn evaluate_model_with_market(
    model_json: &str,
    market_json: &str,
    as_of: &str,
) -> Result<String, JsValue> {
    let model: finstack_quant_statements::FinancialModelSpec =
        serde_json::from_str(model_json).map_err(to_js_err)?;
    let market: finstack_quant_core::market_data::context::MarketContext =
        serde_json::from_str(market_json).map_err(to_js_err)?;
    let format = time::format_description::well_known::Iso8601::DEFAULT;
    let date = time::Date::parse(as_of, &format).map_err(to_js_err)?;
    let mut evaluator = finstack_quant_statements::evaluator::Evaluator::new();
    let result = evaluator
        .evaluate_with_market(&model, &market, date)
        .map_err(to_js_err)?;
    serde_json::to_string(&result).map_err(to_js_err)
}

// ---------------------------------------------------------------------------
// DSL
// ---------------------------------------------------------------------------

/// Parse a DSL formula and return a debug string for its AST.
///
/// Useful for previewing expression structure in UI tooling before
/// committing a formula to a model.
#[wasm_bindgen(js_name = parseFormula)]
pub fn parse_formula(formula: &str) -> Result<String, JsValue> {
    let ast = finstack_quant_statements::dsl::parse_formula(formula).map_err(to_js_err)?;
    Ok(format!("{ast:?}"))
}

/// Validate that a DSL formula parses and compiles successfully.
///
/// Returns `true` when the formula is valid; throws a `FinstackError`
/// otherwise. This mirrors the Python `validate_formula` API.
#[wasm_bindgen(js_name = validateFormula)]
pub fn validate_formula(formula: &str) -> Result<bool, JsValue> {
    finstack_quant_statements::dsl::parse_and_compile(formula).map_err(to_js_err)?;
    Ok(true)
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

    #[cfg(target_arch = "wasm32")]
    #[test]
    fn validate_financial_model_json_rejects_empty_periods() {
        let model = finstack_quant_statements::FinancialModelSpec::new("test", vec![]);
        #[cfg(not(target_arch = "wasm32"))]
        {
            assert!(
                model.validate_semantics().is_err(),
                "semantic validation should reject empty periods"
            );
        }
        #[cfg(target_arch = "wasm32")]
        {
            let json = serde_json::to_string(&model).expect("model should serialize to JSON");
            assert!(
                validate_financial_model_json(&json).is_err(),
                "semantic validation should reject empty periods"
            );
        }
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
        let out = validate_check_suite_spec(&json).expect("should accept valid spec");
        let rt = serde_json::from_str::<finstack_quant_statements::checks::CheckSuiteSpec>(&out)
            .expect("should roundtrip");
        assert_eq!(rt.name, "test");
    }

    #[test]
    fn validate_waterfall_spec_accepts_default() {
        let spec = finstack_quant_statements::capital_structure::WaterfallSpec::default();
        let json = serde_json::to_string(&spec).expect("serialize");
        let out = validate_waterfall_spec(&json).expect("should accept default spec");
        assert!(out.contains("priority_of_payments"));
    }

    #[test]
    fn validate_waterfall_spec_rejects_inverted_priority() {
        // Sweep after Equity with positive ECF sweep is caught by WaterfallSpec::validate()
        let bad = serde_json::json!({
            "priority_of_payments": ["equity", "sweep"],
            "ecf_sweep": {
                "ebitda_node": "ebitda",
                "sweep_percentage": 0.5,
            },
        });
        let json = bad.to_string();
        let spec: finstack_quant_statements::capital_structure::WaterfallSpec =
            serde_json::from_str(&json).expect("parses");
        assert!(spec.validate().is_err());
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
                        finstack_quant_core::dates::PeriodId::quarter(2025, 1),
                        AmountOrScalar::scalar(100.0),
                    ),
                    (
                        finstack_quant_core::dates::PeriodId::quarter(2025, 2),
                        AmountOrScalar::scalar(110.0),
                    ),
                ],
            )
            .compute("margin", "revenue * 0.4")
            .expect("compute")
            .build()
            .expect("build");
        let json = serde_json::to_string(&model).expect("serialize");
        let out = evaluate_model(&json).expect("evaluate_model should succeed");
        let result: finstack_quant_statements::evaluator::StatementResult =
            serde_json::from_str(&out).expect("deserialize result");
        assert!(result.nodes.contains_key("revenue"));
        assert!(result.nodes.contains_key("margin"));
    }

    #[test]
    fn parse_formula_returns_ast_debug() {
        let out = parse_formula("revenue - cogs").expect("parse_formula should succeed");
        // Debug format contains "BinOp"/"NodeRef" markers
        assert!(!out.is_empty());
    }

    #[test]
    fn validate_formula_accepts_valid() {
        let ok = validate_formula("revenue * 0.5").expect("should accept valid formula");
        assert!(ok);
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
