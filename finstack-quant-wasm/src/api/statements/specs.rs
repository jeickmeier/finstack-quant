//! Free-function twins of the Python spec-object constructors and methods.
//!
//! WASM specs are plain objects, so each Rust constructor or method on
//! `ForecastSpec`, `Adjustment`, `NormalizationConfig`, `CheckConfig` and `CheckSuiteSpec`
//! is a function that returns (or takes) the plain object.

use crate::utils::input::{from_js_json, js_string};
use crate::utils::{to_js_err, to_js_value};
use finstack_quant_statements::adjustments::{
    Adjustment, NormalizationConfig, NormalizationResult,
};
use finstack_quant_statements::checks::{BuiltinCheckSpec, CheckConfig};
use finstack_quant_statements::evaluator::StatementResult;
use wasm_bindgen::prelude::*;

/// Copy of a normalization configuration with one more adjustment.
///
/// Free-function twin of Python `NormalizationConfig.add_adjustment` (Rust
/// `NormalizationConfig::add_adjustment`), which rejects a duplicate
/// adjustment identifier.
/// @param config - `NormalizationConfig` (object or JSON).
/// @param adjustment - `Adjustment` to append (object or JSON).
/// @returns New plain `NormalizationConfig` object.
///
/// # Errors
///
/// Throws with kind `validation` if an input is malformed or the adjustment
/// identifier already exists in the configuration.
#[wasm_bindgen(js_name = normalizationConfigAddAdjustment)]
pub fn normalization_config_add_adjustment(
    config: JsValue,
    adjustment: JsValue,
) -> Result<JsValue, JsValue> {
    let config: NormalizationConfig = from_js_json(&config, "config")?;
    let adjustment: Adjustment = from_js_json(&adjustment, "adjustment")?;
    to_js_value(&config.add_adjustment(adjustment).map_err(to_js_err)?)
}

/// Validate a `NormalizationConfig` and return its canonical JSON.
///
/// JSON wire twin of Python `NormalizationConfig.from_json` followed by
/// `validate` (Rust `NormalizationConfig::validate`).
/// @param json - `NormalizationConfig` (object or JSON).
/// @returns Canonical normalization-configuration JSON.
///
/// # Errors
///
/// Throws with kind `validation` if `json` is malformed or holds duplicate
/// adjustment identifiers.
#[wasm_bindgen(js_name = validateNormalizationConfigJson)]
pub fn validate_normalization_config_json(json: JsValue) -> Result<String, JsValue> {
    crate::api::statements::validated_json(&json, NormalizationConfig::validate)
}

fn normalized(results: &JsValue, config: &JsValue) -> Result<Vec<NormalizationResult>, JsValue> {
    let results: StatementResult = from_js_json(results, "results")?;
    let config: NormalizationConfig = from_js_json(config, "config")?;
    finstack_quant_statements::adjustments::normalize(&results, &config).map_err(to_js_err)
}

/// Normalize a target metric by applying an adjustment catalog period by period.
///
/// Twin of Python `normalize` (Rust `adjustments::normalize`).
/// @param results - Evaluated `StatementResult` holding the target node and any reference nodes (object or JSON).
/// @param config - `NormalizationConfig`: target node plus the adjustments to apply (object or JSON).
/// @returns One `NormalizationResult` per period of the target node: base value, each applied adjustment (raw and capped) and the final value.
///
/// # Errors
///
/// Throws with kind `validation` if an input is malformed, the target node or
/// a referenced node is absent from the results, the configuration is
/// invalid, or the target and a reference node have incompatible units.
#[wasm_bindgen(js_name = normalize)]
pub fn normalize(results: JsValue, config: JsValue) -> Result<JsValue, JsValue> {
    to_js_value(&normalized(&results, &config)?)
}

/// Write normalized values back into statement results as a node.
///
/// Twin of Python `merge_into_results` (Rust
/// `adjustments::merge_into_results`). Each period's `final_value`
/// becomes node `outputNodeId` with the shared unit (and currency for monetary
/// values), replacing any existing node of that name across all periods. The
/// attached check report is cleared because it describes the values before the
/// merge. Rust updates its argument in place; here the merged copy is returned.
/// @param results - Evaluated `StatementResult` to merge into (object or JSON); left unchanged.
/// @param normalization_results - `NormalizationResult` array from `normalize`, all with the same unit and currency (object or JSON); an empty array returns an unchanged copy.
/// @param output_node_id - Node to create or replace, e.g. `"adjusted_ebitda"`.
/// @returns The merged `StatementResult`.
///
/// # Errors
///
/// Throws with kind `validation` if an input is malformed, and the kind of the
/// Rust error if the results mix units or currencies, repeat a period or hold a
/// non-finite value.
#[wasm_bindgen(js_name = mergeIntoResults)]
pub fn merge_into_results(
    results: JsValue,
    normalization_results: JsValue,
    output_node_id: JsValue,
) -> Result<JsValue, JsValue> {
    let mut results: StatementResult = from_js_json(&results, "results")?;
    let normalization_results: Vec<NormalizationResult> =
        from_js_json(&normalization_results, "normalizationResults")?;
    finstack_quant_statements::adjustments::merge_into_results(
        &mut results,
        &normalization_results,
        &js_string(&output_node_id, "outputNodeId")?,
    )
    .map_err(to_js_err)?;
    to_js_value(&results)
}

/// Serde `type` tags of every built-in check a suite spec accepts.
///
/// Free-function twin of Python `CheckSuiteSpec.builtin_check_names` (Rust
/// `BuiltinCheckSpec::names`).
/// @returns Tags accepted by `builtin_checks[].type`, in declaration order.
///
/// # Errors
///
/// Throws only if the list cannot be converted to a JavaScript array.
#[wasm_bindgen(js_name = checkSuiteSpecBuiltinCheckNames)]
pub fn check_suite_spec_builtin_check_names() -> Result<JsValue, JsValue> {
    to_js_value(&BuiltinCheckSpec::names())
}

/// Validate the tolerances and materiality thresholds of a check configuration.
///
/// Free-function twin of Python `CheckConfig.validate` (Rust
/// `CheckConfig::validate`). Returns `undefined` when valid.
/// @param config - `CheckConfig` object or JSON: `default_tolerance`, `default_relative_tolerance`, `materiality_threshold` and `min_severity`; omitted fields take the Rust defaults.
///
/// # Errors
///
/// Throws a `TypeError` when `config` is not an object or JSON string, and a
/// `validation` error when it does not match the `CheckConfig` contract or a
/// tolerance or materiality threshold is negative or non-finite.
#[wasm_bindgen(js_name = checkConfigValidate)]
pub fn check_config_validate(config: JsValue) -> Result<(), JsValue> {
    from_js_json::<CheckConfig>(&config, "config")?
        .validate()
        .map_err(to_js_err)
}
