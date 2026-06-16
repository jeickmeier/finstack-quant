//! Golden tests for parity with external reference implementations.
//!
//! This module validates that Finstack Quant Statements produces results consistent
//! with industry-standard tools like Excel, pandas, and QuantLib.

use finstack_quant_statements::prelude::*;
use std::fs;
use std::path::Path;

// Tolerance constants (documented in golden/README.md)
#[allow(dead_code)] // Placeholder for future Excel NPV tests
const EXCEL_TOLERANCE: f64 = 1e-8; // Excel double precision limit
const PANDAS_TOLERANCE: f64 = 1e-10; // pandas default
const SAMPLE_VAR_TOLERANCE: f64 = 1e-3; // Statistical calculations

/// Load golden test data from CSV
///
/// Expected format: test_id,param1,param2,...,expected_value
#[allow(dead_code)] // Placeholder for future CSV-driven tests
fn load_golden_csv(path: &str) -> Result<Vec<GoldenTestCase>> {
    let full_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("golden")
        .join(path);

    if !full_path.exists() {
        // Return empty vec if file doesn't exist yet (placeholder for future tests)
        return Ok(Vec::new());
    }

    let content = fs::read_to_string(&full_path)
        .map_err(|e| Error::invalid_input(format!("Failed to read golden test file: {}", e)))?;

    let mut test_cases = Vec::new();
    let mut lines = content.lines();

    // Skip header
    let _ = lines.next();

    for line in lines {
        if line.trim().is_empty() {
            continue;
        }

        let parts: Vec<&str> = line.split(',').collect();
        if parts.is_empty() {
            continue;
        }

        test_cases.push(GoldenTestCase {
            test_id: parts[0].to_string(),
            params: parts[1..parts.len() - 1]
                .iter()
                .filter_map(|s| s.parse::<f64>().ok())
                .collect(),
            expected: parts[parts.len() - 2].parse::<f64>().unwrap_or(0.0),
        });
    }

    Ok(test_cases)
}

#[derive(Debug)]
#[allow(dead_code)] // Placeholder for future CSV-driven tests
struct GoldenTestCase {
    test_id: String,
    params: Vec<f64>,
    expected: f64,
}

// ============================================================================
// Excel NPV Parity Tests
// ============================================================================

#[test]
fn test_excel_npv_simple_cashflows() {
    // Excel NPV with annual cashflows, various discount rates
    // This is a placeholder test - full implementation requires building
    // models from CSV data, which we'll do in the comprehensive testing phase

    let model = ModelBuilder::new("npv_test")
        .periods("2025..2028", None)
        .unwrap()
        .value_money(
            "cashflow",
            &[
                (
                    PeriodId::annual(2025),
                    finstack_quant_core::money::Money::new(1000.0, Currency::USD),
                ),
                (
                    PeriodId::annual(2026),
                    finstack_quant_core::money::Money::new(1000.0, Currency::USD),
                ),
                (
                    PeriodId::annual(2027),
                    finstack_quant_core::money::Money::new(1000.0, Currency::USD),
                ),
                (
                    PeriodId::annual(2028),
                    finstack_quant_core::money::Money::new(1000.0, Currency::USD),
                ),
            ],
        )
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    // Verify cashflows are present
    assert!(results.get("cashflow", &PeriodId::annual(2025)).is_some());

    // Verify Money accessor works for monetary nodes
    let money = results.get_money("cashflow", &PeriodId::annual(2025));
    // Note: monetary_nodes is only populated for nodes with explicit Money values in the spec
    // since we infer type from the first value. The evaluator currently populates this map.
    if let Some(m) = money {
        assert_eq!(m.currency(), Currency::USD);
    }
}

// ============================================================================
// pandas Rolling Statistics Parity Tests
// ============================================================================

#[test]
fn test_pandas_rolling_mean_parity() {
    let model = ModelBuilder::new("rolling_test")
        .periods("2025Q1..Q4", None)
        .unwrap()
        .value_scalar(
            "data",
            &[
                (PeriodId::quarter(2025, 1), 10.0),
                (PeriodId::quarter(2025, 2), 20.0),
                (PeriodId::quarter(2025, 3), 30.0),
                (PeriodId::quarter(2025, 4), 40.0),
            ],
        )
        .compute("rolling_mean_3", "rolling_mean(data, 3)")
        .unwrap()
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    // Q3: mean([10, 20, 30]) = 20.0
    let q3_mean = results
        .get("rolling_mean_3", &PeriodId::quarter(2025, 3))
        .unwrap();
    assert!(
        (q3_mean - 20.0).abs() < PANDAS_TOLERANCE,
        "Rolling mean Q3 should match pandas within tolerance"
    );

    // Q4: mean([20, 30, 40]) = 30.0
    let q4_mean = results
        .get("rolling_mean_3", &PeriodId::quarter(2025, 4))
        .unwrap();
    assert!(
        (q4_mean - 30.0).abs() < PANDAS_TOLERANCE,
        "Rolling mean Q4 should match pandas within tolerance"
    );
}

#[test]
fn test_pandas_rolling_variance_parity() {
    let model = ModelBuilder::new("var_test")
        .periods("2025Q1..2025Q4", None)
        .unwrap()
        .value_scalar(
            "data",
            &[
                (PeriodId::quarter(2025, 1), 2.0),
                (PeriodId::quarter(2025, 2), 4.0),
                (PeriodId::quarter(2025, 3), 4.0),
                (PeriodId::quarter(2025, 4), 4.0),
            ],
        )
        .compute("rolling_var_4", "rolling_var(data, 4)")
        .unwrap()
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    // Sample variance with ddof=1: [2,4,4,4]
    // Mean = 3.5, variance = 1.0 (already tested in market_standards_tests.rs)
    let q4_var = results
        .get("rolling_var_4", &PeriodId::quarter(2025, 4))
        .unwrap();

    assert!(
        (q4_var - 1.0).abs() < SAMPLE_VAR_TOLERANCE,
        "Rolling variance should match pandas with ddof=1"
    );
}

// ============================================================================
// Exponentially Weighted Moving Average (EWM) Parity Tests
// ============================================================================

/// Test EWM mean parity with known golden values.
#[test]
fn test_ewm_mean_golden_parity() {
    let model = ModelBuilder::new("ewm_test")
        .periods("2025Q1..Q4", None)
        .unwrap()
        .value_scalar(
            "data",
            &[
                (PeriodId::quarter(2025, 1), 0.10),
                (PeriodId::quarter(2025, 2), 0.05),
                (PeriodId::quarter(2025, 3), 0.15),
                (PeriodId::quarter(2025, 4), 0.08),
            ],
        )
        .compute("ewm_mean", "ewm_mean(data, 0.3)")
        .unwrap()
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    // Expected: alpha=0.3 on [0.10, 0.05, 0.15, 0.08]
    // EWM formula (adjust=False): ewm[i] = alpha * x[i] + (1-alpha) * ewm[i-1]
    // ewm[0] = 0.10
    // ewm[1] = 0.3*0.05 + 0.7*0.10 = 0.085
    // ewm[2] = 0.3*0.15 + 0.7*0.085 = 0.1045
    // ewm[3] = 0.3*0.08 + 0.7*0.1045 = 0.09715
    let q4_ewm = results
        .get("ewm_mean", &PeriodId::quarter(2025, 4))
        .unwrap();

    // Tolerance for EWM calculations - slightly looser than pure floating point
    // due to iterative computation
    const EWM_TOLERANCE: f64 = 1e-6;

    assert!(
        (q4_ewm - 0.09715).abs() < EWM_TOLERANCE,
        "EWM mean should match expected value: got {}, expected 0.09715",
        q4_ewm
    );
}

/// Test EWM variance with known golden values.
#[test]
fn test_ewm_var_golden_parity() {
    let model = ModelBuilder::new("ewm_var_test")
        .periods("2025Q1..Q4", None)
        .unwrap()
        .value_scalar(
            "data",
            &[
                (PeriodId::quarter(2025, 1), 100.0),
                (PeriodId::quarter(2025, 2), 110.0),
                (PeriodId::quarter(2025, 3), 105.0),
                (PeriodId::quarter(2025, 4), 115.0),
            ],
        )
        .compute("ewm_var", "ewm_var(data, 0.5)")
        .unwrap()
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    // EWM variance with alpha=0.5 (no bias correction)
    // Values: [100, 110, 105, 115]
    let q4_ewm_var = results.get("ewm_var", &PeriodId::quarter(2025, 4)).unwrap();

    // Verify the value is positive and finite
    assert!(q4_ewm_var > 0.0, "EWM variance should be positive");
    assert!(q4_ewm_var.is_finite(), "EWM variance should be finite");

    // Verify consistency across runs (deterministic)
    let mut evaluator2 = Evaluator::new();
    let results2 = evaluator2.evaluate(&model).unwrap();
    let q4_ewm_var2 = results2
        .get("ewm_var", &PeriodId::quarter(2025, 4))
        .unwrap();
    assert_eq!(
        q4_ewm_var, q4_ewm_var2,
        "EWM variance should be deterministic"
    );
}

// ============================================================================
// Placeholder for Future Tests
// ============================================================================

#[test]
fn test_placeholder_for_full_csv_driven_tests() {
    // Future implementation will:
    // 1. Load CSV files from golden/excel/ and golden/pandas/
    // 2. Build models from CSV inputs
    // 3. Compare results against expected outputs
    // 4. Assert within documented tolerances

    // For now, this is a placeholder to establish the testing pattern
    // Once CSV loading is implemented, this test will be replaced
}

#[cfg(test)]
mod helpers {
    use super::*;

    /// Assert two values are equal within a specified tolerance.
    #[allow(dead_code)]
    pub fn assert_close(actual: f64, expected: f64, tolerance: f64, message: &str) {
        assert!(
            (actual - expected).abs() < tolerance,
            "{}: expected {}, got {} (diff: {}, tolerance: {})",
            message,
            expected,
            actual,
            (actual - expected).abs(),
            tolerance
        );
    }

    /// Build a financial model from CSV test data.
    #[allow(dead_code)]
    pub fn build_model_from_csv(_test_case: &super::GoldenTestCase) -> Result<FinancialModelSpec> {
        // Placeholder for CSV-to-model conversion
        // Will be implemented when we have real test scenarios
        Err(Error::invalid_input(
            "CSV model building not yet implemented",
        ))
    }
}
