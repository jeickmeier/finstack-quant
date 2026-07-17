//! Fourier pricing method bindings (COS) for WASM.
//!
//! Mirrors `finstack-quant-py`'s `valuations/fourier.rs` module: exposes the Fang-
//! Oosterlee (2008) COS method for European options under Black-Scholes,
//! Variance Gamma, and Merton jump-diffusion characteristic functions.
//!
//! All rates are continuously compounded decimals; `sigma` is annualized vol;
//! `maturity` is time to expiry in years.

use crate::utils::to_js_err;
use finstack_quant_valuations::pricer::cos::{
    bs_cos_price as rust_bs_cos_price, merton_jump_cos_price as rust_merton_jump_cos_price,
    vg_cos_price as rust_vg_cos_price, BlackScholesCosParams, MertonJumpCosParams,
    VarianceGammaCosParams,
};
use wasm_bindgen::prelude::*;

/// Price a European option under the Black-Scholes model using the COS method.
/// @param spot - Current spot price or exchange rate in the documented quote convention.
/// @param strike - Option strike price in the same price units as the underlying.
/// @param rate - Interest rate expressed as a decimal, such as 0.05 for 5%.
/// @param dividend - Continuous dividend yield expressed as a decimal, such as 0.02 for 2%.
/// @param vol - Annualized volatility expressed as a decimal, such as 0.20 for 20%.
/// @param maturity - Time to option expiry in years.
/// @param is_call - Whether to value a call (`true`) or put (`false`); defaults follow the callable's contract.
/// @param n_terms - Optional positive number of COS expansion terms; omit to use the pricer default.
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
    rust_bs_cos_price(BlackScholesCosParams {
        spot,
        strike,
        rate,
        dividend,
        vol,
        maturity,
        is_call,
        n_terms,
    })
    .map_err(to_js_err)
}

/// Price a European option under the Variance Gamma model using the COS method.
/// @param spot - Current spot price or exchange rate in the documented quote convention.
/// @param strike - Option strike price in the same price units as the underlying.
/// @param rate - Interest rate expressed as a decimal, such as 0.05 for 5%.
/// @param dividend - Continuous dividend yield expressed as a decimal, such as 0.02 for 2%.
/// @param sigma - Annualized volatility expressed as a decimal, such as 0.20 for 20%.
/// @param theta - Variance-Gamma drift parameter controlling skew in log returns.
/// @param nu - Variance-Gamma variance-rate parameter; larger values increase tail thickness.
/// @param maturity - Time to option expiry in years.
/// @param is_call - Whether to value a call (`true`) or put (`false`); defaults follow the callable's contract.
/// @param n_terms - Optional positive number of COS expansion terms; omit to use the pricer default.
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
    rust_vg_cos_price(VarianceGammaCosParams {
        spot,
        strike,
        rate,
        dividend,
        sigma,
        theta,
        nu,
        maturity,
        is_call,
        n_terms,
    })
    .map_err(to_js_err)
}

/// Price a European option under Merton (1976) jump-diffusion using the COS method.
/// @param spot - Current spot price or exchange rate in the documented quote convention.
/// @param strike - Option strike price in the same price units as the underlying.
/// @param rate - Interest rate expressed as a decimal, such as 0.05 for 5%.
/// @param dividend - Continuous dividend yield expressed as a decimal, such as 0.02 for 2%.
/// @param sigma - Annualized volatility expressed as a decimal, such as 0.20 for 20%.
/// @param mu_jump - Mean log jump size in the Merton jump-diffusion model.
/// @param sigma_jump - Standard deviation of log jump sizes in the Merton jump-diffusion model.
/// @param lambda - Annual jump-arrival intensity in the Merton jump-diffusion model.
/// @param maturity - Time to option expiry in years.
/// @param is_call - Whether to value a call (`true`) or put (`false`); defaults follow the callable's contract.
/// @param n_terms - Optional positive number of COS expansion terms; omit to use the pricer default.
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
    rust_merton_jump_cos_price(MertonJumpCosParams {
        spot,
        strike,
        rate,
        dividend,
        sigma,
        mu_jump,
        sigma_jump,
        lambda,
        maturity,
        is_call,
        n_terms,
    })
    .map_err(to_js_err)
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
