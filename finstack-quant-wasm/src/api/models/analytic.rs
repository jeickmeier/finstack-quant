//! Closed-form analytic option primitives (Black-Scholes, Black-76, implied vol).
//!
//! Thin wasm-bindgen wrappers around the Rust closed-form formulas in
//! `finstack_quant_models::closed_form`.
//!
//! All rates (`rate`, `divYield`) are continuously compounded decimals; `vol`
//! is annualized lognormal vol (decimal); `normalVol` is an absolute
//! Bachelier vol in the forward's units; `expiry` is time to expiry in years.
//! Greeks scale matches the Rust crate: `vega` and both rho values are per 1%
//! move, `theta` is per-day under the `thetaDays` day-count (ACT/365 by
//! default).
//!
//! Named-model sources: `docs/REFERENCES.md#black-scholes-1973`,
//! `docs/REFERENCES.md#merton-1973`, `docs/REFERENCES.md#garman-kohlhagen-1983`,
//! `docs/REFERENCES.md#black-1976`.

use crate::utils::to_js_err;
use finstack_quant_models::closed_form::implied_vol::{
    black76_implied_vol as black76_implied_vol_core, bs_implied_vol as bs_implied_vol_core,
};
use finstack_quant_models::closed_form::{
    asian_option_price_str, bachelier_call, bachelier_delta_call, bachelier_delta_put,
    bachelier_gamma, bachelier_put, bachelier_vega, barrier_call_str, barrier_put_str, black_call,
    black_delta_call, black_delta_put, black_gamma, black_put, black_shifted_call,
    black_shifted_put, black_shifted_vega, black_vega, bs_greeks as bs_greeks_core,
    bs_price as bs_price_core, checked_closed_form_value, heston_call_price_fourier,
    heston_put_price_fourier, lookback_option_price_str,
    quanto_option_price as quanto_option_price_core,
    vanilla_expiry_payoff as vanilla_expiry_payoff_core, HestonPricingParams,
};
use finstack_quant_models::OptionType;
use wasm_bindgen::prelude::*;

const DEFAULT_THETA_DAYS_PER_YEAR: f64 = 365.0;

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
    spot: f64,
    strike: f64,
    rate: f64,
    div_yield: f64,
    vol: f64,
    expiry: f64,
    is_call: bool,
) -> Result<f64, JsValue> {
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
/// const payoff = valuations.vanillaExpiryPayoff(110, 100, true);
/// // payoff === 10
/// ```
///
/// @throws If `spot` is non-finite or negative, or `strike` is non-finite or
/// not strictly positive.
#[wasm_bindgen(js_name = vanillaExpiryPayoff)]
pub fn vanilla_expiry_payoff(spot: f64, strike: f64, is_call: bool) -> Result<f64, JsValue> {
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
/// @param thetaDays - Day-count denominator for theta. Default `365`.
/// Pass `252` for trading-day theta.
/// @returns Object `{ delta, gamma, vega, theta, rho_r, rho_q }` (snake_case keys
/// matching the Rust/Python canonical `BsGreeks` fields). `vega` and
/// both rho values are **per 1% move**; `theta` is **per day** under
/// `thetaDays`.
/// @throws If serialization to JS fails (should not happen on valid inputs).
///
/// @example
/// ```javascript
/// const g = models.bsGreeks(100, 100, 0.05, 0.0, 0.20, 1.0, true);
/// // g.delta ≈ 0.64, g.gamma ≈ 0.019, g.vega ≈ 0.38 (per 1% vol)
/// ```
#[wasm_bindgen(js_name = bsGreeks)]
#[allow(clippy::too_many_arguments)]
pub fn bs_greeks(
    spot: f64,
    strike: f64,
    rate: f64,
    div_yield: f64,
    vol: f64,
    expiry: f64,
    is_call: bool,
    theta_days: Option<f64>,
) -> Result<JsValue, JsValue> {
    // theta_days validation (finite, > 0) lives in canonical `bs_greeks`.
    let theta_days = theta_days.unwrap_or(DEFAULT_THETA_DAYS_PER_YEAR);
    let g = bs_greeks_core(
        spot,
        strike,
        rate,
        div_yield,
        vol,
        expiry,
        OptionType::from(is_call),
        theta_days,
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
    spot: f64,
    strike: f64,
    rate: f64,
    div_yield: f64,
    expiry: f64,
    price: f64,
    is_call: bool,
) -> Result<f64, JsValue> {
    bs_implied_vol_core(
        spot,
        strike,
        rate,
        div_yield,
        expiry,
        OptionType::from(is_call),
        price,
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
/// @param is_call - Whether to value a call (`true`) or put (`false`).
///
/// # Errors
///
/// Throws a JavaScript exception if an input is non-finite; `expiry`,
/// `forward`, `strike`, `df`, or `price` is not positive; the price is not
/// above intrinsic value or cannot be bracketed; or the implied-volatility
/// solver does not converge.
#[wasm_bindgen(js_name = black76ImpliedVol)]
pub fn black76_implied_vol(
    forward: f64,
    strike: f64,
    df: f64,
    expiry: f64,
    price: f64,
    is_call: bool,
) -> Result<f64, JsValue> {
    black76_implied_vol_core(
        forward,
        strike,
        df,
        expiry,
        OptionType::from(is_call),
        price,
    )
    .map_err(to_js_err)
}

/// Black-76 per-unit price of a European option on a forward: `df * Black(F, K, vol, expiry)`.
///
/// Black (1976): see docs/REFERENCES.md#black-1976.
/// @param forward - Forward price or rate at expiry.
/// @param strike - Strike in the same units as `forward`.
/// @param df - Discount factor from valuation to expiry (positive decimal).
/// @param expiry - Time to expiry in years.
/// @param vol - Annualized lognormal (Black) volatility, decimal.
/// @param is_call - Whether to value a call (`true`) or put (`false`).
///
/// # Errors
///
/// Throws a JavaScript exception if the inputs produce a non-finite price.
#[wasm_bindgen(js_name = black76Price)]
pub fn black76_price(
    forward: f64,
    strike: f64,
    df: f64,
    expiry: f64,
    vol: f64,
    is_call: bool,
) -> Result<f64, JsValue> {
    let undiscounted = if is_call {
        black_call(forward, strike, vol, expiry)
    } else {
        black_put(forward, strike, vol, expiry)
    };
    checked_closed_form_value(df * undiscounted, "Black-76 price").map_err(to_js_err)
}

/// Black-76 undiscounted forward Greeks as a `{delta, gamma, vega}` object.
///
/// `delta` / `gamma` are with respect to the forward; `vega` is per unit
/// (1.0) change in `vol`. Multiply by the discount factor for present-value
/// sensitivities. Black (1976): see docs/REFERENCES.md#black-1976.
/// @param forward - Forward price or rate at expiry.
/// @param strike - Strike in the same units as `forward`.
/// @param expiry - Time to expiry in years.
/// @param vol - Annualized lognormal (Black) volatility, decimal.
/// @param is_call - Whether to value a call (`true`) or put (`false`).
///
/// # Errors
///
/// Throws a JavaScript exception if any Greek is non-finite.
#[wasm_bindgen(js_name = black76Greeks)]
pub fn black76_greeks(
    forward: f64,
    strike: f64,
    expiry: f64,
    vol: f64,
    is_call: bool,
) -> Result<JsValue, JsValue> {
    let delta = if is_call {
        black_delta_call(forward, strike, vol, expiry)
    } else {
        black_delta_put(forward, strike, vol, expiry)
    };
    forward_greeks(
        delta,
        black_gamma(forward, strike, vol, expiry),
        black_vega(forward, strike, vol, expiry),
        "Black-76",
    )
}

/// Bachelier (normal-model) undiscounted per-unit option price.
///
/// Bachelier (1900): see docs/REFERENCES.md#bachelier-1900.
/// @param forward - Forward price or rate at expiry (may be negative).
/// @param strike - Strike in the same units as `forward`.
/// @param normal_vol - Annualized **absolute** (normal) volatility in the
/// units of `forward` (e.g. `0.0075` for 75 bp on decimal rates).
/// @param expiry - Time to expiry in years.
/// @param is_call - Whether to value a call (`true`) or put (`false`).
///
/// # Errors
///
/// Throws a JavaScript exception if the inputs produce a non-finite price.
#[wasm_bindgen(js_name = bachelierPrice)]
pub fn bachelier_price(
    forward: f64,
    strike: f64,
    normal_vol: f64,
    expiry: f64,
    is_call: bool,
) -> Result<f64, JsValue> {
    let value = if is_call {
        bachelier_call(forward, strike, normal_vol, expiry)
    } else {
        bachelier_put(forward, strike, normal_vol, expiry)
    };
    checked_closed_form_value(value, "Bachelier price").map_err(to_js_err)
}

/// Bachelier (normal-model) undiscounted forward Greeks as a `{delta, gamma, vega}` object.
///
/// `vega` is per unit (1.0) change in `normalVol` (absolute units).
/// Bachelier (1900): see docs/REFERENCES.md#bachelier-1900.
/// @param forward - Forward price or rate at expiry (may be negative).
/// @param strike - Strike in the same units as `forward`.
/// @param normal_vol - Annualized absolute (normal) volatility in the units of `forward`.
/// @param expiry - Time to expiry in years.
/// @param is_call - Whether to value a call (`true`) or put (`false`).
///
/// # Errors
///
/// Throws a JavaScript exception if any Greek is non-finite.
#[wasm_bindgen(js_name = bachelierGreeks)]
pub fn bachelier_greeks(
    forward: f64,
    strike: f64,
    normal_vol: f64,
    expiry: f64,
    is_call: bool,
) -> Result<JsValue, JsValue> {
    let delta = if is_call {
        bachelier_delta_call(forward, strike, normal_vol, expiry)
    } else {
        bachelier_delta_put(forward, strike, normal_vol, expiry)
    };
    forward_greeks(
        delta,
        bachelier_gamma(forward, strike, normal_vol, expiry),
        bachelier_vega(forward, strike, normal_vol, expiry),
        "Bachelier",
    )
}

#[derive(serde::Serialize)]
struct ForwardGreeksJs {
    delta: f64,
    gamma: f64,
    vega: f64,
}

fn forward_greeks(delta: f64, gamma: f64, vega: f64, model: &str) -> Result<JsValue, JsValue> {
    for (name, value) in [("delta", delta), ("gamma", gamma), ("vega", vega)] {
        checked_closed_form_value(value, &format!("{model} {name}")).map_err(to_js_err)?;
    }
    crate::utils::to_js_value(&ForwardGreeksJs { delta, gamma, vega })
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
/// @param is_call - Whether to value a call (`true`) or put (`false`).
///
/// # Errors
///
/// Throws a JavaScript exception if the inputs produce a non-finite price.
#[wasm_bindgen(js_name = blackShiftedPrice)]
pub fn black_shifted_price(
    forward: f64,
    strike: f64,
    vol: f64,
    expiry: f64,
    shift: f64,
    is_call: bool,
) -> Result<f64, JsValue> {
    let value = if is_call {
        black_shifted_call(forward, strike, vol, expiry, shift)
    } else {
        black_shifted_put(forward, strike, vol, expiry, shift)
    };
    checked_closed_form_value(value, "shifted Black price").map_err(to_js_err)
}

/// Shifted (displaced) Black vega per unit (1.0) change in `vol`, undiscounted.
/// @param forward - Forward rate at expiry (decimal; may be negative).
/// @param strike - Strike (decimal, same units as `forward`).
/// @param vol - Annualized shifted-lognormal volatility, decimal.
/// @param expiry - Time to expiry in years.
/// @param shift - Displacement added to forward and strike, in rate units.
///
/// # Errors
///
/// Throws a JavaScript exception if the inputs produce a non-finite vega.
#[wasm_bindgen(js_name = blackShiftedVega)]
pub fn black_shifted_vega_js(
    forward: f64,
    strike: f64,
    vol: f64,
    expiry: f64,
    shift: f64,
) -> Result<f64, JsValue> {
    checked_closed_form_value(
        black_shifted_vega(forward, strike, vol, expiry, shift),
        "shifted Black vega",
    )
    .map_err(to_js_err)
}

/// Reiner-Rubinstein continuous-monitoring barrier call price.
///
/// `direction` is `"up"` or `"down"`, `knock` is `"in"` or `"out"`.
/// Reiner-Rubinstein (1991): see docs/REFERENCES.md#reiner-rubinstein-1991.
/// @param spot - Current spot price or exchange rate in the same units as the strike.
/// @param strike - Option strike price in the same price units as the underlying.
/// @param barrier - Continuously monitored barrier level in the same price units as spot.
/// @param rate - Continuously compounded risk-free rate, expressed as a decimal.
/// @param div_yield - Continuous dividend yield or foreign rate, expressed as a decimal.
/// @param vol - Annualized volatility expressed as a decimal, such as 0.20 for 20%.
/// @param expiry - Time to expiry in years.
/// @param direction - Barrier direction: `"up"` for an upper barrier or `"down"` for a lower barrier.
/// @param knock - Barrier activation: `"in"` for knock-in or `"out"` for knock-out.
///
/// # Errors
///
/// Throws a JavaScript exception if `direction` or `knock` is unsupported, or
/// the supplied model inputs produce a non-finite barrier price.
#[wasm_bindgen(js_name = barrierCall)]
#[allow(clippy::too_many_arguments)]
pub fn barrier_call(
    spot: f64,
    strike: f64,
    barrier: f64,
    rate: f64,
    div_yield: f64,
    vol: f64,
    expiry: f64,
    direction: &str,
    knock: &str,
) -> Result<f64, JsValue> {
    barrier_call_str(
        spot, strike, barrier, expiry, rate, div_yield, vol, direction, knock,
    )
    .map_err(to_js_err)
}

/// Reiner-Rubinstein continuous-monitoring barrier put price.
///
/// `direction` is `"up"` or `"down"`, `knock` is `"in"` or `"out"`.
/// Reiner-Rubinstein (1991): see docs/REFERENCES.md#reiner-rubinstein-1991.
/// @param spot - Current spot price or exchange rate in the same units as the strike.
/// @param strike - Option strike price in the same price units as the underlying.
/// @param barrier - Continuously monitored barrier level in the same price units as spot.
/// @param rate - Continuously compounded risk-free rate, expressed as a decimal.
/// @param div_yield - Continuous dividend yield or foreign rate, expressed as a decimal.
/// @param vol - Annualized volatility expressed as a decimal, such as 0.20 for 20%.
/// @param expiry - Time to expiry in years.
/// @param direction - Barrier direction: `"up"` for an upper barrier or `"down"` for a lower barrier.
/// @param knock - Barrier activation: `"in"` for knock-in or `"out"` for knock-out.
///
/// # Errors
///
/// Throws a JavaScript exception if `direction` or `knock` is unsupported, or
/// the supplied model inputs produce a non-finite barrier price.
#[wasm_bindgen(js_name = barrierPut)]
#[allow(clippy::too_many_arguments)]
pub fn barrier_put(
    spot: f64,
    strike: f64,
    barrier: f64,
    rate: f64,
    div_yield: f64,
    vol: f64,
    expiry: f64,
    direction: &str,
    knock: &str,
) -> Result<f64, JsValue> {
    barrier_put_str(
        spot, strike, barrier, expiry, rate, div_yield, vol, direction, knock,
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
/// @param div_yield - Continuous dividend yield or foreign rate, expressed as a decimal.
/// @param vol - Annualized volatility expressed as a decimal, such as 0.20 for 20%.
/// @param expiry - Time to expiry in years.
/// @param num_fixings - Positive number of equally spaced averaging observations before expiry.
/// @param averaging - Asian averaging convention: `"arithmetic"` (default) or `"geometric"`.
/// @param is_call - Whether to value a call (`true`) or put (`false`).
///
/// # Errors
///
/// Throws a JavaScript exception if `averaging` is not `"arithmetic"` or
/// `"geometric"`, or the supplied model inputs produce a non-finite option
/// price.
#[wasm_bindgen(js_name = asianOptionPrice)]
#[allow(clippy::too_many_arguments)]
pub fn asian_option_price(
    spot: f64,
    strike: f64,
    rate: f64,
    div_yield: f64,
    vol: f64,
    expiry: f64,
    num_fixings: usize,
    averaging: Option<String>,
    is_call: Option<bool>,
) -> Result<f64, JsValue> {
    let averaging = averaging.as_deref().unwrap_or("arithmetic");
    let option_type = OptionType::from(is_call.unwrap_or(true));
    asian_option_price_str(
        spot,
        strike,
        expiry,
        rate,
        div_yield,
        vol,
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
/// @param div_yield - Continuous dividend yield or foreign rate, expressed as a decimal.
/// @param vol - Annualized volatility expressed as a decimal, such as 0.20 for 20%.
/// @param expiry - Time to expiry in years.
/// @param extremum - Observed maximum for fixed calls or floating puts, minimum for fixed puts or floating calls, including current spot; in spot-price units.
/// @param strike_type - Lookback payoff convention: `"fixed"` (default) or `"floating"`.
/// @param is_call - Whether to value a call (`true`) or put (`false`).
///
/// # Errors
///
/// Throws a JavaScript exception if `strikeType` is not `"fixed"` or
/// `"floating"`, or the supplied model inputs produce a non-finite option
/// price.
#[wasm_bindgen(js_name = lookbackOptionPrice)]
#[allow(clippy::too_many_arguments)]
pub fn lookback_option_price(
    spot: f64,
    strike: f64,
    rate: f64,
    div_yield: f64,
    vol: f64,
    expiry: f64,
    extremum: f64,
    strike_type: Option<String>,
    is_call: Option<bool>,
) -> Result<f64, JsValue> {
    let strike_type = strike_type.as_deref().unwrap_or("fixed");
    let option_type = OptionType::from(is_call.unwrap_or(true));
    lookback_option_price_str(
        spot,
        strike,
        expiry,
        rate,
        div_yield,
        vol,
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
/// @param rate_domestic - Domestic continuously compounded risk-free rate, expressed as a decimal.
/// @param rate_foreign - Foreign continuously compounded risk-free rate, expressed as a decimal.
/// @param div_yield - Continuous dividend yield expressed as a decimal, such as 0.02 for 2%.
/// @param vol_asset - Annualized asset-price volatility expressed as a decimal.
/// @param vol_fx - Annualized FX-rate volatility expressed as a decimal.
/// @param correlation - Instantaneous correlation between the asset and FX-rate shocks, from -1 to 1.
/// @param is_call - Whether to value a call (`true`) or put (`false`).
#[wasm_bindgen(js_name = quantoOptionPrice)]
#[allow(clippy::too_many_arguments)]
pub fn quanto_option_price(
    spot: f64,
    strike: f64,
    expiry: f64,
    rate_domestic: f64,
    rate_foreign: f64,
    div_yield: f64,
    vol_asset: f64,
    vol_fx: f64,
    correlation: f64,
    is_call: Option<bool>,
) -> Result<f64, JsValue> {
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
/// @param div_yield - Continuous dividend yield or foreign rate, decimal.
/// @param kappa - Mean-reversion speed of the variance process (per year).
/// @param theta - Long-run variance level (variance units).
/// @param sigma_v - Volatility of variance (vol-of-vol).
/// @param rho - Spot/variance correlation in `(-1, 1)`.
/// @param v0 - Initial instantaneous variance (variance, not volatility).
/// @param is_call - Whether to value a call (`true`, default) or put (`false`).
///
/// # Errors
///
/// Throws a JavaScript exception if a parameter is non-finite or outside its
/// domain, or the Fourier integration fails to produce a finite price.
#[wasm_bindgen(js_name = hestonPrice)]
#[allow(clippy::too_many_arguments)]
pub fn heston_price(
    spot: f64,
    strike: f64,
    expiry: f64,
    rate: f64,
    div_yield: f64,
    kappa: f64,
    theta: f64,
    sigma_v: f64,
    rho: f64,
    v0: f64,
    is_call: Option<bool>,
) -> Result<f64, JsValue> {
    let params = HestonPricingParams::new(rate, div_yield, kappa, theta, sigma_v, rho, v0)
        .map_err(to_js_err)?;
    if is_call.unwrap_or(true) {
        heston_call_price_fourier(spot, strike, expiry, &params, None)
    } else {
        heston_put_price_fourier(spot, strike, expiry, &params, None)
    }
    .map_err(to_js_err)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heston_price_call_atm_is_reasonable() {
        let p = heston_price(
            100.0, 100.0, 1.0, 0.05, 0.02, 2.0, 0.04, 0.3, -0.7, 0.04, None,
        )
        .expect("finite price");
        assert!(p > 5.0 && p < 15.0, "price={p}");
    }

    #[test]
    fn barrier_put_dispatches() {
        let p = barrier_put(100.0, 100.0, 80.0, 0.05, 0.0, 0.2, 1.0, "down", "out")
            .expect("finite price");
        assert!(p > 0.0);
        assert!(barrier_put(100.0, 100.0, 80.0, 0.05, 0.0, 0.2, 1.0, "sideways", "out").is_err());
    }

    #[test]
    fn black76_and_bachelier_prices_are_positive() {
        assert!(black76_price(100.0, 100.0, 0.95, 1.0, 0.2, true).expect("price") > 0.0);
        assert!(bachelier_price(0.03, 0.03, 0.0075, 1.0, true).expect("price") > 0.0);
        assert!(black_shifted_price(-0.005, -0.005, 0.25, 1.0, 0.03, true).expect("price") > 0.0);
        assert!(black_shifted_vega_js(-0.005, -0.005, 0.25, 1.0, 0.03).expect("vega") > 0.0);
    }

    #[test]
    fn bs_price_call_atm_is_positive() {
        let p = bs_price(100.0, 100.0, 0.05, 0.02, 0.2, 1.0, true).expect("finite price");
        assert!(p > 0.0);
    }

    #[test]
    fn vanilla_expiry_payoff_call_itm() {
        let payoff = vanilla_expiry_payoff(110.0, 100.0, true).expect("finite payoff");
        assert!((payoff - 10.0).abs() < 1e-12);
    }

    #[test]
    fn vanilla_expiry_payoff_rejects_negative_spot() {
        assert!(vanilla_expiry_payoff(-1.0, 100.0, true).is_err());
        let put = vanilla_expiry_payoff(0.0, 100.0, false).expect("zero spot put");
        assert!((put - 100.0).abs() < 1e-12);
    }

    #[test]
    fn bs_implied_vol_recovers_sigma() {
        let sigma = 0.25;
        let price = bs_price(100.0, 110.0, 0.03, 0.01, sigma, 0.75, true).expect("finite price");
        let iv = bs_implied_vol(100.0, 110.0, 0.03, 0.01, 0.75, price, true)
            .expect("solver should converge");
        assert!((iv - sigma).abs() < 1e-6, "iv={iv} sigma={sigma}");
    }

    #[test]
    fn bs_price_rejects_non_finite_result() {
        // A degenerate input (huge maturity with a negative rate) drives
        // `exp(-r*t)` to `+inf`, which escapes the core's `.max(0.0)` clamp.
        // The binding guard must surface that as a thrown error rather than a
        // silent non-finite value crossing the wasm boundary.
        let result = bs_price(100.0, 100.0, -1.0, 0.0, 0.2, 1.0e6, false);
        assert!(
            result.is_err(),
            "a non-finite Black-Scholes price must produce an error"
        );
        // A well-posed input still returns a finite price unchanged.
        assert!(bs_price(100.0, 100.0, 0.05, 0.02, 0.2, 1.0, true).is_ok());
    }

    #[test]
    fn quanto_option_price_rejects_non_finite_result() {
        // Same degenerate-maturity path: a non-finite quanto price must throw.
        let result = quanto_option_price(
            100.0,
            100.0,
            1.0e6,
            -1.0,
            0.01,
            0.0,
            0.20,
            0.10,
            0.3,
            Some(false),
        );
        assert!(
            result.is_err(),
            "a non-finite quanto option price must produce an error"
        );
        // A well-posed input still returns a finite price.
        assert!(quanto_option_price(
            100.0,
            100.0,
            1.0,
            0.03,
            0.01,
            0.0,
            0.20,
            0.10,
            0.3,
            Some(true)
        )
        .is_ok());
    }
}
