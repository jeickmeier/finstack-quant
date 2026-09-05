//! WASM bindings for the `finstack-quant-scenarios` crate.
//!
//! Exposes scenario specification parsing, validation, composition,
//! and built-in template access via structured JavaScript values.

use crate::utils::{parse_iso_date, to_js_err};
use wasm_bindgen::prelude::*;

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
) -> Result<finstack_quant_scenarios::engine::ApplicationReport, JsValue> {
    if spec.mutates_instruments() && instruments.is_none() {
        return Err(to_js_err(
            "scenario contains instrument-scoped operations but no instruments were supplied",
        ));
    }
    let engine = finstack_quant_scenarios::ScenarioEngine::new().with_recalibration_provider(
        std::sync::Arc::new(
            finstack_quant_calibration::recalibration::CachedRecalibrationProvider::new(),
        ),
    );
    let mut ctx = finstack_quant_scenarios::ExecutionContext {
        market,
        model,
        instruments,
        rate_bindings: None,
        calendar: None,
        as_of,
    };
    engine.apply(spec, &mut ctx).map_err(to_js_err)
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
/// @param json_str - Canonical JSON string to validate and re-serialize.
#[wasm_bindgen(js_name = parseScenarioSpec)]
pub fn parse_scenario_spec(json_str: &str) -> Result<JsValue, JsValue> {
    let spec = parse_scenario_spec_inner(json_str).map_err(to_js_err)?;
    crate::utils::to_js_value(&spec)
}

fn parse_scenario_spec_inner(
    json_str: &str,
) -> Result<finstack_quant_scenarios::ScenarioSpec, String> {
    let spec: finstack_quant_scenarios::ScenarioSpec =
        serde_json::from_str(json_str).map_err(|error| error.to_string())?;
    spec.validate().map_err(|error| error.to_string())?;
    Ok(spec)
}

/// Compose multiple scenario specs (JSON array) into a single scenario.
///
/// Specs are merged in priority order (lower number runs first).
///
/// # Errors
///
/// Rejects malformed structured specs, input specs with mixed
/// `hazard_bump_mode` values, composition that contains more than one time-roll
/// operation, or failure to convert the composed specification.
/// @param specs - Validated ScenarioSpec objects to compose in priority order.
#[wasm_bindgen(js_name = composeScenarios)]
pub fn compose_scenarios(specs: JsValue) -> Result<JsValue, JsValue> {
    let specs: Vec<finstack_quant_scenarios::ScenarioSpec> =
        serde_wasm_bindgen::from_value(specs).map_err(to_js_err)?;
    let composed = compose_scenarios_inner(specs).map_err(to_js_err)?;
    crate::utils::to_js_value(&composed)
}

fn compose_scenarios_inner(
    specs: Vec<finstack_quant_scenarios::ScenarioSpec>,
) -> Result<finstack_quant_scenarios::ScenarioSpec, String> {
    finstack_quant_scenarios::ScenarioSpec::compose(specs).map_err(|error| error.to_string())
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
/// @param json_str - Canonical JSON string to validate and re-serialize.
#[wasm_bindgen(js_name = validateScenarioSpec)]
pub fn validate_scenario_spec(json_str: &str) -> Result<(), JsValue> {
    let spec: finstack_quant_scenarios::ScenarioSpec =
        serde_json::from_str(json_str).map_err(to_js_err)?;

    spec.validate().map_err(to_js_err)?;
    Ok(())
}

/// List all built-in template identifiers.
///
/// Returns a JSON array of template ID strings.
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
pub fn build_from_template(template_id: &str) -> Result<JsValue, JsValue> {
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
pub fn list_template_components(template_id: &str) -> Result<JsValue, JsValue> {
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
pub fn build_template_component(template_id: &str, component_id: &str) -> Result<JsValue, JsValue> {
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
///   `"solve_to_par"` (default) or `"first_order_shift"`.
#[wasm_bindgen(js_name = buildScenarioSpec)]
pub fn build_scenario_spec(
    id: &str,
    operations: JsValue,
    name: Option<String>,
    description: Option<String>,
    priority: Option<i32>,
    resolution_mode: Option<String>,
    hazard_bump_mode: Option<String>,
) -> Result<JsValue, JsValue> {
    let operations: Vec<finstack_quant_scenarios::OperationSpec> =
        serde_wasm_bindgen::from_value(operations).map_err(to_js_err)?;
    let resolution_mode = resolution_mode
        .map(|value| serde_json::from_value(serde_json::Value::String(value)))
        .transpose()
        .map_err(to_js_err)?
        .unwrap_or_default();
    let hazard_bump_mode = hazard_bump_mode
        .map(|value| serde_json::from_value(serde_json::Value::String(value)))
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
    json.map(|json| {
        let envelopes: Vec<finstack_quant_valuations::instruments::InstrumentEnvelope> =
            serde_json::from_str(&json).map_err(to_js_err)?;
        envelopes
            .into_iter()
            .map(|envelope| envelope.into_boxed().map_err(to_js_err))
            .collect()
    })
    .transpose()
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
/// input order. Instrument-scoped operations require an inventory. No holiday
/// calendar is supplied; business-day rolls adjust without holiday information.
///
/// # Errors
///
/// Rejects malformed scenario, market, or model JSON, an invalid ISO `as_of`
/// date, an invalid scenario operation, missing market objects or hierarchy
/// context, statement-model execution failures, failure to encode the mutated
/// contexts, or failure to serialize the application envelope to JavaScript.
/// @param scenario_json - JSON-serialized ScenarioSpec to validate and apply.
/// @param market_json - Canonical market-context JSON supplying curves, quotes, and FX data.
/// @param model_json - JSON-serialized FinancialModelSpec that scenario operations may mutate.
/// @param as_of - ISO-8601 valuation date used to resolve date-dependent market data.
/// @param instruments_json - Optional JSON array of canonical instrument envelopes; required for instrument shocks and returned as shocked copies in input order.
#[wasm_bindgen(js_name = applyScenario)]
pub fn apply_scenario(
    scenario_json: &str,
    market_json: &str,
    model_json: &str,
    as_of: &str,
    instruments_json: Option<String>,
) -> Result<JsValue, JsValue> {
    let spec: finstack_quant_scenarios::ScenarioSpec =
        serde_json::from_str(scenario_json).map_err(to_js_err)?;
    let mut market: finstack_quant_core::market_data::context::MarketContext =
        serde_json::from_str(market_json).map_err(to_js_err)?;
    let mut model: finstack_quant_statements::FinancialModelSpec =
        serde_json::from_str(model_json).map_err(to_js_err)?;
    let date = parse_iso_date(as_of)?;
    let mut instruments = extract_instruments(instruments_json)?;
    let report = apply_with_context(
        &spec,
        &mut market,
        Some(&mut model),
        date,
        instruments.as_mut(),
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
/// the same inventory and calendar rules apply.
///
/// # Errors
///
/// Rejects malformed scenario or market JSON, an invalid ISO `as_of` date, an
/// invalid scenario operation, missing market objects or hierarchy context,
/// failure to encode the mutated market, or failure to serialize the
/// application envelope to JavaScript.
/// @param scenario_json - JSON-serialized ScenarioSpec to validate and apply.
/// @param market_json - Canonical market-context JSON supplying curves, quotes, and FX data.
/// @param as_of - ISO-8601 valuation date used to resolve date-dependent market data.
/// @param instruments_json - Optional JSON array of canonical instrument envelopes; required for instrument shocks and returned as shocked copies in input order.
#[wasm_bindgen(js_name = applyScenarioToMarket)]
pub fn apply_scenario_to_market(
    scenario_json: &str,
    market_json: &str,
    as_of: &str,
    instruments_json: Option<String>,
) -> Result<JsValue, JsValue> {
    let spec: finstack_quant_scenarios::ScenarioSpec =
        serde_json::from_str(scenario_json).map_err(to_js_err)?;
    let mut market: finstack_quant_core::market_data::context::MarketContext =
        serde_json::from_str(market_json).map_err(to_js_err)?;
    let date = parse_iso_date(as_of)?;
    let mut instruments = extract_instruments(instruments_json)?;
    let report = apply_with_context(&spec, &mut market, None, date, instruments.as_mut())?;
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
/// * `method` - Attribution method: "parallel", "waterfall", "metrics_based", "taylor".
///
/// # Returns
///
/// The `HorizonResult` as a structured JavaScript object, matching the Python
/// binding's typed `HorizonResult`.
///
/// # Errors
///
/// Rejects malformed instrument, market, scenario, or configuration JSON; an
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
    instrument_json: &str,
    market_json: &str,
    as_of: &str,
    scenario_json: &str,
    method: Option<String>,
    config_json: Option<String>,
    calendar_id: Option<String>,
) -> Result<JsValue, JsValue> {
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

    let scenario: finstack_quant_scenarios::ScenarioSpec =
        serde_json::from_str(scenario_json).map_err(to_js_err)?;

    // Parse method via the canonical scenarios-crate parser (shared with Python).
    let method_str = method.as_deref().unwrap_or("parallel");
    let attribution_method =
        finstack_quant_scenarios::horizon::attribution_method_from_str(method_str)
            .map_err(to_js_err)?;

    let finstack_config: finstack_quant_core::config::FinstackConfig = match config_json.as_deref()
    {
        Some(json) => serde_json::from_str(json).map_err(to_js_err)?,
        None => finstack_quant_core::config::FinstackConfig::default(),
    };

    let mut analyzer = finstack_quant_scenarios::horizon::HorizonAnalysis::new(
        attribution_method,
        finstack_config,
    )
    .with_recalibration_provider(Arc::new(
        finstack_quant_calibration::recalibration::CachedRecalibrationProvider::new(),
    ));
    if let Some(id) = calendar_id.as_deref() {
        analyzer = analyzer.with_calendar_id(id);
    }
    let result = analyzer
        .compute(&instrument, &market, date, &scenario)
        .map_err(to_js_err)?;

    crate::utils::to_js_value(&result)
}

#[cfg(test)]
mod tests {
    use super::*;
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
        let parsed = parse_scenario_spec_inner(&json).expect("parse");
        assert_eq!(parsed.id, "parsed");

        let composed =
            compose_scenarios_inner(vec![empty_spec("a", 0), empty_spec("b", 1)]).expect("compose");
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

        let error = compose_scenarios_inner(vec![first, second])
            .expect_err("duplicate time rolls should be rejected");
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

        let error = compose_scenarios_inner(vec![first_order, solve_to_par])
            .expect_err("mixed hazard bump modes should be rejected");
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
