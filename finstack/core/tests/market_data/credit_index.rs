use finstack_core::HashMap;
use std::sync::Arc;

use super::test_helpers::{sample_base_correlation_curve, sample_hazard_curve};
use finstack_core::market_data::term_structures::{CreditIndexData, CreditIndexDataBuilder};

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

    assert_eq!(data.num_constituents, 125);
    assert!(!data.has_issuer_curves());
    assert!(Arc::ptr_eq(&data.index_credit_curve, &hazard));
    assert!(Arc::ptr_eq(&data.base_correlation_curve, &base_corr));
}

#[test]
fn credit_index_builder_supports_issuer_curves() {
    let hazard = Arc::new(sample_hazard_curve("CDX"));
    let base_corr = Arc::new(sample_base_correlation_curve("CDX-BC"));

    let mut issuers: HashMap<String, Arc<_>> = HashMap::default();
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
    let issuer_curve = data.get_issuer_curve("IssuerA");
    assert_eq!(issuer_curve.id().as_str(), "IssuerA");

    // Unknown issuer should fall back to index curve
    let unknown_curve = data.get_issuer_curve("Unknown");
    assert_eq!(unknown_curve.id().as_str(), "CDX");
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
    assert!(matches!(err, finstack_core::Error::Input(_)));
}

// =============================================================================
// Additional credit index behavior
// =============================================================================

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

        assert_eq!(data.recovery_rate, rr);
    }
}

#[test]
fn test_credit_index_default_recovery_rate() {
    // Test that default recovery rate is 0.40 (40%)
    let hazard = Arc::new(sample_hazard_curve("CDX"));
    let base_corr = Arc::new(sample_base_correlation_curve("CDX-BC"));

    let data = CreditIndexData::builder()
        .num_constituents(125)
        // No recovery_rate specified, should default to 0.40
        .index_credit_curve(hazard)
        .base_correlation_curve(base_corr)
        .build()
        .expect("valid credit index with default recovery");

    assert_eq!(data.recovery_rate, 0.40);
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

    assert!(matches!(err, finstack_core::Error::Input(_)));
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

    assert!(matches!(err, finstack_core::Error::Input(_)));
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

    assert!(matches!(err, finstack_core::Error::Input(_)));
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

    assert!(matches!(err, finstack_core::Error::Input(_)));
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

    assert!(matches!(err, finstack_core::Error::Input(_)));
}

#[test]
fn test_credit_index_add_issuer_curve_to_empty() {
    // Test adding a single issuer curve when none exist
    let hazard = Arc::new(sample_hazard_curve("CDX"));
    let base_corr = Arc::new(sample_base_correlation_curve("CDX-BC"));
    let issuer_curve = Arc::new(sample_hazard_curve("IssuerB"));

    let data = CreditIndexData::builder()
        .num_constituents(2)
        .recovery_rate(0.4)
        .index_credit_curve(hazard)
        .base_correlation_curve(base_corr)
        .add_issuer_curve("IssuerB".to_string(), issuer_curve)
        .build()
        .expect("add_issuer_curve should work");

    assert!(data.has_issuer_curves());
    assert_eq!(data.issuer_ids(), vec!["IssuerB".to_string()]);
}

#[test]
fn test_credit_index_add_multiple_issuer_curves() {
    // Test adding multiple issuer curves one at a time
    let hazard = Arc::new(sample_hazard_curve("CDX"));
    let base_corr = Arc::new(sample_base_correlation_curve("CDX-BC"));
    let issuer1 = Arc::new(sample_hazard_curve("Issuer1"));
    let issuer2 = Arc::new(sample_hazard_curve("Issuer2"));

    let data = CreditIndexData::builder()
        .num_constituents(2)
        .recovery_rate(0.4)
        .index_credit_curve(hazard)
        .base_correlation_curve(base_corr)
        .add_issuer_curve("Issuer1".to_string(), issuer1)
        .add_issuer_curve("Issuer2".to_string(), issuer2)
        .build()
        .expect("multiple add_issuer_curve should work");

    assert!(data.has_issuer_curves());
    let ids = data.issuer_ids();
    assert_eq!(ids.len(), 2);
    assert!(ids.contains(&"Issuer1".to_string()));
    assert!(ids.contains(&"Issuer2".to_string()));
}

#[test]
fn test_credit_index_issuer_recovery_rates() {
    // Test issuer-specific recovery rates
    let hazard = Arc::new(sample_hazard_curve("CDX"));
    let base_corr = Arc::new(sample_base_correlation_curve("CDX-BC"));
    let mut issuer_curves = HashMap::default();
    issuer_curves.insert(
        "IssuerA".to_string(),
        Arc::new(sample_hazard_curve("IssuerA")),
    );
    issuer_curves.insert(
        "IssuerB".to_string(),
        Arc::new(sample_hazard_curve("IssuerB")),
    );

    let mut recovery_rates = HashMap::default();
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
    assert_eq!(data.get_issuer_recovery("IssuerA"), 0.30);
    assert_eq!(data.get_issuer_recovery("IssuerB"), 0.50);

    // Unknown issuer should return default
    assert_eq!(data.get_issuer_recovery("Unknown"), 0.40);
}

#[test]
fn test_credit_index_issuer_weights() {
    // Test issuer-specific weights
    let hazard = Arc::new(sample_hazard_curve("CDX"));
    let base_corr = Arc::new(sample_base_correlation_curve("CDX-BC"));
    let mut issuer_curves = HashMap::default();
    issuer_curves.insert(
        "IssuerA".to_string(),
        Arc::new(sample_hazard_curve("IssuerA")),
    );
    issuer_curves.insert(
        "IssuerB".to_string(),
        Arc::new(sample_hazard_curve("IssuerB")),
    );

    let mut weights = HashMap::default();
    weights.insert("IssuerA".to_string(), 0.60);
    weights.insert("IssuerB".to_string(), 0.40);

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
    assert_eq!(data.get_issuer_weight("IssuerA"), 0.60);
    assert_eq!(data.get_issuer_weight("IssuerB"), 0.40);

    // Unknown issuer should return equal weight (1/N)
    assert_eq!(data.get_issuer_weight("Unknown"), 0.50); // 1/2
}

#[test]
fn test_credit_index_equal_weighting_fallback() {
    // Test that unknown issuers get equal weight (1/N) when no weights specified
    let hazard = Arc::new(sample_hazard_curve("CDX"));
    let base_corr = Arc::new(sample_base_correlation_curve("CDX-BC"));

    let data = CreditIndexData::builder()
        .num_constituents(125)
        .recovery_rate(0.40)
        .index_credit_curve(hazard)
        .base_correlation_curve(base_corr)
        .build()
        .expect("default weighting should work");

    // All issuers should get equal weight
    let expected_weight = 1.0 / 125.0;
    assert_eq!(data.get_issuer_weight("AnyIssuer"), expected_weight);
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

    let mut weights = HashMap::default();
    weights.insert("IssuerA".to_string(), 1.0);

    let err = CreditIndexData::builder()
        .num_constituents(1)
        .recovery_rate(0.40)
        .index_credit_curve(hazard)
        .base_correlation_curve(base_corr)
        .issuer_weights(weights)
        .build()
        .expect_err("issuer weights without issuer curves should fail");

    assert!(matches!(err, finstack_core::Error::Validation(_)));
}

#[test]
fn test_credit_index_issuer_weights_must_sum_to_one() {
    let hazard = Arc::new(sample_hazard_curve("CDX"));
    let base_corr = Arc::new(sample_base_correlation_curve("CDX-BC"));
    let mut issuer_curves = HashMap::default();
    issuer_curves.insert(
        "IssuerA".to_string(),
        Arc::new(sample_hazard_curve("IssuerA")),
    );
    issuer_curves.insert(
        "IssuerB".to_string(),
        Arc::new(sample_hazard_curve("IssuerB")),
    );

    let mut weights = HashMap::default();
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

    assert!(matches!(err, finstack_core::Error::Validation(_)));
}

#[test]
fn test_credit_index_issuer_recovery_rates_reject_unknown_issuer() {
    let hazard = Arc::new(sample_hazard_curve("CDX"));
    let base_corr = Arc::new(sample_base_correlation_curve("CDX-BC"));
    let mut issuer_curves = HashMap::default();
    issuer_curves.insert(
        "IssuerA".to_string(),
        Arc::new(sample_hazard_curve("IssuerA")),
    );

    let mut recovery_rates = HashMap::default();
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

    assert!(matches!(err, finstack_core::Error::Validation(_)));
}
