//! Closed-form analytic option primitives (Black-Scholes, Black-76, implied vol).
//!
//! Thin wasm-bindgen wrappers around the Rust closed-form formulas in
//! `finstack_valuations::models::closed_form`.
//!
//! All rates are continuously compounded decimals; `sigma` is annualized vol;
//! `t` is time to expiry in years. Greeks scale matches the Rust crate:
//! `vega` and both rho values are per 1% move, `theta` is per-day under the
//! `thetaDays` day-count (ACT/365 by default).

use crate::utils::to_js_err;
use finstack_valuations::instruments::OptionType;
use finstack_valuations::models::closed_form::implied_vol::{black76_implied_vol, bs_implied_vol};
use finstack_valuations::models::closed_form::{
    arithmetic_asian_call_tw, arithmetic_asian_put_tw, bs_greeks, bs_price, down_in_call,
    down_out_call, fixed_strike_lookback_call, fixed_strike_lookback_put,
    floating_strike_lookback_call, floating_strike_lookback_put, geometric_asian_call,
    geometric_asian_put, quanto_call, quanto_put, up_in_call, up_out_call,
};
use wasm_bindgen::prelude::*;

fn option_type(is_call: bool) -> OptionType {
    if is_call {
        OptionType::Call
    } else {
        OptionType::Put
    }
}

/// Guard a closed-form price against non-finite results.
///
/// The underlying `closed_form` formulas return a raw `f64` and yield `NaN`
/// or `±inf` for degenerate / out-of-domain inputs (e.g. negative volatility).
/// Surfacing that as a thrown error — rather than a silent `NaN` crossing the
/// wasm boundary — keeps these wrappers consistent with `bsImpliedVol`, which
/// already returns a `Result`. `what` names the quantity for the message.
fn finite_price(value: f64, what: &str) -> Result<f64, JsValue> {
    if value.is_finite() {
        Ok(value)
    } else {
        Err(to_js_err(format!(
            "{what} is not finite ({value}); check inputs (volatility, time \
             to expiry, spot, strike) are in the model's valid domain"
        )))
    }
}

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
/// import init, { valuations } from "finstack-wasm";
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
pub fn bs_price_js(
    spot: f64,
    strike: f64,
    r: f64,
    q: f64,
    sigma: f64,
    t: f64,
    is_call: bool,
) -> Result<f64, JsValue> {
    finite_price(
        bs_price(spot, strike, r, q, sigma, t, option_type(is_call)),
        "Black-Scholes price",
    )
}

/// Black-Scholes / Garman-Kohlhagen Greeks as a `{delta, gamma, vega, theta, rho, rhoQ}` object.
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
/// @returns Object `{ delta, gamma, vega, theta, rho, rhoQ }`. `vega` and
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
pub fn bs_greeks_js(
    spot: f64,
    strike: f64,
    r: f64,
    q: f64,
    sigma: f64,
    t: f64,
    is_call: bool,
    theta_days: Option<f64>,
) -> Result<JsValue, JsValue> {
    let g = bs_greeks(
        spot,
        strike,
        r,
        q,
        sigma,
        t,
        option_type(is_call),
        theta_days.unwrap_or(365.0),
    );
    let obj = js_sys::Object::new();
    js_sys::Reflect::set(&obj, &"delta".into(), &g.delta.into())?;
    js_sys::Reflect::set(&obj, &"gamma".into(), &g.gamma.into())?;
    js_sys::Reflect::set(&obj, &"vega".into(), &g.vega.into())?;
    js_sys::Reflect::set(&obj, &"theta".into(), &g.theta.into())?;
    js_sys::Reflect::set(&obj, &"rho".into(), &g.rho_r.into())?;
    js_sys::Reflect::set(&obj, &"rhoQ".into(), &g.rho_q.into())?;
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
pub fn bs_implied_vol_js(
    spot: f64,
    strike: f64,
    r: f64,
    q: f64,
    t: f64,
    price: f64,
    is_call: bool,
) -> Result<f64, JsValue> {
    bs_implied_vol(spot, strike, r, q, t, option_type(is_call), price).map_err(to_js_err)
}

/// Solve for Black-76 (forward-based) implied volatility.
#[wasm_bindgen(js_name = black76ImpliedVol)]
pub fn black76_implied_vol_js(
    forward: f64,
    strike: f64,
    df: f64,
    t: f64,
    price: f64,
    is_call: bool,
) -> Result<f64, JsValue> {
    black76_implied_vol(forward, strike, df, t, option_type(is_call), price).map_err(to_js_err)
}

// ---------------------------------------------------------------------------
// Closed-form exotics: barrier / asian / lookback / quanto
// ---------------------------------------------------------------------------

/// Reiner-Rubinstein continuous-monitoring barrier call price.
///
/// `direction` is `"up"` or `"down"`, `knock` is `"in"` or `"out"`.
#[wasm_bindgen(js_name = barrierCall)]
#[allow(clippy::too_many_arguments)]
pub fn barrier_call_js(
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
    Ok(match (direction, knock) {
        ("up", "in") => up_in_call(spot, strike, barrier, t, r, q, sigma),
        ("up", "out") => up_out_call(spot, strike, barrier, t, r, q, sigma),
        ("down", "in") => down_in_call(spot, strike, barrier, t, r, q, sigma),
        ("down", "out") => down_out_call(spot, strike, barrier, t, r, q, sigma),
        _ => {
            return Err(to_js_err(format!(
                "unknown barrier spec: direction='{direction}' knock='{knock}'"
            )));
        }
    })
}

/// Arithmetic (Turnbull-Wakeman) or geometric (Kemna-Vorst) Asian option.
#[wasm_bindgen(js_name = asianOptionPrice)]
#[allow(clippy::too_many_arguments)]
pub fn asian_option_price_js(
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
    Ok(match (averaging, is_call) {
        ("arithmetic", true) => arithmetic_asian_call_tw(spot, strike, t, r, q, sigma, num_fixings),
        ("arithmetic", false) => arithmetic_asian_put_tw(spot, strike, t, r, q, sigma, num_fixings),
        ("geometric", true) => geometric_asian_call(spot, strike, t, r, q, sigma, num_fixings),
        ("geometric", false) => geometric_asian_put(spot, strike, t, r, q, sigma, num_fixings),
        _ => {
            return Err(to_js_err(format!(
                "unknown averaging '{averaging}'; expected 'arithmetic' or 'geometric'"
            )));
        }
    })
}

/// Conze-Viswanathan lookback option.
///
/// `strike_type` is `"fixed"` (default) or `"floating"`. For `"floating"`,
/// `strike` is ignored and `extremum` is the observed min/max to date.
#[wasm_bindgen(js_name = lookbackOptionPrice)]
#[allow(clippy::too_many_arguments)]
pub fn lookback_option_price_js(
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
    Ok(match (strike_type, is_call) {
        ("fixed", true) => fixed_strike_lookback_call(spot, strike, t, r, q, sigma, extremum),
        ("fixed", false) => fixed_strike_lookback_put(spot, strike, t, r, q, sigma, extremum),
        ("floating", true) => floating_strike_lookback_call(spot, t, r, q, sigma, extremum),
        ("floating", false) => floating_strike_lookback_put(spot, t, r, q, sigma, extremum),
        _ => {
            return Err(to_js_err(format!(
                "unknown strike_type '{strike_type}'; expected 'fixed' or 'floating'"
            )));
        }
    })
}

/// Quanto option (FX-adjusted cross-currency) price in domestic currency.
///
/// @throws If the inputs produce a non-finite price.
#[wasm_bindgen(js_name = quantoOptionPrice)]
#[allow(clippy::too_many_arguments)]
pub fn quanto_option_price_js(
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
    finite_price(price, "quanto option price")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bs_price_call_atm_is_positive() {
        let p = bs_price_js(100.0, 100.0, 0.05, 0.02, 0.2, 1.0, true).expect("finite price");
        assert!(p > 0.0);
    }

    #[test]
    fn bs_implied_vol_recovers_sigma() {
        let sigma = 0.25;
        let price = bs_price_js(100.0, 110.0, 0.03, 0.01, sigma, 0.75, true).expect("finite price");
        let iv = bs_implied_vol_js(100.0, 110.0, 0.03, 0.01, 0.75, price, true)
            .expect("solver should converge");
        assert!((iv - sigma).abs() < 1e-6, "iv={iv} sigma={sigma}");
    }

    #[test]
    fn bs_price_rejects_non_finite_result() {
        // A degenerate input (huge maturity with a negative rate) drives
        // `exp(-r*t)` to `+inf`, which escapes the core's `.max(0.0)` clamp.
        // The binding guard must surface that as a thrown error rather than a
        // silent non-finite value crossing the wasm boundary.
        let result = bs_price_js(100.0, 100.0, -1.0, 0.0, 0.2, 1.0e6, false);
        assert!(
            result.is_err(),
            "a non-finite Black-Scholes price must produce an error"
        );
        // A well-posed input still returns a finite price unchanged.
        assert!(bs_price_js(100.0, 100.0, 0.05, 0.02, 0.2, 1.0, true).is_ok());
    }

    #[test]
    fn quanto_option_price_rejects_non_finite_result() {
        // Same degenerate-maturity path: a non-finite quanto price must throw.
        let result = quanto_option_price_js(
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
        assert!(quanto_option_price_js(
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
