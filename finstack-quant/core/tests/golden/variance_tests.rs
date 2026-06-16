//! Golden tests for realized variance estimators.
//!
//! These tests load expected values from `data/realized_variance.json` and
//! compare against computed results from the `finstack_quant_core::math::stats` module.

use finstack_quant_core::math::stats::{realized_variance_ohlc, RealizedVarMethod};
use finstack_quant_test_utils::golden::{load_suite_from_path, ExpectedValue, GoldenAssert};
use finstack_quant_test_utils::golden_path;
use serde::Deserialize;

/// Input data for variance tests.
#[derive(Debug, Deserialize)]
struct VarianceInputs {
    open: Vec<f64>,
    high: Vec<f64>,
    low: Vec<f64>,
    close: Vec<f64>,
    annualization_factor: f64,
    method: String,
}

/// Expected output for variance tests.
#[derive(Debug, Deserialize)]
struct VarianceExpected {
    annualized_variance: ExpectedValue,
}

/// A test case for variance calculations.
#[derive(Debug, Deserialize)]
struct VarianceCase {
    id: String,
    inputs: VarianceInputs,
    expected: VarianceExpected,
}

fn method_from_str(s: &str) -> RealizedVarMethod {
    match s.to_lowercase().as_str() {
        "parkinson" => RealizedVarMethod::Parkinson,
        "garman_klass" => RealizedVarMethod::GarmanKlass,
        "rogers_satchell" => RealizedVarMethod::RogersSatchell,
        "yang_zhang" => RealizedVarMethod::YangZhang,
        _ => RealizedVarMethod::CloseToClose,
    }
}

#[test]
fn test_realized_variance_golden() {
    let path = golden_path!("data/realized_variance.json");
    let suite =
        load_suite_from_path::<VarianceCase>(&path).expect("should load realized_variance.json");

    assert!(!suite.cases.is_empty(), "Suite should have test cases");

    for case in &suite.cases {
        let method = method_from_str(&case.inputs.method);
        let result = realized_variance_ohlc(
            &case.inputs.open,
            &case.inputs.high,
            &case.inputs.low,
            &case.inputs.close,
            method,
            case.inputs.annualization_factor,
        )
        .expect("realized_variance_ohlc should succeed for certified golden cases");

        let assert = GoldenAssert::new(&suite.meta, &case.id);
        assert
            .expected(
                "annualized_variance",
                result,
                &case.expected.annualized_variance,
            )
            .unwrap_or_else(|e| panic!("{}", e));
    }
}

#[test]
fn test_variance_suite_metadata() {
    let path = golden_path!("data/realized_variance.json");
    let suite =
        load_suite_from_path::<VarianceCase>(&path).expect("should load realized_variance.json");

    // Verify provenance metadata is present
    assert_eq!(suite.meta.suite_id, "realized_variance");
    assert!(!suite.meta.reference_source.name.is_empty());
    assert!(!suite.meta.generated.at.is_empty());
    assert!(!suite.meta.generated.by.is_empty());
    assert_eq!(suite.meta.status, "certified");
}
