use super::step::*;
use crate::structured_credit::types::{CoverageTestType, Waterfall};
use finstack_quant_core::currency::Currency;

use crate::structured_credit::types::{CoverageTestAction, CoverageTestSpec, WaterfallTier};

fn waterfall_with_oc_test(id: &str, trigger_level: f64) -> Waterfall {
    let spec = CoverageTestSpec {
        id: id.to_string(),
        tranche_id: "CLASS_A".to_string(),
        kind: CoverageTestType::Oc,
        trigger_level,
        action: CoverageTestAction::PayDownSenior,
        placement: None,
        divert_pct: None,
    };
    Waterfall::builder(Currency::USD)
        .add_tier(WaterfallTier::coverage_tests("oc_tier", 1, vec![spec]))
        .build()
        .expect("valid test waterfall")
}

#[test]
fn missing_coverage_test_spec_returns_internal_error_instead_of_nan() {
    let waterfall = waterfall_with_oc_test("OC_CLASS_A", 1.20);
    let evaluated = vec![("OC_UNKNOWN".to_string(), Some(1.35), true)];
    let err = build_coverage_test_diagnostics(&waterfall, &evaluated)
        .expect_err("unknown coverage test ID must error instead of emitting NaN");
    assert!(
        matches!(err, finstack_quant_core::Error::Internal(_)),
        "expected Internal error, got: {err:?}"
    );
    assert!(
        err.to_string().contains("OC_UNKNOWN"),
        "error must name missing test_id, got: {err}"
    );
}

#[test]
fn known_coverage_test_spec_populates_finite_trigger_and_cushion() {
    let waterfall = waterfall_with_oc_test("OC_CLASS_A", 1.20);
    let evaluated = vec![("OC_CLASS_A".to_string(), Some(1.35), true)];
    let diagnostics =
        build_coverage_test_diagnostics(&waterfall, &evaluated).expect("matching spec");
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].test_id, "OC_CLASS_A");
    assert_eq!(diagnostics[0].trigger_level, 1.20);
    assert!((diagnostics[0].cushion.expect("cushion") - 0.15).abs() < 1e-12);
    assert!(diagnostics[0].passing);
}
