//! Closed-form analytic option primitives (Black-Scholes, Black-76, implied vol).
//!
//! Thin wasm-bindgen wrappers around the Rust closed-form formulas in
//! `finstack_quant_models::closed_form`.
//!
//! All rates (`rate`, `divYield`) are continuously compounded decimals; `vol`
//! is annualized lognormal vol (decimal); `normalVol` is an absolute
//! Bachelier vol in the forward's units; `expiry` is time to expiry in years.
//! Greeks scale matches the Rust crate: `vega` and both rho values are per 1%
//! move, `theta` is per-day under the `thetaDaysPerYear` day-count (ACT/365 by
//! default).
//!
//! Named-model sources: `docs/REFERENCES.md#black-scholes-1973`,
//! `docs/REFERENCES.md#merton-1973`, `docs/REFERENCES.md#garman-kohlhagen-1983`,
//! `docs/REFERENCES.md#black-1976`.

use crate::utils::input::{
    from_js_json, js_bool, js_f64, js_opt_bool, js_opt_f64, js_opt_string, js_string, js_uint,
};
use crate::utils::to_js_err;
use finstack_quant_core::wire::serde_parse;
use finstack_quant_models::closed_form::{
    DEFAULT_ASIAN_AVERAGING, DEFAULT_LOOKBACK_STRIKE_TYPE, DEFAULT_THETA_DAYS_PER_YEAR,
    HestonPricingParams, OptionCalculation, asian_option_price as asian_option_price_core,
    bachelier_greeks as bachelier_greeks_core, bachelier_price as bachelier_price_core,
    barrier_option_price as barrier_option_price_core,
    black_shifted_price as black_shifted_price_core, black_shifted_vega as black_shifted_vega_core,
    black76_greeks as black76_greeks_core, black76_implied_vol as black76_implied_vol_core,
    black76_price as black76_price_core, bs_greeks as bs_greeks_core,
    bs_implied_vol as bs_implied_vol_core, bs_price as bs_price_core,
    heston_price as heston_price_core, lookback_option_price as lookback_option_price_core,
    option_calculation as option_calculation_core, quanto_option_price as quanto_option_price_core,
    vanilla_expiry_payoff as vanilla_expiry_payoff_core,
};
use finstack_quant_models::monte_carlo::payoff::asian::AveragingMethod;
use finstack_quant_models::types::OptionType;
use wasm_bindgen::prelude::*;

/// Price a closed-form option or invert its price, with Greeks at the
/// resulting volatility.
///
/// One call replaces choosing and sequencing `bsPrice`, `black76Price`,
/// `bachelierPrice`, their implied-volatility inverses and their Greeks.
///
/// # Arguments
///
/// * `request` - `OptionCalculation` object or JSON text: `model` tagged by
///   `kind` (`"black_scholes"` with `spot`, `strike`, continuously compounded
///   decimal `rate` and `div_yield`, `expiry` in years, `is_call` and an
///   optional `theta_days_per_year` defaulting to 365; `"black76"` with
///   `forward`, `strike`, discount factor `df`, `expiry`, `is_call`;
///   `"bachelier"` with `forward`, `strike`, `expiry`, `is_call`) and `solve`
///   (`{kind: "price", vol}` with a decimal Black vol or an absolute normal vol
///   for Bachelier, or `{kind: "implied_vol", price}` discounted for
///   Black-Scholes and Black-76, undiscounted for Bachelier).
///
/// # Errors
///
/// Throws a `validation` error for a malformed request, an unknown field or
/// kind, an input outside the model's domain, or a price whose implied
/// volatility cannot be solved; `invalid_type` for a non-finite number.
/// @param request - `OptionCalculation` object or JSON text: `model` tagged by `kind` (`"black_scholes"` with `spot`, `strike`, continuously compounded decimal `rate` and `div_yield`, `expiry` in years, `is_call` and an optional `theta_days_per_year` defaulting to 365; `"black76"` with `forward`, `strike`, discount factor `df`, `expiry`, `is_call`; `"bachelier"` with `forward`, `strike`, `expiry`, `is_call`) and `solve` (`{kind: "price", vol}` with a decimal Black vol or an absolute normal vol for Bachelier, or `{kind: "implied_vol", price}` discounted for Black-Scholes and Black-76, undiscounted for Bachelier).
/// @returns The price, the volatility both are at (the supplied vol, or the solved implied vol) and the model's Greeks tagged by `kind` (`"black_scholes"` or `"forward"`).
///
/// @example
/// ```typescript
/// import init, { models } from "finstack-quant-wasm";
/// await init();
/// const result = models.optionCalculation({
///   model: { kind: "black76", forward: 100, strike: 100, df: 0.98, expiry: 1, is_call: true },
///   solve: { kind: "price", vol: 0.2 },
/// });
/// console.log(result.price, result.greeks.kind); // ..., "forward"
/// ```
#[wasm_bindgen(js_name = optionCalculation, unchecked_return_type = "OptionCalculationResult")]
pub fn option_calculation(
    #[wasm_bindgen(unchecked_param_type = "OptionCalculation | string")] request: JsValue,
) -> Result<JsValue, JsValue> {
    let request: OptionCalculation = from_js_json(&request, "request")?;
    let result = option_calculation_core(&request).map_err(to_js_err)?;
    crate::utils::to_js_value(&result)
}

/// Per-unit Black-Scholes / Garman-Kohlhagen price of a European option.
///
/// Black-Scholes (1973): see docs/REFERENCES.md#black-scholes-1973.
/// Merton (1973): see docs/REFERENCES.md#merton-1973.
/// Garman-Kohlhagen (1983): see docs/REFERENCES.md#garman-kohlhagen-1983.
///
/// @param spot - Spot price of the underlying.
/// @param strike - Strike of the option.
/// @param rate - Risk-free rate, **decimal** continuously compounded
/// (e.g. `0.05` for 5%).
/// @param divYield - Continuous dividend yield (or foreign rate for FX),
/// **decimal** continuously compounded.
/// @param vol - Annualized volatility, **decimal**
/// (e.g. `0.20` for 20%).
/// @param expiry - Time to expiry in **years**.
/// @param isCall - `true` for a call, `false` for a put.
/// @returns Per-unit option price.
///
/// @example
/// ```javascript
/// import init, { models } from "finstack-quant-wasm";
/// await init();
/// const price = models.bsPrice(
///   100,    // spot
///   100,    // strike (ATM)
///   0.05,   // rate = 5%
///   0.0,    // divYield = 0
///   0.20,   // vol = 20%
///   1.0,    // expiry = 1 year
///   true,   // call
/// );
/// // price ≈ 10.45
/// ```
///
/// @throws If spot or strike is non-positive, volatility or expiry is negative, any numerical input is non-finite, or discounted legs or price overflow.
#[wasm_bindgen(js_name = bsPrice)]
pub fn bs_price(
    #[wasm_bindgen(unchecked_param_type = "number")] spot: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] strike: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] rate: JsValue,
    #[wasm_bindgen(js_name = "divYield", unchecked_param_type = "number")] div_yield: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] vol: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] expiry: JsValue,
    #[wasm_bindgen(js_name = "isCall", unchecked_param_type = "boolean")] is_call: JsValue,
) -> Result<f64, JsValue> {
    let spot = js_f64(&spot, "spot")?;
    let strike = js_f64(&strike, "strike")?;
    let rate = js_f64(&rate, "rate")?;
    let div_yield = js_f64(&div_yield, "divYield")?;
    let vol = js_f64(&vol, "vol")?;
    let expiry = js_f64(&expiry, "expiry")?;
    let is_call = js_bool(&is_call, "isCall")?;
    bs_price_core(
        spot,
        strike,
        rate,
        div_yield,
        vol,
        expiry,
        OptionType::from(is_call),
    )
    .map_err(to_js_err)
}

/// Vanilla option payoff at expiry: `max(±(spot - strike), 0)`.
///
/// @param spot - Underlying level at expiry, in the same price units as `strike`.
///   Must be finite and non-negative; zero spot is allowed.
/// @param strike - Exercise price; must be finite and strictly positive.
/// @param isCall - `true` for a call (`max(spot - strike, 0)`), `false` for a
/// put (`max(strike - spot, 0)`).
/// @returns Undiscounted expiry payoff in the same units as `spot` and `strike`.
///
/// @example
/// ```javascript
/// import init, { models } from "finstack-quant-wasm";
/// await init();
/// const payoff = models.vanillaExpiryPayoff(110, 100, true);
/// // payoff === 10
/// ```
///
/// @throws If `spot` is non-finite or negative, or `strike` is non-finite or
/// not strictly positive.
#[wasm_bindgen(js_name = vanillaExpiryPayoff)]
pub fn vanilla_expiry_payoff(
    #[wasm_bindgen(unchecked_param_type = "number")] spot: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] strike: JsValue,
    #[wasm_bindgen(js_name = "isCall", unchecked_param_type = "boolean")] is_call: JsValue,
) -> Result<f64, JsValue> {
    let spot = js_f64(&spot, "spot")?;
    let strike = js_f64(&strike, "strike")?;
    let is_call = js_bool(&is_call, "isCall")?;
    vanilla_expiry_payoff_core(spot, strike, OptionType::from(is_call)).map_err(to_js_err)
}

/// Black-Scholes / Garman-Kohlhagen Greeks as a `{delta, gamma, vega, theta, rho_r, rho_q}` object.
///
/// Black-Scholes (1973): see docs/REFERENCES.md#black-scholes-1973.
/// Merton (1973): see docs/REFERENCES.md#merton-1973.
/// Garman-Kohlhagen (1983): see docs/REFERENCES.md#garman-kohlhagen-1983.
///
/// @param spot - Spot price of the underlying.
/// @param strike - Strike of the option.
/// @param rate - Risk-free rate, **decimal** continuously compounded.
/// @param divYield - Dividend yield (or foreign rate for FX), **decimal**
/// continuously compounded.
/// @param vol - Annualized volatility, **decimal**; must be positive.
/// @param expiry - Time to expiry in **years**; must be positive.
/// @param isCall - `true` for a call, `false` for a put.
/// @param thetaDaysPerYear - Day-count denominator for theta. Default `365`.
/// Pass `252` for trading-day theta.
/// @returns Object `{ delta, gamma, vega, theta, rho_r, rho_q }` (snake_case keys
/// matching the Rust/Python canonical `BsGreeks` fields). `vega` and
/// both rho values are **per 1% move**; `theta` is **per day** under
/// `thetaDaysPerYear`.
/// @throws `FinstackError` (kind `validation`) if any input is non-finite, `spot` or `strike` is not positive, `vol`, `expiry` or `thetaDaysPerYear` is not positive, or a computed Greek is non-finite.
///
/// @example
/// ```javascript
/// const g = models.bsGreeks(100, 100, 0.05, 0.0, 0.20, 1.0, true);
/// // g.delta ≈ 0.64, g.gamma ≈ 0.019, g.vega ≈ 0.38 (per 1% vol)
/// ```
#[wasm_bindgen(js_name = bsGreeks, unchecked_return_type = "BsGreeks")]
#[allow(clippy::too_many_arguments)]
pub fn bs_greeks(
    #[wasm_bindgen(unchecked_param_type = "number")] spot: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] strike: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] rate: JsValue,
    #[wasm_bindgen(js_name = "divYield", unchecked_param_type = "number")] div_yield: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] vol: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] expiry: JsValue,
    #[wasm_bindgen(js_name = "isCall", unchecked_param_type = "boolean")] is_call: JsValue,
    #[wasm_bindgen(js_name = "thetaDaysPerYear", unchecked_optional_param_type = "number")]
    theta_days_per_year: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let spot = js_f64(&spot, "spot")?;
    let strike = js_f64(&strike, "strike")?;
    let rate = js_f64(&rate, "rate")?;
    let div_yield = js_f64(&div_yield, "divYield")?;
    let vol = js_f64(&vol, "vol")?;
    let expiry = js_f64(&expiry, "expiry")?;
    let theta_days_per_year = js_opt_f64(theta_days_per_year.as_ref(), "thetaDaysPerYear")?;
    let is_call = js_bool(&is_call, "isCall")?;
    // theta_days_per_year validation (finite, > 0) lives in canonical `bs_greeks`.
    let theta_days_per_year = theta_days_per_year.unwrap_or(DEFAULT_THETA_DAYS_PER_YEAR);
    let g = bs_greeks_core(
        spot,
        strike,
        rate,
        div_yield,
        vol,
        expiry,
        OptionType::from(is_call),
        theta_days_per_year,
    )
    .map_err(to_js_err)?;
    // Serialize the canonical `BsGreeks` struct so the keys are exactly the
    // Rust / Python field names (`rho_r`, `rho_q`, ...).
    crate::utils::to_js_value(&g)
}

/// Solve for Black-Scholes / Garman-Kohlhagen implied volatility.
///
/// Black-Scholes (1973): see docs/REFERENCES.md#black-scholes-1973.
/// Merton (1973): see docs/REFERENCES.md#merton-1973.
/// Garman-Kohlhagen (1983): see docs/REFERENCES.md#garman-kohlhagen-1983.
///
/// @param spot - Spot price of the underlying.
/// @param strike - Strike of the option.
/// @param rate - Risk-free rate, **decimal** continuously compounded.
/// @param divYield - Dividend yield, **decimal** continuously compounded.
/// @param expiry - Time to expiry in **years**; must be positive.
/// @param price - Observed option price (per unit).
/// @param isCall - `true` for a call, `false` for a put.
/// @returns Annualized implied volatility, **decimal** (e.g. `0.20`).
/// @throws If `expiry` is not positive, `price` is below intrinsic value,
/// above the no-arbitrage upper bound, or the solver fails to converge.
///
/// @example
/// ```javascript
/// const iv = models.bsImpliedVol(100, 100, 0.05, 0.0, 1.0, 10.45, true);
/// // iv ≈ 0.20
/// ```
#[wasm_bindgen(js_name = bsImpliedVol)]
pub fn bs_implied_vol(
    #[wasm_bindgen(unchecked_param_type = "number")] spot: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] strike: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] rate: JsValue,
    #[wasm_bindgen(js_name = "divYield", unchecked_param_type = "number")] div_yield: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] expiry: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] price: JsValue,
    #[wasm_bindgen(js_name = "isCall", unchecked_param_type = "boolean")] is_call: JsValue,
) -> Result<f64, JsValue> {
    let spot = js_f64(&spot, "spot")?;
    let strike = js_f64(&strike, "strike")?;
    let rate = js_f64(&rate, "rate")?;
    let div_yield = js_f64(&div_yield, "divYield")?;
    let expiry = js_f64(&expiry, "expiry")?;
    let price = js_f64(&price, "price")?;
    let is_call = js_bool(&is_call, "isCall")?;
    bs_implied_vol_core(
        spot,
        strike,
        rate,
        div_yield,
        expiry,
        price,
        OptionType::from(is_call),
    )
    .map_err(to_js_err)
}

/// Solve for Black-76 (forward-based) implied volatility.
///
/// Black (1976): see docs/REFERENCES.md#black-1976.
/// @param forward - Forward price or rate in the same quote convention as the strike.
/// @param strike - Option strike price in the same price units as the underlying.
/// @param df - Discount factor from valuation to expiry, expressed as a positive decimal.
/// @param expiry - Time to expiry in years; must be positive.
/// @param price - Observed option price in the same units as the forward.
/// @param isCall - Whether to value a call (`true`) or put (`false`).
///
/// # Errors
///
/// Throws a JavaScript exception if an input is non-finite; `expiry`,
/// `forward`, `strike`, `df`, or `price` is not positive; the price is not
/// above intrinsic value or cannot be bracketed; or the implied-volatility
/// solver does not converge.
/// @returns Annualized Black-76 implied volatility as a decimal.
#[wasm_bindgen(js_name = black76ImpliedVol)]
pub fn black76_implied_vol(
    #[wasm_bindgen(unchecked_param_type = "number")] forward: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] strike: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] df: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] expiry: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] price: JsValue,
    #[wasm_bindgen(js_name = "isCall", unchecked_param_type = "boolean")] is_call: JsValue,
) -> Result<f64, JsValue> {
    let forward = js_f64(&forward, "forward")?;
    let strike = js_f64(&strike, "strike")?;
    let df = js_f64(&df, "df")?;
    let expiry = js_f64(&expiry, "expiry")?;
    let price = js_f64(&price, "price")?;
    let is_call = js_bool(&is_call, "isCall")?;
    black76_implied_vol_core(
        forward,
        strike,
        df,
        expiry,
        price,
        OptionType::from(is_call),
    )
    .map_err(to_js_err)
}

/// Black-76 per-unit price of a European option on a forward: `df * Black(F, K, vol, expiry)`.
///
/// Calls the Rust `closed_form::black76_price`, which owns the call/put
/// dispatch, the discounting and the input validation.
///
/// Black (1976): see docs/REFERENCES.md#black-1976.
/// @param forward - Forward price or rate at expiry.
/// @param strike - Strike in the same units as `forward`.
/// @param df - Discount factor from valuation to expiry; a finite decimal
/// strictly greater than zero (the same domain `black76ImpliedVol` accepts).
/// @param vol - Annualized lognormal (Black) volatility, decimal; non-negative.
/// @param expiry - Time to expiry in years; non-negative.
/// @param isCall - Whether to value a call (`true`) or put (`false`).
///
/// # Errors
///
/// Throws a `FinstackError` (`kind: "validation"`) if an input is non-finite,
/// `forward`, `strike` or `df` is not positive, `vol` or `expiry` is negative,
/// or the price is non-finite.
/// @returns Discounted per-unit option price in the units of `forward`.
#[wasm_bindgen(js_name = black76Price)]
pub fn black76_price(
    #[wasm_bindgen(unchecked_param_type = "number")] forward: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] strike: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] df: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] vol: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] expiry: JsValue,
    #[wasm_bindgen(js_name = "isCall", unchecked_param_type = "boolean")] is_call: JsValue,
) -> Result<f64, JsValue> {
    let forward = js_f64(&forward, "forward")?;
    let strike = js_f64(&strike, "strike")?;
    let df = js_f64(&df, "df")?;
    let vol = js_f64(&vol, "vol")?;
    let expiry = js_f64(&expiry, "expiry")?;
    let is_call = js_bool(&is_call, "isCall")?;
    black76_price_core(forward, strike, df, vol, expiry, OptionType::from(is_call))
        .map_err(to_js_err)
}

/// Black-76 undiscounted forward Greeks as a `{delta, gamma, vega}` object.
///
/// `delta` / `gamma` are with respect to the forward; `vega` is per unit
/// (1.0) change in `vol`. Multiply by the discount factor for present-value
/// sensitivities. Black (1976): see docs/REFERENCES.md#black-1976.
/// @param forward - Forward price or rate at expiry.
/// @param strike - Strike in the same units as `forward`.
/// @param vol - Annualized lognormal (Black) volatility, decimal; non-negative.
/// @param expiry - Time to expiry in years; non-negative.
/// @param isCall - Whether to value a call (`true`) or put (`false`).
/// @returns The Rust `ForwardGreeks` object `{ delta, gamma, vega }`.
///
/// # Errors
///
/// Throws a `FinstackError` (`kind: "validation"`) if an input is non-finite,
/// `forward` or `strike` is not positive, `vol` or `expiry` is negative, or a
/// Greek is non-finite.
#[wasm_bindgen(js_name = black76Greeks, unchecked_return_type = "ForwardGreeks")]
pub fn black76_greeks(
    #[wasm_bindgen(unchecked_param_type = "number")] forward: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] strike: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] vol: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] expiry: JsValue,
    #[wasm_bindgen(js_name = "isCall", unchecked_param_type = "boolean")] is_call: JsValue,
) -> Result<JsValue, JsValue> {
    let forward = js_f64(&forward, "forward")?;
    let strike = js_f64(&strike, "strike")?;
    let vol = js_f64(&vol, "vol")?;
    let expiry = js_f64(&expiry, "expiry")?;
    let is_call = js_bool(&is_call, "isCall")?;
    let greeks = black76_greeks_core(forward, strike, vol, expiry, OptionType::from(is_call))
        .map_err(to_js_err)?;
    crate::utils::to_js_value(&greeks)
}

/// Bachelier (normal-model) undiscounted per-unit option price.
///
/// Bachelier (1900): see docs/REFERENCES.md#bachelier-1900.
/// @param forward - Forward price or rate at expiry (may be negative).
/// @param strike - Strike in the same units as `forward`.
/// @param normalVol - Annualized **absolute** (normal) volatility in the
/// units of `forward` (e.g. `0.0075` for 75 bp on decimal rates).
/// @param expiry - Time to expiry in years; non-negative.
/// @param isCall - Whether to value a call (`true`) or put (`false`).
///
/// # Errors
///
/// Throws a `FinstackError` (`kind: "validation"`) if an input is non-finite,
/// `normalVol` or `expiry` is negative, or the price is non-finite.
/// @returns Undiscounted per-unit option value in the units of `forward`.
#[wasm_bindgen(js_name = bachelierPrice)]
pub fn bachelier_price(
    #[wasm_bindgen(unchecked_param_type = "number")] forward: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] strike: JsValue,
    #[wasm_bindgen(js_name = "normalVol", unchecked_param_type = "number")] normal_vol: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] expiry: JsValue,
    #[wasm_bindgen(js_name = "isCall", unchecked_param_type = "boolean")] is_call: JsValue,
) -> Result<f64, JsValue> {
    let forward = js_f64(&forward, "forward")?;
    let strike = js_f64(&strike, "strike")?;
    let normal_vol = js_f64(&normal_vol, "normalVol")?;
    let expiry = js_f64(&expiry, "expiry")?;
    let is_call = js_bool(&is_call, "isCall")?;
    bachelier_price_core(
        forward,
        strike,
        normal_vol,
        expiry,
        OptionType::from(is_call),
    )
    .map_err(to_js_err)
}

/// Bachelier (normal-model) undiscounted forward Greeks as a `{delta, gamma, vega}` object.
///
/// `vega` is per unit (1.0) change in `normalVol` (absolute units).
/// Bachelier (1900): see docs/REFERENCES.md#bachelier-1900.
/// @param forward - Forward price or rate at expiry (may be negative).
/// @param strike - Strike in the same units as `forward`.
/// @param normalVol - Annualized absolute (normal) volatility in the units of `forward`; non-negative.
/// @param expiry - Time to expiry in years; non-negative.
/// @param isCall - Whether to value a call (`true`) or put (`false`).
/// @returns The Rust `ForwardGreeks` object `{ delta, gamma, vega }`.
///
/// # Errors
///
/// Throws a `FinstackError` (`kind: "validation"`) if an input is non-finite,
/// `normalVol` or `expiry` is negative, or a Greek is non-finite.
#[wasm_bindgen(js_name = bachelierGreeks, unchecked_return_type = "ForwardGreeks")]
pub fn bachelier_greeks(
    #[wasm_bindgen(unchecked_param_type = "number")] forward: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] strike: JsValue,
    #[wasm_bindgen(js_name = "normalVol", unchecked_param_type = "number")] normal_vol: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] expiry: JsValue,
    #[wasm_bindgen(js_name = "isCall", unchecked_param_type = "boolean")] is_call: JsValue,
) -> Result<JsValue, JsValue> {
    let forward = js_f64(&forward, "forward")?;
    let strike = js_f64(&strike, "strike")?;
    let normal_vol = js_f64(&normal_vol, "normalVol")?;
    let expiry = js_f64(&expiry, "expiry")?;
    let is_call = js_bool(&is_call, "isCall")?;
    let greeks = bachelier_greeks_core(
        forward,
        strike,
        normal_vol,
        expiry,
        OptionType::from(is_call),
    )
    .map_err(to_js_err)?;
    crate::utils::to_js_value(&greeks)
}

/// Shifted (displaced) Black undiscounted per-unit price for negative-rate markets.
///
/// Prices `Black(forward + shift, strike + shift, vol, expiry)`.
/// @param forward - Forward rate at expiry (decimal; may be negative).
/// @param strike - Strike (decimal, same units as `forward`).
/// @param vol - Annualized shifted-lognormal volatility, decimal.
/// @param expiry - Time to expiry in years.
/// @param shift - Displacement added to forward and strike, in rate units
/// (e.g. `0.03` for a 3% shift); both shifted values must be positive.
/// @param isCall - Whether to value a call (`true`) or put (`false`).
///
/// # Errors
///
/// Throws a `FinstackError` (`kind: "validation"`) if an input is non-finite,
/// `forward + shift` or `strike + shift` is not positive, `vol` or `expiry` is
/// negative, or the price is non-finite.
/// @returns Undiscounted per-unit option value in the units of `forward`.
#[wasm_bindgen(js_name = blackShiftedPrice)]
pub fn black_shifted_price(
    #[wasm_bindgen(unchecked_param_type = "number")] forward: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] strike: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] vol: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] expiry: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] shift: JsValue,
    #[wasm_bindgen(js_name = "isCall", unchecked_param_type = "boolean")] is_call: JsValue,
) -> Result<f64, JsValue> {
    let forward = js_f64(&forward, "forward")?;
    let strike = js_f64(&strike, "strike")?;
    let vol = js_f64(&vol, "vol")?;
    let expiry = js_f64(&expiry, "expiry")?;
    let shift = js_f64(&shift, "shift")?;
    let is_call = js_bool(&is_call, "isCall")?;
    black_shifted_price_core(
        forward,
        strike,
        vol,
        expiry,
        shift,
        OptionType::from(is_call),
    )
    .map_err(to_js_err)
}

/// Shifted (displaced) Black vega per unit (1.0) change in `vol`, undiscounted.
/// @param forward - Forward rate at expiry (decimal; may be negative).
/// @param strike - Strike (decimal, same units as `forward`).
/// @param vol - Annualized shifted-lognormal volatility, decimal.
/// @param expiry - Time to expiry in years.
/// @param shift - Displacement added to forward and strike, in rate units;
/// both shifted values must be positive.
///
/// # Errors
///
/// Throws a `FinstackError` (`kind: "validation"`) if an input is non-finite,
/// `forward + shift` or `strike + shift` is not positive, `vol` or `expiry` is
/// negative, or the vega is non-finite.
/// @returns Undiscounted vega in the units of `forward` per unit vol.
#[wasm_bindgen(js_name = blackShiftedVega)]
pub fn black_shifted_vega(
    #[wasm_bindgen(unchecked_param_type = "number")] forward: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] strike: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] vol: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] expiry: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] shift: JsValue,
) -> Result<f64, JsValue> {
    let forward = js_f64(&forward, "forward")?;
    let strike = js_f64(&strike, "strike")?;
    let vol = js_f64(&vol, "vol")?;
    let expiry = js_f64(&expiry, "expiry")?;
    let shift = js_f64(&shift, "shift")?;
    black_shifted_vega_core(forward, strike, vol, expiry, shift).map_err(to_js_err)
}

/// Reiner-Rubinstein continuous-monitoring barrier option price.
///
/// `direction` is `"up"` or `"down"`, `knock` is `"in"` or `"out"`.
/// Reiner-Rubinstein (1991): see docs/REFERENCES.md#reiner-rubinstein-1991.
/// @param spot - Current spot price or exchange rate in the same units as the strike.
/// @param strike - Option strike price in the same price units as the underlying.
/// @param barrier - Continuously monitored barrier level in the same price units as spot.
/// @param rate - Continuously compounded risk-free rate, expressed as a decimal.
/// @param divYield - Continuous dividend yield or foreign rate, expressed as a decimal.
/// @param vol - Annualized volatility expressed as a decimal, such as 0.20 for 20%.
/// @param expiry - Time to expiry in years.
/// @param direction - Barrier direction: `"up"` for an upper barrier or `"down"` for a lower barrier.
/// @param knock - Barrier activation: `"in"` for knock-in or `"out"` for knock-out.
/// @param isCall - Whether to value a call (`true`, default) or put (`false`).
///
/// # Errors
///
/// Throws a JavaScript exception if `direction` or `knock` is unsupported, or
/// the supplied model inputs produce a non-finite barrier price.
/// @returns Discounted barrier option price in the same units as `spot`.
#[wasm_bindgen(js_name = barrierOptionPrice)]
#[allow(clippy::too_many_arguments)]
pub fn barrier_option_price(
    #[wasm_bindgen(unchecked_param_type = "number")] spot: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] strike: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] barrier: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] rate: JsValue,
    #[wasm_bindgen(js_name = "divYield", unchecked_param_type = "number")] div_yield: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] vol: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] expiry: JsValue,
    #[wasm_bindgen(unchecked_param_type = "'up' | 'down'")] direction: JsValue,
    #[wasm_bindgen(unchecked_param_type = "'in' | 'out'")] knock: JsValue,
    #[wasm_bindgen(js_name = "isCall", unchecked_optional_param_type = "boolean")] is_call: Option<
        JsValue,
    >,
) -> Result<f64, JsValue> {
    let spot = js_f64(&spot, "spot")?;
    let strike = js_f64(&strike, "strike")?;
    let barrier = js_f64(&barrier, "barrier")?;
    let rate = js_f64(&rate, "rate")?;
    let div_yield = js_f64(&div_yield, "divYield")?;
    let vol = js_f64(&vol, "vol")?;
    let expiry = js_f64(&expiry, "expiry")?;
    let direction: &str = &js_string(&direction, "direction")?;
    let knock: &str = &js_string(&knock, "knock")?;
    let is_call = js_opt_bool(is_call.as_ref(), "isCall")?;
    barrier_option_price_core(
        spot,
        strike,
        barrier,
        rate,
        div_yield,
        vol,
        expiry,
        direction,
        knock,
        OptionType::from(is_call.unwrap_or(true)),
    )
    .map_err(to_js_err)
}

/// Arithmetic (Turnbull-Wakeman) or geometric (Kemna-Vorst) Asian option.
///
/// Kemna-Vorst (1990): see docs/REFERENCES.md#kemna-vorst-1990.
/// Turnbull-Wakeman (1991): see docs/REFERENCES.md#turnbull-wakeman-1991.
/// @param spot - Current spot price or exchange rate in the same units as the strike.
/// @param strike - Option strike price in the same price units as the underlying.
/// @param rate - Continuously compounded risk-free rate, expressed as a decimal.
/// @param divYield - Continuous dividend yield or foreign rate, expressed as a decimal.
/// @param vol - Annualized volatility expressed as a decimal, such as 0.20 for 20%.
/// @param expiry - Time to expiry in years.
/// @param numFixings - Non-negative integer number of equally spaced averaging observations, at most 4294967295; zero selects continuous monitoring.
/// @param averaging - Asian averaging convention: `"arithmetic"` (default) or `"geometric"`.
/// @param isCall - Whether to value a call (`true`, default) or put (`false`).
///
/// # Errors
///
/// Throws a JavaScript exception if `numFixings` is not a non-negative whole
/// number, `averaging` is not `"arithmetic"` or `"geometric"`, or the supplied
/// model inputs produce a non-finite option price.
/// @returns Discounted Asian option price in the same units as `spot`.
#[wasm_bindgen(js_name = asianOptionPrice)]
#[allow(clippy::too_many_arguments)]
pub fn asian_option_price(
    #[wasm_bindgen(unchecked_param_type = "number")] spot: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] strike: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] rate: JsValue,
    #[wasm_bindgen(js_name = "divYield", unchecked_param_type = "number")] div_yield: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] vol: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] expiry: JsValue,
    #[wasm_bindgen(js_name = "numFixings", unchecked_param_type = "number")] num_fixings: JsValue,
    #[wasm_bindgen(unchecked_optional_param_type = "'arithmetic' | 'geometric'")] averaging: Option<
        JsValue,
    >,
    #[wasm_bindgen(js_name = "isCall", unchecked_optional_param_type = "boolean")] is_call: Option<
        JsValue,
    >,
) -> Result<f64, JsValue> {
    let spot = js_f64(&spot, "spot")?;
    let strike = js_f64(&strike, "strike")?;
    let rate = js_f64(&rate, "rate")?;
    let div_yield = js_f64(&div_yield, "divYield")?;
    let vol = js_f64(&vol, "vol")?;
    let expiry = js_f64(&expiry, "expiry")?;
    let num_fixings: usize = js_uint(&num_fixings, "numFixings")?;
    let averaging = js_opt_string(averaging.as_ref(), "averaging")?
        .map(|label| serde_parse::<AveragingMethod>(&label))
        .transpose()
        .map_err(to_js_err)?
        .unwrap_or(DEFAULT_ASIAN_AVERAGING);
    let is_call = js_opt_bool(is_call.as_ref(), "isCall")?;
    let option_type = OptionType::from(is_call.unwrap_or(true));
    asian_option_price_core(
        spot,
        strike,
        rate,
        div_yield,
        vol,
        expiry,
        num_fixings,
        averaging,
        option_type,
    )
    .map_err(to_js_err)
}

/// Conze-Viswanathan lookback option.
///
/// `strike_type` is `"fixed"` (default) or `"floating"`. For `"floating"`,
/// `strike` is ignored and `extremum` is the observed min/max to date.
/// Conze-Viswanathan (1991): see docs/REFERENCES.md#conze-viswanathan-1991.
/// @param spot - Current spot price or exchange rate in the same units as the strike.
/// @param strike - Option strike price in the same price units as the underlying.
/// @param rate - Continuously compounded risk-free rate, expressed as a decimal.
/// @param divYield - Continuous dividend yield or foreign rate, expressed as a decimal.
/// @param vol - Annualized volatility expressed as a decimal, such as 0.20 for 20%.
/// @param expiry - Time to expiry in years.
/// @param extremum - Observed maximum for fixed calls or floating puts, minimum for fixed puts or floating calls, including current spot; in spot-price units.
/// @param strikeType - Lookback payoff convention: `"fixed"` (default) or `"floating"`.
/// @param isCall - Whether to value a call (`true`, default) or put (`false`).
///
/// # Errors
///
/// Throws a JavaScript exception if `strikeType` is not `"fixed"` or
/// `"floating"`, or the supplied model inputs produce a non-finite option
/// price.
/// @returns Discounted lookback option price in the same units as `spot`.
#[wasm_bindgen(js_name = lookbackOptionPrice)]
#[allow(clippy::too_many_arguments)]
pub fn lookback_option_price(
    #[wasm_bindgen(unchecked_param_type = "number")] spot: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] strike: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] rate: JsValue,
    #[wasm_bindgen(js_name = "divYield", unchecked_param_type = "number")] div_yield: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] vol: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] expiry: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] extremum: JsValue,
    #[wasm_bindgen(
        js_name = "strikeType",
        unchecked_optional_param_type = "'fixed' | 'floating'"
    )]
    strike_type: Option<JsValue>,
    #[wasm_bindgen(js_name = "isCall", unchecked_optional_param_type = "boolean")] is_call: Option<
        JsValue,
    >,
) -> Result<f64, JsValue> {
    let spot = js_f64(&spot, "spot")?;
    let strike = js_f64(&strike, "strike")?;
    let rate = js_f64(&rate, "rate")?;
    let div_yield = js_f64(&div_yield, "divYield")?;
    let vol = js_f64(&vol, "vol")?;
    let expiry = js_f64(&expiry, "expiry")?;
    let extremum = js_f64(&extremum, "extremum")?;
    let strike_type = js_opt_string(strike_type.as_ref(), "strikeType")?;
    let is_call = js_opt_bool(is_call.as_ref(), "isCall")?;
    let strike_type = strike_type
        .as_deref()
        .unwrap_or(DEFAULT_LOOKBACK_STRIKE_TYPE);
    let option_type = OptionType::from(is_call.unwrap_or(true));
    lookback_option_price_core(
        spot,
        strike,
        rate,
        div_yield,
        vol,
        expiry,
        extremum,
        strike_type,
        option_type,
    )
    .map_err(to_js_err)
}

/// Quanto option (FX-adjusted cross-currency) price in domestic currency.
///
/// Garman-Kohlhagen (1983): see docs/REFERENCES.md#garman-kohlhagen-1983.
/// Brigo-Mercurio (2006): see docs/REFERENCES.md#brigo-mercurio-2006-interest-rate-models.
///
/// @throws If the inputs produce a non-finite price.
/// @param spot - Current spot price or exchange rate in the same units as the strike.
/// @param strike - Option strike price in the same price units as the underlying.
/// @param expiry - Time to expiry in years.
/// @param rateDomestic - Domestic continuously compounded risk-free rate, expressed as a decimal.
/// @param rateForeign - Foreign continuously compounded risk-free rate, expressed as a decimal.
/// @param divYield - Continuous dividend yield expressed as a decimal, such as 0.02 for 2%.
/// @param volAsset - Annualized asset-price volatility expressed as a decimal.
/// @param volFx - Annualized FX-rate volatility expressed as a decimal.
/// @param correlation - Instantaneous correlation between the asset and FX-rate shocks, from -1 to 1.
/// @param isCall - Whether to value a call (`true`, default) or put (`false`).
/// @returns Discounted quanto option price in domestic currency units.
#[wasm_bindgen(js_name = quantoOptionPrice)]
#[allow(clippy::too_many_arguments)]
pub fn quanto_option_price(
    #[wasm_bindgen(unchecked_param_type = "number")] spot: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] strike: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] expiry: JsValue,
    #[wasm_bindgen(js_name = "rateDomestic", unchecked_param_type = "number")]
    rate_domestic: JsValue,
    #[wasm_bindgen(js_name = "rateForeign", unchecked_param_type = "number")] rate_foreign: JsValue,
    #[wasm_bindgen(js_name = "divYield", unchecked_param_type = "number")] div_yield: JsValue,
    #[wasm_bindgen(js_name = "volAsset", unchecked_param_type = "number")] vol_asset: JsValue,
    #[wasm_bindgen(js_name = "volFx", unchecked_param_type = "number")] vol_fx: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] correlation: JsValue,
    #[wasm_bindgen(js_name = "isCall", unchecked_optional_param_type = "boolean")] is_call: Option<
        JsValue,
    >,
) -> Result<f64, JsValue> {
    let spot = js_f64(&spot, "spot")?;
    let strike = js_f64(&strike, "strike")?;
    let expiry = js_f64(&expiry, "expiry")?;
    let rate_domestic = js_f64(&rate_domestic, "rateDomestic")?;
    let rate_foreign = js_f64(&rate_foreign, "rateForeign")?;
    let div_yield = js_f64(&div_yield, "divYield")?;
    let vol_asset = js_f64(&vol_asset, "volAsset")?;
    let vol_fx = js_f64(&vol_fx, "volFx")?;
    let correlation = js_f64(&correlation, "correlation")?;
    let is_call = js_opt_bool(is_call.as_ref(), "isCall")?;
    quanto_option_price_core(
        spot,
        strike,
        expiry,
        rate_domestic,
        rate_foreign,
        div_yield,
        vol_asset,
        vol_fx,
        correlation,
        OptionType::from(is_call.unwrap_or(true)),
    )
    .map_err(to_js_err)
}

/// Closed-form (Fourier) Heston price of a European option.
///
/// Heston (1993): see docs/REFERENCES.md#heston-1993.
/// Albrecher et al. (2007): see docs/REFERENCES.md#albrecher-2007-little-heston-trap.
/// @param spot - Current spot price in the same units as the strike.
/// @param strike - Option strike price.
/// @param expiry - Time to expiry in years; a non-positive value returns intrinsic.
/// @param rate - Continuously compounded risk-free rate, decimal.
/// @param divYield - Continuous dividend yield or foreign rate, decimal.
/// @param kappa - Mean-reversion speed of the variance process (per year).
/// @param theta - Long-run variance level (variance units).
/// @param sigmaV - Volatility of variance (vol-of-vol).
/// @param rho - Spot/variance correlation in `(-1, 1)`.
/// @param v0 - Initial instantaneous variance (variance, not volatility).
/// @param isCall - Whether to value a call (`true`, default) or put (`false`).
///
/// # Errors
///
/// Throws a JavaScript exception if a parameter is non-finite or outside its
/// domain, or the Fourier integration fails to produce a finite price.
/// @returns Present-value per-unit option price.
#[wasm_bindgen(js_name = hestonPrice)]
#[allow(clippy::too_many_arguments)]
pub fn heston_price(
    #[wasm_bindgen(unchecked_param_type = "number")] spot: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] strike: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] expiry: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] rate: JsValue,
    #[wasm_bindgen(js_name = "divYield", unchecked_param_type = "number")] div_yield: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] kappa: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] theta: JsValue,
    #[wasm_bindgen(js_name = "sigmaV", unchecked_param_type = "number")] sigma_v: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] rho: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] v0: JsValue,
    #[wasm_bindgen(js_name = "isCall", unchecked_optional_param_type = "boolean")] is_call: Option<
        JsValue,
    >,
) -> Result<f64, JsValue> {
    let spot = js_f64(&spot, "spot")?;
    let strike = js_f64(&strike, "strike")?;
    let expiry = js_f64(&expiry, "expiry")?;
    let rate = js_f64(&rate, "rate")?;
    let div_yield = js_f64(&div_yield, "divYield")?;
    let kappa = js_f64(&kappa, "kappa")?;
    let theta = js_f64(&theta, "theta")?;
    let sigma_v = js_f64(&sigma_v, "sigmaV")?;
    let rho = js_f64(&rho, "rho")?;
    let v0 = js_f64(&v0, "v0")?;
    let is_call = js_opt_bool(is_call.as_ref(), "isCall")?;
    let params = HestonPricingParams::new(rate, div_yield, kappa, theta, sigma_v, rho, v0)
        .map_err(to_js_err)?;
    let option_type = OptionType::from(is_call.unwrap_or(true));
    heston_price_core(spot, strike, expiry, &params, option_type, None).map_err(to_js_err)
}

/// Whether a set of Black-Scholes Greeks is internally consistent: gamma and
/// vega non-negative and every value finite.
///
/// Twin of the Rust and Python `BsGreeks.is_valid`. Delta is not bounded here
/// because its true bound `exp(-q T)` depends on the carry.
/// @param greeks - `BsGreeks` object or JSON, as returned by `bsGreeks`.
/// @returns `true` when the Greeks pass the consistency checks.
///
/// # Errors
///
/// Throws a `TypeError` if `greeks` is neither a string nor a plain object,
/// and a `validation` error if it is malformed.
#[wasm_bindgen(js_name = bsGreeksIsValid)]
pub fn bs_greeks_is_valid(
    #[wasm_bindgen(unchecked_param_type = "BsGreeks | string")] greeks: JsValue,
) -> Result<bool, JsValue> {
    let greeks: finstack_quant_models::closed_form::BsGreeks =
        crate::utils::input::from_js_json(&greeks, "greeks")?;
    Ok(greeks.is_valid())
}
