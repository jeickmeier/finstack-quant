//! WASM bindings for the Monte Carlo engine in `finstack-quant-models`.
//!
//! Mirrors `finstack-quant-py/src/bindings/models/monte_carlo/`: Heston and
//! GBM convenience pricers, GBM path simulation and finite-difference Greeks.
//! Estimates cross the boundary as the canonical Rust `MoneyEstimate` /
//! `Estimate` serde objects. Closed-form Black-Scholes references live in
//! `models.bsPrice`; processes, discretizations and RNGs remain Rust-only.

use crate::utils::input::{
    from_js_json, js_bool, js_f64, js_opt_bool, js_opt_f64, js_opt_string, js_opt_u64, js_opt_uint,
    js_u64, js_uint,
};
use std::str::FromStr;

use crate::utils::{to_js_err, to_js_value};
use finstack_quant_core::currency::Currency;
use finstack_quant_models::monte_carlo::convenience::{self, LsmcConvenience};
use finstack_quant_models::monte_carlo::greeks::gbm_european::{
    finite_diff_delta_crn_gbm, finite_diff_delta_gbm, finite_diff_gamma_crn_gbm,
    finite_diff_gamma_gbm, GbmEuropeanFdSpec,
};
use finstack_quant_models::monte_carlo::pricer::european::EuropeanPricer;
use finstack_quant_models::monte_carlo::pricer::path_dependent::PathDependentPricer;
use finstack_quant_models::monte_carlo::results::MoneyEstimate;
use finstack_quant_models::OptionType;
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
/// @param num_steps - Optional time steps per simulated path; omitted uses the Rust registry default.
/// @param currency - Optional ISO-4217 code stamped on the estimate; omitted uses the Rust registry default currency.
#[wasm_bindgen(js_name = priceHestonCall)]
pub fn price_heston_call(
    spot: JsValue,
    strike: JsValue,
    rate: JsValue,
    div_yield: JsValue,
    kappa: JsValue,
    theta: JsValue,
    vol_of_vol: JsValue,
    rho: JsValue,
    v0: JsValue,
    expiry: JsValue,
    num_paths: Option<JsValue>,
    seed: Option<JsValue>,
    num_steps: Option<JsValue>,
    currency: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let spot = js_f64(&spot, "spot")?;
    let strike = js_f64(&strike, "strike")?;
    let rate = js_f64(&rate, "rate")?;
    let div_yield = js_f64(&div_yield, "divYield")?;
    let kappa = js_f64(&kappa, "kappa")?;
    let theta = js_f64(&theta, "theta")?;
    let vol_of_vol = js_f64(&vol_of_vol, "volOfVol")?;
    let rho = js_f64(&rho, "rho")?;
    let v0 = js_f64(&v0, "v0")?;
    let expiry = js_f64(&expiry, "expiry")?;
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
/// @param num_steps - Optional time steps per simulated path; omitted uses the Rust registry default.
/// @param currency - Optional ISO-4217 code stamped on the estimate; omitted uses the Rust registry default currency.
#[wasm_bindgen(js_name = priceHestonPut)]
pub fn price_heston_put(
    spot: JsValue,
    strike: JsValue,
    rate: JsValue,
    div_yield: JsValue,
    kappa: JsValue,
    theta: JsValue,
    vol_of_vol: JsValue,
    rho: JsValue,
    v0: JsValue,
    expiry: JsValue,
    num_paths: Option<JsValue>,
    seed: Option<JsValue>,
    num_steps: Option<JsValue>,
    currency: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let spot = js_f64(&spot, "spot")?;
    let strike = js_f64(&strike, "strike")?;
    let rate = js_f64(&rate, "rate")?;
    let div_yield = js_f64(&div_yield, "divYield")?;
    let kappa = js_f64(&kappa, "kappa")?;
    let theta = js_f64(&theta, "theta")?;
    let vol_of_vol = js_f64(&vol_of_vol, "volOfVol")?;
    let rho = js_f64(&rho, "rho")?;
    let v0 = js_f64(&v0, "v0")?;
    let expiry = js_f64(&expiry, "expiry")?;
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

/// The six GBM option inputs shared by the convenience entry points.
struct GbmInputs {
    spot: f64,
    strike: f64,
    rate: f64,
    div_yield: f64,
    vol: f64,
    expiry: f64,
}

fn gbm_inputs(
    spot: &JsValue,
    strike: &JsValue,
    rate: &JsValue,
    div_yield: &JsValue,
    vol: &JsValue,
    expiry: &JsValue,
) -> Result<GbmInputs, JsValue> {
    Ok(GbmInputs {
        spot: js_f64(spot, "spot")?,
        strike: js_f64(strike, "strike")?,
        rate: js_f64(rate, "rate")?,
        div_yield: js_f64(div_yield, "divYield")?,
        vol: js_f64(vol, "vol")?,
        expiry: js_f64(expiry, "expiry")?,
    })
}

fn js_opt_currency(currency: Option<&JsValue>) -> Result<Option<Currency>, JsValue> {
    js_opt_string(currency, "currency")?
        .map(|code| Currency::from_str(&code).map_err(to_js_err))
        .transpose()
}

fn js_currency(currency: Option<&JsValue>) -> Result<Currency, JsValue> {
    convenience::resolve_currency(js_opt_currency(currency)?).map_err(to_js_err)
}

/// Whether Heston parameters satisfy the Feller condition `2 * kappa * theta >= volOfVol^2`.
/// @param kappa - Mean-reversion speed of the variance, per year.
/// @param theta - Long-run variance level (annualized, as a decimal).
/// @param vol_of_vol - Volatility of variance (annualized, as a decimal).
/// @returns `true` when the variance process stays strictly positive.
///
/// # Errors
///
/// Throws a `TypeError` if an argument is not a number.
#[wasm_bindgen(js_name = hestonSatisfiesFeller)]
pub fn heston_satisfies_feller(
    kappa: JsValue,
    theta: JsValue,
    vol_of_vol: JsValue,
) -> Result<bool, JsValue> {
    Ok(
        finstack_quant_models::monte_carlo::process::heston::feller_condition(
            js_f64(&kappa, "kappa")?,
            js_f64(&theta, "theta")?,
            js_f64(&vol_of_vol, "volOfVol")?,
        ),
    )
}

/// Simulate a compact set of GBM spot paths.
/// @param spot - Spot level at time 0.
/// @param rate - Continuously compounded risk-free rate (decimal, annualized).
/// @param div_yield - Continuous dividend yield (decimal, annualized).
/// @param vol - Annualized GBM volatility (decimal).
/// @param expiry - Horizon in years; the grid is uniform from 0 to `expiry`.
/// @param num_steps - Number of time-grid steps; a positive safe integer.
/// @param num_paths - Number of captured paths; a positive safe integer.
/// @param seed - Optional RNG seed as a safe integer or `bigint`; omitted uses the Rust `GbmPathConfig` default.
/// @param antithetic - Optional; when `true`, paths are generated in antithetic pairs. Omitted uses the Rust default (`false`).
/// @returns The `GbmPathSummary` object (`num_paths`, `num_simulated_paths`, `times`, `paths`).
///
/// # Errors
///
/// Throws a `TypeError` if a count is not a safe integer, and a `validation`
/// error if an input is non-finite or out of range.
#[allow(clippy::too_many_arguments)]
#[wasm_bindgen(js_name = simulateGbmPaths)]
pub fn simulate_gbm_paths(
    spot: JsValue,
    rate: JsValue,
    div_yield: JsValue,
    vol: JsValue,
    expiry: JsValue,
    num_steps: JsValue,
    num_paths: JsValue,
    seed: Option<JsValue>,
    antithetic: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let mut config = finstack_quant_models::monte_carlo::GbmPathConfig::new(
        js_f64(&spot, "spot")?,
        js_f64(&rate, "rate")?,
        js_f64(&div_yield, "divYield")?,
        js_f64(&vol, "vol")?,
        js_f64(&expiry, "expiry")?,
        js_uint(&num_steps, "numSteps")?,
        js_uint(&num_paths, "numPaths")?,
    );
    if let Some(seed) = js_opt_u64(seed.as_ref(), "seed")? {
        config = config.with_seed(seed);
    }
    if let Some(antithetic) = js_opt_bool(antithetic.as_ref(), "antithetic")? {
        config = config.with_antithetic(antithetic);
    }
    let summary =
        finstack_quant_models::monte_carlo::simulate_gbm_paths(&config).map_err(to_js_err)?;
    to_js_value(&summary)
}

/// Relative standard error of a Monte Carlo money estimate, `stderr / |mean|`.
///
/// Twin of the Rust and Python `MoneyEstimate.relative_stderr`.
/// @param estimate - `MoneyEstimate` object or JSON, as returned by the pricing functions of this namespace.
/// @returns The relative standard error; `Infinity` when the mean is numerically zero.
///
/// # Errors
///
/// Throws a `validation` error if `estimate` is malformed.
#[wasm_bindgen(js_name = relativeStderr)]
pub fn relative_stderr(estimate: JsValue) -> Result<f64, JsValue> {
    let estimate: MoneyEstimate = from_js_json(&estimate, "estimate")?;
    Ok(estimate.relative_stderr())
}

#[allow(clippy::too_many_arguments)]
fn fd_spec(
    spot: &JsValue,
    strike: &JsValue,
    rate: &JsValue,
    div_yield: &JsValue,
    vol: &JsValue,
    expiry: &JsValue,
    is_call: &JsValue,
    num_paths: Option<&JsValue>,
    seed: Option<&JsValue>,
    num_steps: Option<&JsValue>,
    bump_size: Option<&JsValue>,
    currency: Option<&JsValue>,
) -> Result<GbmEuropeanFdSpec, JsValue> {
    let gbm = gbm_inputs(spot, strike, rate, div_yield, vol, expiry)?;
    Ok(GbmEuropeanFdSpec {
        spot: gbm.spot,
        strike: gbm.strike,
        rate: gbm.rate,
        div_yield: gbm.div_yield,
        vol: gbm.vol,
        expiry: gbm.expiry,
        num_paths: js_opt_uint(num_paths, "numPaths")?,
        seed: js_opt_u64(seed, "seed")?,
        num_steps: js_opt_uint(num_steps, "numSteps")?,
        bump_size: js_opt_f64(bump_size, "bumpSize")?,
        option_type: OptionType::from(js_bool(is_call, "isCall")?),
        currency: js_opt_currency(currency)?,
    })
}

/// Monte Carlo finite-difference delta of a GBM European option, with independent draws per bump.
/// @param spot - Spot level at time 0.
/// @param strike - Exercise price in the same units as `spot`.
/// @param rate - Continuously compounded risk-free rate (decimal, annualized).
/// @param div_yield - Continuous dividend yield (decimal, annualized).
/// @param vol - Annualized GBM volatility (decimal); positive.
/// @param expiry - Time to expiry in years.
/// @param is_call - `true` for a call payoff, `false` for a put.
/// @param num_paths - Optional paths per evaluation; omitted uses the registry default.
/// @param seed - Optional RNG seed as a safe integer or `bigint`; omitted uses the registry default.
/// @param num_steps - Optional time-grid steps; omitted uses the registry default.
/// @param bump_size - Optional relative spot shock (0.01 is 1% of spot); omitted uses the registry default.
/// @param currency - Optional ISO-4217 code of the simulated payoffs; omitted uses the registry default.
/// @returns The `Estimate` object for delta (`mean`, `stderr`, `ci_lower`, `ci_upper`, `num_paths`).
///
/// # Errors
///
/// Throws a `validation` error if an input is non-finite or out of range or
/// the currency code is unknown.
#[allow(clippy::too_many_arguments)]
#[wasm_bindgen(js_name = finiteDiffDelta)]
pub fn finite_diff_delta(
    spot: JsValue,
    strike: JsValue,
    rate: JsValue,
    div_yield: JsValue,
    vol: JsValue,
    expiry: JsValue,
    is_call: JsValue,
    num_paths: Option<JsValue>,
    seed: Option<JsValue>,
    num_steps: Option<JsValue>,
    bump_size: Option<JsValue>,
    currency: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let spec = fd_spec(
        &spot,
        &strike,
        &rate,
        &div_yield,
        &vol,
        &expiry,
        &is_call,
        num_paths.as_ref(),
        seed.as_ref(),
        num_steps.as_ref(),
        bump_size.as_ref(),
        currency.as_ref(),
    )?;
    to_js_value(&finite_diff_delta_gbm(spec).map_err(to_js_err)?)
}

/// Monte Carlo finite-difference delta of a GBM European option under common random numbers.
/// @param spot - Spot level at time 0.
/// @param strike - Exercise price in the same units as `spot`.
/// @param rate - Continuously compounded risk-free rate (decimal, annualized).
/// @param div_yield - Continuous dividend yield (decimal, annualized).
/// @param vol - Annualized GBM volatility (decimal); positive.
/// @param expiry - Time to expiry in years.
/// @param is_call - `true` for a call payoff, `false` for a put.
/// @param num_paths - Optional paths per evaluation; omitted uses the registry default.
/// @param seed - Optional RNG seed as a safe integer or `bigint`; omitted uses the registry default.
/// @param num_steps - Optional time-grid steps; omitted uses the registry default.
/// @param bump_size - Optional relative spot shock (0.01 is 1% of spot); omitted uses the registry default.
/// @param currency - Optional ISO-4217 code of the simulated payoffs; omitted uses the registry default.
/// @returns The `Estimate` object for delta (`mean`, `stderr`, `ci_lower`, `ci_upper`, `num_paths`).
///
/// # Errors
///
/// Throws a `validation` error if an input is non-finite or out of range or
/// the currency code is unknown.
#[allow(clippy::too_many_arguments)]
#[wasm_bindgen(js_name = finiteDiffDeltaCrn)]
pub fn finite_diff_delta_crn(
    spot: JsValue,
    strike: JsValue,
    rate: JsValue,
    div_yield: JsValue,
    vol: JsValue,
    expiry: JsValue,
    is_call: JsValue,
    num_paths: Option<JsValue>,
    seed: Option<JsValue>,
    num_steps: Option<JsValue>,
    bump_size: Option<JsValue>,
    currency: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let spec = fd_spec(
        &spot,
        &strike,
        &rate,
        &div_yield,
        &vol,
        &expiry,
        &is_call,
        num_paths.as_ref(),
        seed.as_ref(),
        num_steps.as_ref(),
        bump_size.as_ref(),
        currency.as_ref(),
    )?;
    to_js_value(&finite_diff_delta_crn_gbm(spec).map_err(to_js_err)?)
}

/// Monte Carlo finite-difference gamma of a GBM European option, with independent draws per bump.
/// @param spot - Spot level at time 0.
/// @param strike - Exercise price in the same units as `spot`.
/// @param rate - Continuously compounded risk-free rate (decimal, annualized).
/// @param div_yield - Continuous dividend yield (decimal, annualized).
/// @param vol - Annualized GBM volatility (decimal); positive.
/// @param expiry - Time to expiry in years.
/// @param is_call - `true` for a call payoff, `false` for a put.
/// @param num_paths - Optional paths per evaluation; omitted uses the registry default.
/// @param seed - Optional RNG seed as a safe integer or `bigint`; omitted uses the registry default.
/// @param num_steps - Optional time-grid steps; omitted uses the registry default.
/// @param bump_size - Optional relative spot shock (0.01 is 1% of spot); omitted uses the registry default.
/// @param currency - Optional ISO-4217 code of the simulated payoffs; omitted uses the registry default.
/// @returns The `Estimate` object for gamma (`mean`, `stderr`, `ci_lower`, `ci_upper`, `num_paths`).
///
/// # Errors
///
/// Throws a `validation` error if an input is non-finite or out of range or
/// the currency code is unknown.
#[allow(clippy::too_many_arguments)]
#[wasm_bindgen(js_name = finiteDiffGamma)]
pub fn finite_diff_gamma(
    spot: JsValue,
    strike: JsValue,
    rate: JsValue,
    div_yield: JsValue,
    vol: JsValue,
    expiry: JsValue,
    is_call: JsValue,
    num_paths: Option<JsValue>,
    seed: Option<JsValue>,
    num_steps: Option<JsValue>,
    bump_size: Option<JsValue>,
    currency: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let spec = fd_spec(
        &spot,
        &strike,
        &rate,
        &div_yield,
        &vol,
        &expiry,
        &is_call,
        num_paths.as_ref(),
        seed.as_ref(),
        num_steps.as_ref(),
        bump_size.as_ref(),
        currency.as_ref(),
    )?;
    to_js_value(&finite_diff_gamma_gbm(spec).map_err(to_js_err)?)
}

/// Monte Carlo finite-difference gamma of a GBM European option under common random numbers.
/// @param spot - Spot level at time 0.
/// @param strike - Exercise price in the same units as `spot`.
/// @param rate - Continuously compounded risk-free rate (decimal, annualized).
/// @param div_yield - Continuous dividend yield (decimal, annualized).
/// @param vol - Annualized GBM volatility (decimal); positive.
/// @param expiry - Time to expiry in years.
/// @param is_call - `true` for a call payoff, `false` for a put.
/// @param num_paths - Optional paths per evaluation; omitted uses the registry default.
/// @param seed - Optional RNG seed as a safe integer or `bigint`; omitted uses the registry default.
/// @param num_steps - Optional time-grid steps; omitted uses the registry default.
/// @param bump_size - Optional relative spot shock (0.01 is 1% of spot); omitted uses the registry default.
/// @param currency - Optional ISO-4217 code of the simulated payoffs; omitted uses the registry default.
/// @returns The `Estimate` object for gamma (`mean`, `stderr`, `ci_lower`, `ci_upper`, `num_paths`).
///
/// # Errors
///
/// Throws a `validation` error if an input is non-finite or out of range or
/// the currency code is unknown.
#[allow(clippy::too_many_arguments)]
#[wasm_bindgen(js_name = finiteDiffGammaCrn)]
pub fn finite_diff_gamma_crn(
    spot: JsValue,
    strike: JsValue,
    rate: JsValue,
    div_yield: JsValue,
    vol: JsValue,
    expiry: JsValue,
    is_call: JsValue,
    num_paths: Option<JsValue>,
    seed: Option<JsValue>,
    num_steps: Option<JsValue>,
    bump_size: Option<JsValue>,
    currency: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let spec = fd_spec(
        &spot,
        &strike,
        &rate,
        &div_yield,
        &vol,
        &expiry,
        &is_call,
        num_paths.as_ref(),
        seed.as_ref(),
        num_steps.as_ref(),
        bump_size.as_ref(),
        currency.as_ref(),
    )?;
    to_js_value(&finite_diff_gamma_crn_gbm(spec).map_err(to_js_err)?)
}

/// Monte Carlo pricer for European options under geometric Brownian motion.
#[wasm_bindgen(js_name = EuropeanPricer)]
pub struct JsEuropeanPricer {
    inner: EuropeanPricer,
}

#[wasm_bindgen(js_class = EuropeanPricer)]
impl JsEuropeanPricer {
    /// Pricer whose omitted settings come from the Rust Monte Carlo registry.
    /// @param num_paths - Optional number of paths; a positive safe integer. Omitted uses the registry default.
    /// @param seed - Optional seed of the path-indexed random streams, as a safe integer or `bigint`. Omitted uses the registry default.
    /// @param use_parallel - Optional; run paths on the thread pool. WebAssembly always runs sequentially with identical results. Omitted uses the registry default.
    ///
    /// # Errors
    ///
    /// Throws a `validation` error if `numPaths` is zero.
    #[wasm_bindgen(constructor)]
    pub fn new(
        num_paths: Option<JsValue>,
        seed: Option<JsValue>,
        use_parallel: Option<JsValue>,
    ) -> Result<JsEuropeanPricer, JsValue> {
        convenience::european_pricer(
            js_opt_uint(num_paths.as_ref(), "numPaths")?,
            js_opt_u64(seed.as_ref(), "seed")?,
            js_opt_bool(use_parallel.as_ref(), "useParallel")?,
        )
        .map(|inner| Self { inner })
        .map_err(to_js_err)
    }

    /// Number of Monte Carlo paths.
    #[wasm_bindgen(getter, js_name = numPaths)]
    pub fn num_paths(&self) -> usize {
        self.inner.num_paths()
    }

    /// Seed of the path-indexed random streams.
    #[wasm_bindgen(getter)]
    pub fn seed(&self) -> u64 {
        self.inner.seed()
    }

    /// Whether paths run on the thread pool on native targets.
    #[wasm_bindgen(getter, js_name = useParallel)]
    pub fn use_parallel(&self) -> bool {
        self.inner.use_parallel()
    }

    /// Price a European call under GBM.
    /// @param spot - Spot level at time 0.
    /// @param strike - Exercise price in the same units as `spot`.
    /// @param rate - Continuously compounded risk-free rate (decimal, annualized).
    /// @param div_yield - Continuous dividend yield (decimal, annualized).
    /// @param vol - Annualized GBM volatility (decimal).
    /// @param expiry - Time to expiry in years.
    /// @param num_steps - Optional time-grid steps; omitted uses the registry default.
    /// @param currency - Optional ISO-4217 code stamped on the estimate; omitted uses the registry default.
    /// @returns The `MoneyEstimate` object (`mean`, `stderr`, `ci_lower`, `ci_upper`, `num_paths`).
    ///
    /// # Errors
    ///
    /// Throws a `validation` error if an input is non-finite or out of range or
    /// the currency code is unknown.
    #[allow(clippy::too_many_arguments)]
    #[wasm_bindgen(js_name = priceCall)]
    pub fn price_call(
        &self,
        spot: JsValue,
        strike: JsValue,
        rate: JsValue,
        div_yield: JsValue,
        vol: JsValue,
        expiry: JsValue,
        num_steps: Option<JsValue>,
        currency: Option<JsValue>,
    ) -> Result<JsValue, JsValue> {
        let gbm = gbm_inputs(&spot, &strike, &rate, &div_yield, &vol, &expiry)?;
        let num_steps =
            convenience::european_num_steps(js_opt_uint(num_steps.as_ref(), "numSteps")?)
                .map_err(to_js_err)?;
        let currency = js_currency(currency.as_ref())?;
        let estimate = self
            .inner
            .price_gbm_call(
                gbm.spot,
                gbm.strike,
                gbm.rate,
                gbm.div_yield,
                gbm.vol,
                gbm.expiry,
                num_steps,
                currency,
            )
            .map_err(to_js_err)?;
        to_js_value(&estimate)
    }

    /// Price a European put under GBM.
    /// @param spot - Spot level at time 0.
    /// @param strike - Exercise price in the same units as `spot`.
    /// @param rate - Continuously compounded risk-free rate (decimal, annualized).
    /// @param div_yield - Continuous dividend yield (decimal, annualized).
    /// @param vol - Annualized GBM volatility (decimal).
    /// @param expiry - Time to expiry in years.
    /// @param num_steps - Optional time-grid steps; omitted uses the registry default.
    /// @param currency - Optional ISO-4217 code stamped on the estimate; omitted uses the registry default.
    /// @returns The `MoneyEstimate` object (`mean`, `stderr`, `ci_lower`, `ci_upper`, `num_paths`).
    ///
    /// # Errors
    ///
    /// Throws a `validation` error if an input is non-finite or out of range or
    /// the currency code is unknown.
    #[allow(clippy::too_many_arguments)]
    #[wasm_bindgen(js_name = pricePut)]
    pub fn price_put(
        &self,
        spot: JsValue,
        strike: JsValue,
        rate: JsValue,
        div_yield: JsValue,
        vol: JsValue,
        expiry: JsValue,
        num_steps: Option<JsValue>,
        currency: Option<JsValue>,
    ) -> Result<JsValue, JsValue> {
        let gbm = gbm_inputs(&spot, &strike, &rate, &div_yield, &vol, &expiry)?;
        let num_steps =
            convenience::european_num_steps(js_opt_uint(num_steps.as_ref(), "numSteps")?)
                .map_err(to_js_err)?;
        let currency = js_currency(currency.as_ref())?;
        let estimate = self
            .inner
            .price_gbm_put(
                gbm.spot,
                gbm.strike,
                gbm.rate,
                gbm.div_yield,
                gbm.vol,
                gbm.expiry,
                num_steps,
                currency,
            )
            .map_err(to_js_err)?;
        to_js_value(&estimate)
    }
}

/// Monte Carlo pricer for arithmetic-average Asian options under GBM.
#[wasm_bindgen(js_name = PathDependentPricer)]
pub struct JsPathDependentPricer {
    inner: PathDependentPricer,
}

#[wasm_bindgen(js_class = PathDependentPricer)]
impl JsPathDependentPricer {
    /// Pricer whose omitted settings come from the Rust Monte Carlo registry.
    /// @param num_paths - Optional number of paths; a positive safe integer. Omitted uses the registry default.
    /// @param seed - Optional seed of the random streams, as a safe integer or `bigint`. Omitted uses the registry default.
    /// @param use_parallel - Optional; run paths on the thread pool. WebAssembly always runs sequentially with identical results. Omitted uses the registry default.
    /// @param antithetic - Optional; use antithetic variates. Omitted keeps the Rust configuration default.
    /// @param use_sobol - Optional; use Sobol quasi-random numbers. Omitted keeps the Rust configuration default.
    /// @param use_brownian_bridge - Optional; build paths by Brownian bridge. Omitted keeps the Rust configuration default.
    ///
    /// # Errors
    ///
    /// Throws a `validation` error if the configuration is invalid, for example
    /// a zero path count or Sobol combined with antithetic variates.
    #[wasm_bindgen(constructor)]
    pub fn new(
        num_paths: Option<JsValue>,
        seed: Option<JsValue>,
        use_parallel: Option<JsValue>,
        antithetic: Option<JsValue>,
        use_sobol: Option<JsValue>,
        use_brownian_bridge: Option<JsValue>,
    ) -> Result<JsPathDependentPricer, JsValue> {
        convenience::path_dependent_pricer(
            js_opt_uint(num_paths.as_ref(), "numPaths")?,
            js_opt_u64(seed.as_ref(), "seed")?,
            js_opt_bool(use_parallel.as_ref(), "useParallel")?,
            js_opt_bool(antithetic.as_ref(), "antithetic")?,
            js_opt_bool(use_sobol.as_ref(), "useSobol")?,
            js_opt_bool(use_brownian_bridge.as_ref(), "useBrownianBridge")?,
        )
        .map(|inner| Self { inner })
        .map_err(to_js_err)
    }

    /// Number of Monte Carlo paths.
    #[wasm_bindgen(getter, js_name = numPaths)]
    pub fn num_paths(&self) -> usize {
        self.inner.config().num_paths
    }

    /// Seed of the random streams.
    #[wasm_bindgen(getter)]
    pub fn seed(&self) -> u64 {
        self.inner.config().seed
    }

    /// Whether paths run on the thread pool on native targets.
    #[wasm_bindgen(getter, js_name = useParallel)]
    pub fn use_parallel(&self) -> bool {
        self.inner.config().use_parallel
    }

    /// Whether antithetic variates are used.
    #[wasm_bindgen(getter)]
    pub fn antithetic(&self) -> bool {
        self.inner.config().antithetic
    }

    /// Whether Sobol quasi-random numbers are used.
    #[wasm_bindgen(getter, js_name = useSobol)]
    pub fn use_sobol(&self) -> bool {
        self.inner.config().use_sobol
    }

    /// Whether paths are built by Brownian bridge.
    #[wasm_bindgen(getter, js_name = useBrownianBridge)]
    pub fn use_brownian_bridge(&self) -> bool {
        self.inner.config().use_brownian_bridge
    }

    /// Price an arithmetic-average Asian call under GBM.
    /// @param spot - Spot level at time 0.
    /// @param strike - Exercise price in the same units as `spot`.
    /// @param rate - Continuously compounded risk-free rate (decimal, annualized).
    /// @param div_yield - Continuous dividend yield (decimal, annualized).
    /// @param vol - Annualized GBM volatility (decimal).
    /// @param expiry - Time to expiry in years.
    /// @param num_steps - Optional number of averaging steps; omitted uses the registry default.
    /// @param currency - Optional ISO-4217 code stamped on the estimate; omitted uses the registry default.
    /// @returns The `MoneyEstimate` object (`mean`, `stderr`, `ci_lower`, `ci_upper`, `num_paths`).
    ///
    /// # Errors
    ///
    /// Throws a `validation` error if an input is non-finite or out of range or
    /// the currency code is unknown.
    #[allow(clippy::too_many_arguments)]
    #[wasm_bindgen(js_name = priceAsianCall)]
    pub fn price_asian_call(
        &self,
        spot: JsValue,
        strike: JsValue,
        rate: JsValue,
        div_yield: JsValue,
        vol: JsValue,
        expiry: JsValue,
        num_steps: Option<JsValue>,
        currency: Option<JsValue>,
    ) -> Result<JsValue, JsValue> {
        let gbm = gbm_inputs(&spot, &strike, &rate, &div_yield, &vol, &expiry)?;
        let num_steps =
            convenience::path_dependent_num_steps(js_opt_uint(num_steps.as_ref(), "numSteps")?)
                .map_err(to_js_err)?;
        let currency = js_currency(currency.as_ref())?;
        let estimate = self
            .inner
            .price_gbm_asian_call(
                gbm.spot,
                gbm.strike,
                gbm.rate,
                gbm.div_yield,
                gbm.vol,
                gbm.expiry,
                num_steps,
                currency,
            )
            .map_err(to_js_err)?;
        to_js_value(&estimate)
    }

    /// Price an arithmetic-average Asian put under GBM.
    /// @param spot - Spot level at time 0.
    /// @param strike - Exercise price in the same units as `spot`.
    /// @param rate - Continuously compounded risk-free rate (decimal, annualized).
    /// @param div_yield - Continuous dividend yield (decimal, annualized).
    /// @param vol - Annualized GBM volatility (decimal).
    /// @param expiry - Time to expiry in years.
    /// @param num_steps - Optional number of averaging steps; omitted uses the registry default.
    /// @param currency - Optional ISO-4217 code stamped on the estimate; omitted uses the registry default.
    /// @returns The `MoneyEstimate` object (`mean`, `stderr`, `ci_lower`, `ci_upper`, `num_paths`).
    ///
    /// # Errors
    ///
    /// Throws a `validation` error if an input is non-finite or out of range or
    /// the currency code is unknown.
    #[allow(clippy::too_many_arguments)]
    #[wasm_bindgen(js_name = priceAsianPut)]
    pub fn price_asian_put(
        &self,
        spot: JsValue,
        strike: JsValue,
        rate: JsValue,
        div_yield: JsValue,
        vol: JsValue,
        expiry: JsValue,
        num_steps: Option<JsValue>,
        currency: Option<JsValue>,
    ) -> Result<JsValue, JsValue> {
        let gbm = gbm_inputs(&spot, &strike, &rate, &div_yield, &vol, &expiry)?;
        let num_steps =
            convenience::path_dependent_num_steps(js_opt_uint(num_steps.as_ref(), "numSteps")?)
                .map_err(to_js_err)?;
        let currency = js_currency(currency.as_ref())?;
        let estimate = self
            .inner
            .price_gbm_asian_put(
                gbm.spot,
                gbm.strike,
                gbm.rate,
                gbm.div_yield,
                gbm.vol,
                gbm.expiry,
                num_steps,
                currency,
            )
            .map_err(to_js_err)?;
        to_js_value(&estimate)
    }
}

/// Longstaff-Schwartz Monte Carlo pricer for American options under GBM.
#[wasm_bindgen(js_name = LsmcPricer)]
pub struct JsLsmcPricer {
    inner: LsmcConvenience,
}

/// One LSMC pricing call after its overrides are resolved by Rust.
struct LsmcCall {
    pricer: finstack_quant_models::monte_carlo::pricer::lsmc::LsmcPricer,
    gbm: GbmInputs,
    num_steps: usize,
    currency: Currency,
    basis: finstack_quant_models::monte_carlo::pricer::basis::BasisKind,
    basis_degree: usize,
}

impl JsLsmcPricer {
    #[allow(clippy::too_many_arguments)]
    fn call(
        &self,
        spot: &JsValue,
        strike: &JsValue,
        rate: &JsValue,
        div_yield: &JsValue,
        vol: &JsValue,
        expiry: &JsValue,
        currency: Option<&JsValue>,
        num_steps: Option<&JsValue>,
        basis: Option<&JsValue>,
        basis_degree: Option<&JsValue>,
    ) -> Result<LsmcCall, JsValue> {
        let gbm = gbm_inputs(spot, strike, rate, div_yield, vol, expiry)?;
        let currency = js_currency(currency)?;
        let basis = js_opt_string(basis, "basis")?;
        let (pricer, num_steps, basis, basis_degree) = self
            .inner
            .call_config(
                js_opt_uint(num_steps, "numSteps")?,
                basis.as_deref(),
                js_opt_uint(basis_degree, "basisDegree")?,
            )
            .map_err(to_js_err)?;
        Ok(LsmcCall {
            pricer,
            gbm,
            num_steps,
            currency,
            basis,
            basis_degree,
        })
    }
}

#[wasm_bindgen(js_class = LsmcPricer)]
impl JsLsmcPricer {
    /// Pricer whose omitted settings come from the Rust Monte Carlo registry.
    /// @param num_paths - Optional number of paths; a positive safe integer. Omitted uses the registry default.
    /// @param seed - Optional seed of the random streams, as a safe integer or `bigint`. Omitted uses the registry default.
    /// @param use_parallel - Optional; run paths on the thread pool. WebAssembly always runs sequentially with identical results. Omitted uses the registry default.
    /// @param num_steps - Optional number of exercise dates between 0 and expiry. Omitted uses the registry default.
    /// @param basis - Optional regression basis name, `"laguerre"` or `"polynomial"`. Omitted uses the registry default.
    /// @param basis_degree - Optional highest polynomial degree of the regression basis. Omitted uses the registry default.
    /// @param antithetic - Optional; use antithetic variates. Omitted uses the registry default.
    ///
    /// # Errors
    ///
    /// Throws a `validation` error for an unknown basis name or a zero path or
    /// step count.
    #[wasm_bindgen(constructor)]
    pub fn new(
        num_paths: Option<JsValue>,
        seed: Option<JsValue>,
        use_parallel: Option<JsValue>,
        num_steps: Option<JsValue>,
        basis: Option<JsValue>,
        basis_degree: Option<JsValue>,
        antithetic: Option<JsValue>,
    ) -> Result<JsLsmcPricer, JsValue> {
        let basis = js_opt_string(basis.as_ref(), "basis")?;
        LsmcConvenience::new(
            js_opt_uint(num_paths.as_ref(), "numPaths")?,
            js_opt_u64(seed.as_ref(), "seed")?,
            js_opt_bool(use_parallel.as_ref(), "useParallel")?,
            js_opt_uint(num_steps.as_ref(), "numSteps")?,
            basis.as_deref(),
            js_opt_uint(basis_degree.as_ref(), "basisDegree")?,
            js_opt_bool(antithetic.as_ref(), "antithetic")?,
        )
        .map(|inner| Self { inner })
        .map_err(to_js_err)
    }

    /// Number of Monte Carlo paths.
    #[wasm_bindgen(getter, js_name = numPaths)]
    pub fn num_paths(&self) -> usize {
        self.inner.pricer().config().num_paths
    }

    /// Seed of the random streams.
    #[wasm_bindgen(getter)]
    pub fn seed(&self) -> u64 {
        self.inner.pricer().config().seed
    }

    /// Whether paths run on the thread pool on native targets.
    #[wasm_bindgen(getter, js_name = useParallel)]
    pub fn use_parallel(&self) -> bool {
        self.inner.pricer().config().use_parallel
    }

    /// Whether antithetic variates are used.
    #[wasm_bindgen(getter)]
    pub fn antithetic(&self) -> bool {
        self.inner.pricer().config().antithetic
    }

    /// Default regression basis name.
    #[wasm_bindgen(getter)]
    pub fn basis(&self) -> String {
        self.inner.basis().as_str().to_owned()
    }

    /// Default highest polynomial degree of the regression basis.
    #[wasm_bindgen(getter, js_name = basisDegree)]
    pub fn basis_degree(&self) -> usize {
        self.inner.basis_degree()
    }

    /// Price an American put by Longstaff-Schwartz regression (in-sample exercise policy).
    /// @param spot - Spot level at time 0.
    /// @param strike - Exercise price in the same units as `spot`.
    /// @param rate - Continuously compounded risk-free rate (decimal, annualized).
    /// @param div_yield - Continuous dividend yield (decimal, annualized).
    /// @param vol - Annualized GBM volatility (decimal).
    /// @param expiry - Time to expiry in years.
    /// @param currency - Optional ISO-4217 code stamped on the estimate; omitted uses the registry default.
    /// @param num_steps - Optional exercise dates for this call; omitted keeps the pricer's value.
    /// @param basis - Optional regression basis for this call; omitted keeps the pricer's basis.
    /// @param basis_degree - Optional basis degree for this call; omitted keeps the pricer's degree.
    /// @returns The `MoneyEstimate` object (`mean`, `stderr`, `ci_lower`, `ci_upper`, `num_paths`).
    ///
    /// # Errors
    ///
    /// Throws a `validation` error if an input is out of range, the basis name
    /// or currency code is unknown, or the regression is ill-conditioned.
    #[allow(clippy::too_many_arguments)]
    #[wasm_bindgen(js_name = priceAmericanPut)]
    pub fn price_american_put(
        &self,
        spot: JsValue,
        strike: JsValue,
        rate: JsValue,
        div_yield: JsValue,
        vol: JsValue,
        expiry: JsValue,
        currency: Option<JsValue>,
        num_steps: Option<JsValue>,
        basis: Option<JsValue>,
        basis_degree: Option<JsValue>,
    ) -> Result<JsValue, JsValue> {
        let call = self.call(
            &spot,
            &strike,
            &rate,
            &div_yield,
            &vol,
            &expiry,
            currency.as_ref(),
            num_steps.as_ref(),
            basis.as_ref(),
            basis_degree.as_ref(),
        )?;
        let estimate = call
            .pricer
            .price_gbm_american_put(
                call.gbm.spot,
                call.gbm.strike,
                call.gbm.rate,
                call.gbm.div_yield,
                call.gbm.vol,
                call.gbm.expiry,
                call.num_steps,
                call.currency,
                call.basis,
                call.basis_degree,
            )
            .map_err(to_js_err)?;
        to_js_value(&estimate)
    }

    /// Price an American call by Longstaff-Schwartz regression (in-sample exercise policy).
    /// @param spot - Spot level at time 0.
    /// @param strike - Exercise price in the same units as `spot`.
    /// @param rate - Continuously compounded risk-free rate (decimal, annualized).
    /// @param div_yield - Continuous dividend yield (decimal, annualized).
    /// @param vol - Annualized GBM volatility (decimal).
    /// @param expiry - Time to expiry in years.
    /// @param currency - Optional ISO-4217 code stamped on the estimate; omitted uses the registry default.
    /// @param num_steps - Optional exercise dates for this call; omitted keeps the pricer's value.
    /// @param basis - Optional regression basis for this call; omitted keeps the pricer's basis.
    /// @param basis_degree - Optional basis degree for this call; omitted keeps the pricer's degree.
    /// @returns The `MoneyEstimate` object (`mean`, `stderr`, `ci_lower`, `ci_upper`, `num_paths`).
    ///
    /// # Errors
    ///
    /// Throws a `validation` error if an input is out of range, the basis name
    /// or currency code is unknown, or the regression is ill-conditioned.
    #[allow(clippy::too_many_arguments)]
    #[wasm_bindgen(js_name = priceAmericanCall)]
    pub fn price_american_call(
        &self,
        spot: JsValue,
        strike: JsValue,
        rate: JsValue,
        div_yield: JsValue,
        vol: JsValue,
        expiry: JsValue,
        currency: Option<JsValue>,
        num_steps: Option<JsValue>,
        basis: Option<JsValue>,
        basis_degree: Option<JsValue>,
    ) -> Result<JsValue, JsValue> {
        let call = self.call(
            &spot,
            &strike,
            &rate,
            &div_yield,
            &vol,
            &expiry,
            currency.as_ref(),
            num_steps.as_ref(),
            basis.as_ref(),
            basis_degree.as_ref(),
        )?;
        let estimate = call
            .pricer
            .price_gbm_american_call(
                call.gbm.spot,
                call.gbm.strike,
                call.gbm.rate,
                call.gbm.div_yield,
                call.gbm.vol,
                call.gbm.expiry,
                call.num_steps,
                call.currency,
                call.basis,
                call.basis_degree,
            )
            .map_err(to_js_err)?;
        to_js_value(&estimate)
    }

    /// Price an American put with an out-of-sample exercise policy: the
    /// regression is fitted on the pricer's paths and applied to fresh paths
    /// drawn from `pricingSeed`, removing the in-sample high bias.
    /// @param spot - Spot level at time 0.
    /// @param strike - Exercise price in the same units as `spot`.
    /// @param rate - Continuously compounded risk-free rate (decimal, annualized).
    /// @param div_yield - Continuous dividend yield (decimal, annualized).
    /// @param vol - Annualized GBM volatility (decimal).
    /// @param expiry - Time to expiry in years.
    /// @param pricing_seed - Seed of the independent pricing paths, as a safe integer or `bigint`; must differ from the pricer's seed.
    /// @param currency - Optional ISO-4217 code stamped on the estimate; omitted uses the registry default.
    /// @param num_steps - Optional exercise dates for this call; omitted keeps the pricer's value.
    /// @param basis - Optional regression basis for this call; omitted keeps the pricer's basis.
    /// @param basis_degree - Optional basis degree for this call; omitted keeps the pricer's degree.
    /// @returns The `MoneyEstimate` object (`mean`, `stderr`, `ci_lower`, `ci_upper`, `num_paths`).
    ///
    /// # Errors
    ///
    /// Throws a `validation` error if an input is out of range, the basis name
    /// or currency code is unknown, or `pricingSeed` equals the training seed.
    #[allow(clippy::too_many_arguments)]
    #[wasm_bindgen(js_name = priceAmericanPutUnbiased)]
    pub fn price_american_put_unbiased(
        &self,
        spot: JsValue,
        strike: JsValue,
        rate: JsValue,
        div_yield: JsValue,
        vol: JsValue,
        expiry: JsValue,
        pricing_seed: JsValue,
        currency: Option<JsValue>,
        num_steps: Option<JsValue>,
        basis: Option<JsValue>,
        basis_degree: Option<JsValue>,
    ) -> Result<JsValue, JsValue> {
        let pricing_seed = js_u64(&pricing_seed, "pricingSeed")?;
        let call = self.call(
            &spot,
            &strike,
            &rate,
            &div_yield,
            &vol,
            &expiry,
            currency.as_ref(),
            num_steps.as_ref(),
            basis.as_ref(),
            basis_degree.as_ref(),
        )?;
        let estimate = call
            .pricer
            .price_gbm_american_put_unbiased(
                call.gbm.spot,
                call.gbm.strike,
                call.gbm.rate,
                call.gbm.div_yield,
                call.gbm.vol,
                call.gbm.expiry,
                call.num_steps,
                call.currency,
                call.basis,
                call.basis_degree,
                pricing_seed,
            )
            .map_err(to_js_err)?;
        to_js_value(&estimate)
    }

    /// Price an American call with an out-of-sample exercise policy: the
    /// regression is fitted on the pricer's paths and applied to fresh paths
    /// drawn from `pricingSeed`, removing the in-sample high bias.
    /// @param spot - Spot level at time 0.
    /// @param strike - Exercise price in the same units as `spot`.
    /// @param rate - Continuously compounded risk-free rate (decimal, annualized).
    /// @param div_yield - Continuous dividend yield (decimal, annualized).
    /// @param vol - Annualized GBM volatility (decimal).
    /// @param expiry - Time to expiry in years.
    /// @param pricing_seed - Seed of the independent pricing paths, as a safe integer or `bigint`; must differ from the pricer's seed.
    /// @param currency - Optional ISO-4217 code stamped on the estimate; omitted uses the registry default.
    /// @param num_steps - Optional exercise dates for this call; omitted keeps the pricer's value.
    /// @param basis - Optional regression basis for this call; omitted keeps the pricer's basis.
    /// @param basis_degree - Optional basis degree for this call; omitted keeps the pricer's degree.
    /// @returns The `MoneyEstimate` object (`mean`, `stderr`, `ci_lower`, `ci_upper`, `num_paths`).
    ///
    /// # Errors
    ///
    /// Throws a `validation` error if an input is out of range, the basis name
    /// or currency code is unknown, or `pricingSeed` equals the training seed.
    #[allow(clippy::too_many_arguments)]
    #[wasm_bindgen(js_name = priceAmericanCallUnbiased)]
    pub fn price_american_call_unbiased(
        &self,
        spot: JsValue,
        strike: JsValue,
        rate: JsValue,
        div_yield: JsValue,
        vol: JsValue,
        expiry: JsValue,
        pricing_seed: JsValue,
        currency: Option<JsValue>,
        num_steps: Option<JsValue>,
        basis: Option<JsValue>,
        basis_degree: Option<JsValue>,
    ) -> Result<JsValue, JsValue> {
        let pricing_seed = js_u64(&pricing_seed, "pricingSeed")?;
        let call = self.call(
            &spot,
            &strike,
            &rate,
            &div_yield,
            &vol,
            &expiry,
            currency.as_ref(),
            num_steps.as_ref(),
            basis.as_ref(),
            basis_degree.as_ref(),
        )?;
        let estimate = call
            .pricer
            .price_gbm_american_call_unbiased(
                call.gbm.spot,
                call.gbm.strike,
                call.gbm.rate,
                call.gbm.div_yield,
                call.gbm.vol,
                call.gbm.expiry,
                call.num_steps,
                call.currency,
                call.basis,
                call.basis_degree,
                pricing_seed,
            )
            .map_err(to_js_err)?;
        to_js_value(&estimate)
    }
}
