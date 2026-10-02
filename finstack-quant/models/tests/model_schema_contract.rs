//! Generated model schemas describe exactly what serde writes.
//!
//! Covers the contracts with a custom wire shape: `try_from` raw/wire
//! structs, hand-written `Serialize`/`Deserialize`, nalgebra matrices (a
//! `[data, nrows, ncols]` tuple), `SmallVec` state vectors and date adapters.
//! Each value is serialized, validated against the schema the registry
//! publishes for its type (a root artifact or a `$defs` entry of one) and read
//! back.

use finstack_quant_core::dates::{Date, Month};
use finstack_quant_models::credit::migration::{
    GeneratorMatrix, MigrationSimulator, RatingScale, TransitionMatrix,
};
use finstack_quant_models::credit::pd::MasterScale;
use finstack_quant_models::liquidity::LiquidityProfile;
use finstack_quant_models::monte_carlo::paths::PathPoint;
use finstack_quant_models::monte_carlo::process::ou::HullWhite1FParams;
use finstack_quant_models::rates::dtsm::{DieboldLi, YieldPanel, YieldPca};
use finstack_quant_models::rates::hull_white::HullWhiteParams;
use finstack_quant_models::volatility::rough_heston::RoughHestonFourierParams;
use finstack_quant_models::volatility::svi::SviParams;
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::{json, Value};

/// Schema for `type_name`: its root artifact, or a `$defs` entry of any
/// registered artifact, wrapped so its local references still resolve.
fn schema_for(type_name: &str) -> Value {
    let artifacts = finstack_quant_models::factor::schema::ARTIFACTS
        .iter()
        .chain(finstack_quant_models::schema::ARTIFACTS);
    let mut nested = None;
    for artifact in artifacts {
        let schema = artifact.generate().expect("schema renders");
        if artifact.type_name() == type_name {
            return schema;
        }
        if nested.is_none() && schema["$defs"].get(type_name).is_some() {
            nested = Some(json!({
                "$schema": "https://json-schema.org/draft/2020-12/schema",
                "$ref": format!("#/$defs/{type_name}"),
                "$defs": schema["$defs"].clone(),
            }));
        }
    }
    nested.unwrap_or_else(|| panic!("{type_name} is not published by the models registry"))
}

fn assert_wire_matches_schema<T: Serialize + DeserializeOwned>(type_name: &str, value: &T) {
    let wire = serde_json::to_value(value).expect("serialize");
    let validator = jsonschema::options()
        .should_validate_formats(true)
        .build(&schema_for(type_name))
        .expect("schema compiles");
    let errors: Vec<String> = validator
        .iter_errors(&wire)
        .map(|e| e.to_string())
        .collect();
    assert!(errors.is_empty(), "{type_name}: {errors:?}\n{wire}");
    let back: T = serde_json::from_value(wire.clone()).expect("deserialize");
    assert_eq!(
        serde_json::to_value(&back).expect("reserialize"),
        wire,
        "{type_name} round trip"
    );
}

fn yield_panel() -> YieldPanel {
    let dates = (1..=8)
        .map(|day| Date::from_calendar_date(2025, Month::March, day).expect("valid date"))
        .collect();
    let rows = (0..8)
        .map(|t| {
            let t = f64::from(t);
            let level = 0.0010 * (1.3 * t).sin();
            let slope = 0.0008 * (0.7 * t + 0.4).cos();
            let curve = 0.0005 * (2.1 * t + 1.0).sin();
            [0.5_f64, 2.0, 5.0, 10.0]
                .iter()
                .map(|tenor| {
                    0.03 + level + slope * tenor / 10.0 + curve * (tenor / 5.0 - 1.0).powi(2)
                })
                .collect()
        })
        .collect();
    YieldPanel::from_rows(vec![0.5, 2.0, 5.0, 10.0], rows, Some(dates)).expect("valid panel")
}

#[test]
fn migration_contracts_validate_against_their_schemas() {
    let scale = RatingScale::custom(vec!["IG".into(), "HY".into(), "D".into()]).expect("scale");
    assert_wire_matches_schema("RatingScale", &scale);
    let matrix = TransitionMatrix::new(
        scale,
        &[0.90, 0.08, 0.02, 0.10, 0.80, 0.10, 0.0, 0.0, 1.0],
        1.0,
    )
    .expect("valid matrix");
    assert_wire_matches_schema("TransitionMatrix", &matrix);
    let generator = GeneratorMatrix::from_transition_matrix(&matrix).expect("generator");
    assert_wire_matches_schema("GeneratorMatrix", &generator);
    let simulator = MigrationSimulator::new(generator, 1.0).expect("simulator");
    assert_wire_matches_schema("MigrationSimulator", &simulator);
    let master = MasterScale::sp_assumptions().expect("embedded master scale");
    assert_wire_matches_schema("MasterScale", &master);
}

#[test]
fn nalgebra_matrices_validate_as_data_rows_cols_tuples() {
    let panel = yield_panel();
    let wire = serde_json::to_value(&panel).expect("serialize");
    assert_eq!(wire["yields"][1], json!(8));
    assert_eq!(wire["yields"][2], json!(4));
    assert_wire_matches_schema("YieldPanel", &panel);
    assert_wire_matches_schema("YieldPca", &YieldPca::fit(&panel).expect("pca"));
    let model = DieboldLi::with_default_lambda().fit(&panel).expect("fit");
    assert_wire_matches_schema("DieboldLi", &model);
}

#[test]
fn raw_parameter_wires_validate_against_their_schemas() {
    assert_wire_matches_schema(
        "HullWhiteParams",
        &HullWhiteParams::constant(0.05, 0.01).expect("params"),
    );
    assert_wire_matches_schema(
        "HullWhite1FParams",
        &HullWhite1FParams::new(0.05, 0.01, 0.03).expect("params"),
    );
    assert_wire_matches_schema(
        "RoughHestonFourierParams",
        &RoughHestonFourierParams::new(0.04, 1.5, 0.04, 0.5, -0.7, 0.1).expect("params"),
    );
    let svi: SviParams = serde_json::from_value(json!({
        "a": 0.04, "b": 0.1, "rho": -0.3, "m": 0.0, "sigma": 0.2
    }))
    .expect("svi");
    assert_wire_matches_schema("SviParams", &svi);
}

#[test]
fn liquidity_profile_and_path_point_validate_against_their_schemas() {
    let profile =
        LiquidityProfile::new("BOND-1", 100.0, 99.5, 100.5, 1.0e6, 5.0e3, 0.02).expect("profile");
    assert_wire_matches_schema("LiquidityProfile", &profile);
    let mut unknown = serde_json::to_value(&profile).expect("serialize");
    unknown["typo"] = json!(1);
    let validator = jsonschema::options()
        .build(&schema_for("LiquidityProfile"))
        .expect("schema compiles");
    assert!(
        !validator.is_valid(&unknown),
        "unknown keys are rejected like serde"
    );
    assert!(serde_json::from_value::<LiquidityProfile>(unknown).is_err());

    let point: PathPoint = serde_json::from_value(json!({
        "step": 3, "time": 0.25, "state": [101.0, 0.04], "payoff_value": null, "cashflows": []
    }))
    .expect("path point");
    assert_wire_matches_schema("PathPoint", &point);
}
