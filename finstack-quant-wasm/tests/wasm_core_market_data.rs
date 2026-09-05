//! wasm-bindgen-test suite for `api::core` market-data and date bindings.

#![cfg(target_arch = "wasm32")]

use finstack_quant_wasm::api::core::dates::{create_date, JsDayCount, JsDayCountContext, JsTenor};
use finstack_quant_wasm::api::core::market_data::{
    JsDiscountCurve, JsForwardCurve, JsFxConversionPolicy, JsFxDeltaVolSurface, JsFxMatrix,
    JsVolCube,
};
use finstack_quant_wasm::api::models::volatility::{
    get_cube_normal_vol, get_cube_normal_vol_clamped, get_fx_delta_pillar_vols, get_fx_delta_vol,
};
use js_sys::Float64Array;
use wasm_bindgen_test::*;

#[wasm_bindgen_test]
fn fx_matrix_rate_returns_structured_result() {
    let matrix = JsFxMatrix::new();
    matrix.set_quote("EUR", "USD", 1.10).unwrap();

    let result = matrix
        .rate(
            "EUR",
            "USD",
            "2024-01-02",
            &JsFxConversionPolicy::cashflow_date(),
        )
        .unwrap();

    assert!((result.rate() - 1.10).abs() < 1e-12);
    assert!(!result.triangulated());
}

#[wasm_bindgen_test]
fn forward_curve_projection_grid_and_rate_between() {
    let t_3m = 91.0 / 360.0;
    let t_6m = 183.0 / 360.0;
    let options = js_sys::JSON::parse(
        &serde_json::json!({
            "id": "USD-SOFR-3M",
            "tenor": 0.25,
            "baseDate": "2025-01-01",
            "knots": [0.0, 0.04, t_3m, 0.045],
            "dayCount": "act_360",
            "interp": "linear",
            "extrapolation": "flat_forward",
            "projectionGrid": [0.0, t_3m, t_6m],
            "resetLag": 3
        })
        .to_string(),
    )
    .expect("valid options");
    let curve = JsForwardCurve::new(options).expect("forward curve");

    assert!((curve.rate_between(0.0, t_3m).expect("first period") - 0.04).abs() < 1e-14);
    assert!((curve.rate_between(t_3m, t_6m).expect("second period") - 0.045).abs() < 1e-14);
    assert!(curve.rate_between(t_3m, t_3m).is_err());
    assert!(curve.rate_between(t_6m, t_3m).is_err());

    let grid = Float64Array::new(&curve.projection_grid());
    assert_eq!(grid.length(), 3);
    assert!((grid.get_index(1) - t_3m).abs() < 1e-14);
    assert_eq!(curve.reset_lag(), 3);
}

#[wasm_bindgen_test]
fn discount_curve_negative_rate_validation_mode_is_explicit() {
    assert!(JsDiscountCurve::new(
        "CHF-OIS",
        "2025-01-01",
        &[0.0, 1.0, 1.0, 1.002],
        None,
        None,
        None,
        None,
        None,
    )
    .is_err());

    let curve = JsDiscountCurve::new(
        "CHF-OIS",
        "2025-01-01",
        &[0.0, 1.0, 1.0, 1.002],
        None,
        None,
        None,
        Some("negative_rate_friendly".to_string()),
        Some(-0.01),
    )
    .expect("negative-rate-friendly curve");
    assert!(curve.forward(0.0, 1.0).expect("negative forward") < 0.0);

    for floor in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(JsDiscountCurve::new(
            "CHF-OIS",
            "2025-01-01",
            &[0.0, 1.0, 1.0, 1.002],
            None,
            None,
            None,
            Some("negative_rate_friendly".to_string()),
            Some(floor),
        )
        .is_err());
    }
}

#[wasm_bindgen_test]
fn fx_matrix_rate_defaults_policy_to_cashflow_date() {
    let matrix = JsFxMatrix::new();
    matrix.set_quote("GBP", "USD", 1.25).unwrap();

    let result = matrix.rate_default("GBP", "USD", "2024-01-02").unwrap();

    assert!((result.rate() - 1.25).abs() < 1e-12);
    assert!(!result.triangulated());
}

#[wasm_bindgen_test]
fn fx_matrix_policy_can_be_reused() {
    let matrix = JsFxMatrix::new();
    let policy = JsFxConversionPolicy::cashflow_date();
    matrix
        .set_quote_on("EUR", "USD", "2024-01-02", &policy, 1.10)
        .unwrap();
    let first = matrix.rate("EUR", "USD", "2024-01-02", &policy).unwrap();
    let second = matrix.rate("EUR", "USD", "2024-01-02", &policy).unwrap();
    assert_eq!(first.rate(), second.rate());
}

#[wasm_bindgen_test]
fn fx_delta_vol_surface_basic_accessors_and_implied_vol() {
    let surface = JsFxDeltaVolSurface::new(
        "EURUSD-DELTA-VOL",
        &[0.25, 0.5, 1.0],
        &[0.08, 0.085, 0.09],
        &[0.01, 0.012, 0.015],
        &[0.005, 0.006, 0.007],
        None,
        None,
    )
    .unwrap();

    assert_eq!(surface.id(), "EURUSD-DELTA-VOL");
    assert_eq!(surface.num_expiries(), 3);
    assert_eq!(surface.expiries().as_ref(), [0.25, 0.5, 1.0]);

    let pillar = get_fx_delta_pillar_vols(&surface, 0).unwrap();
    assert!((pillar[0] - 0.08).abs() < 1e-12);

    // ATM-DNS strike at expiry 1.0 should recover the 0.09 ATM vol.
    let forward: f64 = 1.20;
    let atm_vol: f64 = 0.09;
    let k_atm = forward * (0.5 * atm_vol * atm_vol * 1.0_f64).exp();
    let vol = get_fx_delta_vol(&surface, 1.0, k_atm, forward).unwrap();
    assert!((vol - atm_vol).abs() < 1e-9);
}

#[wasm_bindgen_test]
fn fx_delta_vol_surface_rejects_mixed_10d_arguments() {
    match JsFxDeltaVolSurface::new(
        "BAD",
        &[0.25, 0.5],
        &[0.08, 0.085],
        &[0.01, 0.012],
        &[0.005, 0.006],
        Some(vec![0.018, 0.020]),
        None,
    ) {
        Ok(_) => panic!("mixed rr10d/bf10d must error"),
        Err(err) => {
            // Structured errors are `js_sys::Error` objects: the message lives
            // on `.message`, not on the value's string form.
            let msg = js_sys::Reflect::get(&err, &"message".into())
                .ok()
                .and_then(|m| m.as_string())
                .unwrap_or_default();
            let kind = js_sys::Reflect::get(&err, &"kind".into())
                .ok()
                .and_then(|k| k.as_string())
                .unwrap_or_default();
            assert!(msg.contains("rr10d"), "unexpected error message: {msg}");
            assert_eq!(kind, "validation");
        }
    }
}

#[wasm_bindgen_test]
fn normal_sabr_requires_positive_shifted_levels_when_beta_is_positive() {
    let cev = JsVolCube::new(
        "CEV",
        &[1.0],
        &[2.0],
        &[0.01, 0.5, -0.2, 0.4, f64::NAN],
        &[-0.01],
        None,
    )
    .unwrap();
    assert!(get_cube_normal_vol(&cev, 1.0, 2.0, -0.01).is_err());
    assert!(get_cube_normal_vol_clamped(&cev, 1.0, 2.0, -0.01).is_nan());

    let normal = JsVolCube::new(
        "NORMAL",
        &[1.0],
        &[2.0],
        &[0.01, 0.0, -0.2, 0.4, f64::NAN],
        &[-0.01],
        None,
    )
    .unwrap();
    assert!(get_cube_normal_vol(&normal, 1.0, 2.0, -0.02)
        .unwrap()
        .is_finite());
}

#[wasm_bindgen_test]
fn day_count_context_supports_context_dependent_conventions() {
    let start = create_date(2024, 1, 1).unwrap();
    let end = create_date(2024, 7, 1).unwrap();

    assert!(JsDayCount::act_act_isma()
        .year_fraction(start, end)
        .is_err());

    let isma_ctx = JsDayCountContext::new().with_frequency(&JsTenor::semi_annual());
    let isma = JsDayCount::act_act_isma()
        .year_fraction_with_context(start, end, &isma_ctx)
        .unwrap();
    assert!((isma - 0.5).abs() < 1e-12);

    let bus_ctx = JsDayCountContext::new().with_calendar("target2");
    let bus = JsDayCount::bus252()
        .year_fraction_with_context(start, end, &bus_ctx)
        .unwrap();
    assert!(bus > 0.0);
}

#[wasm_bindgen_test]
fn day_count_exposes_act365l_and_signed_fraction() {
    let start = create_date(2024, 1, 1).unwrap();
    let end = create_date(2025, 1, 1).unwrap();
    assert_eq!(
        JsDayCount::act365l()
            .signed_year_fraction(start, end)
            .unwrap(),
        1.0
    );
    assert_eq!(
        JsDayCount::act365l()
            .signed_year_fraction(end, start)
            .unwrap(),
        -1.0
    );
}
