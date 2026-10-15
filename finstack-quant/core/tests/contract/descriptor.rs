use finstack_quant_core::contract::{
    ContractDescriptor, ContractError, LoadLimits, DEFAULT_MAX_ARTIFACTS, DEFAULT_MAX_BYTES,
    DEFAULT_MAX_DEPTH, DEFAULT_MAX_DIAGNOSTICS, DEFAULT_MAX_POSITIONS,
};

#[test]
fn descriptor_defaults_to_a_strict_current_version() {
    let descriptor = ContractDescriptor::new("finstack_quant.instrument");

    assert_eq!(descriptor.id, "finstack_quant.instrument");
    assert_eq!(ContractDescriptor::VERSION, 1);
    assert_eq!(descriptor.schema_string(), "finstack_quant.instrument/1");
}

#[test]
fn descriptor_parses_only_its_exact_supported_schema() {
    let descriptor = ContractDescriptor::new("finstack_quant.instrument");

    assert_eq!(
        descriptor
            .parse_schema("finstack_quant.instrument/1")
            .unwrap(),
        1
    );
}

#[test]
fn descriptor_rejects_malformed_alias_zero_and_future_schemas() {
    let descriptor = ContractDescriptor::new("finstack_quant.instrument");

    for malformed in [
        "",
        "finstack_quant.instrument",
        "finstack_quant.instrument/",
        "finstack_quant.instrument/not-a-version",
        "finstack_quant.instrument/1/extra",
        "finstack_quant.instruments/1",
    ] {
        assert!(
            matches!(
                descriptor.parse_schema(malformed),
                Err(ContractError::MalformedSchema { .. })
            ),
            "{malformed:?} should be malformed"
        );
    }

    for unsupported in ["finstack_quant.instrument/0", "finstack_quant.instrument/2"] {
        assert!(
            matches!(
                descriptor.parse_schema(unsupported),
                Err(ContractError::UnsupportedVersion {
                    contract,
                    found,
                    min: 1,
                    max: 1,
                }) if contract == "finstack_quant.instrument"
                    && found == unsupported.rsplit_once('/').unwrap().1.parse::<u32>().unwrap()
            ),
            "{unsupported:?} should be unsupported"
        );
    }
}

#[test]
fn descriptor_requires_an_explicit_v1_version() {
    let strict = ContractDescriptor::new("finstack_quant.instrument");
    assert!(matches!(
        strict.resolve(None),
        Err(ContractError::MissingVersion { contract })
            if contract == "finstack_quant.instrument"
    ));
    assert_eq!(strict.resolve(Some(1)).unwrap(), 1);
    assert!(matches!(
        strict.resolve(Some(2)),
        Err(ContractError::UnsupportedVersion {
            found: 2,
            min: 1,
            max: 1,
            ..
        })
    ));
}

#[test]
fn load_limits_defaults_and_builders_are_explicit() {
    let defaults = LoadLimits::default();
    assert_eq!(defaults.max_bytes, DEFAULT_MAX_BYTES);
    assert_eq!(defaults.max_artifacts, DEFAULT_MAX_ARTIFACTS);
    assert_eq!(defaults.max_positions, DEFAULT_MAX_POSITIONS);
    assert_eq!(defaults.max_depth, DEFAULT_MAX_DEPTH);
    assert_eq!(defaults.max_diagnostics, DEFAULT_MAX_DIAGNOSTICS);

    let custom = LoadLimits::default()
        .with_max_bytes(1)
        .with_max_artifacts(2)
        .with_max_positions(3)
        .with_max_depth(4)
        .with_max_diagnostics(5);
    assert_eq!(custom.max_bytes, 1);
    assert_eq!(custom.max_artifacts, 2);
    assert_eq!(custom.max_positions, 3);
    assert_eq!(custom.max_depth, 4);
    assert_eq!(custom.max_diagnostics, 5);
}
