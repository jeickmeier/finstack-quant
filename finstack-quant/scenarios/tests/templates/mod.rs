//! Tests for the surrounding crate component and its documented behavior.
//!
use finstack_quant_scenarios::{AssetClass, OperationSpec, ScenarioSpec, TemplateRegistry};

fn builtin_ids() -> Vec<&'static str> {
    vec![
        "gfc_2008",
        "covid_2020",
        "rate_shock_2022",
        "svb_2023",
        "ltcm_1998",
    ]
}

#[test]
fn embedded_registry_contains_all_five_builtins_end_to_end() {
    let registry = TemplateRegistry::embedded_builtins()
        .unwrap_or_else(|error| panic!("failed to load embedded templates: {error}"));
    let listed_ids: Vec<_> = registry
        .list()
        .into_iter()
        .map(|metadata| metadata.id.as_str())
        .collect();

    assert_eq!(listed_ids, builtin_ids());

    for template_id in builtin_ids() {
        let scenario = registry
            .build(template_id)
            .unwrap_or_else(|error| panic!("missing builtin template {template_id}: {error}"));
        let component_ids = registry
            .component_ids(template_id)
            .unwrap_or_else(|error| panic!("missing builtin template {template_id}: {error}"));

        assert_eq!(scenario.id, template_id);
        assert!(!scenario.operations.is_empty());
        assert_eq!(component_ids.len(), 5);

        for component_id in component_ids {
            let component = registry
                .build_component(template_id, component_id)
                .unwrap_or_else(|error| panic!("missing component {component_id}: {error}"));
            assert_eq!(component.id, component_id);
            assert!(!component.operations.is_empty());
        }
    }
}

#[test]
fn embedded_registry_filters_historical_cross_asset_builtins() {
    let registry = TemplateRegistry::embedded_builtins()
        .unwrap_or_else(|error| panic!("failed to load embedded templates: {error}"));

    let historical_ids: Vec<_> = registry
        .list()
        .into_iter()
        .filter(|metadata| metadata.tags.iter().any(|tag| tag == "historical"))
        .map(|metadata| metadata.id.as_str())
        .collect();
    let fx_ids: Vec<_> = registry
        .list()
        .into_iter()
        .filter(|metadata| metadata.asset_classes.contains(&AssetClass::FX))
        .map(|metadata| metadata.id.as_str())
        .collect();

    assert_eq!(historical_ids, builtin_ids());
    assert_eq!(fx_ids, builtin_ids());
}

#[test]
fn cross_template_component_composition_still_works() {
    let registry = TemplateRegistry::embedded_builtins()
        .unwrap_or_else(|error| panic!("failed to load embedded templates: {error}"));
    let rate_spec = registry
        .build_component("rate_shock_2022", "rate_shock_2022_rates")
        .unwrap_or_else(|error| panic!("missing rate_shock_2022_rates: {error}"));
    let svb_credit_spec = registry
        .build_component("svb_2023", "svb_2023_credit")
        .unwrap_or_else(|error| panic!("missing svb_2023_credit: {error}"));

    let mut composed = ScenarioSpec::compose(vec![rate_spec.clone(), svb_credit_spec.clone()])
        .unwrap_or_else(|error| panic!("failed to compose scenarios: {error}"));
    composed.id = "cross_template".to_string();
    composed
        .validate()
        .unwrap_or_else(|error| panic!("failed to validate composed scenario: {error}"));

    assert_eq!(composed.id, "cross_template");
    assert_eq!(
        composed.operations.len(),
        rate_spec.operations.len() + svb_credit_spec.operations.len()
    );
}

#[test]
fn embedded_registry_svb_credit_component_contains_attr_spread_shock() {
    let registry = TemplateRegistry::embedded_builtins()
        .unwrap_or_else(|error| panic!("failed to load embedded templates: {error}"));
    let credit = registry
        .build_component("svb_2023", "svb_2023_credit")
        .unwrap_or_else(|error| panic!("missing svb_2023_credit: {error}"));

    assert!(credit.operations.iter().any(|operation| {
        matches!(
            operation,
            OperationSpec::InstrumentSpreadBpByAttr { attrs, bp }
                if attrs.get("sector").map(String::as_str) == Some("regional_banks")
                    && (*bp - 150.0).abs() < f64::EPSILON
        )
    }));
}

#[test]
fn embedded_registry_built_scenario_roundtrips_through_serde_and_validation() {
    let registry = TemplateRegistry::embedded_builtins()
        .unwrap_or_else(|error| panic!("failed to load embedded templates: {error}"));
    let scenario = registry
        .build("ltcm_1998")
        .unwrap_or_else(|error| panic!("missing ltcm_1998: {error}"));

    let json = serde_json::to_string(&scenario)
        .unwrap_or_else(|error| panic!("failed to serialize scenario: {error}"));
    let roundtrip: ScenarioSpec = serde_json::from_str(&json)
        .unwrap_or_else(|error| panic!("failed to deserialize scenario: {error}"));

    roundtrip
        .validate()
        .unwrap_or_else(|error| panic!("roundtrip scenario should validate: {error}"));
    assert_eq!(roundtrip.id, "ltcm_1998");
    assert_eq!(roundtrip.operations.len(), scenario.operations.len());
}

/// Everything the built-in registry exposes, in registry order: metadata,
/// the composite scenario, and every component scenario.
fn built_registry_snapshot() -> serde_json::Value {
    let registry = TemplateRegistry::embedded_builtins()
        .unwrap_or_else(|error| panic!("failed to load embedded templates: {error}"));
    let templates = registry
        .list()
        .into_iter()
        .map(|metadata| {
            let id = metadata.id.as_str();
            let scenario = registry
                .build(id)
                .unwrap_or_else(|error| panic!("build {id}: {error}"));
            let components = registry
                .component_ids(id)
                .unwrap_or_else(|error| panic!("components of {id}: {error}"))
                .into_iter()
                .map(|component_id| {
                    registry
                        .build_component(id, component_id)
                        .unwrap_or_else(|error| panic!("component {component_id}: {error}"))
                })
                .collect::<Vec<_>>();
            serde_json::json!({
                "metadata": metadata,
                "scenario": scenario,
                "components": components,
            })
        })
        .collect::<Vec<_>>();
    serde_json::Value::Array(templates)
}

/// Pins what the built-in templates produce, so a change to the embedded
/// document format or the loader cannot change a host-visible scenario.
/// Regenerate with `FQ_UPDATE_CANONICAL_GOLDENS=1` after an intended change.
#[test]
fn embedded_registry_output_matches_golden() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/data/templates_built.json"
    );
    let rendered = serde_json::to_string_pretty(&built_registry_snapshot())
        .unwrap_or_else(|error| panic!("serialize snapshot: {error}"))
        + "\n";
    if std::env::var_os("FQ_UPDATE_CANONICAL_GOLDENS").is_some() {
        std::fs::write(path, &rendered).unwrap_or_else(|error| panic!("write {path}: {error}"));
    }
    let golden =
        std::fs::read_to_string(path).unwrap_or_else(|error| panic!("read {path}: {error}"));
    assert_eq!(
        rendered, golden,
        "built-in template output drifted from {path}"
    );
}
