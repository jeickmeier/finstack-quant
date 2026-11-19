//! Tests for the surrounding crate component and its documented behavior.
//!
use finstack_quant_core::FxHashMap;
use std::sync::Arc;

use super::test_helpers::{sample_base_correlation_curve, sample_hazard_curve};
use finstack_quant_core::market_data::term_structures::{CreditIndexData, CreditIndexDataBuilder};

#[test]
fn credit_index_builder_validates_inputs() {
    let hazard = Arc::new(sample_hazard_curve("CDX"));
    let base_corr = Arc::new(sample_base_correlation_curve("CDX-BC"));

    let data = CreditIndexData::builder()
        .num_constituents(125)
        .recovery_rate(0.4)
        .index_credit_curve(Arc::clone(&hazard))
        .base_correlation_curve(Arc::clone(&base_corr))
        .build()
        .expect("valid credit index");

    assert_eq!(data.num_constituents(), 125);
    assert!(!data.has_issuer_curves());
    assert!(Arc::ptr_eq(data.index_credit_curve(), &hazard));
    assert!(Arc::ptr_eq(data.base_correlation_curve(), &base_corr));
}

#[test]
fn credit_index_builder_supports_issuer_curves() {
    let hazard = Arc::new(sample_hazard_curve("CDX"));
    let base_corr = Arc::new(sample_base_correlation_curve("CDX-BC"));

    let mut issuers: FxHashMap<String, Arc<_>> = FxHashMap::default();
    issuers.insert(
        "IssuerA".to_string(),
        Arc::new(sample_hazard_curve("IssuerA")),
    );

    let data = CreditIndexDataBuilder::default()
        .num_constituents(1)
        .recovery_rate(0.35)
        .index_credit_curve(hazard)
        .base_correlation_curve(base_corr)
        .issuer_curves(issuers.clone())
        .build()
        .expect("builder with issuer curves");

    assert!(data.has_issuer_curves());
    assert_eq!(data.issuer_ids(), vec!["IssuerA".to_string()]);

    // Test that we get the right curves - compare IDs since we can't directly compare references
    let issuer_curve = data.get_issuer_curve("IssuerA").unwrap();
    assert_eq!(issuer_curve.id().as_str(), "IssuerA");

    assert!(data.get_issuer_curve("Unknown").is_err());
}

#[test]
fn credit_index_builder_validates_bad_input() {
    let hazard = Arc::new(sample_hazard_curve("CDX"));
    let base_corr = Arc::new(sample_base_correlation_curve("CDX-BC"));

    let err = CreditIndexData::builder()
        .recovery_rate(-0.1)
        .index_credit_curve(hazard)
        .base_correlation_curve(base_corr)
        .build()
        .expect_err("invalid recovery should fail");
    assert!(
        matches!(err, finstack_quant_core::Error::Validation(_)),
        "{err}"
    );
}

// Additional credit index behavior

#[test]
fn test_credit_index_recovery_rate_scenarios() {
    // Test with various recovery rates: 0%, 40%, 100%
    let hazard = Arc::new(sample_hazard_curve("CDX"));
    let base_corr = Arc::new(sample_base_correlation_curve("CDX-BC"));

    let recovery_rates = [0.0, 0.4, 1.0];

    for rr in recovery_rates {
        let data = CreditIndexData::builder()
            .num_constituents(125)
            .recovery_rate(rr)
            .index_credit_curve(Arc::clone(&hazard))
            .base_correlation_curve(Arc::clone(&base_corr))
            .build()
            .expect("valid recovery rate");

        assert_eq!(data.recovery_rate(), rr);
    }
}

#[test]
fn test_credit_index_requires_explicit_recovery_rate() {
    let hazard = Arc::new(sample_hazard_curve("CDX"));
    let base_corr = Arc::new(sample_base_correlation_curve("CDX-BC"));

    let err = CreditIndexData::builder()
        .num_constituents(125)
        .index_credit_curve(hazard)
        .base_correlation_curve(base_corr)
        .build()
        .expect_err("missing recovery rate must fail");

    assert!(err.to_string().contains("explicit recovery_rate"));
}

#[test]
fn test_credit_index_recovery_rate_above_one_fails() {
    // Test that recovery rate > 1.0 fails validation
    let hazard = Arc::new(sample_hazard_curve("CDX"));
    let base_corr = Arc::new(sample_base_correlation_curve("CDX-BC"));

    let err = CreditIndexData::builder()
        .num_constituents(125)
        .recovery_rate(1.5) // Invalid: > 1.0
        .index_credit_curve(hazard)
        .base_correlation_curve(base_corr)
        .build()
        .expect_err("recovery rate > 1.0 should fail");

    assert!(matches!(err, finstack_quant_core::Error::Validation(_)));
    assert!(err.to_string().contains("recovery_rate"), "{err}");
}

#[test]
fn test_credit_index_zero_constituents_fails() {
    // Test that num_constituents = 0 fails validation
    let hazard = Arc::new(sample_hazard_curve("CDX"));
    let base_corr = Arc::new(sample_base_correlation_curve("CDX-BC"));

    let err = CreditIndexData::builder()
        .num_constituents(0) // Invalid: must be > 0
        .recovery_rate(0.4)
        .index_credit_curve(hazard)
        .base_correlation_curve(base_corr)
        .build()
        .expect_err("num_constituents = 0 should fail");

    assert!(
        matches!(err, finstack_quant_core::Error::Validation(_)),
        "{err}"
    );
}

#[test]
fn test_credit_index_missing_index_curve_fails() {
    // Test that missing index_credit_curve fails build()
    let base_corr = Arc::new(sample_base_correlation_curve("CDX-BC"));

    let err = CreditIndexData::builder()
        .num_constituents(125)
        .recovery_rate(0.4)
        // Missing: .index_credit_curve()
        .base_correlation_curve(base_corr)
        .build()
        .expect_err("missing index_credit_curve should fail");

    assert!(
        matches!(err, finstack_quant_core::Error::Validation(_)),
        "{err}"
    );
}

#[test]
fn test_credit_index_missing_base_correlation_fails() {
    // Test that missing base_correlation_curve fails build()
    let hazard = Arc::new(sample_hazard_curve("CDX"));

    let err = CreditIndexData::builder()
        .num_constituents(125)
        .recovery_rate(0.4)
        .index_credit_curve(hazard)
        // Missing: .base_correlation_curve()
        .build()
        .expect_err("missing base_correlation_curve should fail");

    assert!(
        matches!(err, finstack_quant_core::Error::Validation(_)),
        "{err}"
    );
}

#[test]
fn test_credit_index_missing_num_constituents_fails() {
    // Test that missing num_constituents fails build()
    let hazard = Arc::new(sample_hazard_curve("CDX"));
    let base_corr = Arc::new(sample_base_correlation_curve("CDX-BC"));

    let err = CreditIndexData::builder()
        // Missing: .num_constituents()
        .recovery_rate(0.4)
        .index_credit_curve(hazard)
        .base_correlation_curve(base_corr)
        .build()
        .expect_err("missing num_constituents should fail");

    assert!(
        matches!(err, finstack_quant_core::Error::Validation(_)),
        "{err}"
    );
}
#[test]
fn test_credit_index_issuer_recovery_rates() {
    // Test issuer-specific recovery rates
    let hazard = Arc::new(sample_hazard_curve("CDX"));
    let base_corr = Arc::new(sample_base_correlation_curve("CDX-BC"));
    let mut issuer_curves = FxHashMap::default();
    issuer_curves.insert(
        "IssuerA".to_string(),
        Arc::new(sample_hazard_curve("IssuerA")),
    );
    issuer_curves.insert(
        "IssuerB".to_string(),
        Arc::new(sample_hazard_curve("IssuerB")),
    );

    let mut recovery_rates = FxHashMap::default();
    recovery_rates.insert("IssuerA".to_string(), 0.30);
    recovery_rates.insert("IssuerB".to_string(), 0.50);

    let data = CreditIndexData::builder()
        .num_constituents(2)
        .recovery_rate(0.40) // Default recovery
        .index_credit_curve(hazard)
        .base_correlation_curve(base_corr)
        .issuer_curves(issuer_curves)
        .issuer_recovery_rates(recovery_rates)
        .build()
        .expect("issuer recovery rates should work");

    // Should return issuer-specific rates
    assert_eq!(data.get_issuer_recovery("IssuerA").unwrap(), 0.30);
    assert_eq!(data.get_issuer_recovery("IssuerB").unwrap(), 0.50);

    assert!(data.get_issuer_recovery("Unknown").is_err());
}

#[test]
fn test_credit_index_issuer_weights() {
    // Test issuer-specific weights
    let hazard = Arc::new(sample_hazard_curve("CDX"));
    let base_corr = Arc::new(sample_base_correlation_curve("CDX-BC"));
    let mut issuer_curves = FxHashMap::default();
    issuer_curves.insert(
        "IssuerA".to_string(),
        Arc::new(sample_hazard_curve("IssuerA")),
    );
    issuer_curves.insert(
        "IssuerB".to_string(),
        Arc::new(sample_hazard_curve("IssuerB")),
    );

    let mut weights = FxHashMap::default();
    weights.insert("IssuerA".to_string(), 0.90);
    weights.insert("IssuerB".to_string(), 0.10);

    let data = CreditIndexData::builder()
        .num_constituents(2)
        .recovery_rate(0.40)
        .index_credit_curve(hazard)
        .base_correlation_curve(base_corr)
        .issuer_curves(issuer_curves)
        .issuer_weights(weights)
        .build()
        .expect("issuer weights should work");

    // Should return issuer-specific weights
    assert_eq!(data.get_issuer_weight("IssuerA").unwrap(), 0.90);
    assert_eq!(data.get_issuer_weight("IssuerB").unwrap(), 0.10);

    assert!(data.get_issuer_weight("Unknown").is_err());
    assert!(data.get_issuer_recovery("C").is_err());
    assert!(data.get_issuer_curve("C").is_err());
    assert!(data.get_issuer_weight("C").is_err());
}

#[test]
fn homogeneous_pool_has_count_and_no_named_members() {
    let hazard = Arc::new(sample_hazard_curve("CDX"));
    let base_corr = Arc::new(sample_base_correlation_curve("CDX-BC"));

    let data = CreditIndexData::builder()
        .num_constituents(125)
        .recovery_rate(0.40)
        .index_credit_curve(hazard)
        .base_correlation_curve(base_corr)
        .build()
        .expect("default weighting should work");

    assert!(data.get_issuer_weight("AnyIssuer").is_err());
    assert_eq!(data.num_constituents(), 125);
}

#[test]
fn test_credit_index_no_issuer_curves() {
    // Test issuer_ids() returns empty when no issuer curves
    let hazard = Arc::new(sample_hazard_curve("CDX"));
    let base_corr = Arc::new(sample_base_correlation_curve("CDX-BC"));

    let data = CreditIndexData::builder()
        .num_constituents(125)
        .recovery_rate(0.40)
        .index_credit_curve(hazard)
        .base_correlation_curve(base_corr)
        .build()
        .expect("index without issuer curves");

    assert!(!data.has_issuer_curves());
    assert!(data.issuer_ids().is_empty());
}

#[test]
fn test_credit_index_issuer_weights_require_curves() {
    let hazard = Arc::new(sample_hazard_curve("CDX"));
    let base_corr = Arc::new(sample_base_correlation_curve("CDX-BC"));

    let mut weights = FxHashMap::default();
    weights.insert("IssuerA".to_string(), 1.0);

    let err = CreditIndexData::builder()
        .num_constituents(1)
        .recovery_rate(0.40)
        .index_credit_curve(hazard)
        .base_correlation_curve(base_corr)
        .issuer_weights(weights)
        .build()
        .expect_err("issuer weights without issuer curves should fail");

    assert!(matches!(err, finstack_quant_core::Error::Validation(_)));
}

#[test]
fn test_credit_index_issuer_weights_must_sum_to_one() {
    let hazard = Arc::new(sample_hazard_curve("CDX"));
    let base_corr = Arc::new(sample_base_correlation_curve("CDX-BC"));
    let mut issuer_curves = FxHashMap::default();
    issuer_curves.insert(
        "IssuerA".to_string(),
        Arc::new(sample_hazard_curve("IssuerA")),
    );
    issuer_curves.insert(
        "IssuerB".to_string(),
        Arc::new(sample_hazard_curve("IssuerB")),
    );

    let mut weights = FxHashMap::default();
    weights.insert("IssuerA".to_string(), 0.70);
    weights.insert("IssuerB".to_string(), 0.20);

    let err = CreditIndexData::builder()
        .num_constituents(2)
        .recovery_rate(0.40)
        .index_credit_curve(hazard)
        .base_correlation_curve(base_corr)
        .issuer_curves(issuer_curves)
        .issuer_weights(weights)
        .build()
        .expect_err("issuer weights must sum to one");

    assert!(matches!(err, finstack_quant_core::Error::Validation(_)));
}

#[test]
fn test_credit_index_issuer_recovery_rates_reject_unknown_issuer() {
    let hazard = Arc::new(sample_hazard_curve("CDX"));
    let base_corr = Arc::new(sample_base_correlation_curve("CDX-BC"));
    let mut issuer_curves = FxHashMap::default();
    issuer_curves.insert(
        "IssuerA".to_string(),
        Arc::new(sample_hazard_curve("IssuerA")),
    );

    let mut recovery_rates = FxHashMap::default();
    recovery_rates.insert("IssuerB".to_string(), 0.30);

    let err = CreditIndexData::builder()
        .num_constituents(1)
        .recovery_rate(0.40)
        .index_credit_curve(hazard)
        .base_correlation_curve(base_corr)
        .issuer_curves(issuer_curves)
        .issuer_recovery_rates(recovery_rates)
        .build()
        .expect_err("unknown issuer-specific recovery should fail");

    assert!(matches!(err, finstack_quant_core::Error::Validation(_)));
}

#[test]
fn issuer_keys_and_weight_mass_are_validated_before_pricing() {
    let hazard = Arc::new(sample_hazard_curve("CDX"));
    let base_corr = Arc::new(sample_base_correlation_curve("CDX-BC"));
    let base = |count| {
        CreditIndexData::builder()
            .num_constituents(count)
            .recovery_rate(0.4)
            .index_credit_curve(Arc::clone(&hazard))
            .base_correlation_curve(Arc::clone(&base_corr))
    };

    let single = || base(1).issuer_curves([("A".into(), Arc::clone(&hazard))]);
    assert!(
        base(1)
            .issuer_curves([(" ".into(), Arc::clone(&hazard))])
            .build()
            .is_err()
    );
    assert!(
        base(1)
            .issuer_curves([
                ("A".into(), Arc::clone(&hazard)),
                ("A".into(), Arc::clone(&hazard))
            ])
            .build()
            .is_err()
    );
    assert!(
        single()
            .issuer_recovery_rates([("A".into(), 0.3), ("A".into(), 0.4)])
            .build()
            .is_err()
    );
    assert!(
        single()
            .issuer_weights([("A".into(), 0.9), ("A".into(), 1.0)])
            .build()
            .is_err()
    );
    assert!(
        single()
            .issuer_weights([("A".into(), 1.0_f64.next_up())])
            .build()
            .is_err()
    );
    assert!(
        single()
            .issuer_weights([("A".into(), 1.0 - 2.0 * f64::EPSILON)])
            .build()
            .is_err()
    );

    let equal = base(49)
        .issuer_curves((0..49).map(|i| (i.to_string(), Arc::clone(&hazard))))
        .issuer_weights((0..49).map(|i| (i.to_string(), 1.0 / 49.0)))
        .build()
        .expect("compensated sum admits 49 equal weights");
    assert_eq!(equal.get_issuer_weight("10").unwrap(), 1.0 / 49.0);
}

#[test]
fn credit_index_state_rejects_duplicate_and_blank_issuer_keys_in_wire_maps() {
    use finstack_quant_core::market_data::context::CreditIndexState;
    for (field, entries) in [
        ("issuer_credit_curve_ids", r#""A":"one","A":"two""#),
        ("issuer_recovery_rates", r#""A":0.3,"A":0.4"#),
        ("issuer_weights", r#""A":0.5,"A":1.0"#),
        ("issuer_weights", r#"" ":1.0"#),
    ] {
        let wire = format!(
            r#"{{"id":"CDX","num_constituents":1,"recovery_rate":0.4,"index_credit_curve_id":"CDX","base_correlation_curve_id":"BC","{field}":{{{entries}}}}}"#
        );
        assert!(
            serde_json::from_str::<CreditIndexState>(&wire).is_err(),
            "{wire}"
        );
    }
}

#[test]
fn credit_index_in_tagged_envelope_accepts_null_issuer_maps() {
    use finstack_quant_core::market_data::context::CreditIndexState;

    #[derive(serde::Deserialize)]
    #[serde(tag = "kind")]
    enum Envelope {
        Index(CreditIndexState),
    }
    let wire = r#"{"kind":"Index","id":"CDX","num_constituents":1,"recovery_rate":0.4,
        "index_credit_curve_id":"CDX","base_correlation_curve_id":"BC",
        "issuer_credit_curve_ids":null,"issuer_recovery_rates":null,"issuer_weights":null}"#;
    let Envelope::Index(state) = serde_json::from_str(wire).expect("homogeneous pool");
    assert!(state.issuer_credit_curve_ids.is_none());
    assert!(state.issuer_recovery_rates.is_none());
    assert!(state.issuer_weights.is_none());
}
