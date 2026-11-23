//! Scenario specification parsing, composition, validation, and template builders.

use crate::utils::input::{from_js_json, js_string, json_text, opt_json_text};
use crate::utils::to_js_err;
use wasm_bindgen::prelude::*;

/// Process-wide builtin template registry, parsed once by the scenarios crate.
fn builtin_registry() -> Result<&'static finstack_quant_scenarios::TemplateRegistry, JsValue> {
    finstack_quant_scenarios::TemplateRegistry::embedded_builtins().map_err(to_js_err)
}

/// Parse the optional `FinstackConfig` argument; omitted means the Rust default.
/// Parse and validate a `ScenarioSpec` argument (object or JSON text).
pub(super) fn scenario_spec(
    value: &JsValue,
    name: &str,
) -> Result<finstack_quant_scenarios::ScenarioSpec, JsValue> {
    finstack_quant_scenarios::ScenarioSpec::from_json(&json_text(value, name)?).map_err(to_js_err)
}

pub(super) fn parse_config(
    config_json: Option<&JsValue>,
) -> Result<finstack_quant_core::config::FinstackConfig, JsValue> {
    opt_json_text(config_json, "configJson")?
        .as_deref()
        .map(serde_json::from_str)
        .transpose()
        .map_err(to_js_err)
        .map(Option::unwrap_or_default)
}

/// Parse and validate a scenario specification from JSON.
///
/// Returns the validated scenario as a plain JavaScript object.
///
/// # Errors
///
/// Rejects malformed or schema-incompatible `json_str`, a blank scenario ID,
/// multiple time-roll operations, invalid operation identifiers or numeric
/// fields, variant-specific operation violations, or serialization failure.
/// @param jsonStr - Scenario specification JSON string to deserialize and validate; the result is returned as a plain object.
/// @returns Validated structured scenario specification.
#[wasm_bindgen(js_name = parseScenarioSpec, unchecked_return_type = "ScenarioSpec")]
pub fn parse_scenario_spec(
    #[wasm_bindgen(js_name = "jsonStr", unchecked_param_type = "JsonInput")] json_str: JsValue,
) -> Result<JsValue, JsValue> {
    crate::utils::to_js_value(&scenario_spec(&json_str, "jsonStr")?)
}

/// Compose multiple scenario specs (JSON array) into a single scenario.
///
/// Specs are merged in priority order (lower number runs first; equal
/// priorities keep input order). The composed spec takes its `id` and `name`
/// from the inputs joined with `+` / `" + "`, its `description` from the
/// present input descriptions joined with `" + "` (`null` when no input has
/// one), and the lowest input `priority` (`0` for an empty array), all in
/// execution order.
///
/// # Errors
///
/// Rejects malformed structured specs, any input spec that fails
/// `ScenarioSpec` validation (blank ID, non-finite numbers, invalid
/// identifiers, tenors or operations; the message names the scenario), input
/// specs with mixed `resolution_mode` values, composition that contains more
/// than one time-roll operation, or failure to convert the composed
/// specification.
/// @param specs - ScenarioSpec objects to validate and compose in priority order.
/// @returns Structured composed scenario specification.
#[wasm_bindgen(js_name = composeScenarios, unchecked_return_type = "ScenarioSpec")]
pub fn compose_scenarios(
    #[wasm_bindgen(unchecked_param_type = "ScenarioSpec[]")] specs: JsValue,
) -> Result<JsValue, JsValue> {
    let specs: Vec<finstack_quant_scenarios::ScenarioSpec> = from_js_json(&specs, "specs")?;
    let composed = finstack_quant_scenarios::ScenarioSpec::compose(specs).map_err(to_js_err)?;
    crate::utils::to_js_value(&composed)
}

/// Check scenario structure and return diagnostics without applying market effects.
/// @param scenario - Bare typed scenario object or JSON text in the canonical ScenarioSpec shape.
/// @returns Structural errors, advisory diagnostics, instrument requirements and operation count.
/// @throws FinstackWasmError if the input cannot be decoded into the canonical shape or the report cannot be serialized.
#[wasm_bindgen(js_name = validateScenarioSpec, unchecked_return_type = "generated.scenarios.ScenarioValidationReport")]
pub fn validate_scenario_spec(
    #[wasm_bindgen(unchecked_param_type = "generated.scenarios.ScenarioSpec | JsonInput")] scenario: JsValue,
) -> Result<JsValue, JsValue> {
    let spec = from_js_json(&scenario, "scenario")?;
    crate::utils::to_js_value(&finstack_quant_scenarios::validate_scenario_spec(&spec))
}

/// List all built-in template identifiers.
///
/// Returns a JavaScript array of built-in template ID strings in registry order.
///
/// # Errors
///
/// Rejects if the embedded template registry cannot be parsed and validated,
/// or if its template identifiers cannot be serialized to JavaScript.
/// @returns Built-in scenario template identifiers.
#[wasm_bindgen(js_name = listBuiltinTemplates, unchecked_return_type = "string[]")]
pub fn list_builtin_templates() -> Result<JsValue, JsValue> {
    let registry = builtin_registry()?;
    let ids: Vec<String> = registry.list().iter().map(|m| m.id.clone()).collect();
    crate::utils::to_js_value(&ids)
}

/// Get typed metadata for all built-in templates as plain JavaScript objects.
///
/// # Errors
///
/// Rejects if the embedded template registry cannot be parsed and validated,
/// or if its metadata cannot be serialized to JSON.
/// @returns Metadata objects in deterministic registry order.
#[wasm_bindgen(js_name = listBuiltinTemplateMetadata, unchecked_return_type = "TemplateMetadata[]")]
pub fn list_builtin_template_metadata() -> Result<JsValue, JsValue> {
    let metadata = builtin_registry()?.list();
    crate::utils::to_js_value(&metadata)
}

/// Build a scenario spec from a built-in template.
///
/// Returns a structured `ScenarioSpec` object.
///
/// # Errors
///
/// Rejects a failure to load the embedded registry, an unknown `template_id`,
/// a template whose resolved scenario fails validation, or failure to serialize
/// the scenario.
/// @param templateId - Identifier of a built-in scenario template in the embedded registry.
/// @returns Validated scenario specification from the built-in template.
#[wasm_bindgen(js_name = buildFromTemplate, unchecked_return_type = "ScenarioSpec")]
pub fn build_from_template(
    #[wasm_bindgen(js_name = "templateId", unchecked_param_type = "string")] template_id: JsValue,
) -> Result<JsValue, JsValue> {
    let template_id: &str = &js_string(&template_id, "templateId")?;
    let spec = builtin_registry()?.build(template_id).map_err(to_js_err)?;
    crate::utils::to_js_value(&spec)
}

/// List component IDs for a built-in composite template.
///
/// Returns a JS array of component ID strings.
///
/// # Errors
///
/// Rejects a failure to load the embedded registry, an unknown `template_id`,
/// or component identifiers that cannot be serialized to JavaScript.
/// @param templateId - Identifier of a built-in scenario template in the embedded registry.
/// @returns Component identifiers of the selected composite template.
#[wasm_bindgen(js_name = listTemplateComponents, unchecked_return_type = "string[]")]
pub fn list_template_components(
    #[wasm_bindgen(js_name = "templateId", unchecked_param_type = "string")] template_id: JsValue,
) -> Result<JsValue, JsValue> {
    let template_id: &str = &js_string(&template_id, "templateId")?;
    let ids: Vec<String> = builtin_registry()?
        .component_ids(template_id)
        .map_err(to_js_err)?
        .into_iter()
        .map(str::to_string)
        .collect();
    crate::utils::to_js_value(&ids)
}

/// Build a specific component from a built-in composite template.
///
/// # Errors
///
/// Rejects a failure to load the embedded registry, an unknown `template_id`
/// or `component_id`, a component scenario that fails validation, or failure to
/// serialize the scenario.
/// @param templateId - Identifier of a built-in scenario template in the embedded registry.
/// @param componentId - Identifier of a component within the selected composite template.
/// @returns Validated structured scenario specification for the selected component.
#[wasm_bindgen(js_name = buildTemplateComponent, unchecked_return_type = "ScenarioSpec")]
pub fn build_template_component(
    #[wasm_bindgen(js_name = "templateId", unchecked_param_type = "string")] template_id: JsValue,
    #[wasm_bindgen(js_name = "componentId", unchecked_param_type = "string")] component_id: JsValue,
) -> Result<JsValue, JsValue> {
    let template_id: &str = &js_string(&template_id, "templateId")?;
    let component_id: &str = &js_string(&component_id, "componentId")?;
    let spec = builtin_registry()?
        .build_component(template_id, component_id)
        .map_err(to_js_err)?;
    crate::utils::to_js_value(&spec)
}
