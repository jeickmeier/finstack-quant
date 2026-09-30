//! wasm-bindgen-test suite for `api::core::math`.
//!
//! Covers the typed-array linear algebra, statistics, and summation wrappers.

#![cfg(target_arch = "wasm32")]

use finstack_quant_wasm::api::core::math::*;
use wasm_bindgen::JsValue;
use wasm_bindgen_test::*;

const TOL: f64 = 1e-4;

// ---- Linear algebra ----

#[wasm_bindgen_test]
fn cholesky_decomposition_returns_row_major_factor() {
    let matrix = [4.0, 2.0, 2.0, 3.0];
    let result = cholesky_decomposition(JsValue::from(matrix.to_vec()), JsValue::from(2)).unwrap();
    assert_eq!(result.len(), 4);
    assert!((result[0] - 2.0).abs() < 1e-10);
}

#[wasm_bindgen_test]
fn cholesky_decomposition_identity() {
    let result = cholesky_decomposition(
        JsValue::from([1.0, 0.0, 0.0, 1.0].to_vec()),
        JsValue::from(2),
    )
    .unwrap();
    assert!((result[0] - 1.0).abs() < 1e-10);
    assert!((result[3] - 1.0).abs() < 1e-10);
}

#[wasm_bindgen_test]
fn cholesky_solve_solves_system() {
    let chol = cholesky_decomposition(
        JsValue::from([4.0, 2.0, 2.0, 3.0].to_vec()),
        JsValue::from(2),
    )
    .unwrap();
    let x = cholesky_solve(
        JsValue::from(chol.to_vec()),
        JsValue::from([2.0, 1.0].to_vec()),
    )
    .unwrap();
    assert_eq!(x.len(), 2);
    assert!((x[0] - 0.5).abs() < 1e-10);
}

#[wasm_bindgen_test]
fn cholesky_solve_rejects_wrong_rhs_length() {
    assert!(cholesky_solve(
        JsValue::from([1.0, 0.0, 0.0, 1.0].to_vec()),
        JsValue::from([1.0].to_vec())
    )
    .is_err());
}

// ---- Statistics ----

#[wasm_bindgen_test]
fn statistics_of_known_values() {
    let data = [1.0, 2.0, 3.0, 4.0, 5.0];
    assert!((mean(JsValue::from(data.to_vec())).unwrap() - 3.0).abs() < 1e-10);
    assert!(variance(JsValue::from(data.to_vec())).unwrap() > 0.0);
    assert!(population_variance(JsValue::from(data.to_vec())).unwrap() > 0.0);
    assert!(
        (quantile(JsValue::from(data.to_vec()), JsValue::from(0.5)).unwrap() - 3.0).abs() < 1e-10
    );
    assert!(
        (correlation(
            JsValue::from(data.to_vec()),
            JsValue::from([2.0, 4.0, 6.0, 8.0, 10.0].to_vec())
        )
        .unwrap()
            - 1.0)
            .abs()
            < 1e-10
    );
    assert!(
        covariance(
            JsValue::from(data.to_vec()),
            JsValue::from([1.0, 2.0, 3.0, 4.0, 5.0].to_vec())
        )
        .unwrap()
            > 0.0
    );
}

// ---- Summation ----

#[wasm_bindgen_test]
fn summation_and_positive_run() {
    assert!(
        (kahan_sum(JsValue::from([1.0, 2.0, 3.0, 4.0].to_vec())).unwrap() - 10.0).abs() < 1e-10
    );
    assert!(
        (neumaier_sum(JsValue::from([1e16, 1.0, -1e16, 1.0].to_vec())).unwrap() - 2.0).abs()
            < 1e-10
    );
    assert_eq!(
        longest_positive_run(JsValue::from([1.0, 2.0, 3.0, -1.0, 2.0].to_vec())).unwrap(),
        3
    );
}

#[wasm_bindgen_test]
fn norm_cdf_reference_values() {
    assert!((norm_cdf(JsValue::from(0.0)).unwrap() - 0.5).abs() < TOL);
    assert!((norm_cdf(JsValue::from(3.0)).unwrap() - 0.9987).abs() < 1e-3);
}

#[wasm_bindgen_test]
fn norm_pdf_at_zero() {
    assert!((norm_pdf(JsValue::from(0.0)).unwrap() - 0.3989).abs() < TOL);
}

#[wasm_bindgen_test]
fn standard_normal_inv_cdf_reference_values() {
    assert!(standard_normal_inv_cdf(JsValue::from(0.5)).unwrap().abs() < TOL);
    assert!((standard_normal_inv_cdf(JsValue::from(0.975)).unwrap() - 1.96).abs() < 1e-2);
}

#[wasm_bindgen_test]
fn erf_reference_values() {
    assert_eq!(erf(JsValue::from(0.0)).unwrap(), 0.0);
    assert!((erf(JsValue::from(1.0)).unwrap() - 0.8427).abs() < TOL);
}

#[wasm_bindgen_test]
fn ln_gamma_reference_values() {
    assert!(ln_gamma(JsValue::from(1.0)).unwrap().abs() < TOL);
    assert!((ln_gamma(JsValue::from(5.0)).unwrap() - 24f64.ln()).abs() < TOL);
}

#[wasm_bindgen_test]
fn norm_cdf_extremes() {
    assert!(norm_cdf(JsValue::from(-10.0)).unwrap() < 1e-15);
    assert!((norm_cdf(JsValue::from(10.0)).unwrap() - 1.0).abs() < 1e-15);
}

#[wasm_bindgen_test]
fn erf_negative_symmetry() {
    let pos = erf(JsValue::from(1.0)).unwrap();
    let neg = erf(JsValue::from(-1.0)).unwrap();
    assert!((pos + neg).abs() < 1e-12, "erf is odd");
}
