//! wasm-bindgen-test suite for `api::core` market-data and date bindings.

#![cfg(target_arch = "wasm32")]

use finstack_quant_wasm::api::core::dates::{create_date, JsDayCount, JsDayCountContext, JsTenor};
use finstack_quant_wasm::api::core::market_data::*;
use finstack_quant_wasm::api::core::market_data::{
    JsDiscountCurve, JsForwardCurve, JsFxConversionPolicy, JsFxDeltaVolSurface, JsFxMatrix,
    JsVolCube,
};
use finstack_quant_wasm::api::models::volatility::{
    get_cube_normal_vol, get_cube_normal_vol_clamped, get_fx_delta_pillar_vols, get_fx_delta_vol,
};
use js_sys::Float64Array;
use wasm_bindgen::JsValue;
use wasm_bindgen_test::*;

#[wasm_bindgen_test]
fn fx_matrix_rate_returns_structured_result() {
    let matrix = JsFxMatrix::new();
    matrix
        .set_quote(
            JsValue::from("EUR"),
            JsValue::from("USD"),
            JsValue::from(1.10),
        )
        .unwrap();

    let result = matrix
        .rate(
            JsValue::from("EUR"),
            JsValue::from("USD"),
            JsValue::from("2024-01-02"),
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

    assert!(
        (curve
            .rate_between(JsValue::from(0.0), JsValue::from(t_3m))
            .expect("first period")
            - 0.04)
            .abs()
            < 1e-14
    );
    assert!(
        (curve
            .rate_between(JsValue::from(t_3m), JsValue::from(t_6m))
            .expect("second period")
            - 0.045)
            .abs()
            < 1e-14
    );
    assert!(curve
        .rate_between(JsValue::from(t_3m), JsValue::from(t_3m))
        .is_err());
    assert!(curve
        .rate_between(JsValue::from(t_6m), JsValue::from(t_3m))
        .is_err());

    let grid = Float64Array::new(&curve.projection_grid());
    assert_eq!(grid.length(), 3);
    assert!((grid.get_index(1) - t_3m).abs() < 1e-14);
    assert_eq!(curve.reset_lag(), 3);
}

/// `DiscountCurve` options object built from JSON text.
fn discount_options(options: serde_json::Value) -> JsValue {
    js_sys::JSON::parse(&options.to_string()).expect("valid options")
}

#[wasm_bindgen_test]
fn discount_curve_negative_rate_validation_mode_is_explicit() {
    let chf = serde_json::json!({
        "id": "CHF-OIS",
        "baseDate": "2025-01-01",
        "knots": [0.0, 1.0, 1.0, 1.002],
    });
    assert!(JsDiscountCurve::new(discount_options(chf.clone())).is_err());

    let mut friendly = chf.clone();
    friendly["validationMode"] = "negative_rate_friendly".into();
    friendly["forwardFloor"] = (-0.01).into();
    let curve = JsDiscountCurve::new(discount_options(friendly.clone()))
        .expect("negative-rate-friendly curve");
    assert!(
        curve
            .forward(JsValue::from(0.0), JsValue::from(1.0))
            .expect("negative forward")
            < 0.0
    );

    // A non-finite floor cannot reach Rust through JSON; a floor without the
    // negative-rate preset is rejected by `ValidationMode::from_preset`.
    let mut stray_floor = chf.clone();
    stray_floor["forwardFloor"] = (-0.01).into();
    assert!(JsDiscountCurve::new(discount_options(stray_floor)).is_err());

    let mut unknown_key = friendly;
    unknown_key["forward_floor"] = (-0.01).into();
    assert!(JsDiscountCurve::new(discount_options(unknown_key)).is_err());
}

#[wasm_bindgen_test]
fn fx_matrix_rate_defaults_policy_to_cashflow_date() {
    let matrix = JsFxMatrix::new();
    matrix
        .set_quote(
            JsValue::from("GBP"),
            JsValue::from("USD"),
            JsValue::from(1.25),
        )
        .unwrap();

    let result = matrix
        .rate_with_default_policy(
            JsValue::from("GBP"),
            JsValue::from("USD"),
            JsValue::from("2024-01-02"),
        )
        .unwrap();

    assert!((result.rate() - 1.25).abs() < 1e-12);
    assert!(!result.triangulated());
}

#[wasm_bindgen_test]
fn fx_matrix_policy_can_be_reused() {
    let matrix = JsFxMatrix::new();
    let policy = JsFxConversionPolicy::cashflow_date();
    matrix
        .set_quote_on(
            JsValue::from("EUR"),
            JsValue::from("USD"),
            JsValue::from("2024-01-02"),
            &policy,
            JsValue::from(1.10),
        )
        .unwrap();
    let first = matrix
        .rate(
            JsValue::from("EUR"),
            JsValue::from("USD"),
            JsValue::from("2024-01-02"),
            &policy,
        )
        .unwrap();
    let second = matrix
        .rate(
            JsValue::from("EUR"),
            JsValue::from("USD"),
            JsValue::from("2024-01-02"),
            &policy,
        )
        .unwrap();
    assert_eq!(first.rate(), second.rate());
}

#[wasm_bindgen_test]
fn fx_delta_vol_surface_basic_accessors_and_implied_vol() {
    let surface = JsFxDeltaVolSurface::new(
        JsValue::from("EURUSD-DELTA-VOL"),
        JsValue::from([0.25, 0.5, 1.0].to_vec()),
        JsValue::from([0.08, 0.085, 0.09].to_vec()),
        JsValue::from([0.01, 0.012, 0.015].to_vec()),
        JsValue::from([0.005, 0.006, 0.007].to_vec()),
        None,
        None,
    )
    .unwrap();

    assert_eq!(surface.id(), "EURUSD-DELTA-VOL");
    assert_eq!(surface.num_expiries(), 3);
    assert_eq!(surface.expiries().as_ref(), [0.25, 0.5, 1.0]);

    let pillar = get_fx_delta_pillar_vols(&surface, JsValue::from(0)).unwrap();
    assert!((pillar[0] - 0.08).abs() < 1e-12);

    // ATM-DNS strike at expiry 1.0 should recover the 0.09 ATM vol.
    let forward: f64 = 1.20;
    let atm_vol: f64 = 0.09;
    let k_atm = forward * (0.5 * atm_vol * atm_vol * 1.0_f64).exp();
    let vol = get_fx_delta_vol(
        &surface,
        JsValue::from(1.0),
        JsValue::from(k_atm),
        JsValue::from(forward),
    )
    .unwrap();
    assert!((vol - atm_vol).abs() < 1e-9);
}

#[wasm_bindgen_test]
fn fx_delta_vol_surface_rejects_mixed_10d_arguments() {
    match JsFxDeltaVolSurface::new(
        JsValue::from("BAD"),
        JsValue::from([0.25, 0.5].to_vec()),
        JsValue::from([0.08, 0.085].to_vec()),
        JsValue::from([0.01, 0.012].to_vec()),
        JsValue::from([0.005, 0.006].to_vec()),
        Some(JsValue::from(vec![0.018, 0.020])),
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
            assert!(
                msg.contains("rr_10d and bf_10d must both be provided or both omitted"),
                "unexpected error message: {msg}"
            );
            assert_eq!(kind, "validation");
        }
    }
}

#[wasm_bindgen_test]
fn normal_sabr_requires_positive_shifted_levels_when_beta_is_positive() {
    let cev = JsVolCube::new(
        JsValue::from("CEV"),
        JsValue::from([1.0].to_vec()),
        JsValue::from([2.0].to_vec()),
        JsValue::from([0.01, 0.5, -0.2, 0.4, f64::NAN].to_vec()),
        JsValue::from([-0.01].to_vec()),
        None,
    )
    .unwrap();
    assert!(get_cube_normal_vol(
        &cev,
        JsValue::from(1.0),
        JsValue::from(2.0),
        JsValue::from(-0.01)
    )
    .is_err());
    assert!(get_cube_normal_vol_clamped(
        &cev,
        JsValue::from(1.0),
        JsValue::from(2.0),
        JsValue::from(-0.01)
    )
    .unwrap()
    .is_nan());

    let normal = JsVolCube::new(
        JsValue::from("NORMAL"),
        JsValue::from([1.0].to_vec()),
        JsValue::from([2.0].to_vec()),
        JsValue::from([0.01, 0.0, -0.2, 0.4, f64::NAN].to_vec()),
        JsValue::from([-0.01].to_vec()),
        None,
    )
    .unwrap();
    assert!(get_cube_normal_vol(
        &normal,
        JsValue::from(1.0),
        JsValue::from(2.0),
        JsValue::from(-0.02)
    )
    .unwrap()
    .is_finite());
}

#[wasm_bindgen_test]
fn day_count_context_supports_context_dependent_conventions() {
    let start = create_date(JsValue::from(2024), JsValue::from(1), JsValue::from(1)).unwrap();
    let end = create_date(JsValue::from(2024), JsValue::from(7), JsValue::from(1)).unwrap();

    assert!(JsDayCount::act_act_isma()
        .year_fraction(
            JsValue::from(start),
            JsValue::from(end),
            &JsDayCountContext::new()
        )
        .is_err());

    let isma_ctx = JsDayCountContext::new().with_frequency(&JsTenor::semi_annual());
    let isma = JsDayCount::act_act_isma()
        .year_fraction(JsValue::from(start), JsValue::from(end), &isma_ctx)
        .unwrap();
    assert!((isma - 0.5).abs() < 1e-12);

    let bus_ctx = JsDayCountContext::new()
        .with_calendar(JsValue::from("target2"))
        .expect("context");
    let bus = JsDayCount::bus252()
        .year_fraction(JsValue::from(start), JsValue::from(end), &bus_ctx)
        .unwrap();
    assert!(bus > 0.0);
}

#[wasm_bindgen_test]
fn day_count_exposes_act365l_and_signed_fraction() {
    let start = create_date(JsValue::from(2024), JsValue::from(1), JsValue::from(1)).unwrap();
    let end = create_date(JsValue::from(2025), JsValue::from(1), JsValue::from(1)).unwrap();
    assert_eq!(
        JsDayCount::act365l()
            .signed_year_fraction(
                JsValue::from(start),
                JsValue::from(end),
                &JsDayCountContext::new()
            )
            .unwrap(),
        1.0
    );
    assert_eq!(
        JsDayCount::act365l()
            .signed_year_fraction(
                JsValue::from(end),
                JsValue::from(start),
                &JsDayCountContext::new()
            )
            .unwrap(),
        -1.0
    );
}

#[wasm_bindgen_test]
fn discount_curve_new_and_accessors() {
    let curve = JsDiscountCurve::new(discount_options(serde_json::json!({
        "id": "USD-OIS",
        "baseDate": "2024-01-15",
        "knots": [0.5, 0.99, 1.0, 0.98, 2.0, 0.96],
    })))
    .expect("discount curve");
    assert_eq!(curve.id(), "USD-OIS");
    assert_eq!(curve.base_date(), "2024-01-15");
    assert!((curve.df(JsValue::from(0.5)).unwrap() - 0.99).abs() < 1e-6);
    assert!((curve.df(JsValue::from(1.0)).unwrap() - 0.98).abs() < 1e-6);
    assert!(curve.zero(JsValue::from(1.0)).unwrap() > 0.0);
    let f = curve
        .forward(JsValue::from(0.5), JsValue::from(1.0))
        .expect("forward rate");
    assert!(f > 0.0);
}

#[wasm_bindgen_test]
fn discount_curve_flat_uses_continuous_compounding() {
    let curve = JsDiscountCurve::flat(
        JsValue::from("USD-OIS"),
        JsValue::from("2024-01-15"),
        JsValue::from(0.04),
    )
    .expect("flat discount curve");

    for t in [0.0_f64, 0.25, 1.0, 5.0, 30.0] {
        assert!((curve.df(JsValue::from(t)).unwrap() - (-0.04 * t).exp()).abs() < 1e-12);
    }
    assert!(
        (curve
            .forward(JsValue::from(2.0), JsValue::from(9.0))
            .expect("flat forward")
            - 0.04)
            .abs()
            < 1e-12
    );
}

#[wasm_bindgen_test]
fn fx_matrix_quote_and_rate() {
    let m = JsFxMatrix::new();
    m.set_quote(
        JsValue::from("USD"),
        JsValue::from("EUR"),
        JsValue::from(0.92),
    )
    .expect("set quote");
    let r = m
        .rate_with_default_policy(
            JsValue::from("USD"),
            JsValue::from("EUR"),
            JsValue::from("2024-01-15"),
        )
        .expect("fx rate");
    assert!((r.rate() - 0.92).abs() < 1e-9);
    assert!(!r.triangulated());
}

#[wasm_bindgen_test]
fn fx_pair_convention_helpers() {
    let conv =
        fx_pair_convention(JsValue::from("USD"), JsValue::from("JPY")).expect("USDJPY convention");
    assert_eq!(conv.base().code(), "USD");
    assert_eq!(conv.quote().code(), "JPY");
    assert_eq!(conv.usd_quotation().to_string(), "indirect");
    assert!((conv.pip_size() - 0.01).abs() < 1e-12);
    assert_eq!(conv.settlement_days(), 2);
    assert!(
        (fx_pip_size(JsValue::from("EUR"), JsValue::from("USD")).expect("EURUSD pip") - 0.0001)
            .abs()
            < 1e-12
    );
    let inverted = invert_fx_rate(JsValue::from(1.10)).expect("positive rate");
    assert!((inverted - 1.0 / 1.10).abs() < 1e-12);
    assert_eq!(
        JsFxQuoteConvention::from_name(JsValue::from("direct"))
            .expect("direct")
            .to_string(),
        "direct"
    );
}
