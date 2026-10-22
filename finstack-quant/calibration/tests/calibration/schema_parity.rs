//! Calibration JSON-schema parity checks.

use serde_json::Value;

const CALIBRATION_STEP_KINDS: &[&str] = &[
    "base_correlation",
    "cap_floor_hull_white",
    "discount",
    "forward",
    "hazard",
    "hull_white",
    "inflation",
    "parametric",
    "student_t",
    "svi_surface",
    "swaption_vol",
    "vol_surface",
    "xccy_basis",
];

fn tagged_discriminators(schema: &Value) -> Vec<&str> {
    schema
        .get("oneOf")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|variant| {
            variant
                .pointer("/properties/kind/const")
                .and_then(Value::as_str)
        })
        .collect()
}

#[test]
fn calibration_step_kinds_match_checked_in_schema() {
    let schema: Value = serde_json::from_str(include_str!(
        "../../schemas/calibration/1/calibration.schema.json"
    ))
    .expect("calibration schema JSON");
    let step_params = schema
        .pointer("/$defs/StepParams")
        .or_else(|| schema.pointer("/$defs/CalibrationStep"))
        .expect("StepParams schema definition");
    let mut actual = tagged_discriminators(step_params);
    let mut expected = CALIBRATION_STEP_KINDS.to_vec();
    actual.sort_unstable();
    expected.sort_unstable();
    assert_eq!(actual, expected);
}

#[test]
#[cfg(feature = "json-schema")]
fn published_quote_examples_pass_domain_validation() {
    use finstack_quant_calibration::{json_schema, quotes::MarketQuote};
    for (file, class) in [
        ("rate_quote", "rates"),
        ("cds_quote", "cds"),
        ("cds_tranche_quote", "cds_tranche"),
        ("inflation_quote", "inflation"),
        ("vol_quote", "vol"),
        ("xccy_quote", "xccy"),
    ] {
        let path = format!("schemas/market/1/{file}.schema.json");
        let artifact = json_schema::artifacts()
            .iter()
            .find(|artifact| artifact.relative_path == path)
            .expect("published quote family");
        let schema = artifact.generate().expect("quote schema");
        let examples = schema["examples"].as_array().expect("quote examples");
        assert!(!examples.is_empty(), "{file}");
        for example in examples {
            let mut tagged = example.clone();
            tagged["class"] = Value::String(class.to_string());
            let quote: MarketQuote = serde_json::from_value(tagged).expect("typed quote example");
            quote
                .validate()
                .unwrap_or_else(|error| panic!("{file}: {error}"));
            if class == "cds_tranche" {
                assert_eq!(
                    example["upfront_pct"], 0.0425,
                    "4.25% is a decimal fraction"
                );
            }
        }
    }
}
