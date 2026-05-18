//! Tests for TermLoan capital structure integration.
//!
//! This test suite validates that TermLoan instruments from the valuations crate
//! can be properly integrated into financial statement models.

use finstack_statements::types::CapitalStructureSpec;
use finstack_statements::types::DebtInstrumentSpec;

// ============================================================================
// TermLoan Variant Tests
// ============================================================================

#[test]
fn test_term_loan_variant_serialization() {
    let spec = DebtInstrumentSpec {
        id: "TL-001".to_string(),
        spec: serde_json::json!({
            "type": "term_loan",
            "spec": {
                "id": "TL-001",
                "notional": { "amount": 5000000.0, "currency": "USD" }
            }
        }),
    };

    // Test serialization roundtrip
    let json = serde_json::to_string(&spec).unwrap();
    let deserialized: DebtInstrumentSpec = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized.id, "TL-001");
}

// ============================================================================
// Capital Structure with TermLoan (Placeholder)
// ============================================================================

#[test]
fn test_term_loan_in_capital_structure_placeholder() {
    let capital_structure = CapitalStructureSpec {
        debt_instruments: vec![DebtInstrumentSpec {
            id: "TL-001".to_string(),
            spec: serde_json::json!({
                "type": "term_loan",
                "spec": {
                    "id": "TL-001",
                    "notional": { "amount": 5000000.0, "currency": "USD" }
                }
            }),
        }],
        equity_instruments: vec![],
        meta: indexmap::IndexMap::new(),
        reporting_currency: None,
        fx_policy: None,
        waterfall: None,
    };

    // Verify it serializes correctly
    let json = serde_json::to_string(&capital_structure).unwrap();
    let deserialized: CapitalStructureSpec = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized.debt_instruments.len(), 1);
    assert_eq!(deserialized.debt_instruments[0].id, "TL-001");
    // Round-trip equality confirms DebtInstrumentSpec / CapitalStructureSpec
    // retain PartialEq after the enum -> struct collapse.
    assert_eq!(capital_structure, deserialized);
}

// Note: Full end-to-end TermLoan integration tests require:
// - Proper TermLoan spec construction matching valuations crate requirements
// - Market context with discount and forward curves
// - These will be added as the integration matures
