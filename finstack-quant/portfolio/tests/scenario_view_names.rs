//! Public naming contract for serialize-only scenario output views.

use finstack_quant_portfolio::scenarios::{
    ScenarioPnlBatchItem, ScenarioRevalueView, apply_scenario_and_revalue, scenario_pnl,
};

#[test]
fn public_scenario_views_are_serializable_and_exported() {
    fn assert_serialize<T: serde::Serialize>() {}

    assert_serialize::<ScenarioRevalueView>();
    assert_serialize::<ScenarioPnlBatchItem>();
    assert_eq!(
        std::any::type_name::<ScenarioRevalueView>(),
        "finstack_quant_portfolio::scenarios::ScenarioRevalueView"
    );
    assert_eq!(
        std::any::type_name::<ScenarioPnlBatchItem>(),
        "finstack_quant_portfolio::scenarios::ScenarioPnlBatchItem"
    );

    let _ = apply_scenario_and_revalue;
    let _ = scenario_pnl;
}
