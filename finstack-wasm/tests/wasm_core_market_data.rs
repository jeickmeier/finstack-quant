//! wasm-bindgen-test suite for `api::core` market-data and date bindings.

#![cfg(target_arch = "wasm32")]

use finstack_wasm::api::core::dates::{create_date, DayCount, DayCountContext, Tenor};
use finstack_wasm::api::core::market_data::{FxConversionPolicy, FxMatrix};
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
            Some(FxConversionPolicy::cashflow_date()),
        )
        .unwrap();

    assert!((result.get_rate() - 1.10).abs() < 1e-12);
    assert!(!result.get_triangulated());
    assert_eq!(result.get_policy().to_string(), "cashflow_date");
}

#[wasm_bindgen_test]
fn fx_matrix_rate_defaults_policy_to_cashflow_date() {
    let matrix = FxMatrix::new();
    matrix.set_quote("GBP", "USD", 1.25).unwrap();

    let result = matrix.rate("GBP", "USD", "2024-01-02", None).unwrap();

    assert!((result.get_rate() - 1.25).abs() < 1e-12);
    assert_eq!(result.get_policy().to_string(), "cashflow_date");
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
