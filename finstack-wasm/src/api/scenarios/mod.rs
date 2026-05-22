//! WASM bindings for the `finstack-scenarios` crate.
//!
//! Exposes scenario specification parsing, validation, composition,
//! and built-in template access via JSON round-trip functions.

use crate::utils::to_js_err;
use std::sync::OnceLock;
use wasm_bindgen::prelude::*;

/// Lazily-initialised builtin template registry.  Constructed once on first
/// access, then reused for the lifetime of the WASM module.
fn builtin_registry() -> Result<&'static finstack_scenarios::TemplateRegistry, JsValue> {
    static REGISTRY: OnceLock<Result<finstack_scenarios::TemplateRegistry, String>> =
        OnceLock::new();
    let stored = REGISTRY.get_or_init(|| {
        finstack_scenarios::TemplateRegistry::with_embedded_builtins().map_err(|e| e.to_string())
    });
    stored.as_ref().map_err(to_js_err)
}

fn parse_date(as_of: &str) -> Result<time::Date, JsValue> {
    let format = time::format_description::well_known::Iso8601::DEFAULT;
    time::Date::parse(as_of, &format).map_err(to_js_err)
}

fn to_json_string<T: serde::Serialize>(value: &T) -> Result<String, JsValue> {
    serde_json::to_string(value).map_err(to_js_err)
}

fn apply_with_context(
    spec: &finstack_scenarios::ScenarioSpec,
    market: &mut finstack_core::market_data::context::MarketContext,
    model: Option<&mut finstack_statements::FinancialModelSpec>,
    as_of: time::Date,
) -> Result<finstack_scenarios::engine::ApplicationReport, JsValue> {
    let engine = finstack_scenarios::ScenarioEngine::new();
    let mut ctx = finstack_scenarios::ExecutionContext {
        market,
        model,
        instruments: None,
        rate_bindings: None,
        calendar: None,
        as_of,
    };
    engine.apply(spec, &mut ctx).map_err(to_js_err)
}

/// Parse and validate a scenario specification from JSON.
///
/// Returns the validated, re-serialized JSON.
#[wasm_bindgen(js_name = parseScenarioSpec)]
pub fn parse_scenario_spec(json_str: &str) -> Result<String, JsValue> {
    let spec: finstack_scenarios::ScenarioSpec =
        serde_json::from_str(json_str).map_err(to_js_err)?;

    spec.validate().map_err(to_js_err)?;

    serde_json::to_string(&spec).map_err(to_js_err)
}

/// Compose multiple scenario specs (JSON array) into a single scenario.
///
/// Specs are merged in priority order (lower number runs first).
#[wasm_bindgen(js_name = composeScenarios)]
pub fn compose_scenarios(specs_json: &str) -> Result<String, JsValue> {
    compose_scenarios_json(specs_json).map_err(to_js_err)
}

fn compose_scenarios_json(specs_json: &str) -> Result<String, String> {
    let specs: Vec<finstack_scenarios::ScenarioSpec> =
        serde_json::from_str(specs_json).map_err(|e| e.to_string())?;

    let engine = finstack_scenarios::ScenarioEngine::new();
    let composed = engine.try_compose(specs).map_err(|e| e.to_string())?;

    serde_json::to_string(&composed).map_err(|e| e.to_string())
}

/// Validate a scenario specification JSON without executing it.
///
/// Returns `true` if valid, throws on error.
#[wasm_bindgen(js_name = validateScenarioSpec)]
pub fn validate_scenario_spec(json_str: &str) -> Result<bool, JsValue> {
    let spec: finstack_scenarios::ScenarioSpec =
        serde_json::from_str(json_str).map_err(to_js_err)?;

    spec.validate().map_err(to_js_err)?;
    Ok(true)
}

/// List all built-in template identifiers.
///
/// Returns a JSON array of template ID strings.
#[wasm_bindgen(js_name = listBuiltinTemplates)]
pub fn list_builtin_templates() -> Result<JsValue, JsValue> {
    let registry = builtin_registry()?;
    let ids: Vec<String> = registry.list().iter().map(|m| m.id.clone()).collect();
    serde_wasm_bindgen::to_value(&ids).map_err(to_js_err)
}

/// Get metadata for all built-in templates as a JSON string.
#[wasm_bindgen(js_name = listBuiltinTemplateMetadata)]
pub fn list_builtin_template_metadata() -> Result<String, JsValue> {
    let registry = builtin_registry()?;
    let metadata: Vec<&finstack_scenarios::TemplateMetadata> = registry.list();
    serde_json::to_string(&metadata).map_err(to_js_err)
}

/// Build a scenario spec from a built-in template.
///
/// Returns JSON-serialized `ScenarioSpec`.
#[wasm_bindgen(js_name = buildFromTemplate)]
pub fn build_from_template(template_id: &str) -> Result<String, JsValue> {
    let registry = builtin_registry()?;
    let entry = registry
        .get(template_id)
        .ok_or_else(|| to_js_err(format!("Unknown template: '{template_id}'")))?;

    let spec = entry.builder().build().map_err(to_js_err)?;
    serde_json::to_string(&spec).map_err(to_js_err)
}

/// List component IDs for a built-in composite template.
///
/// Returns a JS array of component ID strings.
#[wasm_bindgen(js_name = listTemplateComponents)]
pub fn list_template_components(template_id: &str) -> Result<JsValue, JsValue> {
    let registry = builtin_registry()?;
    let entry = registry
        .get(template_id)
        .ok_or_else(|| to_js_err(format!("Unknown template: '{template_id}'")))?;

    let ids: Vec<String> = entry
        .component_ids()
        .into_iter()
        .map(str::to_string)
        .collect();
    serde_wasm_bindgen::to_value(&ids).map_err(to_js_err)
}

/// Build a specific component from a built-in composite template.
#[wasm_bindgen(js_name = buildTemplateComponent)]
pub fn build_template_component(template_id: &str, component_id: &str) -> Result<String, JsValue> {
    let registry = builtin_registry()?;
    let entry = registry
        .get(template_id)
        .ok_or_else(|| to_js_err(format!("Unknown template: '{template_id}'")))?;
    let builder = entry.component(component_id).ok_or_else(|| {
        to_js_err(format!(
            "Unknown component '{component_id}' in template '{template_id}'"
        ))
    })?;
    let spec = builder.build().map_err(to_js_err)?;
    serde_json::to_string(&spec).map_err(to_js_err)
}

/// Build a scenario spec from fields.
#[wasm_bindgen(js_name = buildScenarioSpec)]
pub fn build_scenario_spec(
    id: &str,
    operations_json: &str,
    name: Option<String>,
    description: Option<String>,
    priority: i32,
) -> Result<String, JsValue> {
    let operations: Vec<finstack_scenarios::OperationSpec> =
        serde_json::from_str(operations_json).map_err(to_js_err)?;
    let spec = finstack_scenarios::ScenarioSpec {
        id: id.to_string(),
        name,
        description,
        operations,
        priority,
        resolution_mode: Default::default(),
    };
    spec.validate().map_err(to_js_err)?;
    serde_json::to_string(&spec).map_err(to_js_err)
}

/// Apply a scenario to a market context and financial model.
///
/// Returns a JSON object with `market_json`, `model_json`,
/// `operations_applied`, `user_operations`, `expanded_operations`, and `warnings`.
#[wasm_bindgen(js_name = applyScenario)]
pub fn apply_scenario(
    scenario_json: &str,
    market_json: &str,
    model_json: &str,
    as_of: &str,
) -> Result<JsValue, JsValue> {
    let spec: finstack_scenarios::ScenarioSpec =
        serde_json::from_str(scenario_json).map_err(to_js_err)?;
    let mut market: finstack_core::market_data::context::MarketContext =
        serde_json::from_str(market_json).map_err(to_js_err)?;
    let mut model: finstack_statements::FinancialModelSpec =
        serde_json::from_str(model_json).map_err(to_js_err)?;
    let date = parse_date(as_of)?;
    let report = apply_with_context(&spec, &mut market, Some(&mut model), date)?;

    let out = serde_json::json!({
        "market_json": to_json_string(&market)?,
        "model_json": to_json_string(&model)?,
        "operations_applied": report.operations_applied,
        "user_operations": report.user_operations,
        "expanded_operations": report.expanded_operations,
        "warnings": report.warnings,
    });
    serde_wasm_bindgen::to_value(&out).map_err(to_js_err)
}

/// Apply a scenario to a market context only (no model mutations).
#[wasm_bindgen(js_name = applyScenarioToMarket)]
pub fn apply_scenario_to_market(
    scenario_json: &str,
    market_json: &str,
    as_of: &str,
) -> Result<JsValue, JsValue> {
    let spec: finstack_scenarios::ScenarioSpec =
        serde_json::from_str(scenario_json).map_err(to_js_err)?;
    let mut market: finstack_core::market_data::context::MarketContext =
        serde_json::from_str(market_json).map_err(to_js_err)?;
    let date = parse_date(as_of)?;
    let report = apply_with_context(&spec, &mut market, None, date)?;

    let out = serde_json::json!({
        "market_json": to_json_string(&market)?,
        "operations_applied": report.operations_applied,
        "user_operations": report.user_operations,
        "expanded_operations": report.expanded_operations,
        "warnings": report.warnings,
    });
    serde_wasm_bindgen::to_value(&out).map_err(to_js_err)
}

/// Compute horizon total return under a scenario.
///
/// Applies a scenario specification to project an instrument forward, then
/// decomposes the resulting P&L using factor-based attribution.
///
/// # Arguments
///
/// * `instrument_json` - JSON-serialized instrument (tagged).
/// * `market_json` - JSON-serialized `MarketContext`.
/// * `as_of` - Valuation date (ISO 8601).
/// * `scenario_json` - JSON-serialized `ScenarioSpec`.
/// * `method` - Attribution method: "parallel", "waterfall", "metrics_based", "taylor".
///
/// # Returns
///
/// JSON-serialized `HorizonResult`.
#[wasm_bindgen(js_name = computeHorizonReturn)]
pub fn compute_horizon_return(
    instrument_json: &str,
    market_json: &str,
    as_of: &str,
    scenario_json: &str,
    method: Option<String>,
    config_json: Option<String>,
) -> Result<String, JsValue> {
    use finstack_valuations::attribution::AttributionMethod;
    use finstack_valuations::instruments::InstrumentJson;
    use std::sync::Arc;

    // Parse instrument
    let inst: InstrumentJson = serde_json::from_str(instrument_json).map_err(to_js_err)?;
    let boxed = inst.into_boxed().map_err(to_js_err)?;
    let instrument: Arc<dyn finstack_valuations::instruments::Instrument> = Arc::from(boxed);

    // Parse market
    let market: finstack_core::market_data::context::MarketContext =
        serde_json::from_str(market_json).map_err(to_js_err)?;

    // Parse date
    let date = parse_date(as_of)?;

    // Parse scenario
    let scenario: finstack_scenarios::ScenarioSpec =
        serde_json::from_str(scenario_json).map_err(to_js_err)?;

    // Parse method
    let method_str = method.as_deref().unwrap_or("parallel");
    let attribution_method = match method_str {
        "parallel" => AttributionMethod::Parallel,
        "waterfall" => {
            AttributionMethod::Waterfall(
                finstack_valuations::attribution::default_waterfall_order(),
            )
        }
        "metrics_based" => AttributionMethod::MetricsBased,
        "taylor" => {
            AttributionMethod::Taylor(
                finstack_valuations::attribution::TaylorAttributionConfig::default(),
            )
        }
        other => return Err(to_js_err(format!(
            "Unknown attribution method '{other}'. Expected: parallel, waterfall, metrics_based, taylor"
        ))),
    };

    // Parse config
    let finstack_config: finstack_core::config::FinstackConfig = match config_json.as_deref() {
        Some(json) => serde_json::from_str(json).map_err(to_js_err)?,
        None => finstack_core::config::FinstackConfig::default(),
    };

    let analyzer =
        finstack_scenarios::horizon::HorizonAnalysis::new(attribution_method, finstack_config);
    let result = analyzer
        .compute(&instrument, &market, date, &scenario)
        .map_err(to_js_err)?;

    to_json_string(&result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_builtin_template_metadata_is_non_empty_json_array() {
        let json = list_builtin_template_metadata().expect("metadata");
        let v: serde_json::Value = serde_json::from_str(&json).expect("parse metadata");
        let arr = v.as_array().expect("array");
        assert!(!arr.is_empty());
    }

    #[test]
    fn build_from_template_and_build_template_component_succeed_for_builtin() {
        let meta = list_builtin_template_metadata().expect("metadata");
        let items: Vec<serde_json::Value> = serde_json::from_str(&meta).expect("parse");
        let first_id = items[0]["id"].as_str().expect("template id");
        let built = build_from_template(first_id).expect("build_from_template");
        assert!(!built.is_empty());

        let component_json =
            build_template_component("gfc_2008", "gfc_2008_rates").expect("component");
        assert!(!component_json.is_empty());
    }

    #[test]
    fn build_validate_parse_compose_roundtrip_empty_operations() {
        let spec_json = build_scenario_spec("test_id", "[]", Some("Test".to_string()), None, 0)
            .expect("build_scenario_spec");
        assert!(validate_scenario_spec(&spec_json).expect("validate"));
        let parsed = parse_scenario_spec(&spec_json).expect("parse");
        let before: serde_json::Value = serde_json::from_str(&spec_json).expect("before");
        let after: serde_json::Value = serde_json::from_str(&parsed).expect("after");
        assert_eq!(before, after);

        let composed = compose_scenarios("[]").expect("compose");
        assert!(validate_scenario_spec(&composed).expect("composed valid"));
    }

    #[test]
    fn build_scenario_with_name_and_description() {
        let spec_json = build_scenario_spec(
            "stress_1",
            "[]",
            Some("Stress scenario".to_string()),
            Some("A description".to_string()),
            10,
        )
        .expect("build");
        let parsed: serde_json::Value = serde_json::from_str(&spec_json).expect("json");
        assert_eq!(parsed["id"], "stress_1");
        assert_eq!(parsed["priority"], 10);
    }

    #[test]
    fn compose_multiple_scenarios() {
        let s1 = build_scenario_spec("s1", "[]", None, None, 0).expect("s1");
        let s2 = build_scenario_spec("s2", "[]", None, None, 1).expect("s2");
        let arr = format!("[{s1},{s2}]");
        let composed = compose_scenarios(&arr).expect("compose");
        assert!(validate_scenario_spec(&composed).expect("valid"));
    }

    #[test]
    fn compose_scenarios_rejects_duplicate_time_rolls() {
        use finstack_core::market_data::hierarchy::ResolutionMode;
        use finstack_scenarios::{OperationSpec, ScenarioSpec, TimeRollMode};

        let specs = serde_json::to_string(&vec![
            ScenarioSpec {
                id: "roll_1m".into(),
                name: None,
                description: None,
                operations: vec![OperationSpec::TimeRollForward {
                    period: "1M".into(),
                    apply_shocks: true,
                    roll_mode: TimeRollMode::BusinessDays,
                }],
                priority: 0,
                resolution_mode: ResolutionMode::Cumulative,
            },
            ScenarioSpec {
                id: "roll_3m".into(),
                name: None,
                description: None,
                operations: vec![OperationSpec::TimeRollForward {
                    period: "3M".into(),
                    apply_shocks: true,
                    roll_mode: TimeRollMode::BusinessDays,
                }],
                priority: 1,
                resolution_mode: ResolutionMode::Cumulative,
            },
        ])
        .expect("serialize specs");

        let err =
            compose_scenarios_json(&specs).expect_err("duplicate time rolls should be rejected");
        assert!(err.contains("TimeRollForward"), "unexpected error: {err}");
    }

    #[test]
    fn build_all_builtin_templates() {
        let meta = list_builtin_template_metadata().expect("metadata");
        let items: Vec<serde_json::Value> = serde_json::from_str(&meta).expect("parse");
        for item in &items {
            let id = item["id"].as_str().expect("id");
            let built = build_from_template(id).expect("build");
            assert!(!built.is_empty(), "template {id} produced empty output");
        }
    }
}
