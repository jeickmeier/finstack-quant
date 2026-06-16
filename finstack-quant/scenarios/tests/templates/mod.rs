use finstack_quant_scenarios::{
    AssetClass, OperationSpec, ScenarioSpec, ScenarioSpecBuilder, TemplateRegistry,
};

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
fn default_registry_contains_all_five_builtins_end_to_end() {
    let registry = TemplateRegistry::default();
    let listed_ids: Vec<_> = registry
        .list()
        .into_iter()
        .map(|metadata| metadata.id.as_str())
        .collect();

    assert_eq!(listed_ids, builtin_ids());

    for template_id in builtin_ids() {
        let entry = registry
            .get(template_id)
            .unwrap_or_else(|| panic!("missing builtin template: {template_id}"));
        let scenario = entry
            .builder()
            .build()
            .unwrap_or_else(|error| panic!("failed to build builtin {template_id}: {error}"));

        assert_eq!(scenario.id, template_id);
        assert!(!scenario.operations.is_empty());
        assert_eq!(entry.component_ids().len(), 5);

        for component_id in entry.component_ids() {
            let component = entry
                .component(component_id)
                .unwrap_or_else(|| panic!("missing component {component_id}"))
                .build()
                .unwrap_or_else(|error| {
                    panic!("failed to build component {component_id}: {error}")
                });
            assert_eq!(component.id, component_id);
            assert!(!component.operations.is_empty());
        }
    }
}

#[test]
fn default_registry_filters_historical_cross_asset_builtins() {
    let registry = TemplateRegistry::default();

    let historical_ids: Vec<_> = registry
        .filter_by_tag("historical")
        .into_iter()
        .map(|metadata| metadata.id.as_str())
        .collect();
    let fx_ids: Vec<_> = registry
        .filter_by_asset_class(AssetClass::FX)
        .into_iter()
        .map(|metadata| metadata.id.as_str())
        .collect();

    assert_eq!(historical_ids, builtin_ids());
    assert_eq!(fx_ids, builtin_ids());
}

#[test]
fn cross_template_component_composition_still_works() {
    let registry = TemplateRegistry::default();
    let rate_builder = registry
        .get("rate_shock_2022")
        .unwrap_or_else(|| panic!("missing rate_shock_2022"))
        .component("rate_shock_2022_rates")
        .unwrap_or_else(|| panic!("missing rate_shock_2022_rates"));
    let svb_credit_builder = registry
        .get("svb_2023")
        .unwrap_or_else(|| panic!("missing svb_2023"))
        .component("svb_2023_credit")
        .unwrap_or_else(|| panic!("missing svb_2023_credit"));

    let rate_spec = rate_builder
        .clone()
        .build()
        .unwrap_or_else(|error| panic!("failed to build rate component: {error}"));
    let svb_credit_spec = svb_credit_builder
        .clone()
        .build()
        .unwrap_or_else(|error| panic!("failed to build svb credit component: {error}"));
    let composed = ScenarioSpecBuilder::compose(vec![rate_builder, svb_credit_builder])
        .id("cross_template")
        .build()
        .unwrap_or_else(|error| panic!("failed to build composed scenario: {error}"));

    assert_eq!(composed.id, "cross_template");
    assert_eq!(
        composed.operations.len(),
        rate_spec.operations.len() + svb_credit_spec.operations.len()
    );
}

#[test]
fn default_registry_svb_credit_component_contains_attr_spread_shock() {
    let registry = TemplateRegistry::default();
    let credit = registry
        .get("svb_2023")
        .unwrap_or_else(|| panic!("missing svb_2023"))
        .component("svb_2023_credit")
        .unwrap_or_else(|| panic!("missing svb_2023_credit"))
        .build()
        .unwrap_or_else(|error| panic!("failed to build svb credit component: {error}"));

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
fn default_registry_built_scenario_roundtrips_through_serde_and_validation() {
    let registry = TemplateRegistry::default();
    let scenario = registry
        .get("ltcm_1998")
        .unwrap_or_else(|| panic!("missing ltcm_1998"))
        .builder()
        .build()
        .unwrap_or_else(|error| panic!("failed to build ltcm_1998: {error}"));

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
