//! Tests for the Rust functions that replaced logic the host bindings used to
//! carry themselves (labels, checked accessors and row views). Python and
//! WebAssembly both call these, so each case pins the one behaviour the two
//! hosts share.

use finstack_quant_core::ErrorKind;
use finstack_quant_factor::credit::hierarchy::{dimension_key, HierarchyDimension};
use finstack_quant_factor::credit::VolHorizon;
use finstack_quant_factor::risk::parametric_var_decomposition;
use finstack_quant_factor::{FactorCovarianceMatrix, FactorId};

#[test]
fn vol_horizon_descriptor_round_trips_through_parse() {
    for horizon in [
        VolHorizon::OneStep,
        VolHorizon::Unconditional,
        VolHorizon::NSteps(5),
        VolHorizon::years(0.25).expect("valid years"),
    ] {
        assert_eq!(
            VolHorizon::parse(&horizon.descriptor()).expect("descriptor parses"),
            horizon
        );
    }
    assert_eq!(VolHorizon::NSteps(5).kind(), "n_steps");
    assert_eq!(VolHorizon::OneStep.kind(), "one_step");
    assert_eq!(VolHorizon::Unconditional.kind(), "unconditional");
    assert_eq!(VolHorizon::Years(1.0).kind(), "years");
    assert!(VolHorizon::years(-1.0).is_err());
    assert!(VolHorizon::years(f64::NAN).is_err());
}

#[test]
fn hierarchy_level_names_are_the_serde_dimension_keys() {
    for (dim, key) in [
        (HierarchyDimension::Rating, "rating"),
        (HierarchyDimension::Region, "region"),
        (HierarchyDimension::Sector, "sector"),
    ] {
        assert_eq!(dimension_key(&dim), key);
        assert_eq!(
            serde_json::to_value(&dim).expect("serialize"),
            serde_json::json!(key)
        );
    }
    assert_eq!(
        dimension_key(&HierarchyDimension::Custom("Currency".into())),
        "Currency"
    );
}

#[test]
fn factor_covariance_rows_follow_factor_order() {
    let matrix = FactorCovarianceMatrix::new(
        vec![FactorId::new("rates"), FactorId::new("credit")],
        vec![0.04, 0.01, 0.01, 0.09],
    )
    .expect("valid covariance");
    assert_eq!(matrix.to_rows(), vec![vec![0.04, 0.01], vec![0.01, 0.09]]);
}

#[test]
fn component_var_lookup_fails_with_not_found_for_an_unknown_position() {
    let ids = vec!["A".to_string(), "B".to_string()];
    let decomposition = parametric_var_decomposition(
        &ids,
        &[1.0, 2.0],
        &[0.04, 0.01, 0.01, 0.09],
        Some(0.95),
        false,
    )
    .expect("valid inputs");

    let known = decomposition.component_var("A").expect("known id");
    assert_eq!(known, decomposition.var_contributions[0].component_var);
    assert_eq!(decomposition.var_contributions[0].position_id, "A");
    let err = decomposition
        .component_var("missing")
        .expect_err("unknown id");
    assert_eq!(err.kind(), ErrorKind::NotFound);
}

#[test]
fn issuer_beta_policy_kind_names_the_variant_without_its_payload() {
    use finstack_quant_factor::credit::hierarchy::IssuerBetaPolicy;
    let dynamic = IssuerBetaPolicy::Dynamic {
        min_history: 12,
        overrides: std::collections::BTreeMap::new(),
    };
    assert_eq!(dynamic.kind(), "dynamic");
    assert_eq!(IssuerBetaPolicy::GloballyOff.kind(), "globally_off");
    // The label is the serde tag of each variant.
    let json = serde_json::to_value(&dynamic).expect("serialize");
    assert!(json.get("dynamic").is_some());
    assert_eq!(
        serde_json::to_value(IssuerBetaPolicy::GloballyOff).expect("serialize"),
        serde_json::json!("globally_off")
    );
}
