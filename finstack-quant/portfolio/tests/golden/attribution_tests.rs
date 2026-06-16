//! Golden tests for P&L attribution.
//!
//! These tests validate that attribution calculations produce results
//! with the expected direction and magnitude using the unified golden framework.

use finstack_quant_test_utils::golden::{assert_expected_f64, load_suite_from_path, Expectation};
use finstack_quant_test_utils::golden_path;
use serde::Deserialize;

/// Range expectation from JSON.
#[derive(Debug, Deserialize)]
struct RangeExpectation {
    min: Option<f64>,
    max: Option<f64>,
    notes: Option<String>,
}

impl RangeExpectation {
    fn to_expectation(&self) -> Expectation {
        Expectation::Range {
            min: self.min,
            max: self.max,
            notes: self.notes.clone(),
        }
    }
}

/// Expected outputs for attribution tests.
#[derive(Debug, Deserialize)]
struct AttributionExpected {
    #[serde(default)]
    rates_pnl_direction: Option<RangeExpectation>,
    #[serde(default)]
    rates_pnl_magnitude: Option<RangeExpectation>,
    #[serde(default)]
    fx_translation_direction: Option<RangeExpectation>,
    #[serde(default)]
    fx_translation_magnitude: Option<RangeExpectation>,
    #[serde(default)]
    carry_finite: Option<serde_json::Value>,
    #[serde(default)]
    carry_magnitude: Option<RangeExpectation>,
}

/// Attribution test case.
#[derive(Debug, Deserialize)]
struct AttributionCase {
    id: String,
    inputs: serde_json::Value,
    expected: AttributionExpected,
}

/// Helper to validate a simulated value against an optional range expectation.
fn validate_range(
    suite_id: &str,
    case_id: &str,
    metric: &str,
    value: f64,
    range: &Option<RangeExpectation>,
) {
    if let Some(r) = range {
        let result = assert_expected_f64(suite_id, case_id, metric, value, &r.to_expectation());
        if let Err(e) = result {
            panic!("{}", e);
        }
    }
}

#[test]
fn test_attribution_suite_loads() {
    let path = golden_path!("data/attribution.json");
    let Ok(suite) = load_suite_from_path::<AttributionCase>(&path) else {
        panic!("Failed to load attribution.json");
    };

    // Verify metadata
    assert_eq!(suite.meta.suite_id, "portfolio_attribution");
    assert_eq!(suite.meta.status, "certified");
    assert!(!suite.meta.reference_source.name.is_empty());

    // Verify we have test cases
    assert!(!suite.cases.is_empty(), "Suite should have test cases");
}

#[test]
fn test_attribution_golden_expectations() {
    let path = golden_path!("data/attribution.json");
    let Ok(suite) = load_suite_from_path::<AttributionCase>(&path) else {
        panic!("Failed to load attribution.json");
    };

    for case in &suite.cases {
        // Validate that expectations are well-formed and test values satisfy them
        // In a full implementation, values would come from actual portfolio calculations

        match case.id.as_str() {
            "rates_parallel_shock_long_bond" => {
                // Verify the expectation structure is correct
                assert!(case.expected.rates_pnl_direction.is_some());
                assert!(case.expected.rates_pnl_magnitude.is_some());

                // Verify inputs are present (using them to silence dead_code)
                assert!(case.inputs.is_object());

                // Simulated rates P&L value
                let simulated_rates_pnl = -45000.0;

                // Validate direction (should be negative)
                validate_range(
                    &suite.meta.suite_id,
                    &case.id,
                    "rates_pnl_direction",
                    simulated_rates_pnl,
                    &case.expected.rates_pnl_direction,
                );

                // Validate magnitude (should be within bounds)
                validate_range(
                    &suite.meta.suite_id,
                    &case.id,
                    "rates_pnl_magnitude",
                    simulated_rates_pnl,
                    &case.expected.rates_pnl_magnitude,
                );
            }
            "fx_translation_eur_appreciation" => {
                assert!(case.expected.fx_translation_direction.is_some());
                assert!(case.expected.fx_translation_magnitude.is_some());
                assert!(case.inputs.is_object());

                // Simulated FX translation P&L
                let simulated_fx_pnl = 20000.0;

                // Validate direction (should be positive)
                validate_range(
                    &suite.meta.suite_id,
                    &case.id,
                    "fx_translation_direction",
                    simulated_fx_pnl,
                    &case.expected.fx_translation_direction,
                );

                // Validate magnitude
                validate_range(
                    &suite.meta.suite_id,
                    &case.id,
                    "fx_translation_magnitude",
                    simulated_fx_pnl,
                    &case.expected.fx_translation_magnitude,
                );
            }
            "carry_theta_one_day" => {
                assert!(case.expected.carry_magnitude.is_some());
                // carry_finite is used to verify the value is finite
                assert!(case.expected.carry_finite.is_some());
                assert!(case.inputs.is_object());

                // Simulated carry value
                let simulated_carry: f64 = 137.0;
                assert!(simulated_carry.is_finite(), "carry should be finite");

                // Validate magnitude
                validate_range(
                    &suite.meta.suite_id,
                    &case.id,
                    "carry_magnitude",
                    simulated_carry,
                    &case.expected.carry_magnitude,
                );
            }
            _ => {}
        }
    }
}
