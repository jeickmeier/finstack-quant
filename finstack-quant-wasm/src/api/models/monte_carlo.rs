//! WASM bindings for the Monte Carlo engine in `finstack-quant-models`.
//!
//! Provides the host-neutral subset shared with Python: Heston Monte Carlo
//! pricing. Closed-form Black-Scholes references live in `models.bsPrice`.
//! Advanced Rust processes, discretizations, RNGs, payoffs, and Greeks remain
//! Rust-only.
//!

use std::str::FromStr;

use crate::utils::to_js_err;
use finstack_quant_core::currency::Currency;
use finstack_quant_models::monte_carlo::results::MoneyEstimate;
use wasm_bindgen::prelude::*;

/// Serializable result shape returned to JavaScript.
///
/// Field layout mirrors the accessors on the Python `MoneyEstimate`
/// binding so both hosts see the same vocabulary.
#[derive(serde::Serialize)]
struct McResultJs {
    /// Discounted mean present value.
    mean: f64,
    /// Currency code of the estimate.
    currency: String,
    /// Standard error of the mean.
    stderr: f64,
    /// Sample standard deviation (if available).
    std_dev: Option<f64>,
    /// Lower 95% confidence bound.
    ci_lower: f64,
    /// Upper 95% confidence bound.
    ci_upper: f64,
    /// Number of independent path estimators contributing to the result.
    ///
    /// Equals the configured `num_paths` without variance reduction. With
    /// antithetic variates enabled each estimator averages a `(z, -z)` pair,
    /// so `num_simulated_paths == 2 * num_paths`.
    num_paths: usize,
    /// Total number of simulated sample paths driving the estimator.
    num_simulated_paths: usize,
    /// Median of captured discounted path values (if captured).
    median: Option<f64>,
    /// 25th percentile of captured discounted path values (if captured).
    percentile_25: Option<f64>,
    /// 75th percentile of captured discounted path values (if captured).
    percentile_75: Option<f64>,
    /// Minimum of captured discounted path values (if captured).
    min: Option<f64>,
    /// Maximum of captured discounted path values (if captured).
    max: Option<f64>,
    /// Relative standard error (`stderr / |mean|`); `f64::INFINITY` near zero.
    relative_stderr: f64,
}

impl McResultJs {
    /// Convert a [`MoneyEstimate`] into the JS-friendly shape.
    fn from_estimate(est: &MoneyEstimate) -> Self {
        Self {
            mean: est.mean.amount(),
            currency: est.mean.currency().to_string(),
            stderr: est.stderr,
            std_dev: est.std_dev,
            ci_lower: est.ci_95.0.amount(),
            ci_upper: est.ci_95.1.amount(),
            num_paths: est.num_paths,
            num_simulated_paths: est.num_simulated_paths,
            median: est.median,
            percentile_25: est.percentile_25,
            percentile_75: est.percentile_75,
            min: est.min,
            max: est.max,
            relative_stderr: est.relative_stderr(),
        }
    }

    fn to_js_value(&self) -> Result<JsValue, JsValue> {
        crate::utils::to_js_value(self)
    }
}

fn estimate_to_js(est: &MoneyEstimate) -> Result<JsValue, JsValue> {
    McResultJs::from_estimate(est).to_js_value()
}

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
/// @param num_paths - Number of simulated stochastic paths; larger values improve sampling precision.
/// @param seed - Deterministic random-number seed used to reproduce simulation output.
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
    num_paths: usize,
    seed: u64,
    num_steps: Option<usize>,
    currency: Option<String>,
) -> Result<JsValue, JsValue> {
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
/// @param num_paths - Number of simulated stochastic paths; larger values improve sampling precision.
/// @param seed - Deterministic random-number seed used to reproduce simulation output.
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
    num_paths: usize,
    seed: u64,
    num_steps: Option<usize>,
    currency: Option<String>,
) -> Result<JsValue, JsValue> {
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
    num_paths: usize,
    seed: u64,
    num_steps: Option<usize>,
    currency: Option<String>,
) -> Result<JsValue, JsValue> {
    use finstack_quant_models::monte_carlo::pricer::heston as canonical;

    // The canonical entry point owns the registry defaults for step count,
    // currency, and (wasm-gated) parallelism; the binding only marshals an
    // explicitly supplied currency.
    let ccy = currency
        .as_deref()
        .map(|code| Currency::from_str(code).map_err(to_js_err))
        .transpose()?;
    let est = if is_call {
        canonical::price_heston_call(
            spot,
            strike,
            rate,
            div_yield,
            kappa,
            theta,
            vol_of_vol,
            rho,
            v0,
            expiry,
            Some(num_paths),
            Some(seed),
            num_steps,
            ccy,
        )
    } else {
        canonical::price_heston_put(
            spot,
            strike,
            rate,
            div_yield,
            kappa,
            theta,
            vol_of_vol,
            rho,
            v0,
            expiry,
            Some(num_paths),
            Some(seed),
            num_steps,
            ccy,
        )
    }
    .map_err(to_js_err)?;
    estimate_to_js(&est)
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::money::Money;

    #[test]
    fn mc_result_js_from_estimate_maps_fields() {
        let est = MoneyEstimate {
            mean: Money::new(10.0, Currency::USD).expect("valid money fixture"),
            stderr: 0.25,
            ci_95: (
                Money::new(9.0, Currency::USD).expect("valid money fixture"),
                Money::new(11.0, Currency::USD).expect("valid money fixture"),
            ),
            num_paths: 1000,
            num_simulated_paths: 2000,
            std_dev: Some(5.0),
            median: None,
            percentile_25: None,
            percentile_75: None,
            min: None,
            max: None,
        };
        let js = McResultJs::from_estimate(&est);
        assert!((js.mean - 10.0).abs() < 1e-12);
        assert_eq!(js.currency, "USD");
        assert!((js.stderr - 0.25).abs() < 1e-12);
        assert_eq!(js.std_dev, Some(5.0));
        assert!((js.ci_lower - 9.0).abs() < 1e-12);
        assert!((js.ci_upper - 11.0).abs() < 1e-12);
        assert_eq!(js.num_paths, 1000);
        assert_eq!(js.num_simulated_paths, 2000);
    }
}
