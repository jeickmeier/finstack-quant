//! wasm-bindgen-test suite for `api::core::math`.
//!
//! Covers the typed-array linear algebra, statistics, and summation wrappers.

#![cfg(target_arch = "wasm32")]

use finstack_quant_wasm::api::core::math::*;
use wasm_bindgen_test::*;

// ---- Linear algebra ----

#[wasm_bindgen_test]
fn cholesky_decomposition_returns_row_major_factor() {
    let matrix = [4.0, 2.0, 2.0, 3.0];
    let result = cholesky_decomposition(&matrix, 2.0).unwrap();
    assert_eq!(result.len(), 4);
    assert!((result[0] - 2.0).abs() < 1e-10);
}

#[wasm_bindgen_test]
fn cholesky_decomposition_identity() {
    let result = cholesky_decomposition(&[1.0, 0.0, 0.0, 1.0], 2.0).unwrap();
    assert!((result[0] - 1.0).abs() < 1e-10);
    assert!((result[3] - 1.0).abs() < 1e-10);
}

#[wasm_bindgen_test]
fn cholesky_solve_solves_system() {
    let chol = cholesky_decomposition(&[4.0, 2.0, 2.0, 3.0], 2.0).unwrap();
    let x = cholesky_solve(&chol, &[2.0, 1.0], 2.0).unwrap();
    assert_eq!(x.len(), 2);
    assert!((x[0] - 0.5).abs() < 1e-10);
}

#[wasm_bindgen_test]
fn cholesky_solve_rejects_wrong_rhs_length() {
    assert!(cholesky_solve(&[1.0, 0.0, 0.0, 1.0], &[1.0], 2.0).is_err());
}

// ---- Statistics ----

#[wasm_bindgen_test]
fn statistics_of_known_values() {
    let data = [1.0, 2.0, 3.0, 4.0, 5.0];
    assert!((mean(&data) - 3.0).abs() < 1e-10);
    assert!(variance(&data) > 0.0);
    assert!(population_variance(&data) > 0.0);
    assert!((quantile(&data, 0.5) - 3.0).abs() < 1e-10);
    assert!((correlation(&data, &[2.0, 4.0, 6.0, 8.0, 10.0]) - 1.0).abs() < 1e-10);
    assert!(covariance(&data, &[1.0, 2.0, 3.0, 4.0, 5.0]) > 0.0);
}

// ---- Summation ----

#[wasm_bindgen_test]
fn summation_and_positive_run() {
    assert!((kahan_sum(&[1.0, 2.0, 3.0, 4.0]) - 10.0).abs() < 1e-10);
    assert!((neumaier_sum(&[1e16, 1.0, -1e16, 1.0]) - 2.0).abs() < 1e-10);
    assert_eq!(longest_positive_run(&[1.0, 2.0, 3.0, -1.0, 2.0]), 3);
}
