//! Fourier pricing method bindings (COS) for WASM.
//!
//! Mirrors `finstack-quant-py`'s `models/fourier.rs` module: exposes the Fang-
//! Oosterlee (2008) COS method for European options under Black-Scholes,
//! Variance Gamma, and Merton jump-diffusion characteristic functions.
//!
//! All rates are continuously compounded decimals; `vol` / `sigma` are
//! annualized vols; `expiry` is time to expiry in years.
//!
//! Fang-Oosterlee (2008): see docs/REFERENCES.md#fang-oosterlee-2008.

use crate::utils::input::{js_bool, js_f64, js_opt_uint};
use crate::utils::to_js_err;
use finstack_quant_models::fourier::cos::{
    bs_cos_price as rust_bs_cos_price, merton_jump_cos_price as rust_merton_jump_cos_price,
    vg_cos_price as rust_vg_cos_price, BlackScholesCosParams, MertonJumpCosParams,
    VarianceGammaCosParams,
};
use wasm_bindgen::prelude::*;

/// Price a European option under the Black-Scholes model using the COS method.
///
/// Fang-Oosterlee (2008): see docs/REFERENCES.md#fang-oosterlee-2008.
/// Black-Scholes (1973): see docs/REFERENCES.md#black-scholes-1973.
/// @param spot - Current spot price or exchange rate in the same units as the strike.
/// @param strike - Option strike price in the same price units as the underlying.
/// @param rate - Interest rate expressed as a decimal, such as 0.05 for 5%.
/// @param div_yield - Continuous dividend yield expressed as a decimal, such as 0.02 for 2%.
/// @param vol - Annualized volatility expressed as a decimal, such as 0.20 for 20%; must be positive.
/// @param expiry - Time to option expiry in years.
/// @param is_call - Whether to value a call (`true`) or put (`false`).
/// @param n_terms - Optional number of COS expansion terms in `1..=65536`; omit to use the pricer default (128).
///
/// # Errors
///
/// Throws a `validation` error if `nTerms` is outside `1..=65536` or `vol` is
/// not positive, and a `computation` error if the model produces a degenerate
/// or invalid COS truncation range, a non-finite characteristic-function value
/// or forward moment, or a non-finite option price.
#[wasm_bindgen(js_name = bsCosPrice)]
#[allow(clippy::too_many_arguments)]
pub fn bs_cos_price(
    spot: JsValue,
    strike: JsValue,
    rate: JsValue,
    div_yield: JsValue,
    vol: JsValue,
    expiry: JsValue,
    is_call: JsValue,
    n_terms: Option<JsValue>,
) -> Result<f64, JsValue> {
    let spot = js_f64(&spot, "spot")?;
    let strike = js_f64(&strike, "strike")?;
    let rate = js_f64(&rate, "rate")?;
    let div_yield = js_f64(&div_yield, "divYield")?;
    let vol = js_f64(&vol, "vol")?;
    let expiry = js_f64(&expiry, "expiry")?;
    let is_call = js_bool(&is_call, "isCall")?;
    let n_terms: Option<usize> = js_opt_uint(n_terms.as_ref(), "nTerms")?;
    rust_bs_cos_price(BlackScholesCosParams {
        spot,
        strike,
        rate,
        div_yield,
        vol,
        expiry,
        is_call,
        n_terms,
    })
    .map_err(to_js_err)
}

/// Price a European option under the Variance Gamma model using the COS method.
///
/// Fang-Oosterlee (2008): see docs/REFERENCES.md#fang-oosterlee-2008.
/// Madan-Carr-Chang (1998): see docs/REFERENCES.md#madan-carr-chang-1998.
/// @param spot - Current spot price or exchange rate in the same units as the strike.
/// @param strike - Option strike price in the same price units as the underlying.
/// @param rate - Interest rate expressed as a decimal, such as 0.05 for 5%.
/// @param div_yield - Continuous dividend yield expressed as a decimal, such as 0.02 for 2%.
/// @param sigma - Annualized volatility expressed as a decimal, such as 0.20 for 20%.
/// @param theta - Variance-Gamma drift parameter controlling skew in log returns.
/// @param nu - Variance-Gamma variance-rate parameter; larger values increase tail thickness.
/// @param expiry - Time to option expiry in years.
/// @param is_call - Whether to value a call (`true`) or put (`false`).
/// @param n_terms - Optional number of COS expansion terms in `1..=65536`; omit to use the pricer default (128).
///
/// # Errors
///
/// Throws a `validation` error if `nTerms` is outside `1..=65536`, and a
/// `computation` error if the model produces a degenerate or invalid COS
/// truncation range, a non-finite characteristic-function value or forward
/// moment, or a non-finite option price.
#[wasm_bindgen(js_name = vgCosPrice)]
#[allow(clippy::too_many_arguments)]
pub fn vg_cos_price(
    spot: JsValue,
    strike: JsValue,
    rate: JsValue,
    div_yield: JsValue,
    sigma: JsValue,
    theta: JsValue,
    nu: JsValue,
    expiry: JsValue,
    is_call: JsValue,
    n_terms: Option<JsValue>,
) -> Result<f64, JsValue> {
    let spot = js_f64(&spot, "spot")?;
    let strike = js_f64(&strike, "strike")?;
    let rate = js_f64(&rate, "rate")?;
    let div_yield = js_f64(&div_yield, "divYield")?;
    let sigma = js_f64(&sigma, "sigma")?;
    let theta = js_f64(&theta, "theta")?;
    let nu = js_f64(&nu, "nu")?;
    let expiry = js_f64(&expiry, "expiry")?;
    let is_call = js_bool(&is_call, "isCall")?;
    let n_terms: Option<usize> = js_opt_uint(n_terms.as_ref(), "nTerms")?;
    rust_vg_cos_price(VarianceGammaCosParams {
        spot,
        strike,
        rate,
        div_yield,
        sigma,
        theta,
        nu,
        expiry,
        is_call,
        n_terms,
    })
    .map_err(to_js_err)
}

/// Price a European option under Merton (1976) jump-diffusion using the COS method.
///
/// Fang-Oosterlee (2008): see docs/REFERENCES.md#fang-oosterlee-2008.
/// Merton jump-diffusion (1976): see docs/REFERENCES.md#merton-1976-jump.
/// @param spot - Current spot price or exchange rate in the same units as the strike.
/// @param strike - Option strike price in the same price units as the underlying.
/// @param rate - Interest rate expressed as a decimal, such as 0.05 for 5%.
/// @param div_yield - Continuous dividend yield expressed as a decimal, such as 0.02 for 2%.
/// @param sigma - Annualized volatility expressed as a decimal, such as 0.20 for 20%.
/// @param mu_jump - Mean log jump size in the Merton jump-diffusion model.
/// @param sigma_jump - Standard deviation of log jump sizes in the Merton jump-diffusion model.
/// @param lambda - Annual jump-arrival intensity in the Merton jump-diffusion model.
/// @param expiry - Time to option expiry in years.
/// @param is_call - Whether to value a call (`true`) or put (`false`).
/// @param n_terms - Optional number of COS expansion terms in `1..=65536`; omit to use the pricer default (128).
///
/// # Errors
///
/// Throws a `validation` error if `nTerms` is outside `1..=65536`, and a
/// `computation` error if the model produces a degenerate or invalid COS
/// truncation range, a non-finite characteristic-function value or forward
/// moment, or a non-finite option price.
#[wasm_bindgen(js_name = mertonJumpCosPrice)]
#[allow(clippy::too_many_arguments)]
pub fn merton_jump_cos_price(
    spot: JsValue,
    strike: JsValue,
    rate: JsValue,
    div_yield: JsValue,
    sigma: JsValue,
    mu_jump: JsValue,
    sigma_jump: JsValue,
    lambda: JsValue,
    expiry: JsValue,
    is_call: JsValue,
    n_terms: Option<JsValue>,
) -> Result<f64, JsValue> {
    let spot = js_f64(&spot, "spot")?;
    let strike = js_f64(&strike, "strike")?;
    let rate = js_f64(&rate, "rate")?;
    let div_yield = js_f64(&div_yield, "divYield")?;
    let sigma = js_f64(&sigma, "sigma")?;
    let mu_jump = js_f64(&mu_jump, "muJump")?;
    let sigma_jump = js_f64(&sigma_jump, "sigmaJump")?;
    let lambda = js_f64(&lambda, "lambda")?;
    let expiry = js_f64(&expiry, "expiry")?;
    let is_call = js_bool(&is_call, "isCall")?;
    let n_terms: Option<usize> = js_opt_uint(n_terms.as_ref(), "nTerms")?;
    rust_merton_jump_cos_price(MertonJumpCosParams {
        spot,
        strike,
        rate,
        div_yield,
        sigma,
        mu_jump,
        sigma_jump,
        lambda,
        expiry,
        is_call,
        n_terms,
    })
    .map_err(to_js_err)
}
