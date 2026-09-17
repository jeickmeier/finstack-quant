use std::cell::Cell;
use std::fs;
use std::path::Path;

use finstack_quant_core::contract::LoadLimits;
use finstack_quant_core::market_data::context::{MarketContext, MarketContextState};
use finstack_quant_core::{
    canonical_bytes_of_value, content_hash, to_canonical_bytes, Error, InputError,
    CANONICAL_VERSION,
};
use indexmap::IndexMap;
use serde::Serialize;
use serde_json::json;
use serde_json::value::RawValue;

#[test]
fn canonical_bytes_sort_map_keys_recursively_without_reordering_arrays() {
    let mut nested_first = IndexMap::new();
    nested_first.insert("z", 2);
    nested_first.insert("a", 1);
    let mut first = IndexMap::new();
    first.insert("outer", nested_first);
    first.insert("array", IndexMap::from([("second", 2), ("first", 1)]));

    let mut nested_second = IndexMap::new();
    nested_second.insert("a", 1);
    nested_second.insert("z", 2);
    let mut second = IndexMap::new();
    second.insert("array", IndexMap::from([("first", 1), ("second", 2)]));
    second.insert("outer", nested_second);

    let first_bytes = to_canonical_bytes(&first).unwrap();
    let second_bytes = to_canonical_bytes(&second).unwrap();

    assert_eq!(first_bytes, second_bytes);
    assert_eq!(
        first_bytes,
        br#"{"array":{"first":1,"second":2},"outer":{"a":1,"z":2}}"#
    );

    let value = json!({"items": [3, 1, 2], "z": 0, "a": 1});
    assert_eq!(
        canonical_bytes_of_value(&value).unwrap(),
        br#"{"a":1,"items":[3,1,2],"z":0}"#
    );
}

#[test]
fn canonical_bytes_use_compact_serde_json_number_and_string_forms() {
    let value = json!({
        "unicode": "é",
        "float": 1.25,
        "integer": 9007199254740991_i64,
        "escaped": "line\nbreak"
    });

    assert_eq!(
        canonical_bytes_of_value(&value).unwrap(),
        r#"{"escaped":"line\nbreak","float":1.25,"integer":9007199254740991,"unicode":"é"}"#
            .as_bytes()
    );
}

#[derive(Serialize)]
struct NestedFloat {
    values: Vec<f64>,
}

struct FiniteThenNan {
    serializations: Cell<usize>,
}

impl Serialize for FiniteThenNan {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let serializations = self.serializations.get();
        self.serializations.set(serializations + 1);
        if serializations == 0 {
            serializer.serialize_f64(1.0)
        } else {
            serializer.serialize_f64(f64::NAN)
        }
    }
}

#[derive(Serialize)]
struct RawValueContainer<'a> {
    embedded: &'a RawValue,
    stateful: &'a FiniteThenNan,
}

#[test]
fn canonicalization_embeds_nested_raw_json_as_its_public_value() {
    let embedded = RawValue::from_string(r#"{"z":2,"a":{"y":1,"x":0}}"#.to_string())
        .expect("raw JSON is valid");
    let stateful = FiniteThenNan {
        serializations: Cell::new(0),
    };

    let canonical = to_canonical_bytes(&RawValueContainer {
        embedded: &embedded,
        stateful: &stateful,
    })
    .expect("raw JSON canonicalizes");

    assert_eq!(
        canonical,
        br#"{"embedded":{"a":{"x":0,"y":1},"z":2},"stateful":1.0}"#
    );
    assert!(!canonical
        .windows(b"$serde_json".len())
        .any(|window| window == b"$serde_json"));
    assert_eq!(stateful.serializations.get(), 1);
}

#[test]
fn raw_value_support_does_not_weaken_nested_non_finite_rejection() {
    #[derive(Serialize)]
    struct Container<'a> {
        embedded: &'a RawValue,
        invalid: NestedFloat,
    }

    let embedded =
        RawValue::from_string(r#"{"finite":1.0}"#.to_string()).expect("raw JSON is valid");
    let error = to_canonical_bytes(&Container {
        embedded: &embedded,
        invalid: NestedFloat {
            values: vec![f64::INFINITY],
        },
    })
    .expect_err("non-finite sibling must still fail");

    assert!(matches!(
        error,
        Error::Input(InputError::NonFiniteValue { .. })
    ));
}

#[test]
fn canonicalization_rejects_non_finite_floats_in_nested_values() {
    for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let error = to_canonical_bytes(&NestedFloat {
            values: vec![1.0, value],
        })
        .unwrap_err();

        assert!(matches!(
            error,
            Error::Input(InputError::NonFiniteValue { .. })
        ));
    }
}

#[test]
fn canonicalization_uses_the_single_validated_serialization_result() {
    let value = FiniteThenNan {
        serializations: Cell::new(0),
    };

    let bytes = to_canonical_bytes(&value).unwrap();

    assert_eq!(bytes, b"1.0");
    assert_eq!(value.serializations.get(), 1);
}

#[test]
fn content_hash_has_a_stable_versioned_sha256_fixture() {
    let fixture = json!({"b": [3, 2, 1], "a": "value"});

    assert_eq!(CANONICAL_VERSION, "c1");
    assert_eq!(
        content_hash(&fixture).unwrap(),
        "sha256:74915f5f41757ee0504d9b559beabd604bc9a4f92c87a0cbc2119db46fed727d"
    );
}

#[test]
fn extension_metadata_participates_in_content_hashes() {
    let first = json!({"id": "A", "meta": {"source": "desk-a"}});
    let second = json!({"id": "A", "meta": {"source": "desk-b"}});

    assert_ne!(
        content_hash(&first).unwrap(),
        content_hash(&second).unwrap()
    );
}

fn market_state_with_tags(tags: &str) -> (Vec<u8>, MarketContextState) {
    let json = format!(
        r#"{{
            "schema_version": 1,
            "curves": [],
            "fx": null,
            "surfaces": [],
            "prices": {{}},
            "series": [],
            "inflation_indices": [],
            "dividends": [],
            "credit_indices": [],
            "fx_delta_vol_surfaces": [],
            "vol_cubes": [],
            "collateral": {{}},
            "hierarchy": {{
                "roots": {{
                    "Rates": {{
                        "tags": {tags},
                        "children": {{}},
                        "curves": []
                    }}
                }}
            }}
        }}"#
    );
    let bytes = json.into_bytes();
    MarketContext::from_state_slice(&bytes, &LoadLimits::default()).expect("market state is valid");
    let state = serde_json::from_slice(&bytes).expect("market state deserializes");
    (bytes, state)
}

#[test]
fn market_context_state_has_golden_canonical_bytes_hash_and_order_invariance() {
    let (_, first) = market_state_with_tags(r#"{"desk":"rates","region":"us"}"#);
    let (_, second) = market_state_with_tags(r#"{"region":"us","desk":"rates"}"#);

    let canonical = to_canonical_bytes(&first).expect("market state canonicalizes");
    let fixture = include_bytes!("../data/canonical/market_context_state.json");
    let fixture = fixture.strip_suffix(b"\n").unwrap_or(fixture);
    assert_eq!(canonical, fixture);
    assert_eq!(
        first.content_hash().expect("market state hashes"),
        include_str!("../data/canonical/market_context_state.sha256").trim()
    );
    assert_eq!(
        canonical,
        to_canonical_bytes(&second).expect("reverse-order state canonicalizes")
    );
}

fn persisted_struct_block<'a>(source: &'a str, type_name: &str) -> &'a str {
    let marker = format!("pub struct {type_name}");
    let start = source
        .find(&marker)
        .unwrap_or_else(|| panic!("missing persisted type {type_name}"));
    let tail = &source[start..];
    let end = tail
        .find("\n}")
        .unwrap_or_else(|| panic!("unterminated persisted type {type_name}"));
    &tail[..=end]
}

#[test]
fn persisted_contract_fields_do_not_use_fx_hash_maps() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("core crate is inside workspace");
    let checks = [
        ("core/src/market_data/hierarchy/mod.rs", "HierarchyNode"),
        ("models/src/credit/rating_factors.rs", "RatingFactorTable"),
        (
            "models/src/volatility/arbitrage/types.rs",
            "ArbitrageReport",
        ),
        (
            "core/src/market_data/term_structures/credit_index.rs",
            "CreditIndexData",
        ),
        ("calibration/src/api/schema.rs", "CalibrationPlan"),
        (
            "valuations/src/instruments/fixed_income/structured_credit/types/mod.rs",
            "MarketConditions",
        ),
        (
            "valuations/src/instruments/fixed_income/structured_credit/types/mod.rs",
            "CreditFactors",
        ),
        (
            "valuations/src/instruments/fixed_income/structured_credit/types/results.rs",
            "TrancheValuation",
        ),
        (
            "valuations/src/instruments/fixed_income/structured_credit/types/waterfall.rs",
            "WaterfallDistribution",
        ),
        (
            "valuations/src/instruments/fixed_income/structured_credit/types/waterfall.rs",
            "CoverageRules",
        ),
        ("margin/src/regulatory/sa_ccr/types.rs", "EadResult"),
        ("margin/src/regulatory/frtb/types.rs", "FrtbSbaResult"),
    ];

    let mut violations = Vec::new();
    for (relative, type_name) in checks {
        let source = fs::read_to_string(workspace.join(relative)).expect("source is readable");
        let block = persisted_struct_block(&source, type_name);
        if block.contains("HashMap<") {
            violations.push(format!("{relative}:{type_name}"));
        }
    }

    assert!(
        violations.is_empty(),
        "serialized FxHashMap fields remain in persisted contract types: {violations:?}"
    );
}
