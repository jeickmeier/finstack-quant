//! WASM bindings for the Monte Carlo engine in `finstack-quant-models`.
//!
//! Provides the host-neutral subset shared with Python: Heston Monte Carlo
//! pricing, returned as the canonical Rust `MoneyEstimate` serde object.
//! Closed-form Black-Scholes references live in `models.bsPrice`. GBM path
//! simulation, the European/LSMC/path-dependent pricers and the finite-difference
//! Greeks are bound in Python only (`parity_contract.toml` backlog for
//! `finstack_quant.models.monte_carlo`); processes, discretizations and RNGs
//! remain Rust-only.
//!

use crate::utils::input::{js_opt_string, js_opt_u64, js_opt_uint};
use std::str::FromStr;

use crate::utils::to_js_err;
use finstack_quant_core::currency::Currency;
use wasm_bindgen::prelude::*;

#[allow(clippy::too_many_arguments)]
/// Price a European call under Heston stochastic volatility.
///
/// # Errors
///
/// Throws a JavaScript exception if `currency` is unknown; embedded defaults cannot be
/// loaded when `num_steps` is omitted; `rate` or `div_yield` is non-finite;
/// `kappa`, `theta`, `vol_of_vol`, or `v0` is non-finite or non-positive;
/// `rho` is outside `[-1, 1]`; the expiry, step count, path count, or computed
/// discount factor fails validation; a simulated discounted payoff is
/// non-finite; or the result cannot be serialized.
/// @returns The Rust `MoneyEstimate` serde object: `mean` and `ci_95` as
/// `{amount, currency}` money, plus `stderr`, `num_paths`,
/// `num_simulated_paths` and the optional statistics (`null` when not captured).
/// @param spot - Current spot price or exchange rate in the same units as the strike.
/// @param strike - Option strike price in the same price units as the underlying.
/// @param rate - Interest rate expressed as a decimal, such as 0.05 for 5%.
/// @param div_yield - Continuous dividend yield expressed as a decimal, such as 0.02 for 2%.
/// @param kappa - Mean-reversion speed of variance in the Heston stochastic-volatility model.
/// @param theta - Long-run variance level in the Heston stochastic-volatility model.
/// @param vol_of_vol - Annualized volatility of variance in the Heston stochastic-volatility model.
/// @param rho - Instantaneous correlation between the asset and variance shocks.
/// @param v0 - Initial instantaneous variance in the Heston stochastic-volatility model.
/// @param expiry - Time to option expiry in years on the model's annual time basis.
/// @param num_paths - Number of simulated stochastic paths; omitted or `null` uses the Rust
/// registry European-pricer default (100 000).
/// @param seed - Deterministic random-number seed (number or BigInt); omitted or `null` uses
/// the Rust registry default seed, so results stay reproducible.
/// @param num_steps - Number of time steps per simulated path.
/// @param currency - ISO-4217 currency code for the monetary amount or market convention.
#[wasm_bindgen(js_name = priceHestonCall)]
pub fn price_heston_call(
    spot: f64,
    strike: f64,
    rate: f64,
    div_yield: f64,
    kappa: f64,
    theta: f64,
    vol_of_vol: f64,
    rho: f64,
    v0: f64,
    expiry: f64,
    num_paths: Option<JsValue>,
    seed: Option<JsValue>,
    num_steps: Option<JsValue>,
    currency: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let num_paths: Option<usize> = js_opt_uint(num_paths.as_ref(), "numPaths")?;
    let seed = js_opt_u64(seed.as_ref(), "seed")?;
    let num_steps: Option<usize> = js_opt_uint(num_steps.as_ref(), "numSteps")?;
    let currency = js_opt_string(currency.as_ref(), "currency")?;
    price_heston(
        true, spot, strike, rate, div_yield, kappa, theta, vol_of_vol, rho, v0, expiry, num_paths,
        seed, num_steps, currency,
    )
}

#[allow(clippy::too_many_arguments)]
/// Price a European put under Heston stochastic volatility.
///
/// # Errors
///
/// Throws a JavaScript exception if `currency` is unknown; embedded defaults cannot be
/// loaded when `num_steps` is omitted; `rate` or `div_yield` is non-finite;
/// `kappa`, `theta`, `vol_of_vol`, or `v0` is non-finite or non-positive;
/// `rho` is outside `[-1, 1]`; the expiry, step count, path count, or computed
/// discount factor fails validation; a simulated discounted payoff is
/// non-finite; or the result cannot be serialized.
/// @returns The Rust `MoneyEstimate` serde object: `mean` and `ci_95` as
/// `{amount, currency}` money, plus `stderr`, `num_paths`,
/// `num_simulated_paths` and the optional statistics (`null` when not captured).
/// @param spot - Current spot price or exchange rate in the same units as the strike.
/// @param strike - Option strike price in the same price units as the underlying.
/// @param rate - Interest rate expressed as a decimal, such as 0.05 for 5%.
/// @param div_yield - Continuous dividend yield expressed as a decimal, such as 0.02 for 2%.
/// @param kappa - Mean-reversion speed of variance in the Heston stochastic-volatility model.
/// @param theta - Long-run variance level in the Heston stochastic-volatility model.
/// @param vol_of_vol - Annualized volatility of variance in the Heston stochastic-volatility model.
/// @param rho - Instantaneous correlation between the asset and variance shocks.
/// @param v0 - Initial instantaneous variance in the Heston stochastic-volatility model.
/// @param expiry - Time to option expiry in years on the model's annual time basis.
/// @param num_paths - Number of simulated stochastic paths; omitted or `null` uses the Rust
/// registry European-pricer default (100 000).
/// @param seed - Deterministic random-number seed (number or BigInt); omitted or `null` uses
/// the Rust registry default seed, so results stay reproducible.
/// @param num_steps - Number of time steps per simulated path.
/// @param currency - ISO-4217 currency code for the monetary amount or market convention.
#[wasm_bindgen(js_name = priceHestonPut)]
pub fn price_heston_put(
    spot: f64,
    strike: f64,
    rate: f64,
    div_yield: f64,
    kappa: f64,
    theta: f64,
    vol_of_vol: f64,
    rho: f64,
    v0: f64,
    expiry: f64,
    num_paths: Option<JsValue>,
    seed: Option<JsValue>,
    num_steps: Option<JsValue>,
    currency: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let num_paths: Option<usize> = js_opt_uint(num_paths.as_ref(), "numPaths")?;
    let seed = js_opt_u64(seed.as_ref(), "seed")?;
    let num_steps: Option<usize> = js_opt_uint(num_steps.as_ref(), "numSteps")?;
    let currency = js_opt_string(currency.as_ref(), "currency")?;
    price_heston(
        false, spot, strike, rate, div_yield, kappa, theta, vol_of_vol, rho, v0, expiry, num_paths,
        seed, num_steps, currency,
    )
}

#[allow(clippy::too_many_arguments)]
fn price_heston(
    is_call: bool,
    spot: f64,
    strike: f64,
    rate: f64,
    div_yield: f64,
    kappa: f64,
    theta: f64,
    vol_of_vol: f64,
    rho: f64,
    v0: f64,
    expiry: f64,
    num_paths: Option<usize>,
    seed: Option<u64>,
    num_steps: Option<usize>,
    currency: Option<String>,
) -> Result<JsValue, JsValue> {
    use finstack_quant_models::monte_carlo::pricer::heston as canonical;

    // The canonical entry point owns the registry defaults for path count,
    // seed, step count, currency, and (wasm-gated) parallelism; the binding
    // only marshals explicitly supplied values.
    let ccy = currency
        .as_deref()
        .map(|code| Currency::from_str(code).map_err(to_js_err))
        .transpose()?;
    let est = if is_call {
        canonical::price_heston_call(
            spot, strike, rate, div_yield, kappa, theta, vol_of_vol, rho, v0, expiry, num_paths,
            seed, num_steps, ccy,
        )
    } else {
        canonical::price_heston_put(
            spot, strike, rate, div_yield, kappa, theta, vol_of_vol, rho, v0, expiry, num_paths,
            seed, num_steps, ccy,
        )
    }
    .map_err(to_js_err)?;
    crate::utils::to_js_value(&est)
}
