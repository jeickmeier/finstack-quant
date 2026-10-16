use super::*;
use rust_decimal::Decimal;
use serde_json::json;

#[derive(serde::Serialize, serde::Deserialize, Debug)]
struct NonFiniteHolder {
    #[serde(with = "super::non_finite_f64")]
    value: f64,
}

#[test]
fn non_finite_f64_round_trips_infinities() {
    // serde_json writes these as `null` and then refuses to read `null`
    // back as an f64, so a ratio documented to reach `+inf` used to lose
    // its value on to_json/from_json.
    for (value, expected_json) in [
        (f64::INFINITY, json!({"value": "inf"})),
        (f64::NEG_INFINITY, json!({"value": "-inf"})),
    ] {
        let holder = NonFiniteHolder { value };
        let encoded = serde_json::to_value(&holder).expect("serialize");
        assert_eq!(encoded, expected_json);
        let decoded: NonFiniteHolder = serde_json::from_value(encoded).expect("deserialize");
        assert_eq!(decoded.value, value);
    }
}

#[test]
fn non_finite_f64_round_trips_nan_and_finite() {
    let nan: NonFiniteHolder =
        serde_json::from_value(serde_json::to_value(NonFiniteHolder { value: f64::NAN }).unwrap())
            .expect("nan");
    assert!(nan.value.is_nan());

    let finite: NonFiniteHolder = serde_json::from_value(json!({"value": 1.5})).expect("finite");
    assert_eq!(finite.value, 1.5);
    // Finite values stay ordinary JSON numbers, so the wire format is
    // unchanged for every payload that never hit a zero denominator.
    assert_eq!(
        serde_json::to_value(NonFiniteHolder { value: 1.5 }).unwrap(),
        json!({"value": 1.5})
    );
}

#[test]
fn non_finite_f64_rejects_null() {
    // `null` is not a value this adapter ever emits; accepting it would
    // silently turn a missing field into NaN.
    assert!(serde_json::from_value::<NonFiniteHolder>(json!({"value": null})).is_err());
}

#[test]
fn non_finite_f64_rejects_unknown_sentinel() {
    assert!(serde_json::from_value::<NonFiniteHolder>(json!({"value": "huge"})).is_err());
}

#[test]
fn parse_sentinel_inverts_serialize() {
    for value in [f64::INFINITY, f64::NEG_INFINITY] {
        let encoded = serde_json::to_value(NonFiniteHolder { value }).expect("serialize");
        let text = encoded["value"].as_str().expect("sentinel string");
        assert_eq!(non_finite_f64::parse_sentinel(text), Some(value));
    }
    assert!(non_finite_f64::parse_sentinel(" NaN ").is_some_and(f64::is_nan));
    assert_eq!(non_finite_f64::parse_sentinel("huge"), None);
}

#[test]
fn date_schema_has_date_format() {
    let schema = serde_json::to_value(schemars::schema_for!(DateWire)).expect("schema");
    assert_eq!(schema["type"], "string");
    assert_eq!(schema["format"], "date");
}

#[test]
fn non_finite_wire_schema_lists_exactly_the_serialized_sentinels() {
    let schema = serde_json::to_value(schemars::schema_for!(NonFiniteF64Wire)).expect("schema");
    let sentinel = &schema["$defs"]["NonFiniteSentinel"];
    let mut spellings: Vec<String> = sentinel["oneOf"]
        .as_array()
        .map(|variants| {
            variants
                .iter()
                .map(|variant| variant["const"].as_str().expect("const").to_string())
                .collect()
        })
        .or_else(|| {
            sentinel["enum"].as_array().map(|values| {
                values
                    .iter()
                    .map(|value| value.as_str().expect("string").to_string())
                    .collect()
            })
        })
        .expect("sentinel spellings");
    spellings.sort();
    assert_eq!(spellings, ["-inf", "inf", "nan"]);
    for value in [f64::INFINITY, f64::NEG_INFINITY, f64::NAN] {
        let wire = serde_json::to_value(NonFiniteF64Wire::from(value)).expect("serialize");
        let adapter = serde_json::to_value(NonFiniteHolder { value }).expect("serialize");
        assert_eq!(adapter["value"], wire, "adapter and wire type agree");
        assert!(spellings.iter().any(|spelling| wire == json!(spelling)));
    }
    assert_eq!(
        serde_json::to_value(NonFiniteF64Wire::from(1.5)).expect("serialize"),
        json!(1.5)
    );
}

#[test]
fn decimal_wire_is_string_only() {
    let decimal: DecimalWire = serde_json::from_value(json!("-1.25e+2")).expect("decimal");
    assert_eq!(decimal.0, Decimal::new(-125, 0));
    assert!(serde_json::from_value::<DecimalWire>(json!(1.25)).is_err());

    let schema = serde_json::to_value(schemars::schema_for!(DecimalWire)).expect("schema");
    assert_eq!(schema["type"], "string");
    assert_eq!(schema["pattern"], r"^-?\d+(\.\d+)?([eE][+-]?\d+)?$");
}

#[test]
fn schema_version_accepts_only_numeric_one() {
    assert_eq!(
        serde_json::from_value::<SchemaVersion>(json!(1)).expect("v1"),
        SchemaVersion::CURRENT
    );
    assert!(serde_json::from_value::<SchemaVersion>(json!(0)).is_err());
    assert!(serde_json::from_value::<SchemaVersion>(json!(2)).is_err());
    assert!(serde_json::from_value::<SchemaVersion>(json!("1")).is_err());
    assert_eq!(
        serde_json::to_value(SchemaVersion::CURRENT).expect("serialize v1"),
        json!(1)
    );

    let schema = serde_json::to_value(schemars::schema_for!(SchemaVersion)).expect("schema");
    assert_eq!(schema["minimum"], 1);
    assert_eq!(schema["maximum"], 1);
}

#[test]
fn bounded_numeric_wires_match_runtime_contracts() {
    for invalid in [json!(0.0), json!(-1.0)] {
        assert!(serde_json::from_value::<PositiveF64Wire>(invalid).is_err());
    }
    assert_eq!(
        serde_json::from_value::<PositiveF64Wire>(json!(0.5))
            .expect("positive")
            .into_inner(),
        0.5
    );

    for invalid in [json!(0.0), json!(1.0), json!(-0.1), json!(1.1)] {
        assert!(serde_json::from_value::<OpenUnitIntervalF64Wire>(invalid).is_err());
    }
    for invalid in [json!(-0.1), json!(1.1)] {
        assert!(serde_json::from_value::<ClosedUnitIntervalF64Wire>(invalid).is_err());
    }
    assert!(serde_json::from_value::<NonNegativeF64Wire>(json!(-0.1)).is_err());
    for invalid in [json!(-1.1), json!(1.1)] {
        assert!(serde_json::from_value::<CorrelationWire>(invalid).is_err());
    }
    for invalid in [json!(-100.1), json!(100.1)] {
        assert!(serde_json::from_value::<PercentageQuantityWire>(invalid).is_err());
    }

    let positive =
        serde_json::to_value(schemars::schema_for!(PositiveF64Wire)).expect("positive schema");
    assert_eq!(positive["exclusiveMinimum"], 0.0);
    let probability = serde_json::to_value(schemars::schema_for!(OpenUnitIntervalF64Wire))
        .expect("probability schema");
    assert_eq!(probability["exclusiveMinimum"], 0.0);
    assert_eq!(probability["exclusiveMaximum"], 1.0);
    let closed_probability = serde_json::to_value(schemars::schema_for!(ClosedUnitIntervalF64Wire))
        .expect("closed probability schema");
    assert_eq!(closed_probability["minimum"], 0.0);
    assert_eq!(closed_probability["maximum"], 1.0);
    let non_negative = serde_json::to_value(schemars::schema_for!(NonNegativeF64Wire))
        .expect("non-negative schema");
    assert_eq!(non_negative["minimum"], 0.0);
    let correlation =
        serde_json::to_value(schemars::schema_for!(CorrelationWire)).expect("correlation schema");
    assert_eq!(correlation["minimum"], -1.0);
    assert_eq!(correlation["maximum"], 1.0);
    let percentage = serde_json::to_value(schemars::schema_for!(PercentageQuantityWire))
        .expect("percentage schema");
    assert_eq!(percentage["minimum"], -100.0);
    assert_eq!(percentage["maximum"], 100.0);
}

#[test]
fn serde_tag_reads_unit_and_data_variants() {
    #[derive(Serialize)]
    #[serde(rename_all = "snake_case")]
    enum Shape {
        Constant,
        InversePower { exponent: f64 },
    }

    assert_eq!(serde_tag(&Shape::Constant).expect("unit tag"), "constant");
    assert_eq!(
        serde_tag(&Shape::InversePower { exponent: 2.0 }).expect("data tag"),
        "inverse_power"
    );
    assert_eq!(serde_label(&Shape::Constant).expect("label"), "constant");
    assert!(serde_label(&Shape::InversePower { exponent: 2.0 }).is_err());
}

#[test]
fn serde_tag_rejects_untagged_forms() {
    assert!(serde_tag(&1.5_f64).is_err());
    assert!(serde_tag(&json!({})).is_err());
    assert!(serde_tag(&json!({"a": 1, "b": 2})).is_err());
}
