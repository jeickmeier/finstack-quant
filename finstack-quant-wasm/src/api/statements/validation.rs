//! Financial model loading, editing, and spec-validation WASM entry points.

use crate::utils::input::{from_js_json, validated_json};
use crate::utils::to_js_err;
use finstack_quant_statements::types::FinancialModelSpec;
use wasm_bindgen::prelude::*;

/// Load a model argument (object or JSON) through the one Rust loader.
///
/// # Arguments
///
/// * `value` - The JavaScript model argument.
/// * `label` - The camelCase argument name, used in a type error.
///
/// # Errors
///
/// A decode failure is a `ContractValidationError` with its `report`; a
/// model that is not runnable is the Rust error with its kind.
pub(crate) fn load_model(value: &JsValue, label: &str) -> Result<FinancialModelSpec, JsValue> {
    load_model_text(&crate::utils::input::json_text_exact(value, label)?)
}

/// [`load_model`] for model JSON already held as text.
pub(crate) fn load_model_text(json: &str) -> Result<FinancialModelSpec, JsValue> {
    FinancialModelSpec::from_json(json).map_err(crate::utils::contract_to_js_err)
}

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
/// @returns Canonical financial-model JSON after semantic validation.
#[wasm_bindgen(js_name = validateFinancialModelJson)]
pub fn validate_financial_model_json(
    #[wasm_bindgen(unchecked_param_type = "JsonInput")] json: JsValue,
) -> Result<String, JsValue> {
    let json: &str = &crate::utils::input::json_text_exact(&json, "json")?;
    let model = crate::api::statements::load_model_text(json)?;
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
/// @returns Node identifiers in model-declaration order.
#[wasm_bindgen(js_name = modelNodeIds, unchecked_return_type = "string[]")]
pub fn model_node_ids(
    #[wasm_bindgen(unchecked_param_type = "JsonInput")] json: JsValue,
) -> Result<JsValue, JsValue> {
    let json: &str = &crate::utils::input::json_text_exact(&json, "json")?;
    let model = crate::api::statements::load_model_text(json)?;
    let ids: Vec<&str> = model.nodes.keys().map(|k| k.as_str()).collect();
    crate::utils::to_js_value(&ids)
}

/// Apply named edits to a financial model atomically.
///
/// Twin of Python `FinancialModelSpec.apply_edits` (Rust
/// `FinancialModelSpec::apply_edits`). Edits apply in order to a copy of the
/// model, which must pass the runnable-model validation; the input model is
/// never changed.
///
/// @param model - `FinancialModelSpec` to edit (object or JSON).
/// @param edits - `ModelEdit` objects, each tagged by `edit`: `add_node` and `replace_node` (`node`); `remove_node`, `remove_forecast` and `remove_where` (`node_id`); `set_observation` (`node_id`, `period`, `value`, optional `availability_date`); `remove_observation` (`node_id`, `period`); `set_formula` (`node_id`, `formula`); `set_forecast` (`node_id`, `forecast`; the node becomes mixed); `set_where` (`node_id`, `predicate`). An array or its JSON text.
/// @returns The edited `FinancialModelSpec` plain object.
/// @throws Error with kind `validation` if the model or an edit is malformed, an edit adds an id that is already defined or names a node or observation that does not exist, or the edited model fails validation.
#[wasm_bindgen(js_name = applyModelEdits, unchecked_return_type = "generated.statements.FinancialModelSpec")]
pub fn apply_model_edits(
    #[wasm_bindgen(unchecked_param_type = "JsonInput")] model: JsValue,
    #[wasm_bindgen(unchecked_param_type = "generated.statements.ModelEdit[] | string")]
    edits: JsValue,
) -> Result<JsValue, JsValue> {
    let mut model = crate::api::statements::load_model(&model, "model")?;
    let edits: Vec<finstack_quant_statements::types::ModelEdit> = from_js_json(&edits, "edits")?;
    model.apply_edits(&edits).map_err(to_js_err)?;
    crate::utils::to_js_value(&model)
}

/// Validate a `CheckSuiteSpec` JSON string.
///
/// Deserializes the spec, validates default configuration thresholds, and
/// returns canonical JSON. Check-specific tolerances and formula references
/// are validated when the suite runs against a model.
///
/// # Arguments
///
/// * `json` - Serialized `CheckSuiteSpec` with a suite name, optional checks,
///   and optional configuration. Default tolerances and materiality thresholds
///   must be finite and nonnegative; omitted configuration uses Rust defaults.
///
/// # Errors
///
/// Rejects malformed or schema-incompatible `json`, negative or non-finite
/// default configuration thresholds, or failure to serialize the decoded check-suite specification.
/// @param json - Serialized `CheckSuiteSpec` with a suite name, optional checks, and optional configuration. Default tolerances and materiality thresholds must be finite and nonnegative; omitted configuration uses Rust defaults.
/// @returns Canonical check-suite JSON after schema validation.
#[wasm_bindgen(js_name = validateCheckSuiteSpecJson)]
pub fn validate_check_suite_spec_json(
    #[wasm_bindgen(unchecked_param_type = "JsonInput")] json: JsValue,
) -> Result<String, JsValue> {
    validated_json(
        &json,
        |spec: &finstack_quant_statements::checks::CheckSuiteSpec| spec.config.validate(),
    )
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
/// @returns Canonical capital-structure JSON after schema validation.
#[wasm_bindgen(js_name = validateCapitalStructureSpecJson)]
pub fn validate_capital_structure_spec_json(
    #[wasm_bindgen(unchecked_param_type = "JsonInput")] json: JsValue,
) -> Result<String, JsValue> {
    validated_json(
        &json,
        finstack_quant_statements::types::CapitalStructureSpec::validate,
    )
}

/// Validate a `WaterfallSpec` JSON string.
///
/// Deserializes the waterfall and checks its payment order and sizing rules.
/// When `priority_of_payments` is omitted, the order is `fees`, `interest`,
/// `amortization`, and `equity`. Prepayments require an explicit priority
/// before any `equity` entry and a corresponding sizing configuration.
///
/// # Errors
///
/// Rejects malformed or schema-incompatible `json`; duplicate or inconsistent
/// payment priorities; incomplete available-cash priorities; invalid PIK,
/// payment-class, prepayment, or ECF-sweep settings; or failure to serialize
/// the validated waterfall.
/// @param json - Canonical JSON string for a `WaterfallSpec`, including `available_cash_node` and optional `priority_of_payments`, `ecf_sweep`, `pik_toggle`, `payment_classes`, `mandatory_prepayment`, and `voluntary_prepayment` (each `{amount, facilities}`).
/// @returns Canonical waterfall JSON after schema validation.
#[wasm_bindgen(js_name = validateWaterfallSpecJson)]
pub fn validate_waterfall_spec_json(
    #[wasm_bindgen(unchecked_param_type = "JsonInput")] json: JsValue,
) -> Result<String, JsValue> {
    validated_json(
        &json,
        finstack_quant_statements::capital_structure::WaterfallSpec::validate,
    )
}

/// Validate an `EcfSweepSpec` JSON string.
///
/// Deserializes the spec, runs the Rust `EcfSweepSpec::validate`, and returns
/// the canonical JSON. `validateWaterfallSpecJson` checks the enclosing
/// waterfall's `sweep` priority.
///
/// # Errors
///
/// Rejects malformed or schema-incompatible `json`, a `sweep_percentage`
/// outside `[0.0, 1.0]`, an empty cash measure or input expression, an empty
/// or repeated named facility, or failure to serialize the validated
/// ECF-sweep specification.
/// @param json - Canonical JSON string for an `EcfSweepSpec`: `cash_measure` (a formula in the pool currency), `deductions` and `credits` (lists of `{type: "input", expression}`, `{type: "paid_cash_interest"}`, `{type: "paid_fees"}` or `{type: "paid_scheduled_principal"}`), `sweep_percentage` (decimal fraction in `[0, 1]`) and `facilities` (`"all_eligible"` or `{named: [...]}`).
/// @returns Canonical ECF-sweep JSON after schema validation.
#[wasm_bindgen(js_name = validateEcfSweepSpecJson)]
pub fn validate_ecf_sweep_spec_json(
    #[wasm_bindgen(unchecked_param_type = "JsonInput")] json: JsValue,
) -> Result<String, JsValue> {
    validated_json(
        &json,
        finstack_quant_statements::capital_structure::EcfSweepSpec::validate,
    )
}

/// Validate a `PikToggleSpec` JSON string.
///
/// Deserializes the spec, runs the Rust `PikToggleSpec::validate`, and returns
/// the canonical JSON.
///
/// # Errors
///
/// Rejects malformed or schema-incompatible `json`, a non-finite `threshold`,
/// a missing or empty `elections` list, a loan listed twice, a `pik_fraction`
/// outside `(0, 1]`, or failure to serialize the validated PIK-toggle
/// specification.
/// @param json - Canonical JSON string for a `PikToggleSpec`: `liquidity_metric` (a node or formula), `threshold` (finite, in the metric's units; a metric below it requests PIK) and `elections`, one `{instrument_id, pik_fraction, min_coupon_periods}` per term loan authorized to elect PIK, where `pik_fraction` is an exact decimal string in `(0, 1]` and `min_coupon_periods` counts the loan's coupon periods.
/// @returns Canonical PIK-toggle JSON after schema validation.
#[wasm_bindgen(js_name = validatePikToggleSpecJson)]
pub fn validate_pik_toggle_spec_json(
    #[wasm_bindgen(unchecked_param_type = "JsonInput")] json: JsValue,
) -> Result<String, JsValue> {
    validated_json(
        &json,
        finstack_quant_statements::capital_structure::PikToggleSpec::validate,
    )
}
