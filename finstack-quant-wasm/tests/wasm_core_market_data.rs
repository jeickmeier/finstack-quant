//! wasm-bindgen-test suite for `api::core` market-data and date bindings.

#![cfg(target_arch = "wasm32")]

use finstack_quant_wasm::api::core::dates::{create_date, DayCount, DayCountContext, Tenor};
use finstack_quant_wasm::api::core::market_data::{
    FxConversionPolicy, FxDeltaVolSurface, FxMatrix,
};
use wasm_bindgen_test::*;

#[wasm_bindgen_test]
fn fx_matrix_rate_returns_structured_result() {
    let matrix = FxMatrix::new();
    matrix.set_quote("EUR", "USD", 1.10).unwrap();

    let result = matrix
        .rate(
            "EUR",
            "USD",
            "2024-01-02",
            &FxConversionPolicy::cashflow_date(),
        )
        .unwrap();

    assert!((result.rate() - 1.10).abs() < 1e-12);
    assert!(!result.triangulated());
}

#[wasm_bindgen_test]
fn fx_matrix_rate_defaults_policy_to_cashflow_date() {
    let matrix = FxMatrix::new();
    matrix.set_quote("GBP", "USD", 1.25).unwrap();

    let result = matrix.rate_default("GBP", "USD", "2024-01-02").unwrap();

    assert!((result.rate() - 1.25).abs() < 1e-12);
    assert!(!result.triangulated());
}

#[wasm_bindgen_test]
fn fx_matrix_policy_can_be_reused() {
    let matrix = FxMatrix::new();
    let policy = FxConversionPolicy::cashflow_date();
    matrix
        .set_quote_on("EUR", "USD", "2024-01-02", &policy, 1.10)
        .unwrap();
    let first = matrix.rate("EUR", "USD", "2024-01-02", &policy).unwrap();
    let second = matrix.rate("EUR", "USD", "2024-01-02", &policy).unwrap();
    assert_eq!(first.rate(), second.rate());
}

#[wasm_bindgen_test]
fn fx_delta_vol_surface_basic_accessors_and_implied_vol() {
    let surface = FxDeltaVolSurface::new(
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
    assert_eq!(surface.expiries(), vec![0.25, 0.5, 1.0]);

    let pillar = surface.pillar_vols(0).unwrap();
    assert!((pillar[0] - 0.08).abs() < 1e-12);

    // ATM-DNS strike at expiry 1.0 should recover the 0.09 ATM vol.
    let forward: f64 = 1.20;
    let atm_vol: f64 = 0.09;
    let k_atm = forward * (0.5 * atm_vol * atm_vol * 1.0_f64).exp();
    let vol = surface
        .implied_vol(1.0, k_atm, forward, 0.05, 0.03)
        .unwrap();
    assert!((vol - atm_vol).abs() < 1e-9);
}

#[wasm_bindgen_test]
fn fx_delta_vol_surface_rejects_mixed_10d_arguments() {
    match FxDeltaVolSurface::new(
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
            let msg = err.as_string().unwrap_or_default();
            assert!(msg.contains("rr10d"), "unexpected error message: {msg}");
        }
    }
}

#[wasm_bindgen_test]
fn day_count_context_supports_context_dependent_conventions() {
    let start = create_date(2024, 1, 1).unwrap();
    let end = create_date(2024, 7, 1).unwrap();

    assert!(DayCount::act_act_isma().year_fraction(start, end).is_err());

    let isma_ctx = DayCountContext::new().with_frequency(&Tenor::semi_annual());
    let isma = DayCount::act_act_isma()
        .year_fraction_with_context(start, end, &isma_ctx)
        .unwrap();
    assert!((isma - 0.5).abs() < 1e-12);

    let bus_ctx = DayCountContext::new().with_calendar("target2");
    let bus = DayCount::bus252()
        .year_fraction_with_context(start, end, &bus_ctx)
        .unwrap();
    assert!(bus > 0.0);
}

#[wasm_bindgen_test]
fn day_count_exposes_act365l_and_signed_fraction() {
    let start = create_date(2024, 1, 1).unwrap();
    let end = create_date(2025, 1, 1).unwrap();
    assert_eq!(
        DayCount::act365l()
            .signed_year_fraction(start, end)
            .unwrap(),
        1.0
    );
    assert_eq!(
        DayCount::act365l()
            .signed_year_fraction(end, start)
            .unwrap(),
        -1.0
    );
}
