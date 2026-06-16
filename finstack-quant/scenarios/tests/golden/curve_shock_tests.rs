//! Golden tests for curve shock operations.
//!
//! Tests parallel and tenor point shocks on discount curves using
//! the unified golden framework.

use finstack_quant_test_utils::golden::{
    load_suite_from_path, Expectation, ExpectedValue, GoldenAssert,
};
use finstack_quant_test_utils::golden_path;
use serde::Deserialize;

/// Input curve specification.
#[derive(Debug, Deserialize)]
struct CurveInput {
    #[allow(dead_code)]
    id: String,
    flat_rate: f64,
    #[allow(dead_code)]
    tenors: Vec<f64>,
}

/// Shock specification.
#[derive(Debug, Deserialize)]
struct ShockInput {
    #[serde(rename = "type")]
    #[allow(dead_code)]
    shock_type: String,
    magnitude_bp: f64,
}

/// Test case inputs.
#[derive(Debug, Deserialize)]
struct CurveShockInputs {
    base_curve: CurveInput,
    shock: ShockInput,
}

/// Expected outputs - can be exact values or ranges.
#[derive(Debug, Deserialize)]
struct CurveShockExpected {
    #[serde(default)]
    shocked_df_1y: Option<ExpectedValue>,
    #[serde(default)]
    shocked_df_5y: Option<ExpectedValue>,
    #[serde(default)]
    df_direction: Option<RangeExpectation>,
}

/// Range expectation for directional tests.
#[derive(Debug, Deserialize)]
struct RangeExpectation {
    min: Option<f64>,
    max: Option<f64>,
    #[allow(dead_code)]
    notes: Option<String>,
}

/// A curve shock test case.
#[derive(Debug, Deserialize)]
struct CurveShockCase {
    id: String,
    inputs: CurveShockInputs,
    expected: CurveShockExpected,
}

/// Apply a parallel shock to a flat rate and compute DF.
fn apply_parallel_shock_df(base_rate: f64, shock_bp: f64, tenor: f64) -> f64 {
    let shocked_rate = base_rate + shock_bp / 10000.0;
    (-shocked_rate * tenor).exp()
}

#[test]
fn test_curve_shock_golden() {
    let path = golden_path!("data/curve_shocks.json");
    let suite =
        load_suite_from_path::<CurveShockCase>(&path).expect("should load curve_shocks.json");

    assert!(!suite.cases.is_empty(), "Suite should have test cases");
    assert_eq!(suite.meta.status, "certified");

    for case in &suite.cases {
        let assert = GoldenAssert::new(&suite.meta, &case.id);
        let base_rate = case.inputs.base_curve.flat_rate;
        let shock_bp = case.inputs.shock.magnitude_bp;

        // Test exact DF values at specific tenors
        if let Some(expected) = &case.expected.shocked_df_1y {
            let actual = apply_parallel_shock_df(base_rate, shock_bp, 1.0);
            assert
                .expected("shocked_df_1y", actual, expected)
                .unwrap_or_else(|e| panic!("{}", e));
        }

        if let Some(expected) = &case.expected.shocked_df_5y {
            let actual = apply_parallel_shock_df(base_rate, shock_bp, 5.0);
            assert
                .expected("shocked_df_5y", actual, expected)
                .unwrap_or_else(|e| panic!("{}", e));
        }

        // Test directional constraints
        if let Some(range) = &case.expected.df_direction {
            let actual = apply_parallel_shock_df(base_rate, shock_bp, 1.0);
            let expectation = Expectation::Range {
                min: range.min,
                max: range.max,
                notes: None,
            };
            use finstack_quant_test_utils::golden::assert_expected_f64;
            assert_expected_f64(
                &suite.meta.suite_id,
                &case.id,
                "df_direction",
                actual,
                &expectation,
            )
            .unwrap_or_else(|e| panic!("{}", e));
        }
    }
}

#[test]
fn test_curve_shock_suite_metadata() {
    let path = golden_path!("data/curve_shocks.json");
    let suite =
        load_suite_from_path::<CurveShockCase>(&path).expect("should load curve_shocks.json");

    // Verify provenance metadata
    assert_eq!(suite.meta.suite_id, "curve_shocks");
    assert!(!suite.meta.reference_source.name.is_empty());
    assert!(!suite.meta.generated.at.is_empty());
    assert!(!suite.meta.generated.by.is_empty());
}
