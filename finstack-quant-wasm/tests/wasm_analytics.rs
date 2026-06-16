//! wasm-bindgen-test suite for `api::analytics`.
//!
//! Covers the `Performance` panel facade — the entire WASM analytics surface
//! after the Performance-only consolidation.

#![cfg(target_arch = "wasm32")]

use finstack_quant_wasm::api::analytics::WasmPerformance;
use js_sys::{Array, Float64Array, Reflect};
use serde::Deserialize;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_test::*;

const API_INVARIANTS_FIXTURE: &str =
    include_str!("../../finstack-quant/analytics/src/api_invariants_data.json");

#[derive(Deserialize)]
struct AnalyticsFixture {
    returns: Vec<f64>,
    benchmark: Vec<f64>,
    factors: Vec<Vec<f64>>,
    dates: Vec<String>,
}

fn fixture() -> AnalyticsFixture {
    serde_json::from_str(API_INVARIANTS_FIXTURE).unwrap()
}

fn to_js<T: serde::Serialize>(value: &T) -> JsValue {
    serde_wasm_bindgen::to_value(value).unwrap()
}

fn to_f64_matrix(rows: &[Vec<f64>]) -> JsValue {
    let array = Array::new_with_length(rows.len() as u32);
    for (i, row) in rows.iter().enumerate() {
        let row_value: JsValue = Float64Array::from(row.as_slice()).into();
        array.set(i as u32, row_value);
    }
    array.into()
}

/// Deserialize a binding return that is now a `Float64Array` into a `Vec<f64>`.
fn typed_vec(value: JsValue) -> Vec<f64> {
    value
        .dyn_into::<Float64Array>()
        .expect("expected Float64Array")
        .to_vec()
}

/// Deserialize a binding return that is now `Array<Float64Array>` into a `Vec<Vec<f64>>`.
fn typed_matrix(value: JsValue) -> Vec<Vec<f64>> {
    let outer = value.dyn_into::<Array>().expect("expected outer Array");
    let mut rows = Vec::with_capacity(outer.length() as usize);
    for i in 0..outer.length() {
        rows.push(typed_vec(outer.get(i)));
    }
    rows
}

/// Read a key from a plain JS object and convert to f64 array.
fn obj_typed_vec(obj: &JsValue, key: &str) -> Vec<f64> {
    typed_vec(Reflect::get(obj, &JsValue::from_str(key)).expect("missing field"))
}

/// Build a two-ticker Performance ("TARGET", "BENCH") from the fixture's
/// return + benchmark series, with TARGET as the benchmark for greeks etc.
fn build_perf() -> WasmPerformance {
    let fx = fixture();
    // returns matrix is column-major in the binding: returns[i] is the column
    // for ticker i. Two columns => target series, benchmark series.
    let returns = vec![fx.returns.clone(), fx.benchmark.clone()];
    let names = vec!["TARGET".to_string(), "BENCH".to_string()];
    WasmPerformance::from_returns(
        to_js(&fx.dates),
        to_f64_matrix(&returns),
        to_js(&names),
        Some("BENCH".to_string()),
        Some("daily".to_string()),
    )
    .unwrap()
}

// ---- Construction ----

#[wasm_bindgen_test]
fn from_returns_exposes_active_dates() {
    let perf = build_perf();
    // `dates()` is the full constructed grid; `activeDates()` matches it
    // until a window is set.
    let dates = perf.dates();
    assert_eq!(dates.len(), fixture().dates.len());
    let active = perf.active_dates();
    assert_eq!(active, dates);
}

#[wasm_bindgen_test]
fn from_returns_exposes_ticker_specific_active_dates() {
    let dates = vec![
        "2025-01-01".to_string(),
        "2025-01-02".to_string(),
        "2025-01-03".to_string(),
        "2025-01-04".to_string(),
    ];
    let returns = vec![
        vec![0.01, 0.02, 0.03, 0.04],
        vec![f64::NAN, 0.04, 0.06, f64::NAN],
    ];
    let names = vec!["BENCH".to_string(), "PORT".to_string()];
    let perf = WasmPerformance::from_returns(
        to_js(&dates),
        to_f64_matrix(&returns),
        to_js(&names),
        Some("BENCH".to_string()),
        Some("daily".to_string()),
    )
    .unwrap();

    assert_eq!(
        perf.active_dates_for_ticker(1).unwrap(),
        vec!["2025-01-02".to_string(), "2025-01-03".to_string()]
    );
}

#[wasm_bindgen_test]
fn ticker_names_round_trip() {
    let perf = build_perf();
    let names: Vec<String> = serde_wasm_bindgen::from_value(perf.ticker_names().unwrap()).unwrap();
    assert_eq!(names, vec!["TARGET", "BENCH"]);
    assert_eq!(perf.benchmark_idx(), 1);
    assert_eq!(perf.freq(), "daily");
}

// ---- Scalar metrics ----

#[wasm_bindgen_test]
fn cagr_returns_per_ticker_vec() {
    let perf = build_perf();
    let values = typed_vec(perf.cagr().unwrap());
    assert_eq!(values.len(), 2);
    assert!(values.iter().all(|v| v.is_finite()));
}

#[wasm_bindgen_test]
fn sharpe_sortino_volatility_finite() {
    let perf = build_perf();
    for raw in [
        perf.sharpe(Some(0.0)),
        perf.sortino(Some(0.0)),
        perf.volatility(Some(true)),
        perf.mean_return(Some(true)),
    ] {
        let v = typed_vec(raw);
        assert_eq!(v.len(), 2);
        assert!(v.iter().all(|x| x.is_finite()));
    }
}

#[wasm_bindgen_test]
fn tail_metrics_finite() {
    let perf = build_perf();
    for raw in [
        perf.value_at_risk(Some(0.95)),
        perf.expected_shortfall(Some(0.95)),
        perf.parametric_var(Some(0.95)),
        perf.cornish_fisher_var(Some(0.95)),
        perf.tail_ratio(Some(0.95)),
    ] {
        let v = typed_vec(raw);
        assert_eq!(v.len(), 2);
        assert!(v.iter().all(|x| x.is_finite()));
    }
}

#[wasm_bindgen_test]
fn drawdown_scalars_match_panel_width() {
    let perf = build_perf();
    let max_dd = typed_vec(perf.max_drawdown());
    assert_eq!(max_dd.len(), 2);
    assert!(max_dd.iter().all(|v| v <= &0.0));

    let calmar = typed_vec(perf.calmar().unwrap());
    assert_eq!(calmar.len(), 2);
}

// ---- Vector outputs ----

#[wasm_bindgen_test]
fn cumulative_and_drawdown_series_are_per_ticker_panels() {
    let perf = build_perf();
    let cum = typed_matrix(perf.cumulative_returns());
    assert_eq!(cum.len(), 2);
    assert_eq!(cum[0].len(), fixture().dates.len());

    let dd = typed_matrix(perf.drawdown_series());
    assert_eq!(dd.len(), 2);
    assert!(dd[0].iter().all(|v| v <= &0.0));
}

#[wasm_bindgen_test]
fn correlation_matrix_is_square() {
    let perf = build_perf();
    let mat = typed_matrix(perf.correlation_matrix());
    assert_eq!(mat.len(), 2);
    assert_eq!(mat[0].len(), 2);
    assert!((mat[0][0] - 1.0).abs() < 1e-12);
}

// ---- Benchmark / greeks ----

#[wasm_bindgen_test]
fn beta_and_greeks_return_per_ticker_structs() {
    let perf = build_perf();
    let betas: serde_json::Value = serde_wasm_bindgen::from_value(perf.beta().unwrap()).unwrap();
    assert_eq!(betas.as_array().unwrap().len(), 2);

    let greeks: serde_json::Value =
        serde_wasm_bindgen::from_value(perf.greeks(None).unwrap()).unwrap();
    assert_eq!(greeks.as_array().unwrap().len(), 2);
}

#[wasm_bindgen_test]
fn greeks_optional_risk_free_rate_changes_alpha() {
    let dates = vec![
        "2025-01-01".to_string(),
        "2025-01-02".to_string(),
        "2025-01-03".to_string(),
        "2025-01-04".to_string(),
        "2025-01-05".to_string(),
    ];
    let benchmark = vec![-0.02, -0.01, 0.0, 0.01, 0.02];
    let target: Vec<f64> = benchmark.iter().map(|&b| 2.0 * b).collect();
    let names = vec!["TARGET".to_string(), "BENCH".to_string()];
    let perf = WasmPerformance::from_returns(
        to_js(&dates),
        to_f64_matrix(&[target, benchmark]),
        to_js(&names),
        Some("BENCH".to_string()),
        Some("monthly".to_string()),
    )
    .unwrap();

    let zero: serde_json::Value =
        serde_wasm_bindgen::from_value(perf.greeks(None).unwrap()).unwrap();
    let nonzero: serde_json::Value =
        serde_wasm_bindgen::from_value(perf.greeks(Some(0.12)).unwrap()).unwrap();
    let zero_alpha = zero[0]["alpha"].as_f64().unwrap();
    let nonzero_alpha = nonzero[0]["alpha"].as_f64().unwrap();

    assert!(zero_alpha.abs() < 1e-12);
    assert!(nonzero_alpha > zero_alpha);
}

#[wasm_bindgen_test]
fn rolling_greeks_emit_dates_alphas_betas() {
    let perf = build_perf();
    let raw = perf.rolling_greeks(0, Some(5), None).unwrap();
    let date_array: Array = Reflect::get(&raw, &JsValue::from_str("dates"))
        .unwrap()
        .dyn_into()
        .unwrap();
    let alphas = obj_typed_vec(&raw, "alphas");
    let betas = obj_typed_vec(&raw, "betas");
    assert_eq!(date_array.length() as usize, alphas.len());
    assert_eq!(alphas.len(), betas.len());
    assert!(!alphas.is_empty());
}

#[wasm_bindgen_test]
fn rolling_returns_match_dated_series_shape() {
    let perf = build_perf();
    let raw = perf.rolling_returns(0, 3).unwrap();
    let date_array: Array = Reflect::get(&raw, &JsValue::from_str("dates"))
        .unwrap()
        .dyn_into()
        .unwrap();
    let values = obj_typed_vec(&raw, "return");
    assert_eq!(values.len(), date_array.length() as usize);
    assert!(!values.is_empty());
}

#[wasm_bindgen_test]
fn multi_factor_greeks_resolves_to_struct() {
    let perf = build_perf();
    let fx = fixture();
    let raw = perf
        .multi_factor_greeks(0, to_f64_matrix(&fx.factors))
        .unwrap();
    let value: serde_json::Value = serde_wasm_bindgen::from_value(raw).unwrap();
    assert!(value["alpha"].is_number());
    let betas = value["betas"].as_array().unwrap();
    assert_eq!(betas.len(), fx.factors.len());
}

// ---- Lookback & aggregation ----

#[wasm_bindgen_test]
fn lookback_returns_emit_mtd_qtd_ytd() {
    let perf = build_perf();
    let raw = perf
        .lookback_returns("2025-01-12", None, None, None)
        .unwrap();
    let value: serde_json::Value = serde_wasm_bindgen::from_value(raw).unwrap();
    assert_eq!(value["mtd"].as_array().unwrap().len(), 2);
    assert_eq!(value["qtd"].as_array().unwrap().len(), 2);
    assert_eq!(value["ytd"].as_array().unwrap().len(), 2);
}

#[wasm_bindgen_test]
fn period_stats_emit_win_rate() {
    let perf = build_perf();
    let raw = perf
        .period_stats(0, Some("weekly".to_string()), None, None)
        .unwrap();
    let value: serde_json::Value = serde_wasm_bindgen::from_value(raw).unwrap();
    assert!(value["win_rate"].as_f64().is_some());
}

// ---- Mutators ----

#[wasm_bindgen_test]
fn reset_date_range_narrows_active_dates() {
    let mut perf = build_perf();
    let full_len = perf.dates().len();
    perf.reset_date_range("2025-01-05", "2025-01-10").unwrap();
    let active = perf.active_dates();
    assert!(active.first().map(String::as_str) == Some("2025-01-05"));
    assert!(active.last().map(String::as_str) == Some("2025-01-10"));
    // `dates()` keeps Rust semantics: the full constructed grid.
    assert_eq!(perf.dates().len(), full_len);
}

#[wasm_bindgen_test]
fn reset_bench_ticker_updates_index() {
    let mut perf = build_perf();
    perf.reset_bench_ticker("TARGET").unwrap();
    assert_eq!(perf.benchmark_idx(), 0);
}
