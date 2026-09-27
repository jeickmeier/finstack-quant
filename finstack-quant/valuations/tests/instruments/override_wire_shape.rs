//! Focused pricing-override bags use the three canonical wire keys.

use finstack_quant_valuations::instruments::{
    Autocallable, Bond, CapFloor, CdsOption, CommoditySwaption, FxForward,
};
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;

fn assert_canonical_override_wire<T>(instrument: &T, pointers: &[(&str, Value)], label: &str)
where
    T: Serialize + DeserializeOwned,
{
    let value = serde_json::to_value(instrument).unwrap_or_else(|err| {
        panic!("{label}: serialize focused overrides: {err}");
    });
    for (pointer, expected) in pointers {
        assert_eq!(
            value.pointer(pointer),
            Some(expected),
            "{label}: pointer {pointer}"
        );
    }
    assert!(
        value.get("pricing_overrides").is_none(),
        "{label}: legacy pricing_overrides key must be absent"
    );
    let _: T = serde_json::from_value(value).unwrap_or_else(|err| {
        panic!("{label}: deserialize canonical wire: {err}");
    });
}

#[test]
fn focused_overrides_use_canonical_wire_shape() {
    let mut bond = Bond::example().expect("bond example");
    bond.instrument_pricing_overrides.model_config.tree_steps = Some(321);
    bond.instrument_pricing_overrides
        .model_config
        .mc_seed_scenario = Some("dv01_up".to_string());
    bond.scenario_pricing_overrides.scenario_spread_shock_bp = Some(12.5);
    assert_canonical_override_wire(
        &bond,
        &[
            (
                "/instrument_pricing_overrides/model_config/tree_steps",
                serde_json::json!(321),
            ),
            (
                "/instrument_pricing_overrides/model_config/mc_seed_scenario",
                serde_json::json!("dv01_up"),
            ),
            (
                "/scenario_pricing_overrides/scenario_spread_shock_bp",
                serde_json::json!(12.5),
            ),
        ],
        "bond",
    );

    let mut cap = CapFloor::example().expect("cap floor example");
    cap.instrument_pricing_overrides.model_config.hw1f_sigma = Some(0.012);
    cap.instrument_pricing_overrides
        .model_config
        .mc_seed_scenario = Some("vega_up".to_string());
    cap.scenario_pricing_overrides.scenario_price_shock_decimal = Some(-0.04);
    assert_canonical_override_wire(
        &cap,
        &[
            (
                "/instrument_pricing_overrides/model_config/hw1f_sigma",
                serde_json::json!(0.012),
            ),
            (
                "/instrument_pricing_overrides/model_config/mc_seed_scenario",
                serde_json::json!("vega_up"),
            ),
            (
                "/scenario_pricing_overrides/scenario_price_shock_decimal",
                serde_json::json!(-0.04),
            ),
        ],
        "cap_floor",
    );

    let mut forward = FxForward::example().expect("fx forward example");
    forward
        .instrument_pricing_overrides
        .market_quotes
        .implied_volatility = Some(0.17);
    forward
        .instrument_pricing_overrides
        .model_config
        .mc_seed_scenario = Some("rho_up".to_string());
    forward
        .scenario_pricing_overrides
        .scenario_price_shock_decimal = Some(-0.03);
    assert_canonical_override_wire(
        &forward,
        &[
            (
                "/instrument_pricing_overrides/market_quotes/implied_volatility",
                serde_json::json!(0.17),
            ),
            (
                "/instrument_pricing_overrides/model_config/mc_seed_scenario",
                serde_json::json!("rho_up"),
            ),
            (
                "/scenario_pricing_overrides/scenario_price_shock_decimal",
                serde_json::json!(-0.03),
            ),
        ],
        "fx_forward",
    );

    let mut option = CdsOption::example().expect("cds option example");
    option
        .instrument_pricing_overrides
        .market_quotes
        .implied_volatility = Some(0.31);
    option
        .instrument_pricing_overrides
        .model_config
        .mc_seed_scenario = Some("vega_down".to_string());
    option.scenario_pricing_overrides.scenario_spread_shock_bp = Some(8.0);
    assert_canonical_override_wire(
        &option,
        &[
            (
                "/instrument_pricing_overrides/market_quotes/implied_volatility",
                serde_json::json!(0.31),
            ),
            (
                "/instrument_pricing_overrides/model_config/mc_seed_scenario",
                serde_json::json!("vega_down"),
            ),
            (
                "/scenario_pricing_overrides/scenario_spread_shock_bp",
                serde_json::json!(8.0),
            ),
        ],
        "cds_option",
    );

    let mut autocall = Autocallable::example().expect("autocallable example");
    autocall.instrument_pricing_overrides.model_config.mc_paths = Some(12_345);
    autocall
        .instrument_pricing_overrides
        .model_config
        .mc_seed_scenario = Some("delta_up".to_string());
    autocall
        .scenario_pricing_overrides
        .scenario_price_shock_decimal = Some(-0.08);
    assert_canonical_override_wire(
        &autocall,
        &[
            (
                "/instrument_pricing_overrides/model_config/mc_paths",
                serde_json::json!(12_345),
            ),
            (
                "/instrument_pricing_overrides/model_config/mc_seed_scenario",
                serde_json::json!("delta_up"),
            ),
            (
                "/scenario_pricing_overrides/scenario_price_shock_decimal",
                serde_json::json!(-0.08),
            ),
        ],
        "autocallable",
    );

    let mut swaption = CommoditySwaption::example();
    swaption
        .instrument_pricing_overrides
        .market_quotes
        .implied_volatility = Some(0.31);
    swaption
        .instrument_pricing_overrides
        .model_config
        .mc_seed_scenario = Some("vega_up".to_string());
    swaption
        .scenario_pricing_overrides
        .scenario_price_shock_decimal = Some(-0.05);
    assert_canonical_override_wire(
        &swaption,
        &[
            (
                "/instrument_pricing_overrides/market_quotes/implied_volatility",
                serde_json::json!(0.31),
            ),
            (
                "/instrument_pricing_overrides/model_config/mc_seed_scenario",
                serde_json::json!("vega_up"),
            ),
            (
                "/scenario_pricing_overrides/scenario_price_shock_decimal",
                serde_json::json!(-0.05),
            ),
        ],
        "commodity_swaption",
    );
}
