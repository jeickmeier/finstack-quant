//! wasm-bindgen-test suite for `api::models` implied-volatility adapters.
#![cfg(target_arch = "wasm32")]

use finstack_quant_wasm::api::models::analytic::{black76_implied_vol, bs_implied_vol, bs_price};
use wasm_bindgen_test::*;

#[wasm_bindgen_test]
fn implied_vol_adapters_reprice_calls_and_puts_with_carry() {
    for is_call in [true, false] {
        let price = bs_price(100.0, 105.0, -0.02, 0.01, 0.35, 0.5, is_call).expect("price");
        let bs = bs_implied_vol(100.0, 105.0, -0.02, 0.01, 0.5, price, is_call).expect("BS IV");
        let forward = 100.0 * (-0.03_f64 * 0.5).exp();
        let df = (0.02_f64 * 0.5).exp();
        let black = black76_implied_vol(forward, 105.0, df, 0.5, price, is_call).expect("Black IV");
        assert!((bs - 0.35).abs() < 1e-12);
        assert!((black - 0.35).abs() < 1e-12);
    }
    assert!(black76_implied_vol(100.0, 100.0, 1.0, 1.0, 100.0, true).is_err());
    assert!(bs_implied_vol(100.0, 100.0, 0.0, 0.0, 0.0, 5.0, true).is_err());
}
