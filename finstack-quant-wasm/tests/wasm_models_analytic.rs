//! wasm-bindgen-test suite for `api::models` closed-form and Fourier kernels.

#![cfg(target_arch = "wasm32")]

use finstack_quant_wasm::api::models::analytic::*;
use finstack_quant_wasm::api::models::fourier::*;
use wasm_bindgen::JsValue;
use wasm_bindgen_test::*;

#[wasm_bindgen_test]
fn barrier_put_dispatches() {
    let p = barrier_put(
        JsValue::from(100.0),
        JsValue::from(100.0),
        JsValue::from(80.0),
        JsValue::from(0.05),
        JsValue::from(0.0),
        JsValue::from(0.2),
        JsValue::from(1.0),
        JsValue::from("down"),
        JsValue::from("out"),
    )
    .expect("finite price");
    assert!(p > 0.0);
    assert!(barrier_put(
        JsValue::from(100.0),
        JsValue::from(100.0),
        JsValue::from(80.0),
        JsValue::from(0.05),
        JsValue::from(0.0),
        JsValue::from(0.2),
        JsValue::from(1.0),
        JsValue::from("sideways"),
        JsValue::from("out")
    )
    .is_err());
}

#[wasm_bindgen_test]
fn black76_and_bachelier_prices_are_positive() {
    assert!(
        black76_price(
            JsValue::from(100.0),
            JsValue::from(100.0),
            JsValue::from(0.95),
            JsValue::from(1.0),
            JsValue::from(0.2),
            JsValue::from(true)
        )
        .expect("price")
            > 0.0
    );
    assert!(
        bachelier_price(
            JsValue::from(0.03),
            JsValue::from(0.03),
            JsValue::from(0.0075),
            JsValue::from(1.0),
            JsValue::from(true)
        )
        .expect("price")
            > 0.0
    );
    assert!(
        black_shifted_price(
            JsValue::from(-0.005),
            JsValue::from(-0.005),
            JsValue::from(0.25),
            JsValue::from(1.0),
            JsValue::from(0.03),
            JsValue::from(true)
        )
        .expect("price")
            > 0.0
    );
    assert!(
        black_shifted_vega(
            JsValue::from(-0.005),
            JsValue::from(-0.005),
            JsValue::from(0.25),
            JsValue::from(1.0),
            JsValue::from(0.03)
        )
        .expect("vega")
            > 0.0
    );
}

#[wasm_bindgen_test]
fn black76_price_rejects_non_positive_df() {
    assert!(black76_price(
        JsValue::from(100.0),
        JsValue::from(100.0),
        JsValue::from(-0.95),
        JsValue::from(1.0),
        JsValue::from(0.2),
        JsValue::from(true)
    )
    .is_err());
    assert!(black76_price(
        JsValue::from(100.0),
        JsValue::from(100.0),
        JsValue::from(0.0),
        JsValue::from(1.0),
        JsValue::from(0.2),
        JsValue::from(true)
    )
    .is_err());
}

#[wasm_bindgen_test]
fn bs_price_call_atm_is_positive() {
    let p = bs_price(
        JsValue::from(100.0),
        JsValue::from(100.0),
        JsValue::from(0.05),
        JsValue::from(0.02),
        JsValue::from(0.2),
        JsValue::from(1.0),
        JsValue::from(true),
    )
    .expect("finite price");
    assert!(p > 0.0);
}

#[wasm_bindgen_test]
fn vanilla_expiry_payoff_call_itm() {
    let payoff = vanilla_expiry_payoff(
        JsValue::from(110.0),
        JsValue::from(100.0),
        JsValue::from(true),
    )
    .expect("finite payoff");
    assert!((payoff - 10.0).abs() < 1e-12);
}

#[wasm_bindgen_test]
fn vanilla_expiry_payoff_rejects_negative_spot() {
    assert!(vanilla_expiry_payoff(
        JsValue::from(-1.0),
        JsValue::from(100.0),
        JsValue::from(true)
    )
    .is_err());
    let put = vanilla_expiry_payoff(
        JsValue::from(0.0),
        JsValue::from(100.0),
        JsValue::from(false),
    )
    .expect("zero spot put");
    assert!((put - 100.0).abs() < 1e-12);
}

#[wasm_bindgen_test]
fn bs_implied_vol_recovers_sigma() {
    let sigma = 0.25;
    let price = bs_price(
        JsValue::from(100.0),
        JsValue::from(110.0),
        JsValue::from(0.03),
        JsValue::from(0.01),
        JsValue::from(sigma),
        JsValue::from(0.75),
        JsValue::from(true),
    )
    .expect("finite price");
    let iv = bs_implied_vol(
        JsValue::from(100.0),
        JsValue::from(110.0),
        JsValue::from(0.03),
        JsValue::from(0.01),
        JsValue::from(0.75),
        JsValue::from(price),
        JsValue::from(true),
    )
    .expect("solver should converge");
    assert!((iv - sigma).abs() < 1e-6, "iv={iv} sigma={sigma}");
}

#[wasm_bindgen_test]
fn bs_price_rejects_non_finite_result() {
    // A degenerate input (huge maturity with a negative rate) drives
    // `exp(-r*t)` to `+inf`, which escapes the core's `.max(0.0)` clamp.
    // The binding guard must surface that as a thrown error rather than a
    // silent non-finite value crossing the wasm boundary.
    let result = bs_price(
        JsValue::from(100.0),
        JsValue::from(100.0),
        JsValue::from(-1.0),
        JsValue::from(0.0),
        JsValue::from(0.2),
        JsValue::from(1.0e6),
        JsValue::from(false),
    );
    assert!(
        result.is_err(),
        "a non-finite Black-Scholes price must produce an error"
    );
    // A well-posed input still returns a finite price unchanged.
    assert!(bs_price(
        JsValue::from(100.0),
        JsValue::from(100.0),
        JsValue::from(0.05),
        JsValue::from(0.02),
        JsValue::from(0.2),
        JsValue::from(1.0),
        JsValue::from(true)
    )
    .is_ok());
}

#[wasm_bindgen_test]
fn quanto_option_price_rejects_non_finite_result() {
    // Same degenerate-maturity path: a non-finite quanto price must throw.
    let result = quanto_option_price(
        JsValue::from(100.0),
        JsValue::from(100.0),
        JsValue::from(1.0e6),
        JsValue::from(-1.0),
        JsValue::from(0.01),
        JsValue::from(0.0),
        JsValue::from(0.20),
        JsValue::from(0.10),
        JsValue::from(0.3),
        Some(JsValue::from(false)),
    );
    assert!(
        result.is_err(),
        "a non-finite quanto option price must produce an error"
    );
    // A well-posed input still returns a finite price.
    assert!(quanto_option_price(
        JsValue::from(100.0),
        JsValue::from(100.0),
        JsValue::from(1.0),
        JsValue::from(0.03),
        JsValue::from(0.01),
        JsValue::from(0.0),
        JsValue::from(0.20),
        JsValue::from(0.10),
        JsValue::from(0.3),
        Some(JsValue::from(true))
    )
    .is_ok());
}

#[wasm_bindgen_test]
fn bs_cos_call_atm_is_positive() {
    let p = bs_cos_price(
        JsValue::from(100.0),
        JsValue::from(100.0),
        JsValue::from(0.05),
        JsValue::from(0.02),
        JsValue::from(0.2),
        JsValue::from(1.0),
        JsValue::from(true),
        None,
    )
    .expect("price");
    assert!(p > 0.0);
}

#[wasm_bindgen_test]
fn heston_price_call_atm_is_reasonable() {
    let p = heston_price(
        JsValue::from(100.0),
        JsValue::from(100.0),
        JsValue::from(1.0),
        JsValue::from(0.05),
        JsValue::from(0.02),
        JsValue::from(2.0),
        JsValue::from(0.04),
        JsValue::from(0.3),
        JsValue::from(-0.7),
        JsValue::from(0.04),
        None,
    )
    .expect("finite price");
    assert!(p > 5.0 && p < 15.0, "price={p}");
}
