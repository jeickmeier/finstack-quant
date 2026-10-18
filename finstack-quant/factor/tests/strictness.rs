//! Phase 2 strict persisted factor-definition contract regressions.

use finstack_quant_core::market_data::bumps::BumpUnits;
use finstack_quant_core::types::CurveId;
use finstack_quant_factor::{FactorDefinition, FactorId, FactorType, MarketMapping};

#[test]
fn factor_definition_rejects_unknown_fields() {
    let definition = FactorDefinition {
        id: FactorId::new("USD-Rates"),
        factor_type: FactorType::Rates,
        market_mapping: MarketMapping::CurveParallel {
            curve_ids: vec![CurveId::new("USD-OIS")],
            units: BumpUnits::RateBp,
        },
        description: None,
    };
    let mut value = serde_json::to_value(definition).expect("definition serializes");
    value
        .as_object_mut()
        .expect("definition is an object")
        .insert("__nonexistent__".into(), serde_json::json!(true));

    let error =
        serde_json::from_value::<FactorDefinition>(value).expect_err("unknown fields must fail");
    assert!(error.to_string().contains("unknown field"), "{error}");
}
