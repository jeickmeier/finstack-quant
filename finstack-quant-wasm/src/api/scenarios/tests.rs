use finstack_quant_core::market_data::hierarchy::ResolutionMode;
use finstack_quant_scenarios::{OperationSpec, ScenarioSpec, TimeRollMode};

fn empty_spec(id: &str, priority: i32) -> ScenarioSpec {
    ScenarioSpec {
        id: id.into(),
        name: None,
        description: None,
        operations: Vec::new(),
        priority,
        resolution_mode: ResolutionMode::default(),
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
fn builtin_registry_builds_typed_templates_and_components() {
    let registry =
        finstack_quant_scenarios::TemplateRegistry::embedded_builtins().expect("registry");
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
