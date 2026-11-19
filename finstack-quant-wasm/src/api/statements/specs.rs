//! Free-function twins of the Python spec-object constructors and methods.
//!
//! WASM specs are plain objects, so each Rust constructor or method on
//! `ForecastSpec`, `Adjustment`, `NormalizationConfig`, `CheckConfig` and `CheckSuiteSpec`
//! is a function that returns (or takes) the plain object.

use crate::utils::input::from_js_json;
use crate::utils::{to_js_err, to_js_value};
use finstack_quant_statements::adjustments::{Adjustment, NormalizationConfig};
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
#[wasm_bindgen(js_name = normalizationConfigAddAdjustment, unchecked_return_type = "generated.statements.NormalizationConfig")]
pub fn normalization_config_add_adjustment(
    #[wasm_bindgen(unchecked_param_type = "generated.statements.NormalizationConfig | string")]
    config: JsValue,
    #[wasm_bindgen(unchecked_param_type = "generated.statements.Adjustment | string")]
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
pub fn validate_normalization_config_json(
    #[wasm_bindgen(unchecked_param_type = "JsonInput")] json: JsValue,
) -> Result<String, JsValue> {
    crate::utils::input::validated_json(&json, NormalizationConfig::validate)
}

/// Build the ordered bridge from a reported node to its adjusted value for every period.
///
/// Twin of Python `normalize` (Rust `adjustments::normalize`). The whole
/// configuration is validated before any period is calculated. Each period
/// starts from the reported target value and applies the adjustments in
/// declaration order: a signed fixed amount (zero for a period not listed)
/// or a signed decimal percentage of a reference node's reported value. Each
/// cap limits its own adjustment's magnitude and keeps its sign. Money is
/// exact and nothing is converted between currencies. Nothing is written to
/// the results.
/// @param results - Evaluated or rolled-up `StatementResult` holding the target node and any reference nodes, with its period plan (object or JSON).
/// @param config - `NormalizationConfig`: the reported `target_node`, the new `output_node` and the ordered adjustments. A fixed amount and an absolute cap are in the target's units: a number for a scalar target, `{amount, currency}` for a monetary one (object or JSON).
/// @returns One `NormalizationResult` per period of the plan, in timeline order: the reported `base_value`, each row (raw amount, cap base and limit, applied amount and `adjusted_value`, the target through that row) and the `final_value`. A period whose target, percentage reference or cap base has no value carries `unavailable` with the reason and no rows.
///
/// # Errors
///
/// Throws with kind `validation` if an input is malformed, the results carry
/// no period plan, the output node is empty or equals the target, an
/// adjustment id repeats, the target or a referenced node is absent or has no
/// recorded units, units or currencies differ, a fixed amount names a period
/// outside the plan, a number is non-finite or a cap negative, or any row's
/// arithmetic cannot be represented.
#[wasm_bindgen(js_name = normalize, unchecked_return_type = "generated.statements.NormalizationResult[]")]
pub fn normalize(
    #[wasm_bindgen(unchecked_param_type = "StatementResult | string")] results: JsValue,
    #[wasm_bindgen(unchecked_param_type = "generated.statements.NormalizationConfig | string")]
    config: JsValue,
) -> Result<JsValue, JsValue> {
    let results: StatementResult = from_js_json(&results, "results")?;
    let config: NormalizationConfig = from_js_json(&config, "config")?;
    to_js_value(
        &finstack_quant_statements::adjustments::normalize(&results, &config).map_err(to_js_err)?,
    )
}

/// Publish the adjusted metric of a normalization configuration as a new node.
///
/// Twin of Python `merge_into_results` (Rust
/// `adjustments::merge_into_results`). The bridge is calculated from
/// `config`, so the published values always agree with the configuration
/// recorded for them in the result's `normalizations`. The new node is
/// `config.output_node`; a period without an adjusted value is published as
/// an unavailable cell. The reported target, every other node and the check
/// report are left as they are. Rust updates its argument in place; here the
/// updated copy is returned.
/// @param results - `StatementResult` of an evaluation run, or a rollup of one (object or JSON); left unchanged.
/// @param config - `NormalizationConfig` to calculate and publish (object or JSON).
/// @returns The `StatementResult` holding the new node, its units and the configuration under `normalizations`.
///
/// # Errors
///
/// Throws with kind `validation` if an input is malformed, the results record
/// no source run, they already hold a node named `config.output_node`, or
/// `normalize` rejects the configuration. Nothing is published on error.
#[wasm_bindgen(js_name = mergeIntoResults, unchecked_return_type = "StatementResult")]
pub fn merge_into_results(
    #[wasm_bindgen(unchecked_param_type = "StatementResult | string")] results: JsValue,
    #[wasm_bindgen(unchecked_param_type = "generated.statements.NormalizationConfig | string")]
    config: JsValue,
) -> Result<JsValue, JsValue> {
    let mut results: StatementResult = from_js_json(&results, "results")?;
    let config: NormalizationConfig = from_js_json(&config, "config")?;
    finstack_quant_statements::adjustments::merge_into_results(&mut results, &config)
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
#[wasm_bindgen(js_name = checkSuiteSpecBuiltinCheckNames, unchecked_return_type = "string[]")]
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
pub fn check_config_validate(
    #[wasm_bindgen(unchecked_param_type = "generated.statements.CheckConfig | string")]
    config: JsValue,
) -> Result<(), JsValue> {
    from_js_json::<CheckConfig>(&config, "config")?
        .validate()
        .map_err(to_js_err)
}
