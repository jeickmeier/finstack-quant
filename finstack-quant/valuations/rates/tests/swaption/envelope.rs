//! `BermudanSwaption::from_envelope_json`: the single-variant envelope parse
//! that calibration uses instead of the umbrella's `InstrumentJson` union.

use finstack_quant_valuations_rates::swaption::BermudanSwaption;
use serde_json::json;

fn envelope(type_tag: &str, spec: serde_json::Value) -> String {
    json!({
        "schema": "finstack_quant.instrument/1",
        "instrument": { "type": type_tag, "spec": spec },
    })
    .to_string()
}

#[test]
fn parses_the_canonical_bermudan_envelope() {
    let example = BermudanSwaption::example().expect("example");
    let spec = serde_json::to_value(&example).expect("serialize");
    let parsed = BermudanSwaption::from_envelope_json(&envelope("bermudan_swaption", spec.clone()))
        .expect("envelope parses");
    assert_eq!(parsed.id, example.id);
    assert_eq!(serde_json::to_value(&parsed).expect("serialize"), spec);
}

#[test]
fn rejects_another_instrument_type_by_name() {
    let error = BermudanSwaption::from_envelope_json(&envelope("bond", json!({})))
        .expect_err("a bond envelope is not a Bermudan swaption")
        .to_string();
    assert!(
        error.contains("bermudan_swaption") && error.contains("bond"),
        "{error}"
    );
}

#[test]
fn rejects_a_foreign_schema_marker_and_unknown_fields() {
    let example = BermudanSwaption::example().expect("example");
    let spec = serde_json::to_value(&example).expect("serialize");
    let wrong_marker = json!({
        "schema": "finstack_quant.instrument/2",
        "instrument": { "type": "bermudan_swaption", "spec": spec },
    })
    .to_string();
    assert!(BermudanSwaption::from_envelope_json(&wrong_marker).is_err());
    let extra_field = json!({
        "schema": "finstack_quant.instrument/1",
        "instrument": { "type": "bermudan_swaption", "spec": spec, "extra": 1 },
    })
    .to_string();
    assert!(BermudanSwaption::from_envelope_json(&extra_field).is_err());
}
