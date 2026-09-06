//! Schema contract tests for the checked-in margin JSON Schema.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::money::Money;
use finstack_quant_margin::schema::{
    generated_margin_schema, MARGIN_SCHEMA_BASE, MARGIN_SCHEMA_DESCRIPTION, MARGIN_SCHEMA_FILENAME,
    MARGIN_SCHEMA_TITLE,
};
use finstack_quant_margin::{
    CollateralAssetClass, CollateralEligibility, CsaSpec, EligibleCollateralSchedule, ImParameters,
    MarginCall, MarginCallTiming, MarginEnvelope, MaturityConstraints, OtcMarginSpec,
};
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::{json, Value};
use std::fmt::Debug;
use time::macros::date;

const CANONICAL_DECIMAL_PATTERN: &str = r"^-?\d+(\.\d+)?([eE][+-]?\d+)?$";

fn margin_schema() -> Value {
    serde_json::from_str(include_str!("../schemas/margin/1/margin.schema.json"))
        .expect("schema JSON should be valid")
}

fn validate_fixture(schema: &Value, fixture: &Value) -> Result<(), Vec<String>> {
    let validator = jsonschema::options()
        .should_validate_formats(true)
        .build(schema)
        .expect("checked-in schema should compile");
    let errors = validator
        .iter_errors(fixture)
        .map(|error| error.to_string())
        .collect::<Vec<_>>();
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn csa_fixture() -> Value {
    json!({
        "schema": "finstack_quant.margin/1",
        "csa_spec": CsaSpec::usd_regulatory().expect("embedded margin registry should load"),
    })
}

fn margin_call_fixture() -> Value {
    json!({
        "schema": "finstack_quant.margin/1",
        "margin_call": MarginCall::vm_post(
            date!(2026 - 07 - 30),
            date!(2026 - 07 - 31),
            Money::new(1_000_000.0, Currency::USD).expect("valid money fixture"),
            Money::new(1_250_000.0, Currency::USD).expect("valid money fixture"),
            Money::new(0.0, Currency::USD).expect("valid money fixture"),
            Money::new(250_000.0, Currency::USD).expect("valid money fixture"),
        ),
    })
}

fn assert_fixtures_rejected(schema: &Value, fixtures: Vec<(&str, Value)>) {
    let accepted = fixtures
        .into_iter()
        .filter_map(|(name, fixture)| validate_fixture(schema, &fixture).is_ok().then_some(name))
        .collect::<Vec<_>>();
    assert!(
        accepted.is_empty(),
        "invalid fixtures were accepted: {accepted:?}"
    );
}

fn assert_csa_fixtures_rejected_by_schema_and_serde(
    schema: &Value,
    fixtures: Vec<(&str, &str, Value)>,
) {
    for (name, field, fixture) in fixtures {
        assert!(
            validate_fixture(schema, &fixture).is_err(),
            "{name} must be rejected by the published schema"
        );
        let error = serde_json::from_value::<CsaSpec>(fixture["csa_spec"].clone())
            .expect_err("invalid CSA must fail deserialization");
        assert!(
            error.to_string().contains(field),
            "{name} deserialization error must identify {field}: {error}"
        );
    }
}

fn assert_round_trip<T>(value: &T)
where
    T: Debug + DeserializeOwned + PartialEq + Serialize,
{
    let json = serde_json::to_value(value).expect("valid margin value must serialize");
    let round_tripped =
        serde_json::from_value::<T>(json).expect("serialized margin value must deserialize");
    assert_eq!(&round_tripped, value);
}

fn assert_serialization_rejects<T: Serialize>(value: &T, field: &str) {
    let error = serde_json::to_value(value).expect_err("invalid margin value must not serialize");
    assert!(
        error.to_string().contains(field),
        "serialization error must identify {field}: {error}"
    );
}

fn assert_described_one_of_enum(schema_name: &str, schema: &Value) {
    assert!(
        schema.get("enum").is_none(),
        "{schema_name} should use oneOf/const variants instead of enum"
    );
    let variants = schema
        .get("oneOf")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("{schema_name} should declare oneOf variants"));
    assert!(
        variants.iter().all(|variant| {
            variant.get("const").and_then(Value::as_str).is_some()
                && variant.get("description").and_then(Value::as_str).is_some()
        }),
        "{schema_name} oneOf variants should have const and description"
    );
}

#[test]
fn checked_in_schema_matches_generated_margin_contract() {
    assert_eq!(
        margin_schema(),
        generated_margin_schema().expect("margin schema should generate")
    );
}

#[test]
fn margin_schema_preserves_canonical_metadata_and_nested_types() {
    let schema = margin_schema();
    assert_eq!(
        schema["$id"],
        format!("{MARGIN_SCHEMA_BASE}{MARGIN_SCHEMA_FILENAME}")
    );
    assert_eq!(
        schema["$schema"],
        "https://json-schema.org/draft/2020-12/schema"
    );
    assert_eq!(schema["title"], MARGIN_SCHEMA_TITLE);
    assert_eq!(schema["description"], MARGIN_SCHEMA_DESCRIPTION);
    let root_variants = schema["anyOf"]
        .as_array()
        .expect("serde-derived untagged root should be an anyOf union");
    assert_eq!(root_variants.len(), 3);
    for variant in root_variants {
        assert_eq!(variant["additionalProperties"], false);
        assert_eq!(
            variant["properties"]["schema"]["$ref"],
            "#/$defs/MarginSchema"
        );
    }
    // The marker is a one-variant enum, so the emitter collapses schemars'
    // single-branch `oneOf` wrapper and the `const` sits on the definition.
    assert_eq!(
        schema["$defs"]["MarginSchema"]["const"],
        "finstack_quant.margin/1"
    );
    assert!(
        schema["$defs"]["MarginSchema"].get("oneOf").is_none(),
        "a single-branch union must not survive into a published artifact"
    );
    assert_eq!(
        schema["$defs"]["OtcMarginSpec"]["properties"]["csa"]["$ref"],
        "#/$defs/CsaSpec"
    );
    assert!(
        schema["$defs"]["Money"]["properties"]["amount"].is_object(),
        "Money amounts should retain their generated representation"
    );
    assert_eq!(
        schema["$defs"]["MarginCall"]["properties"]["call_type"]["$ref"],
        "#/$defs/MarginCallType"
    );
    let asset_class = &schema["$defs"]["CollateralAssetClass"];
    let values: Vec<&str> = asset_class["oneOf"]
        .as_array()
        .expect("derived collateral variants")
        .iter()
        .filter_map(|variant| variant["const"].as_str())
        .collect();
    assert_eq!(
        values,
        [
            "cash",
            "government_bonds",
            "agency_bonds",
            "covered_bonds",
            "corporate_bonds",
            "equity",
            "gold",
            "mutual_funds",
        ]
    );
}

#[test]
fn margin_schema_applies_canonical_decimal_and_date_normalization() {
    let schema = margin_schema();

    assert_eq!(
        schema.pointer("/$defs/DecimalWire/pattern"),
        Some(&json!(CANONICAL_DECIMAL_PATTERN))
    );
    assert_eq!(
        schema.pointer("/$defs/MarginCall/properties/call_date/format"),
        Some(&json!("date"))
    );
}

#[test]
fn described_margin_enums_use_one_of_const_style() {
    let schema = margin_schema();
    for schema_name in ["ImMethodology", "MarginTenor", "MarginCallType"] {
        let enum_schema = schema
            .pointer(&format!("/$defs/{schema_name}"))
            .unwrap_or_else(|| panic!("{schema_name} should exist in schema"));
        assert_described_one_of_enum(schema_name, enum_schema);
    }
}

#[test]
fn synthetic_margin_schema_validates_representative_union_variants() {
    let schema = margin_schema();
    let csa = CsaSpec::usd_regulatory().expect("embedded margin registry should load");
    let csa_json = serde_json::to_value(&csa).expect("representative CSA serializes");
    serde_json::from_value::<CsaSpec>(csa_json)
        .expect("representative CSA must satisfy serde constraints");
    let otc = OtcMarginSpec::bilateral_simm(csa.clone());
    let margin_call = MarginCall::vm_post(
        date!(2026 - 07 - 30),
        date!(2026 - 07 - 31),
        Money::new(1_000_000.0, Currency::USD).expect("valid money fixture"),
        Money::new(1_250_000.0, Currency::USD).expect("valid money fixture"),
        Money::new(0.0, Currency::USD).expect("valid money fixture"),
        Money::new(250_000.0, Currency::USD).expect("valid money fixture"),
    );
    let fixtures = [
        json!({
            "schema": "finstack_quant.margin/1",
            "otc_margin_spec": otc,
        }),
        json!({
            "schema": "finstack_quant.margin/1",
            "csa_spec": csa,
        }),
        json!({
            "schema": "finstack_quant.margin/1",
            "margin_call": margin_call,
        }),
    ];

    for fixture in fixtures {
        validate_fixture(&schema, &fixture)
            .unwrap_or_else(|errors| panic!("valid fixture was rejected: {errors:#?}"));
        serde_json::from_value::<MarginEnvelope>(fixture)
            .expect("valid fixture should deserialize through the root envelope");
    }
}

#[test]
fn constrained_public_margin_values_serialize_and_round_trip() {
    let maturity = MaturityConstraints {
        min_remaining_years: Some(0.0),
        max_remaining_years: Some(30.0),
    };
    let eligibility = CollateralEligibility {
        asset_class: CollateralAssetClass::GovernmentBonds,
        min_rating: Some("A-".to_string()),
        maturity_constraints: Some(maturity.clone()),
        haircut: 0.02,
        fx_haircut_addon: 0.08,
        concentration_limit: Some(0.5),
    };
    let schedule = EligibleCollateralSchedule {
        eligible: vec![eligibility.clone()],
        default_haircut: Some(0.25),
        rehypothecation_allowed: false,
    };
    let im =
        ImParameters::simm_standard(Currency::USD).expect("embedded margin registry should load");
    let timing = MarginCallTiming {
        notification_deadline_hours: 13,
        response_deadline_hours: 2,
        dispute_resolution_days: 1,
        delivery_grace_days: 1,
    };

    assert_round_trip(&maturity);
    assert_round_trip(&MaturityConstraints::max_maturity(10.0));
    assert_round_trip(&eligibility);
    assert_round_trip(&CollateralEligibility::government_bonds(0.02));
    assert_round_trip(&schedule);
    assert_round_trip(&im);
    assert_round_trip(&timing);
}

#[test]
fn constrained_public_margin_values_fail_serialization_with_field_names() {
    assert_serialization_rejects(
        &MaturityConstraints::max_maturity(-0.01),
        "max_remaining_years",
    );

    let maturity = MaturityConstraints {
        min_remaining_years: Some(-0.01),
        ..MaturityConstraints::default()
    };
    assert_serialization_rejects(&maturity, "min_remaining_years");

    let mut eligibility = CollateralEligibility::government_bonds(-0.01);
    assert_serialization_rejects(&eligibility, "haircut");
    eligibility.haircut = 0.02;
    eligibility.fx_haircut_addon = 1.01;
    assert_serialization_rejects(&eligibility, "fx_haircut_addon");
    eligibility.fx_haircut_addon = 0.08;
    eligibility.concentration_limit = Some(-0.01);
    assert_serialization_rejects(&eligibility, "concentration_limit");

    let schedule = EligibleCollateralSchedule {
        default_haircut: Some(1.01),
        ..EligibleCollateralSchedule::default()
    };
    assert_serialization_rejects(&schedule, "default_haircut");

    let mut im =
        ImParameters::simm_standard(Currency::USD).expect("embedded margin registry should load");
    im.mpor_days = 0;
    assert_serialization_rejects(&im, "mpor_days");

    let timing = MarginCallTiming {
        notification_deadline_hours: 24,
        response_deadline_hours: 2,
        dispute_resolution_days: 1,
        delivery_grace_days: 1,
    };
    assert_serialization_rejects(&timing, "notification_deadline_hours");
}

#[test]
fn margin_schema_rejects_invalid_versions_shapes_and_nested_values() {
    let schema = margin_schema();
    let csa = CsaSpec::usd_regulatory().expect("embedded margin registry should load");
    let invalid_fixtures = [
        json!({
            "schema": "finstack_quant.margin/2",
            "csa_spec": csa,
        }),
        json!({
            "schema": "finstack_quant.margin/1",
            "otc_margin_spec": {
                "settlement_lag": 1
            },
        }),
        json!({
            "schema": "finstack_quant.margin/1",
            "margin_call": {
                "call_date": "2026-07-30",
                "settlement_date": "2026-07-31",
                "call_type": "UnknownCallType",
                "amount": {"amount": "1000000", "currency": "USD"},
                "mtm_trigger": {"amount": "1250000", "currency": "USD"},
                "threshold": {"amount": "0", "currency": "USD"},
                "mta_applied": {"amount": "250000", "currency": "USD"}
            },
        }),
    ];

    for fixture in invalid_fixtures {
        assert!(
            validate_fixture(&schema, &fixture).is_err(),
            "invalid fixture was accepted: {fixture}"
        );
    }
}

#[test]
fn margin_schema_and_serde_reject_non_positive_mpor() {
    let schema = margin_schema();
    let mut fixture = csa_fixture();
    fixture["csa_spec"]["im_params"]["mpor_days"] = json!(0);

    assert_csa_fixtures_rejected_by_schema_and_serde(
        &schema,
        vec![("zero mpor_days", "mpor_days", fixture)],
    );
}

#[test]
fn margin_schema_rejects_non_iso_margin_call_dates() {
    let schema = margin_schema();
    let mut invalid_call_date = margin_call_fixture();
    invalid_call_date["margin_call"]["call_date"] = json!("not-a-date");
    let mut invalid_settlement_date = margin_call_fixture();
    invalid_settlement_date["margin_call"]["settlement_date"] = json!("07/31/2026");

    assert_fixtures_rejected(
        &schema,
        vec![
            ("non-ISO call_date", invalid_call_date),
            ("non-ISO settlement_date", invalid_settlement_date),
        ],
    );
}

#[test]
fn margin_schema_and_serde_reject_out_of_range_collateral_ratios() {
    let schema = margin_schema();
    let mut negative_haircut = csa_fixture();
    negative_haircut["csa_spec"]["eligible_collateral"]["eligible"][0]["haircut"] = json!(-0.01);
    let mut excessive_haircut = csa_fixture();
    excessive_haircut["csa_spec"]["eligible_collateral"]["eligible"][0]["haircut"] = json!(1.01);
    let mut negative_fx_addon = csa_fixture();
    negative_fx_addon["csa_spec"]["eligible_collateral"]["eligible"][0]["fx_haircut_addon"] =
        json!(-0.01);
    let mut excessive_fx_addon = csa_fixture();
    excessive_fx_addon["csa_spec"]["eligible_collateral"]["eligible"][0]["fx_haircut_addon"] =
        json!(1.01);
    let mut negative_concentration = csa_fixture();
    negative_concentration["csa_spec"]["eligible_collateral"]["eligible"][0]
        ["concentration_limit"] = json!(-0.01);
    let mut excessive_concentration = csa_fixture();
    excessive_concentration["csa_spec"]["eligible_collateral"]["eligible"][0]
        ["concentration_limit"] = json!(1.01);
    let mut negative_default_haircut = csa_fixture();
    negative_default_haircut["csa_spec"]["eligible_collateral"]["default_haircut"] = json!(-0.01);
    let mut excessive_default_haircut = csa_fixture();
    excessive_default_haircut["csa_spec"]["eligible_collateral"]["default_haircut"] = json!(1.01);

    assert_csa_fixtures_rejected_by_schema_and_serde(
        &schema,
        vec![
            ("negative haircut", "haircut", negative_haircut),
            ("excessive haircut", "haircut", excessive_haircut),
            (
                "negative FX haircut add-on",
                "fx_haircut_addon",
                negative_fx_addon,
            ),
            (
                "excessive FX haircut add-on",
                "fx_haircut_addon",
                excessive_fx_addon,
            ),
            (
                "negative concentration limit",
                "concentration_limit",
                negative_concentration,
            ),
            (
                "excessive concentration limit",
                "concentration_limit",
                excessive_concentration,
            ),
            (
                "negative default haircut",
                "default_haircut",
                negative_default_haircut,
            ),
            (
                "excessive default haircut",
                "default_haircut",
                excessive_default_haircut,
            ),
        ],
    );
}

#[test]
fn margin_schema_and_serde_reject_negative_maturity_years_and_late_notification_hour() {
    let schema = margin_schema();
    let mut negative_minimum_maturity = csa_fixture();
    negative_minimum_maturity["csa_spec"]["eligible_collateral"]["eligible"][0]
        ["maturity_constraints"] = json!({"min_remaining_years": -0.01});
    let mut negative_maximum_maturity = csa_fixture();
    negative_maximum_maturity["csa_spec"]["eligible_collateral"]["eligible"][0]
        ["maturity_constraints"] = json!({"max_remaining_years": -0.01});
    let mut invalid_notification_hour = csa_fixture();
    invalid_notification_hour["csa_spec"]["call_timing"]["notification_deadline_hours"] = json!(24);

    assert_csa_fixtures_rejected_by_schema_and_serde(
        &schema,
        vec![
            (
                "negative minimum maturity",
                "min_remaining_years",
                negative_minimum_maturity,
            ),
            (
                "negative maximum maturity",
                "max_remaining_years",
                negative_maximum_maturity,
            ),
            (
                "notification hour after 23:00",
                "notification_deadline_hours",
                invalid_notification_hour,
            ),
        ],
    );
}
