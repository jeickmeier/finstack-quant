//! Fourier pricing method bindings (COS) for WASM.
//!
//! Mirrors `finstack-quant-py`'s `models/fourier.rs` module: exposes the Fang-
//! Oosterlee (2008) COS method for European options under Black-Scholes,
//! Variance Gamma, and Merton jump-diffusion characteristic functions.
//!
//! All rates are continuously compounded decimals; `vol` / `sigma` are
//! annualized vols; `time_to_expiry` is time to expiry in years. The shared option and
//! market inputs travel as one `CosMarketParams` object (`market`).
//!
//! Fang-Oosterlee (2008): see docs/REFERENCES.md#fang-oosterlee-2008.

use crate::utils::input::{from_js_json, js_f64};
use crate::utils::to_js_err;
use finstack_quant_models::fourier::cos::{
    CosMarketParams, bs_cos_price as rust_bs_cos_price,
    merton_jump_cos_price as rust_merton_jump_cos_price, vg_cos_price as rust_vg_cos_price,
};
use wasm_bindgen::prelude::*;

/// Price a European option under the Black-Scholes model using the COS method.
///
/// Fang-Oosterlee (2008): see docs/REFERENCES.md#fang-oosterlee-2008.
/// Black-Scholes (1973): see docs/REFERENCES.md#black-scholes-1973.
/// @param market - `CosMarketParams` object or JSON text: `spot` and `strike` in the same price units, `rate` (continuously compounded risk-free rate, decimal), `dividend_yield` (continuous dividend yield, decimal), `time_to_expiry` (years), `option_type` (`"call"` or `"put"`) and optional `n_terms` (COS expansion terms in `1..=65536`; omitted or `null` uses the pricer default, 128).
/// @param vol - Annualized volatility expressed as a decimal, such as 0.20 for 20%; must be positive.
///
/// # Errors
///
/// Throws a `validation` error if `market` does not match `CosMarketParams`,
/// spot, strike, expiry, or volatility is not finite and positive, rates/carry
/// are non-finite, `n_terms` is outside `1..=65536` or `vol` is not positive,
/// and a `computation` error if the model produces a degenerate or invalid COS
/// truncation range, a non-finite characteristic-function value or forward
/// moment, or a non-finite option price.
/// @returns Discounted European option price in the same units as `spot`.
///
/// @example
/// ```javascript
/// const call = models.bsCosPrice(
///   {
///     spot: 100,
///     strike: 100,
///     rate: 0.05,
///     dividend_yield: 0.0,
///     time_to_expiry: 1.0,
///     option_type: "call",
///   },
///   0.2,
/// );
/// // call ≈ 10.45, the Black-Scholes price
/// ```
#[wasm_bindgen(js_name = bsCosPrice)]
pub fn bs_cos_price(
    #[wasm_bindgen(unchecked_param_type = "generated.models.CosMarketParams | string")]
    market: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] vol: JsValue,
) -> Result<f64, JsValue> {
    let market: CosMarketParams = from_js_json(&market, "market")?;
    let vol = js_f64(&vol, "vol")?;
    rust_bs_cos_price(market, vol).map_err(to_js_err)
}

/// Price a European option under the Variance Gamma model using the COS method.
///
/// Fang-Oosterlee (2008): see docs/REFERENCES.md#fang-oosterlee-2008.
/// Madan-Carr-Chang (1998): see docs/REFERENCES.md#madan-carr-chang-1998.
/// @param market - `CosMarketParams` object or JSON text: `spot` and `strike` in the same price units, `rate` (continuously compounded risk-free rate, decimal), `dividend_yield` (continuous dividend yield, decimal), `time_to_expiry` (years), `option_type` (`"call"` or `"put"`) and optional `n_terms` (COS expansion terms in `1..=65536`; omitted or `null` uses the pricer default, 128).
/// @param sigma - Positive finite volatility of the subordinated Brownian motion, as a decimal.
/// @param theta - Finite annual log-return drift of the subordinated Brownian motion; its sign controls skew.
/// @param nu - Positive finite variance rate of the Gamma time change, in years; larger values increase tail thickness.
///
/// # Errors
///
/// Throws a `validation` error if `market` does not match `CosMarketParams`,
/// spot, strike, expiry, sigma, or nu is not finite and positive,
/// rates/carry/theta are non-finite, the VG martingale condition fails or
/// `n_terms` is outside `1..=65536`, and a `computation` error if the model
/// produces a degenerate or invalid COS truncation range, a non-finite
/// characteristic-function value or forward moment, or a non-finite option
/// price.
/// @returns Discounted European option price in the same units as `spot`.
#[wasm_bindgen(js_name = vgCosPrice)]
pub fn vg_cos_price(
    #[wasm_bindgen(unchecked_param_type = "generated.models.CosMarketParams | string")]
    market: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] sigma: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] theta: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] nu: JsValue,
) -> Result<f64, JsValue> {
    let market: CosMarketParams = from_js_json(&market, "market")?;
    let sigma = js_f64(&sigma, "sigma")?;
    let theta = js_f64(&theta, "theta")?;
    let nu = js_f64(&nu, "nu")?;
    rust_vg_cos_price(market, sigma, theta, nu).map_err(to_js_err)
}

/// Price a European option under Merton (1976) jump-diffusion using the COS method.
///
/// Fang-Oosterlee (2008): see docs/REFERENCES.md#fang-oosterlee-2008.
/// Merton jump-diffusion (1976): see docs/REFERENCES.md#merton-1976-jump.
/// @param market - `CosMarketParams` object or JSON text: `spot` and `strike` in the same price units, `rate` (continuously compounded risk-free rate, decimal), `dividend_yield` (continuous dividend yield, decimal), `time_to_expiry` (years), `option_type` (`"call"` or `"put"`) and optional `n_terms` (COS expansion terms in `1..=65536`; omitted or `null` uses the pricer default, 128).
/// @param sigma - Annualized volatility expressed as a decimal, such as 0.20 for 20%.
/// @param muJump - Mean log jump size in the Merton jump-diffusion model.
/// @param sigmaJump - Standard deviation of log jump sizes in the Merton jump-diffusion model.
/// @param lambda - Annual jump-arrival intensity in the Merton jump-diffusion model.
///
/// # Errors
///
/// Throws a `validation` error if `market` does not match `CosMarketParams`,
/// spot, strike, or expiry is not finite and positive, rates/carry or
/// log-jump mean are non-finite, volatility/intensity is negative or
/// non-finite, the jump compensator overflows or `n_terms` is outside
/// `1..=65536`, and a `computation` error if the model produces a degenerate
/// or invalid COS truncation range, a non-finite characteristic-function
/// value or forward moment, or a non-finite option price.
/// @returns Discounted European option price in the same units as `spot`.
#[wasm_bindgen(js_name = mertonJumpCosPrice)]
pub fn merton_jump_cos_price(
    #[wasm_bindgen(unchecked_param_type = "generated.models.CosMarketParams | string")]
    market: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] sigma: JsValue,
    #[wasm_bindgen(js_name = "muJump", unchecked_param_type = "number")] mu_jump: JsValue,
    #[wasm_bindgen(js_name = "sigmaJump", unchecked_param_type = "number")] sigma_jump: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] lambda: JsValue,
) -> Result<f64, JsValue> {
    let market: CosMarketParams = from_js_json(&market, "market")?;
    let sigma = js_f64(&sigma, "sigma")?;
    let mu_jump = js_f64(&mu_jump, "muJump")?;
    let sigma_jump = js_f64(&sigma_jump, "sigmaJump")?;
    let lambda = js_f64(&lambda, "lambda")?;
    rust_merton_jump_cos_price(market, sigma, mu_jump, sigma_jump, lambda).map_err(to_js_err)
}
