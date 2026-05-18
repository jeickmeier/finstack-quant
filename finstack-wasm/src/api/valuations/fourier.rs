//! Fourier pricing method bindings (COS) for WASM.
//!
//! Mirrors `finstack-py`'s `valuations/fourier.rs` module: exposes the Fang-
//! Oosterlee (2008) COS method for European options under Black-Scholes,
//! Variance Gamma, and Merton jump-diffusion characteristic functions.
//!
//! All rates are continuously compounded decimals; `sigma` is annualized vol;
//! `maturity` is time to expiry in years.

use crate::utils::to_js_err;
use finstack_core::math::characteristic_function::{BlackScholesCf, MertonJumpCf, VarianceGammaCf};
use finstack_valuations::pricer::fourier::{CosConfig, CosPricer};
use wasm_bindgen::prelude::*;

/// Build a [`CosConfig`] for the COS pricer.
///
/// When `n_terms` is `None` the term count is taken from
/// [`CosConfig::default()`] — the single canonical source for the COS
/// expansion default, shared with the Python binding (`fourier.rs`). No COS
/// default is hardcoded in the binding layer, so the Python and WASM defaults
/// cannot drift from the core.
fn cos_config(n_terms: Option<usize>) -> CosConfig {
    let default = CosConfig::default();
    CosConfig {
        num_terms: n_terms.unwrap_or(default.num_terms),
        ..default
    }
}

/// Price a European option under the Black-Scholes model using the COS method.
#[wasm_bindgen(js_name = bsCosPrice)]
#[allow(clippy::too_many_arguments)]
pub fn bs_cos_price(
    spot: f64,
    strike: f64,
    rate: f64,
    dividend: f64,
    vol: f64,
    maturity: f64,
    is_call: bool,
    n_terms: Option<usize>,
) -> Result<f64, JsValue> {
    let cf = BlackScholesCf {
        r: rate,
        q: dividend,
        sigma: vol,
    };
    let pricer = CosPricer::new(&cf, cos_config(n_terms));
    if is_call {
        pricer
            .price_call(spot, strike, rate, maturity)
            .map_err(to_js_err)
    } else {
        pricer
            .price_put(spot, strike, rate, maturity)
            .map_err(to_js_err)
    }
}

/// Price a European option under the Variance Gamma model using the COS method.
#[wasm_bindgen(js_name = vgCosPrice)]
#[allow(clippy::too_many_arguments)]
pub fn vg_cos_price(
    spot: f64,
    strike: f64,
    rate: f64,
    dividend: f64,
    sigma: f64,
    theta: f64,
    nu: f64,
    maturity: f64,
    is_call: bool,
    n_terms: Option<usize>,
) -> Result<f64, JsValue> {
    let cf = VarianceGammaCf {
        r: rate,
        q: dividend,
        sigma,
        nu,
        theta,
    };
    let pricer = CosPricer::new(&cf, cos_config(n_terms));
    if is_call {
        pricer
            .price_call(spot, strike, rate, maturity)
            .map_err(to_js_err)
    } else {
        pricer
            .price_put(spot, strike, rate, maturity)
            .map_err(to_js_err)
    }
}

/// Price a European option under Merton (1976) jump-diffusion using the COS method.
#[wasm_bindgen(js_name = mertonJumpCosPrice)]
#[allow(clippy::too_many_arguments)]
pub fn merton_jump_cos_price(
    spot: f64,
    strike: f64,
    rate: f64,
    dividend: f64,
    sigma: f64,
    mu_jump: f64,
    sigma_jump: f64,
    lambda: f64,
    maturity: f64,
    is_call: bool,
    n_terms: Option<usize>,
) -> Result<f64, JsValue> {
    let cf = MertonJumpCf {
        r: rate,
        q: dividend,
        sigma,
        lambda,
        mu_j: mu_jump,
        sigma_j: sigma_jump,
    };
    let pricer = CosPricer::new(&cf, cos_config(n_terms));
    if is_call {
        pricer
            .price_call(spot, strike, rate, maturity)
            .map_err(to_js_err)
    } else {
        pricer
            .price_put(spot, strike, rate, maturity)
            .map_err(to_js_err)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bs_cos_call_atm_is_positive() {
        let p = bs_cos_price(100.0, 100.0, 0.05, 0.02, 0.2, 1.0, true, None).expect("price");
        assert!(p > 0.0);
    }
}
