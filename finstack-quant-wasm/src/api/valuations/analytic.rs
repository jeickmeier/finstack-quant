//! Closed-form analytic option primitives (Black-Scholes, Black-76, implied vol).
//!
//! Thin wasm-bindgen wrappers around the Rust closed-form formulas in
//! `finstack_quant_valuations::models::closed_form`.
//!
//! All rates are continuously compounded decimals; `sigma` is annualized vol;
//! `t` is time to expiry in years. Greeks scale matches the Rust crate:
//! `vega` and both rho values are per 1% move, `theta` is per-day under the
//! `thetaDays` day-count (ACT/365 by default).

use crate::utils::to_js_err;
use finstack_quant_valuations::models::closed_form::implied_vol::{
    black76_implied_vol as black76_implied_vol_core, bs_implied_vol as bs_implied_vol_core,
};
use finstack_quant_valuations::models::closed_form::{
    arithmetic_asian_call_tw, arithmetic_asian_put_tw, bs_greeks_checked as bs_greeks_core,
    bs_price_checked, checked_closed_form_value, down_in_call, down_out_call,
    fixed_strike_lookback_call, fixed_strike_lookback_put, floating_strike_lookback_call,
    floating_strike_lookback_put, geometric_asian_call, geometric_asian_put, option_type_from_bool,
    quanto_call, quanto_put, up_in_call, up_out_call,
};
use wasm_bindgen::prelude::*;

/// Per-unit Black-Scholes / Garman-Kohlhagen price of a European option.
///
/// @param spot - Spot price of the underlying.
/// @param strike - Strike of the option.
/// @param r - Risk-free rate, **decimal** continuously compounded
/// (e.g. `0.05` for 5%).
/// @param q - Continuous dividend yield (or foreign rate for FX),
/// **decimal** continuously compounded.
/// @param sigma - Annualized volatility, **decimal**
/// (e.g. `0.20` for 20%).
/// @param t - Time to expiry in **years**.
/// @param isCall - `true` for a call, `false` for a put.
/// @returns Per-unit option price.
///
/// @example
/// ```javascript
/// import init, { valuations } from "finstack-quant-wasm";
/// await init();
/// const price = valuations.bsPrice(
///   100,    // spot
///   100,    // strike (ATM)
///   0.05,   // r = 5%
///   0.0,    // q = 0
///   0.20,   // sigma = 20%
///   1.0,    // 1 year
///   true,   // call
/// );
/// // price ≈ 10.45
/// ```
///
/// @throws If the inputs produce a non-finite price (e.g. negative volatility).
#[wasm_bindgen(js_name = bsPrice)]
pub fn bs_price(
    spot: f64,
    strike: f64,
    r: f64,
    q: f64,
    sigma: f64,
    t: f64,
    is_call: bool,
) -> Result<f64, JsValue> {
    bs_price_checked(spot, strike, r, q, sigma, t, option_type_from_bool(is_call))
        .map_err(to_js_err)
}

/// Black-Scholes / Garman-Kohlhagen Greeks as a `{delta, gamma, vega, theta, rho, rho_q}` object.
///
/// @param spot - Spot price of the underlying.
/// @param strike - Strike of the option.
/// @param r - Risk-free rate, **decimal** continuously compounded.
/// @param q - Dividend yield (or foreign rate for FX), **decimal**
/// continuously compounded.
/// @param sigma - Annualized volatility, **decimal**.
/// @param t - Time to expiry in **years**.
/// @param isCall - `true` for a call, `false` for a put.
/// @param thetaDays - Day-count denominator for theta. Default `365`.
/// Pass `252` for trading-day theta.
/// @returns Object `{ delta, gamma, vega, theta, rho, rho_q }` (snake_case keys
/// matching the Rust/Python canonical names). `vega` and
/// both rho values are **per 1% move**; `theta` is **per day** under
/// `thetaDays`.
/// @throws If serialization to JS fails (should not happen on valid inputs).
///
/// @example
/// ```javascript
/// const g = valuations.bsGreeks(100, 100, 0.05, 0.0, 0.20, 1.0, true);
/// // g.delta ≈ 0.64, g.gamma ≈ 0.019, g.vega ≈ 0.38 (per 1% vol)
/// ```
#[wasm_bindgen(js_name = bsGreeks)]
#[allow(clippy::too_many_arguments)]
pub fn bs_greeks(
    spot: f64,
    strike: f64,
    r: f64,
    q: f64,
    sigma: f64,
    t: f64,
    is_call: bool,
    theta_days: Option<f64>,
) -> Result<JsValue, JsValue> {
    let theta_days = theta_days.unwrap_or(365.0);
    if !theta_days.is_finite() || theta_days <= 0.0 {
        return Err(JsValue::from_str(&format!(
            "thetaDays must be positive, got {theta_days}"
        )));
    }
    let g = bs_greeks_core(
        spot,
        strike,
        r,
        q,
        sigma,
        t,
        option_type_from_bool(is_call),
        theta_days,
    )
    .map_err(to_js_err)?;
    let obj = js_sys::Object::new();
    js_sys::Reflect::set(&obj, &"delta".into(), &g.delta.into())?;
    js_sys::Reflect::set(&obj, &"gamma".into(), &g.gamma.into())?;
    js_sys::Reflect::set(&obj, &"vega".into(), &g.vega.into())?;
    js_sys::Reflect::set(&obj, &"theta".into(), &g.theta.into())?;
    js_sys::Reflect::set(&obj, &"rho".into(), &g.rho_r.into())?;
    // snake_case to match the Rust canonical field (`rho_q`) and the Python
    // binding; the camelCase `rhoQ` was an outlier that yielded `undefined`
    // for any cross-binding consumer reading `rho_q`.
    js_sys::Reflect::set(&obj, &"rho_q".into(), &g.rho_q.into())?;
    Ok(obj.into())
}

/// Solve for Black-Scholes / Garman-Kohlhagen implied volatility.
///
/// @param spot - Spot price of the underlying.
/// @param strike - Strike of the option.
/// @param r - Risk-free rate, **decimal** continuously compounded.
/// @param q - Dividend yield, **decimal** continuously compounded.
/// @param t - Time to expiry in **years**.
/// @param price - Observed option price (per unit).
/// @param isCall - `true` for a call, `false` for a put.
/// @returns Annualized implied volatility, **decimal** (e.g. `0.20`).
/// @throws If `price` is below intrinsic value, above the no-arbitrage
/// upper bound, or the solver fails to converge.
///
/// @example
/// ```javascript
/// const iv = valuations.bsImpliedVol(100, 100, 0.05, 0.0, 1.0, 10.45, true);
/// // iv ≈ 0.20
/// ```
#[wasm_bindgen(js_name = bsImpliedVol)]
pub fn bs_implied_vol(
    spot: f64,
    strike: f64,
    r: f64,
    q: f64,
    t: f64,
    price: f64,
    is_call: bool,
) -> Result<f64, JsValue> {
    bs_implied_vol_core(spot, strike, r, q, t, option_type_from_bool(is_call), price)
        .map_err(to_js_err)
}

/// Solve for Black-76 (forward-based) implied volatility.
/// @param forward - Forward price or rate in the same quote convention as the strike.
/// @param strike - Option strike price in the same price units as the underlying.
/// @param df - Discount factor from valuation to expiry, expressed as a positive decimal.
/// @param t - Time from the curve base date in years on the documented day-count basis.
/// @param price - Price in the documented quote convention for this instrument.
/// @param is_call - Whether to value a call (`true`) or put (`false`); defaults follow the callable's contract.
#[wasm_bindgen(js_name = black76ImpliedVol)]
pub fn black76_implied_vol(
    forward: f64,
    strike: f64,
    df: f64,
    t: f64,
    price: f64,
    is_call: bool,
) -> Result<f64, JsValue> {
    black76_implied_vol_core(
        forward,
        strike,
        df,
        t,
        option_type_from_bool(is_call),
        price,
    )
    .map_err(to_js_err)
}

// ---------------------------------------------------------------------------
// Closed-form exotics: barrier / asian / lookback / quanto
// ---------------------------------------------------------------------------

/// Reiner-Rubinstein continuous-monitoring barrier call price.
///
/// `direction` is `"up"` or `"down"`, `knock` is `"in"` or `"out"`.
/// @param spot - Current spot price or exchange rate in the documented quote convention.
/// @param strike - Option strike price in the same price units as the underlying.
/// @param barrier - Continuously monitored barrier level in the same price units as spot.
/// @param r - Continuously compounded risk-free rate, expressed as a decimal.
/// @param q - Continuous dividend yield or foreign rate, expressed as a decimal.
/// @param sigma - Annualized volatility expressed as a decimal, such as 0.20 for 20%.
/// @param t - Time from the curve base date in years on the documented day-count basis.
/// @param direction - Barrier direction: `"up"` for an upper barrier or `"down"` for a lower barrier.
/// @param knock - Barrier activation: `"in"` for knock-in or `"out"` for knock-out.
#[wasm_bindgen(js_name = barrierCall)]
#[allow(clippy::too_many_arguments)]
pub fn barrier_call(
    spot: f64,
    strike: f64,
    barrier: f64,
    r: f64,
    q: f64,
    sigma: f64,
    t: f64,
    direction: &str,
    knock: &str,
) -> Result<f64, JsValue> {
    let value = match (direction, knock) {
        ("up", "in") => up_in_call(spot, strike, barrier, t, r, q, sigma),
        ("up", "out") => up_out_call(spot, strike, barrier, t, r, q, sigma),
        ("down", "in") => down_in_call(spot, strike, barrier, t, r, q, sigma),
        ("down", "out") => down_out_call(spot, strike, barrier, t, r, q, sigma),
        _ => {
            return Err(to_js_err(format!(
                "unknown barrier spec: direction='{direction}' knock='{knock}'"
            )));
        }
    };
    finstack_quant_valuations::models::closed_form::checked_closed_form_value(
        value,
        "barrier price",
    )
    .map_err(to_js_err)
}

/// Arithmetic (Turnbull-Wakeman) or geometric (Kemna-Vorst) Asian option.
/// @param spot - Current spot price or exchange rate in the documented quote convention.
/// @param strike - Option strike price in the same price units as the underlying.
/// @param r - Continuously compounded risk-free rate, expressed as a decimal.
/// @param q - Continuous dividend yield or foreign rate, expressed as a decimal.
/// @param sigma - Annualized volatility expressed as a decimal, such as 0.20 for 20%.
/// @param t - Time from the curve base date in years on the documented day-count basis.
/// @param num_fixings - Positive number of equally spaced averaging observations before expiry.
/// @param averaging - Asian averaging convention: `"arithmetic"` (default) or `"geometric"`.
/// @param is_call - Whether to value a call (`true`) or put (`false`); defaults follow the callable's contract.
#[wasm_bindgen(js_name = asianOptionPrice)]
#[allow(clippy::too_many_arguments)]
pub fn asian_option_price(
    spot: f64,
    strike: f64,
    r: f64,
    q: f64,
    sigma: f64,
    t: f64,
    num_fixings: usize,
    averaging: Option<String>,
    is_call: Option<bool>,
) -> Result<f64, JsValue> {
    let averaging = averaging.as_deref().unwrap_or("arithmetic");
    let is_call = is_call.unwrap_or(true);
    let value = match (averaging, is_call) {
        ("arithmetic", true) => arithmetic_asian_call_tw(spot, strike, t, r, q, sigma, num_fixings),
        ("arithmetic", false) => arithmetic_asian_put_tw(spot, strike, t, r, q, sigma, num_fixings),
        ("geometric", true) => geometric_asian_call(spot, strike, t, r, q, sigma, num_fixings),
        ("geometric", false) => geometric_asian_put(spot, strike, t, r, q, sigma, num_fixings),
        _ => {
            return Err(to_js_err(format!(
                "unknown averaging '{averaging}'; expected 'arithmetic' or 'geometric'"
            )));
        }
    };
    checked_closed_form_value(value, "asian option price").map_err(to_js_err)
}

/// Conze-Viswanathan lookback option.
///
/// `strike_type` is `"fixed"` (default) or `"floating"`. For `"floating"`,
/// `strike` is ignored and `extremum` is the observed min/max to date.
/// @param spot - Current spot price or exchange rate in the documented quote convention.
/// @param strike - Option strike price in the same price units as the underlying.
/// @param r - Continuously compounded risk-free rate, expressed as a decimal.
/// @param q - Continuous dividend yield or foreign rate, expressed as a decimal.
/// @param sigma - Annualized volatility expressed as a decimal, such as 0.20 for 20%.
/// @param t - Time from the curve base date in years on the documented day-count basis.
/// @param extremum - Observed running minimum for a call or maximum for a put, in spot-price units.
/// @param strike_type - Lookback payoff convention: `"fixed"` (default) or `"floating"`.
/// @param is_call - Whether to value a call (`true`) or put (`false`); defaults follow the callable's contract.
#[wasm_bindgen(js_name = lookbackOptionPrice)]
#[allow(clippy::too_many_arguments)]
pub fn lookback_option_price(
    spot: f64,
    strike: f64,
    r: f64,
    q: f64,
    sigma: f64,
    t: f64,
    extremum: f64,
    strike_type: Option<String>,
    is_call: Option<bool>,
) -> Result<f64, JsValue> {
    let strike_type = strike_type.as_deref().unwrap_or("fixed");
    let is_call = is_call.unwrap_or(true);
    let value = match (strike_type, is_call) {
        ("fixed", true) => fixed_strike_lookback_call(spot, strike, t, r, q, sigma, extremum),
        ("fixed", false) => fixed_strike_lookback_put(spot, strike, t, r, q, sigma, extremum),
        ("floating", true) => floating_strike_lookback_call(spot, t, r, q, sigma, extremum),
        ("floating", false) => floating_strike_lookback_put(spot, t, r, q, sigma, extremum),
        _ => {
            return Err(to_js_err(format!(
                "unknown strike_type '{strike_type}'; expected 'fixed' or 'floating'"
            )));
        }
    };
    checked_closed_form_value(value, "lookback option price").map_err(to_js_err)
}

/// Quanto option (FX-adjusted cross-currency) price in domestic currency.
///
/// @throws If the inputs produce a non-finite price.
/// @param spot - Current spot price or exchange rate in the documented quote convention.
/// @param strike - Option strike price in the same price units as the underlying.
/// @param t - Time from the curve base date in years on the documented day-count basis.
/// @param rate_domestic - Domestic continuously compounded risk-free rate, expressed as a decimal.
/// @param rate_foreign - Foreign continuously compounded risk-free rate, expressed as a decimal.
/// @param div_yield - Continuous dividend yield expressed as a decimal, such as 0.02 for 2%.
/// @param vol_asset - Annualized asset-price volatility expressed as a decimal.
/// @param vol_fx - Annualized FX-rate volatility expressed as a decimal.
/// @param correlation - Instantaneous correlation between the documented asset and FX-rate shocks, from -1 to 1.
/// @param is_call - Whether to value a call (`true`) or put (`false`); defaults follow the callable's contract.
#[wasm_bindgen(js_name = quantoOptionPrice)]
#[allow(clippy::too_many_arguments)]
pub fn quanto_option_price(
    spot: f64,
    strike: f64,
    t: f64,
    rate_domestic: f64,
    rate_foreign: f64,
    div_yield: f64,
    vol_asset: f64,
    vol_fx: f64,
    correlation: f64,
    is_call: Option<bool>,
) -> Result<f64, JsValue> {
    let price = if is_call.unwrap_or(true) {
        quanto_call(
            spot,
            strike,
            t,
            rate_domestic,
            rate_foreign,
            div_yield,
            vol_asset,
            vol_fx,
            correlation,
        )
    } else {
        quanto_put(
            spot,
            strike,
            t,
            rate_domestic,
            rate_foreign,
            div_yield,
            vol_asset,
            vol_fx,
            correlation,
        )
    };
    checked_closed_form_value(price, "quanto option price").map_err(to_js_err)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bs_price_call_atm_is_positive() {
        let p = bs_price(100.0, 100.0, 0.05, 0.02, 0.2, 1.0, true).expect("finite price");
        assert!(p > 0.0);
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
