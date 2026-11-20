//! European option COS pricing using canonical option inputs.

use crate::utils::input::{from_js_json_exact, js_f64, js_opt_uint};
use crate::utils::to_js_err;
use finstack_quant_models::fourier::cos::{
    bs_cos_price as rust_bs_cos_price, merton_jump_cos_price as rust_merton_jump_cos_price,
    vg_cos_price as rust_vg_cos_price,
};
use finstack_quant_models::types::{OptionContractParams, OptionMarketParams};
use wasm_bindgen::prelude::*;

fn term_count(value: Option<JsValue>) -> Result<Option<usize>, JsValue> {
    js_opt_uint(value.as_ref(), "nTerms")
}

/// Price a European option under a constant annualized lognormal volatility.
/// @param market - Spot, strike, annual continuous decimal discount and carry rates, positive expiry in years, payoff direction and positive decimal annual volatility.
/// @param nTerms - Expansion terms from 1 to 65,536. Omit for 128 terms.
/// @returns Discounted option price per underlying unit.
/// @throws FinstackError for invalid input, invalid term count, degenerate cumulants or an unrepresentable price.
/// @example
/// ```typescript
/// import init, { models } from "finstack-quant-wasm";
/// await init();
/// const price = models.bsCosPrice({
///   spot: 100, strike: 100, rate: 0.05, dividend_yield: 0,
///   time_to_expiry: 1, option_type: "call", volatility: 0.2,
/// });
/// console.log(price); // about 10.45
/// ```
#[wasm_bindgen(js_name = bsCosPrice)]
pub fn bs_cos_price(
    #[wasm_bindgen(unchecked_param_type = "generated.models.OptionMarketParams | string")]
    market: JsValue,
    #[wasm_bindgen(js_name = "nTerms", unchecked_optional_param_type = "number")] n_terms: Option<
        JsValue,
    >,
) -> Result<f64, JsValue> {
    let market: OptionMarketParams = from_js_json_exact(&market, "market")?;
    rust_bs_cos_price(&market, term_count(n_terms)?).map_err(to_js_err)
}

/// Price a European option under the Variance Gamma process.
/// @param contract - Spot and strike in matching units, positive expiry in years, continuous annual discount and carry rates in decimals, and call or put direction.
/// @param sigma - Positive annual volatility of the subordinated Brownian motion as a decimal.
/// @param theta - Annual drift of the subordinated Brownian motion with finite forward moment.
/// @param nu - Positive variance rate of the Gamma subordinator.
/// @param nTerms - Expansion terms from 1 to 65,536. Omit for 128 terms.
/// @returns Discounted option price per underlying unit.
/// @throws FinstackError for invalid input, a missing forward moment, invalid term count, degenerate cumulants or unrepresentable price.
#[wasm_bindgen(js_name = vgCosPrice)]
pub fn vg_cos_price(
    #[wasm_bindgen(unchecked_param_type = "generated.models.OptionContractParams | string")]
    contract: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] sigma: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] theta: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] nu: JsValue,
    #[wasm_bindgen(js_name = "nTerms", unchecked_optional_param_type = "number")] n_terms: Option<
        JsValue,
    >,
) -> Result<f64, JsValue> {
    let contract: OptionContractParams = from_js_json_exact(&contract, "contract")?;
    rust_vg_cos_price(
        &contract,
        js_f64(&sigma, "sigma")?,
        js_f64(&theta, "theta")?,
        js_f64(&nu, "nu")?,
        term_count(n_terms)?,
    )
    .map_err(to_js_err)
}

/// Price a European option under Merton lognormal jump diffusion.
/// @param contract - Spot and strike in matching units, positive expiry in years, continuous annual discount and carry rates in decimals, and call or put direction.
/// @param sigma - Finite nonnegative annualized diffusion volatility as a decimal.
/// @param muJump - Mean log jump size with finite risk-neutral compensator.
/// @param sigmaJump - Finite nonnegative standard deviation of log jump size.
/// @param lambda - Finite nonnegative expected jump count per year.
/// @param nTerms - Expansion terms from 1 to 65,536. Omit for 128 terms.
/// @returns Discounted option price per underlying unit.
/// @throws FinstackError for invalid input, a missing forward moment, invalid term count, degenerate cumulants or unrepresentable price.
#[wasm_bindgen(js_name = mertonJumpCosPrice)]
pub fn merton_jump_cos_price(
    #[wasm_bindgen(unchecked_param_type = "generated.models.OptionContractParams | string")]
    contract: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] sigma: JsValue,
    #[wasm_bindgen(js_name = "muJump", unchecked_param_type = "number")] mu_jump: JsValue,
    #[wasm_bindgen(js_name = "sigmaJump", unchecked_param_type = "number")] sigma_jump: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] lambda: JsValue,
    #[wasm_bindgen(js_name = "nTerms", unchecked_optional_param_type = "number")] n_terms: Option<
        JsValue,
    >,
) -> Result<f64, JsValue> {
    let contract: OptionContractParams = from_js_json_exact(&contract, "contract")?;
    rust_merton_jump_cos_price(
        &contract,
        js_f64(&sigma, "sigma")?,
        js_f64(&mu_jump, "muJump")?,
        js_f64(&sigma_jump, "sigmaJump")?,
        js_f64(&lambda, "lambda")?,
        term_count(n_terms)?,
    )
    .map_err(to_js_err)
}
