//! Generated portfolio schemas describe exactly what serde writes.
//!
//! Covers the contracts with a hand-written wire: candidate positions (the
//! instrument travels as its tagged `InstrumentJson`, referenced from the
//! published valuations artifact) and the margin results (sorted wire
//! structs). Each value is serialized, validated against the schema the
//! registry publishes for its type (a root artifact or a `$defs` entry of one)
//! and read back.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::money::Money;
use finstack_quant_core::HashMap;
use finstack_quant_margin::{ImMethodology, NettingSetId, SimmSensitivities};
use finstack_quant_portfolio::optimization::TradeUniverse;
use finstack_quant_portfolio::{NettingSetMargin, PortfolioMarginResult};
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::{json, Value};
use time::macros::date;

use crate::materialization_schema::external_schema_resources;

/// Schema for `type_name`: its root artifact, or a `$defs` entry of a
/// registered artifact wrapped under that artifact's `$id` so local and
/// published references both resolve.
fn schema_for(type_name: &str) -> Value {
    let mut nested = None;
    for artifact in finstack_quant_portfolio::schema::ARTIFACTS {
        let schema = artifact.generate().expect("schema renders");
        if artifact.type_name() == type_name {
            return schema;
        }
        if nested.is_none() && schema["$defs"].get(type_name).is_some() {
            nested = Some(json!({
                "$id": schema["$id"].clone(),
                "$schema": "https://json-schema.org/draft/2020-12/schema",
                "$ref": format!("#/$defs/{type_name}"),
                "$defs": schema["$defs"].clone(),
            }));
        }
    }
    nested.unwrap_or_else(|| panic!("{type_name} is not published by the portfolio registry"))
}

fn assert_wire_matches_schema<T: Serialize + DeserializeOwned>(type_name: &str, value: &T) {
    let wire = serde_json::to_value(value).expect("serialize");
    let validator = jsonschema::options()
        .should_validate_formats(true)
        .with_resources(external_schema_resources().into_iter())
        .build(&schema_for(type_name))
        .expect("schema compiles");
    let errors: Vec<String> = validator
        .iter_errors(&wire)
        .map(|e| e.to_string())
        .collect();
    assert!(errors.is_empty(), "{type_name}: {errors:?}");
    let back: T = serde_json::from_value(wire.clone()).expect("deserialize");
    assert_eq!(
        serde_json::to_value(&back).expect("reserialize"),
        wire,
        "{type_name} round trip"
    );
}

#[test]
fn trade_universe_candidate_validates_against_published_instrument_schema() {
    let deposit: Value = serde_json::from_str(include_str!(
        "../../valuations/tests/instruments/json_examples/deposit.json"
    ))
    .expect("canonical deposit example parses");
    let universe: TradeUniverse = serde_json::from_value(json!({
        "candidates": [{
            "id": "CAND-1",
            "entity_id": "FUND",
            "instrument_spec": deposit["instrument"].clone(),
            "unit": "units",
            "max_weight": 0.25,
            "min_weight": 0.0
        }]
    }))
    .expect("candidate universe deserializes");
    assert_wire_matches_schema("TradeUniverse", &universe);
}

#[test]
fn margin_results_validate_against_their_wire_schemas() {
    let mut sensitivities = SimmSensitivities::new(Currency::USD);
    sensitivities
        .ir_delta
        .insert((Currency::USD, "5Y".to_string()), 12_500.0);
    let margin = NettingSetMargin::new(
        NettingSetId::bilateral("BANK_A", "CSA_01"),
        date!(2025 - 01 - 15),
        Money::new(1_250_000.0, Currency::USD).expect("valid money"),
        Money::new(150_000.0, Currency::USD).expect("valid money"),
        4,
        ImMethodology::Simm,
    )
    .expect("valid margin")
    .with_simm_breakdown(sensitivities, Default::default());
    assert_wire_matches_schema("NettingSetMargin", &margin);

    let mut by_netting_set = HashMap::default();
    by_netting_set.insert(margin.netting_set_id.clone(), margin.clone());
    let result = PortfolioMarginResult {
        as_of: date!(2025 - 01 - 15),
        base_currency: Currency::USD,
        total_initial_margin: margin.initial_margin,
        total_variation_margin: margin.variation_margin,
        total_margin: margin.total_margin,
        by_netting_set,
        by_csa: HashMap::default(),
        total_required_im_collateral: Money::from((0_i64, Currency::USD)),
        total_im_transfer: Money::from((0_i64, Currency::USD)),
        total_segregated_im: Money::from((0_i64, Currency::USD)),
        total_positions: 4,
        positions_without_margin: 0,
        degraded_positions: vec![("POS-9".into(), "no CSA".to_string())],
    };
    assert_wire_matches_schema("PortfolioMarginResult", &result);
}
