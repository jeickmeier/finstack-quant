//! Public naming contract for serialize-only scenario output views.

use finstack_quant_portfolio::scenarios::{
    apply_and_revalue, scenario_pnl, ScenarioPnlView, ScenarioRevalueView,
};

#[test]
fn public_scenario_views_are_serializable_and_exported() {
    fn assert_serialize<T: serde::Serialize>() {}

    assert_serialize::<ScenarioRevalueView>();
    assert_serialize::<ScenarioPnlView>();
    assert_eq!(
        std::any::type_name::<ScenarioRevalueView>(),
        "finstack_quant_portfolio::scenarios::ScenarioRevalueView"
    );
    assert_eq!(
        std::any::type_name::<ScenarioPnlView>(),
        "finstack_quant_portfolio::scenarios::ScenarioPnlView"
    );

    let _ = apply_and_revalue;
    let _ = scenario_pnl;
}
