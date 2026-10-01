//! WASM bindings for the `finstack-quant-scenarios` crate.
//!
//! Exposes scenario specification parsing, validation, composition,
//! and built-in template access via structured JavaScript values.

use crate::utils::input::{
    from_js_json, js_opt_int, js_opt_string, js_string, json_text, opt_json_text,
};
use crate::utils::{parse_iso_date, to_js_err};
use wasm_bindgen::prelude::*;

pub mod kinds;
pub mod operation_spec;
pub mod results;

/// Process-wide builtin template registry, parsed once by the scenarios crate.
fn builtin_registry() -> Result<&'static finstack_quant_scenarios::TemplateRegistry, JsValue> {
    finstack_quant_scenarios::TemplateRegistry::embedded_builtins().map_err(to_js_err)
}

fn apply_with_context(
    spec: &finstack_quant_scenarios::ScenarioSpec,
    market: &mut finstack_quant_core::market_data::context::MarketContext,
    model: Option<&mut finstack_quant_statements::FinancialModelSpec>,
    as_of: time::Date,
    instruments: Option<&mut Vec<Box<dyn finstack_quant_valuations::instruments::Instrument>>>,
    config: finstack_quant_core::config::FinstackConfig,
) -> Result<finstack_quant_scenarios::engine::ApplicationReport, JsValue> {
    let mut ctx = finstack_quant_scenarios::ExecutionContext {
        market,
        model,
        instruments,
        rate_bindings: None,
        calendar: None,
        as_of,
    };
    finstack_quant_scenarios::ScenarioEngine::with_config(config)
        .apply(spec, &mut ctx)
        .map_err(to_js_err)
}

/// Parse the optional `FinstackConfig` argument; omitted means the Rust default.
fn parse_config(
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
/// @param json_str - Scenario specification JSON string to deserialize and validate; the result is returned as a plain object.
#[wasm_bindgen(js_name = parseScenarioSpec)]
pub fn parse_scenario_spec(json_str: JsValue) -> Result<JsValue, JsValue> {
    let json_str: &str = &json_text(&json_str, "jsonStr")?;
    let spec = finstack_quant_scenarios::ScenarioSpec::from_json(json_str).map_err(to_js_err)?;
    crate::utils::to_js_value(&spec)
}

/// Compose multiple scenario specs (JSON array) into a single scenario.
///
/// Specs are merged in priority order (lower number runs first).
///
/// # Errors
///
/// Rejects malformed structured specs, any input spec that fails
/// `ScenarioSpec` validation (blank ID, non-finite numbers, invalid
/// identifiers, tenors or operations; the message names the scenario), input
/// specs with mixed `hazard_bump_mode` values, composition that contains more
/// than one time-roll operation, or failure to convert the composed
/// specification.
/// @param specs - ScenarioSpec objects to validate and compose in priority order.
#[wasm_bindgen(js_name = composeScenarios)]
pub fn compose_scenarios(specs: JsValue) -> Result<JsValue, JsValue> {
    let specs: Vec<finstack_quant_scenarios::ScenarioSpec> = from_js_json(&specs, "specs")?;
    let composed = finstack_quant_scenarios::ScenarioSpec::compose(specs).map_err(to_js_err)?;
    crate::utils::to_js_value(&composed)
}

/// Validate a scenario specification JSON without executing it.
///
/// Returns `undefined` when the spec is valid, throws on error. This mirrors
/// the Python `validate_scenario_spec` API, which returns `None` — an invalid
/// spec raises rather than returning a falsy value, so
/// `if (validateScenarioSpec(s))` is not a validity check.
///
/// # Errors
///
/// Rejects malformed or schema-incompatible `json_str`, a blank scenario ID,
/// multiple time-roll operations, invalid operation identifiers or numeric
/// fields, or variant-specific operation violations.
/// @param json_str - Scenario specification JSON string to deserialize and validate.
#[wasm_bindgen(js_name = validateScenarioSpec)]
pub fn validate_scenario_spec(json_str: JsValue) -> Result<(), JsValue> {
    let json_str: &str = &json_text(&json_str, "jsonStr")?;
    finstack_quant_scenarios::ScenarioSpec::from_json(json_str).map_err(to_js_err)?;
    Ok(())
}

/// List all built-in template identifiers.
///
/// Returns a JavaScript array of built-in template ID strings in registry order.
///
/// # Errors
///
/// Rejects if the embedded template registry cannot be parsed and validated,
/// or if its template identifiers cannot be serialized to JavaScript.
#[wasm_bindgen(js_name = listBuiltinTemplates)]
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
#[wasm_bindgen(js_name = listBuiltinTemplateMetadata)]
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
/// @param template_id - Identifier of a built-in scenario template in the embedded registry.
#[wasm_bindgen(js_name = buildFromTemplate)]
pub fn build_from_template(template_id: JsValue) -> Result<JsValue, JsValue> {
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
/// @param template_id - Identifier of a built-in scenario template in the embedded registry.
#[wasm_bindgen(js_name = listTemplateComponents)]
pub fn list_template_components(template_id: JsValue) -> Result<JsValue, JsValue> {
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
/// @param template_id - Identifier of a built-in scenario template in the embedded registry.
/// @param component_id - Identifier of a component within the selected composite template.
#[wasm_bindgen(js_name = buildTemplateComponent)]
pub fn build_template_component(
    template_id: JsValue,
    component_id: JsValue,
) -> Result<JsValue, JsValue> {
    let template_id: &str = &js_string(&template_id, "templateId")?;
    let component_id: &str = &js_string(&component_id, "componentId")?;
    let spec = builtin_registry()?
        .build_component(template_id, component_id)
        .map_err(to_js_err)?;
    crate::utils::to_js_value(&spec)
}

/// Build a scenario spec from fields.
///
/// # Errors
///
/// Rejects malformed or schema-incompatible `operations`, an unsupported
/// `resolution_mode` or `hazard_bump_mode`, a blank scenario ID, multiple
/// time-roll operations, invalid operation identifiers or numeric fields,
/// variant-specific operation violations, or failure to serialize the scenario.
/// @param id - Scenario identifier stored on the constructed spec.
/// @param operations - Structured scenario operation specifications in execution order.
/// @param name - Optional human-readable scenario name.
/// @param description - Optional human-readable description of the scenario purpose.
/// @param priority - Optional execution priority; lower values run earlier
///   during composition. Omit for the Rust serde default (`0`), matching the
///   Python `priority=0` keyword default.
/// @param resolution_mode - Optional hierarchy conflict policy:
///   `"most_specific_wins"` (default) or `"cumulative"`.
/// @param hazard_bump_mode - Optional ParCDS delivery:
///   `"solve_to_par"` (default) rebootstraps par quotes; `"first_order_shift"` applies
///   delta hazard = delta spread / (1 - recovery) and reports an approximation warning.
#[wasm_bindgen(js_name = buildScenarioSpec)]
pub fn build_scenario_spec(
    id: JsValue,
    operations: JsValue,
    name: Option<JsValue>,
    description: Option<JsValue>,
    priority: Option<JsValue>,
    resolution_mode: Option<JsValue>,
    hazard_bump_mode: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let id: &str = &js_string(&id, "id")?;
    let name = js_opt_string(name.as_ref(), "name")?;
    let description = js_opt_string(description.as_ref(), "description")?;
    let priority: Option<i32> = js_opt_int(priority.as_ref(), "priority")?;
    let resolution_mode = js_opt_string(resolution_mode.as_ref(), "resolutionMode")?;
    let hazard_bump_mode = js_opt_string(hazard_bump_mode.as_ref(), "hazardBumpMode")?;
    let operations: Vec<finstack_quant_scenarios::OperationSpec> =
        from_js_json(&operations, "operations")?;
    let resolution_mode = resolution_mode
        .as_deref()
        .map(finstack_quant_core::wire::serde_parse)
        .transpose()
        .map_err(to_js_err)?
        .unwrap_or_default();
    let hazard_bump_mode = hazard_bump_mode
        .as_deref()
        .map(finstack_quant_core::wire::serde_parse)
        .transpose()
        .map_err(to_js_err)?
        .unwrap_or_default();
    let spec = finstack_quant_scenarios::ScenarioSpec {
        id: id.to_string(),
        name,
        description,
        operations,
        priority: priority.unwrap_or_default(),
        resolution_mode,
        hazard_bump_mode,
    };
    spec.validate().map_err(to_js_err)?;
    crate::utils::to_js_value(&spec)
}

fn extract_instruments(
    json: Option<String>,
) -> Result<Option<Vec<Box<dyn finstack_quant_valuations::instruments::Instrument>>>, JsValue> {
    json.as_deref()
        .map(finstack_quant_valuations::pricer::json::parse_boxed_instruments_from_json)
        .transpose()
        .map_err(to_js_err)
}

/// Apply a scenario to a market context and financial model.
///
/// Returns a JavaScript object with `market` and `model` (the mutated
/// contexts as objects, not JSON strings), `operations_applied`,
/// `user_operations`, `expanded_operations`, `changes` (a
/// `ScenarioChangeManifest`), `warnings`, `meta` (a `ResultsMeta` audit stamp
/// carrying the numeric mode, rounding context, and FX policy; omitted when
/// absent), and `time_roll` (a `RollForwardReport`, only present when the
/// scenario contained a `time_roll_forward` operation).
///
/// Optional instrument envelopes are copied and returned in `instruments`, in
/// input order. The Rust engine rejects instrument-scoped operations without
/// an inventory. No holiday calendar is supplied; business-day rolls adjust
/// without holiday information.
///
/// # Errors
///
/// Rejects a malformed or invalid scenario (checked before the market is
/// parsed), malformed market, model, instrument, or configuration JSON,
/// instrument-scoped operations without `instruments_json`, a model that fails
/// semantic validation (the same `FinancialModelSpec::from_json` check the
/// statements exports and Python `apply_scenario` apply), an invalid ISO
/// `as_of` date, an invalid scenario operation, missing market objects or
/// hierarchy context, statement-model execution failures, failure to encode
/// the mutated contexts, or failure to serialize the application envelope to
/// JavaScript.
/// @param scenario_json - JSON-serialized ScenarioSpec to validate and apply.
/// @param market_json - Canonical market-context JSON supplying curves, quotes, and FX data.
/// @param model_json - JSON-serialized FinancialModelSpec that scenario operations may mutate.
/// @param as_of - ISO-8601 valuation date used to resolve date-dependent market data.
/// @param instruments_json - Optional JSON array of canonical instrument envelopes; required for instrument shocks and returned as shocked copies in input order.
/// @param config_json - Optional FinstackConfig JSON; its rounding policy is stamped into `meta`. Omit for the library default.
#[wasm_bindgen(js_name = applyScenario)]
pub fn apply_scenario(
    scenario_json: JsValue,
    market_json: JsValue,
    model_json: JsValue,
    as_of: JsValue,
    instruments_json: Option<JsValue>,
    config_json: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let scenario_json: &str = &json_text(&scenario_json, "scenarioJson")?;
    let market_json: &str = &json_text(&market_json, "marketJson")?;
    let model_json: &str = &json_text(&model_json, "modelJson")?;
    let as_of: &str = &js_string(&as_of, "asOf")?;
    let instruments_json = opt_json_text(instruments_json.as_ref(), "instrumentsJson")?;
    let config = parse_config(config_json.as_ref())?;
    let spec =
        finstack_quant_scenarios::ScenarioSpec::from_json(scenario_json).map_err(to_js_err)?;
    let mut market: finstack_quant_core::market_data::context::MarketContext =
        serde_json::from_str(market_json).map_err(to_js_err)?;
    let mut model =
        finstack_quant_statements::FinancialModelSpec::from_json(model_json).map_err(to_js_err)?;
    let date = parse_iso_date(as_of)?;
    let mut instruments = extract_instruments(instruments_json)?;
    let report = apply_with_context(
        &spec,
        &mut market,
        Some(&mut model),
        date,
        instruments.as_mut(),
        config,
    )?;
    let out = finstack_quant_scenarios::ApplicationEnvelope::from_contexts(
        report,
        &market,
        Some(&model),
        instruments.as_deref(),
    )
    .map_err(to_js_err)?;
    crate::utils::to_js_value(&out)
}

/// Apply a scenario to a market context only (no model mutations).
///
/// Returns the same envelope shape as [`apply_scenario`] minus `model`;
/// the same inventory, configuration and calendar rules apply.
///
/// # Errors
///
/// Rejects a malformed or invalid scenario (checked before the market is
/// parsed), malformed market, instrument, or configuration JSON,
/// instrument-scoped operations without `instruments_json`, an invalid ISO `as_of` date, an
/// invalid scenario operation, missing market objects or hierarchy context,
/// failure to encode the mutated market, or failure to serialize the
/// application envelope to JavaScript.
/// @param scenario_json - JSON-serialized ScenarioSpec to validate and apply.
/// @param market_json - Canonical market-context JSON supplying curves, quotes, and FX data.
/// @param as_of - ISO-8601 valuation date used to resolve date-dependent market data.
/// @param instruments_json - Optional JSON array of canonical instrument envelopes; required for instrument shocks and returned as shocked copies in input order.
/// @param config_json - Optional FinstackConfig JSON; its rounding policy is stamped into `meta`. Omit for the library default.
#[wasm_bindgen(js_name = applyScenarioToMarket)]
pub fn apply_scenario_to_market(
    scenario_json: JsValue,
    market_json: JsValue,
    as_of: JsValue,
    instruments_json: Option<JsValue>,
    config_json: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let scenario_json: &str = &json_text(&scenario_json, "scenarioJson")?;
    let market_json: &str = &json_text(&market_json, "marketJson")?;
    let as_of: &str = &js_string(&as_of, "asOf")?;
    let instruments_json = opt_json_text(instruments_json.as_ref(), "instrumentsJson")?;
    let config = parse_config(config_json.as_ref())?;
    let spec =
        finstack_quant_scenarios::ScenarioSpec::from_json(scenario_json).map_err(to_js_err)?;
    let mut market: finstack_quant_core::market_data::context::MarketContext =
        serde_json::from_str(market_json).map_err(to_js_err)?;
    let date = parse_iso_date(as_of)?;
    let mut instruments = extract_instruments(instruments_json)?;
    let report = apply_with_context(&spec, &mut market, None, date, instruments.as_mut(), config)?;
    let out = finstack_quant_scenarios::ApplicationEnvelope::from_contexts(
        report,
        &market,
        None,
        instruments.as_deref(),
    )
    .map_err(to_js_err)?;
    crate::utils::to_js_value(&out)
}

/// Compute horizon total return under a scenario.
///
/// Applies a scenario specification to project an instrument forward, then
/// decomposes the resulting P&L using factor-based attribution.
///
/// # Arguments
///
/// * `instrument_json` - Canonical `finstack_quant.instrument/1` envelope.
/// * `market_json` - JSON-serialized `MarketContext`.
/// * `as_of` - Valuation date (ISO 8601).
/// * `scenario_json` - JSON-serialized `ScenarioSpec`.
/// * `method` - Attribution method: "parallel", "waterfall", "metrics_based",
///   "taylor". Omit for the Rust default (`AttributionMethod::default()`,
///   currently "parallel").
///
/// # Returns
///
/// The serde `HorizonResult` (`attribution`, `initial_value` and
/// `terminal_value` as exact-decimal Money objects, `horizon_days`, null
/// without a time roll, and `scenario_report`) plus `summary`: the Rust-computed
/// `total_return`, `annualized_return`, `currency` and `factor_contributions`
/// that Python exposes as `HorizonResult` accessors. Undefined returns
/// (currency mismatch, non-positive initial value) are null here and NaN in
/// Python.
///
/// # Errors
///
/// Rejects a malformed or invalid scenario; malformed instrument, market, or
/// configuration JSON; an
/// invalid ISO `as_of` date; an unsupported attribution `method`; an unknown
/// `calendar_id`; invalid, unsupported, or unresolved scenario operations;
/// missing market data; pricing or attribution failures; or failure to
/// serialize the horizon result to JavaScript.
/// @param config_json - Optional FinstackConfig JSON for horizon analysis; omit to use defaults.
/// @param calendar_id - Optional holiday calendar (e.g. "nyse", "target") used to
///   business-day adjust `time_roll_forward` targets under `business_days` mode.
///   Omit for a weekends-only calendar; unknown identifiers throw.
#[wasm_bindgen(js_name = computeHorizonReturn)]
pub fn compute_horizon_return(
    instrument_json: JsValue,
    market_json: JsValue,
    as_of: JsValue,
    scenario_json: JsValue,
    method: Option<JsValue>,
    config_json: Option<JsValue>,
    calendar_id: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let instrument_json: &str = &json_text(&instrument_json, "instrumentJson")?;
    let market_json: &str = &json_text(&market_json, "marketJson")?;
    let as_of: &str = &js_string(&as_of, "asOf")?;
    let scenario_json: &str = &json_text(&scenario_json, "scenarioJson")?;
    let method = js_opt_string(method.as_ref(), "method")?;
    let config = parse_config(config_json.as_ref())?;
    let calendar_id = js_opt_string(calendar_id.as_ref(), "calendarId")?;
    use std::sync::Arc;

    let boxed = finstack_quant_valuations::pricer::json::parse_boxed_instrument_from_json(
        instrument_json,
        None,
    )
    .map_err(to_js_err)?;
    let instrument: Arc<dyn finstack_quant_valuations::instruments::Instrument> =
        Arc::from(boxed.into_boxed());

    let market: finstack_quant_core::market_data::context::MarketContext =
        serde_json::from_str(market_json).map_err(to_js_err)?;

    let date = parse_iso_date(as_of)?;

    let scenario =
        finstack_quant_scenarios::ScenarioSpec::from_json(scenario_json).map_err(to_js_err)?;

    let attribution_method = method
        .as_deref()
        .map(finstack_quant_scenarios::horizon::attribution_method_from_str)
        .transpose()
        .map_err(to_js_err)?
        .unwrap_or_default();

    let mut analyzer =
        finstack_quant_scenarios::horizon::HorizonAnalysis::new(attribution_method, config);
    if let Some(id) = calendar_id.as_deref() {
        analyzer = analyzer.with_calendar_id(id);
    }
    let result = analyzer
        .compute(&instrument, &market, date, &scenario)
        .map_err(to_js_err)?;

    crate::utils::to_js_value(&result.report())
}

#[cfg(test)]
mod tests {
    use finstack_quant_core::market_data::hierarchy::ResolutionMode;
    use finstack_quant_scenarios::{HazardBumpMode, OperationSpec, ScenarioSpec, TimeRollMode};

    fn empty_spec(id: &str, priority: i32) -> ScenarioSpec {
        ScenarioSpec {
            id: id.into(),
            name: None,
            description: None,
            operations: Vec::new(),
            priority,
            resolution_mode: ResolutionMode::default(),
            hazard_bump_mode: Default::default(),
        }
    }

    #[test]
    fn parse_and_compose_helpers_return_typed_specs() {
        let json = serde_json::to_string(&empty_spec("parsed", 0)).expect("serialize");
        let parsed = finstack_quant_scenarios::ScenarioSpec::from_json(&json).expect("parse");
        assert_eq!(parsed.id, "parsed");

        let composed = finstack_quant_scenarios::ScenarioSpec::compose(vec![
            empty_spec("a", 0),
            empty_spec("b", 1),
        ])
        .expect("compose");
        assert!(!composed.id.is_empty());
    }

    #[test]
    fn compose_helper_rejects_duplicate_time_rolls() {
        let operations = |period: &str| {
            vec![OperationSpec::TimeRollForward {
                period: period.into(),
                apply_shocks: true,
                roll_mode: TimeRollMode::BusinessDays,
            }]
        };
        let mut first = empty_spec("roll_1m", 0);
        first.operations = operations("1M");
        first.resolution_mode = ResolutionMode::Cumulative;
        let mut second = empty_spec("roll_3m", 1);
        second.operations = operations("3M");
        second.resolution_mode = ResolutionMode::Cumulative;

        let error = finstack_quant_scenarios::ScenarioSpec::compose(vec![first, second])
            .expect_err("duplicate time rolls should be rejected");
        let error = error.to_string();
        assert!(
            error.contains("TimeRollForward"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn compose_helper_rejects_mixed_hazard_bump_modes() {
        let mut first_order = empty_spec("first-order", 0);
        first_order.hazard_bump_mode = HazardBumpMode::FirstOrderShift;
        let solve_to_par = empty_spec("solve-to-par", 1);

        let error =
            finstack_quant_scenarios::ScenarioSpec::compose(vec![first_order, solve_to_par])
                .expect_err("mixed hazard bump modes should be rejected")
                .to_string();
        assert!(
            error.contains("first-order")
                && error.contains("first_order_shift")
                && error.contains("solve-to-par")
                && error.contains("solve_to_par"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn builtin_registry_builds_typed_templates_and_components() {
        let registry =
            finstack_quant_scenarios::TemplateRegistry::with_embedded_builtins().expect("registry");
        assert!(!registry.list().is_empty());
        for metadata in registry.list() {
            let built = registry.build(&metadata.id).expect("template");
            assert_eq!(built.id, metadata.id);
            for component_id in registry.component_ids(&metadata.id).expect("components") {
                let component = registry
                    .build_component(&metadata.id, component_id)
                    .expect("component");
                assert_eq!(component.id, component_id);
            }
        }
    }
}
