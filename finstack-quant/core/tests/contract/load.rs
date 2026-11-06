use finstack_quant_core::contract::{ContractError, LoadLimits, check_json_limits};

#[test]
fn lexical_depth_scan_counts_objects_and_arrays() {
    let limits = LoadLimits::default().with_max_depth(3);
    check_json_limits(br#"{"items":[{"value":1}]}"#, &limits).expect("depth three is accepted");

    let error = check_json_limits(br#"{"items":[[{"value":1}]]}"#, &limits)
        .expect_err("depth four must be rejected");
    assert!(matches!(
        error,
        ContractError::LimitExceeded {
            what: "JSON depth",
            found: 4,
            limit: 3,
        }
    ));
}

#[test]
fn lexical_depth_scan_ignores_delimiters_inside_strings_and_escapes() {
    let document = br#"{"text":"[{\"nested\":\"}\",\"slash\":\"\\\\[\"}]","value":[]}"#;
    check_json_limits(document, &LoadLimits::default().with_max_depth(2))
        .expect("quoted and escaped delimiters do not increase depth");
}

#[test]
fn lexical_depth_scan_leaves_json_syntax_validation_to_deserializer() {
    for malformed in [
        br#"{"unterminated":"[{"#.as_slice(),
        br#"{"mismatched":[}"#.as_slice(),
        br#"}{"#.as_slice(),
    ] {
        check_json_limits(malformed, &LoadLimits::default().with_max_depth(8))
            .expect("the lexical limit pass does not validate JSON syntax");
        assert!(serde_json::from_slice::<serde_json::Value>(malformed).is_err());
    }
}
