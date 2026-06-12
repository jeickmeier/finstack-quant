//! WASM bindings for the `finstack-monte-carlo` crate.
//!
//! Provides the convenience subset of the Rust Monte Carlo crate: European,
//! Asian, and American option pricing, Black-Scholes helpers, and selected
//! non-GBM process wrappers. Advanced Rust process, discretization, RNG,
//! payoff, and Greeks types are not standalone WASM types yet. Results are
//! returned as plain JavaScript objects.

use std::str::FromStr;

use crate::utils::to_js_err;
use finstack_core::currency::Currency;
use finstack_monte_carlo::pricer::basis::LsmcBasis;
use finstack_monte_carlo::pricer::european::EuropeanPricer;
use finstack_monte_carlo::pricer::lsmc::LsmcPricer;
use finstack_monte_carlo::process::gbm::GbmProcess;
use finstack_monte_carlo::results::MoneyEstimate;
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
    /// Legacy skipped-path count; current engines reject non-finite payoffs.
    num_skipped: usize,
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
            num_skipped: est.num_skipped,
            median: est.median,
            percentile_25: est.percentile_25,
            percentile_75: est.percentile_75,
            min: est.min,
            max: est.max,
            relative_stderr: est.relative_stderr(),
        }
    }

    fn to_js_value(&self) -> Result<JsValue, JsValue> {
        serde_wasm_bindgen::to_value(self).map_err(to_js_err)
    }
}

fn estimate_to_js(est: &MoneyEstimate) -> Result<JsValue, JsValue> {
    McResultJs::from_estimate(est).to_js_value()
}

/// Resolve the embedded binding defaults from the registry.
fn binding_defaults(
) -> Result<&'static finstack_monte_carlo::registry::PythonBindingDefaults, JsValue> {
    finstack_monte_carlo::registry::embedded_defaults()
        .map(|defaults| &defaults.python_bindings)
        .map_err(to_js_err)
}

/// Price a European call option via Monte Carlo under GBM dynamics.
///
/// Returns a JSON object with `mean`, `currency`, `stderr`, `std_dev`,
/// `ci_lower`, `ci_upper`, `num_paths`, `num_simulated_paths`, `num_skipped`,
/// `median`, `percentile_25`, `percentile_75`, `min`, `max`, and
/// `relative_stderr`.
#[allow(clippy::too_many_arguments)]
#[wasm_bindgen(js_name = priceEuropeanCall)]
pub fn price_european_call(
    spot: f64,
    strike: f64,
    rate: f64,
    div_yield: f64,
    vol: f64,
    expiry: f64,
    num_paths: usize,
    seed: u64,
    num_steps: Option<usize>,
    currency: Option<String>,
) -> Result<JsValue, JsValue> {
    price_european(
        true, spot, strike, rate, div_yield, vol, expiry, num_paths, seed, num_steps, currency,
    )
}

/// Price a European put option via Monte Carlo under GBM dynamics.
///
/// Returns a JSON object with `mean`, `currency`, `stderr`, `std_dev`,
/// `ci_lower`, `ci_upper`, `num_paths`, `num_simulated_paths`, `num_skipped`,
/// `median`, `percentile_25`, `percentile_75`, `min`, `max`, and
/// `relative_stderr`.
#[allow(clippy::too_many_arguments)]
#[wasm_bindgen(js_name = priceEuropeanPut)]
pub fn price_european_put(
    spot: f64,
    strike: f64,
    rate: f64,
    div_yield: f64,
    vol: f64,
    expiry: f64,
    num_paths: usize,
    seed: u64,
    num_steps: Option<usize>,
    currency: Option<String>,
) -> Result<JsValue, JsValue> {
    price_european(
        false, spot, strike, rate, div_yield, vol, expiry, num_paths, seed, num_steps, currency,
    )
}

#[allow(clippy::too_many_arguments)]
fn price_european(
    is_call: bool,
    spot: f64,
    strike: f64,
    rate: f64,
    div_yield: f64,
    vol: f64,
    expiry: f64,
    num_paths: usize,
    seed: u64,
    num_steps: Option<usize>,
    currency: Option<String>,
) -> Result<JsValue, JsValue> {
    let ccy = resolve_currency(currency.as_deref())?;
    let steps = match num_steps {
        Some(steps) => steps,
        None => binding_defaults()?.european_pricer.num_steps,
    };
    let pricer = build_pricer(num_paths, seed);
    let est = if is_call {
        pricer.price_gbm_call(spot, strike, rate, div_yield, vol, expiry, steps, ccy)
    } else {
        pricer.price_gbm_put(spot, strike, rate, div_yield, vol, expiry, steps, ccy)
    }
    .map_err(to_js_err)?;
    estimate_to_js(&est)
}

/// Price a European call under Heston stochastic volatility.
#[allow(clippy::too_many_arguments)]
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

/// Price a European put under Heston stochastic volatility.
#[allow(clippy::too_many_arguments)]
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

// ---------------------------------------------------------------------------
// Analytical
// ---------------------------------------------------------------------------

/// Black-Scholes call price.
#[wasm_bindgen(js_name = blackScholesCall)]
pub fn black_scholes_call(
    spot: f64,
    strike: f64,
    rate: f64,
    div_yield: f64,
    vol: f64,
    expiry: f64,
) -> f64 {
    finstack_monte_carlo::variance_reduction::control_variate::black_scholes_call(
        spot, strike, expiry, rate, div_yield, vol,
    )
}

/// Black-Scholes put price.
#[wasm_bindgen(js_name = blackScholesPut)]
pub fn black_scholes_put(
    spot: f64,
    strike: f64,
    rate: f64,
    div_yield: f64,
    vol: f64,
    expiry: f64,
) -> f64 {
    finstack_monte_carlo::variance_reduction::control_variate::black_scholes_put(
        spot, strike, expiry, rate, div_yield, vol,
    )
}

// ---------------------------------------------------------------------------
// Path-dependent pricing
// ---------------------------------------------------------------------------

/// Price an Asian call via Monte Carlo under GBM dynamics.
#[allow(clippy::too_many_arguments)]
#[wasm_bindgen(js_name = priceAsianCall)]
pub fn price_asian_call(
    spot: f64,
    strike: f64,
    rate: f64,
    div_yield: f64,
    vol: f64,
    expiry: f64,
    num_paths: usize,
    seed: u64,
    num_steps: Option<usize>,
    currency: Option<String>,
) -> Result<JsValue, JsValue> {
    price_asian(
        true, spot, strike, rate, div_yield, vol, expiry, num_paths, seed, num_steps, currency,
    )
}

/// Price an Asian put via Monte Carlo under GBM dynamics.
#[allow(clippy::too_many_arguments)]
#[wasm_bindgen(js_name = priceAsianPut)]
pub fn price_asian_put(
    spot: f64,
    strike: f64,
    rate: f64,
    div_yield: f64,
    vol: f64,
    expiry: f64,
    num_paths: usize,
    seed: u64,
    num_steps: Option<usize>,
    currency: Option<String>,
) -> Result<JsValue, JsValue> {
    price_asian(
        false, spot, strike, rate, div_yield, vol, expiry, num_paths, seed, num_steps, currency,
    )
}

#[allow(clippy::too_many_arguments)]
fn price_asian(
    is_call: bool,
    spot: f64,
    strike: f64,
    rate: f64,
    div_yield: f64,
    vol: f64,
    expiry: f64,
    num_paths: usize,
    seed: u64,
    num_steps: Option<usize>,
    currency: Option<String>,
) -> Result<JsValue, JsValue> {
    use finstack_monte_carlo::payoff::asian::{
        default_fixing_steps, AsianCall, AsianPut, AveragingMethod,
    };
    use finstack_monte_carlo::pricer::path_dependent::{
        PathDependentPricer, PathDependentPricerConfig,
    };

    let ccy = resolve_currency(currency.as_deref())?;
    let steps = match num_steps {
        Some(steps) => steps,
        None => binding_defaults()?.path_dependent_pricer.num_steps,
    };
    let fixing_steps = default_fixing_steps(steps);
    let df = (-rate * expiry).exp();
    let config = PathDependentPricerConfig::new(num_paths)
        .with_seed(seed)
        .with_parallel(false);
    let pricer = PathDependentPricer::new(config);
    let process = GbmProcess::with_params(rate, div_yield, vol).map_err(to_js_err)?;
    let est = if is_call {
        let payoff = AsianCall::new(strike, 1.0, AveragingMethod::Arithmetic, fixing_steps);
        pricer.price(&process, spot, expiry, steps, &payoff, ccy, df)
    } else {
        let payoff = AsianPut::new(strike, 1.0, AveragingMethod::Arithmetic, fixing_steps);
        pricer.price(&process, spot, expiry, steps, &payoff, ccy, df)
    }
    .map_err(to_js_err)?;
    estimate_to_js(&est)
}

/// Price an American put via LSMC under GBM dynamics.
///
/// Optional knobs:
/// - `use_parallel` (default `false`): run path generation on the rayon pool.
/// - `basis` (default `"laguerre"`): regression basis — `"laguerre"`,
///   `"polynomial"`, or `"normalized_polynomial"`.
/// - `basis_degree` (default `3`): polynomial/Laguerre degree. Must be
///   positive; `"laguerre"` additionally requires degree in `[1, 4]`.
#[allow(clippy::too_many_arguments)]
#[wasm_bindgen(js_name = priceAmericanPut)]
pub fn price_american_put(
    spot: f64,
    strike: f64,
    rate: f64,
    div_yield: f64,
    vol: f64,
    expiry: f64,
    num_paths: usize,
    seed: u64,
    num_steps: Option<usize>,
    currency: Option<String>,
    use_parallel: Option<bool>,
    basis: Option<String>,
    basis_degree: Option<usize>,
) -> Result<JsValue, JsValue> {
    use finstack_monte_carlo::pricer::lsmc::AmericanPut;

    let exercise = AmericanPut::new(strike).map_err(to_js_err)?;
    price_lsmc_gbm(
        spot,
        strike,
        rate,
        div_yield,
        vol,
        expiry,
        num_paths,
        seed,
        num_steps,
        currency,
        use_parallel,
        basis,
        basis_degree,
        &exercise,
    )
}

/// Price an American call via LSMC under GBM dynamics.
///
/// Optional knobs match [`price_american_put`].
#[allow(clippy::too_many_arguments)]
#[wasm_bindgen(js_name = priceAmericanCall)]
pub fn price_american_call(
    spot: f64,
    strike: f64,
    rate: f64,
    div_yield: f64,
    vol: f64,
    expiry: f64,
    num_paths: usize,
    seed: u64,
    num_steps: Option<usize>,
    currency: Option<String>,
    use_parallel: Option<bool>,
    basis: Option<String>,
    basis_degree: Option<usize>,
) -> Result<JsValue, JsValue> {
    use finstack_monte_carlo::pricer::lsmc::AmericanCall;

    let exercise = AmericanCall::new(strike).map_err(to_js_err)?;
    price_lsmc_gbm(
        spot,
        strike,
        rate,
        div_yield,
        vol,
        expiry,
        num_paths,
        seed,
        num_steps,
        currency,
        use_parallel,
        basis,
        basis_degree,
        &exercise,
    )
}

/// Two-pass unbiased American put price (training fit + out-of-sample pricing).
#[allow(clippy::too_many_arguments)]
#[wasm_bindgen(js_name = priceAmericanPutUnbiased)]
pub fn price_american_put_unbiased(
    spot: f64,
    strike: f64,
    rate: f64,
    div_yield: f64,
    vol: f64,
    expiry: f64,
    num_paths: usize,
    seed: u64,
    pricing_seed: u64,
    num_steps: Option<usize>,
    currency: Option<String>,
    use_parallel: Option<bool>,
    basis: Option<String>,
    basis_degree: Option<usize>,
) -> Result<JsValue, JsValue> {
    use finstack_monte_carlo::pricer::lsmc::AmericanPut;

    let exercise = AmericanPut::new(strike).map_err(to_js_err)?;
    price_lsmc_gbm_unbiased(
        spot,
        strike,
        rate,
        div_yield,
        vol,
        expiry,
        num_paths,
        seed,
        pricing_seed,
        num_steps,
        currency,
        use_parallel,
        basis,
        basis_degree,
        &exercise,
    )
}

/// Two-pass unbiased American call price (training fit + out-of-sample pricing).
#[allow(clippy::too_many_arguments)]
#[wasm_bindgen(js_name = priceAmericanCallUnbiased)]
pub fn price_american_call_unbiased(
    spot: f64,
    strike: f64,
    rate: f64,
    div_yield: f64,
    vol: f64,
    expiry: f64,
    num_paths: usize,
    seed: u64,
    pricing_seed: u64,
    num_steps: Option<usize>,
    currency: Option<String>,
    use_parallel: Option<bool>,
    basis: Option<String>,
    basis_degree: Option<usize>,
) -> Result<JsValue, JsValue> {
    use finstack_monte_carlo::pricer::lsmc::AmericanCall;

    let exercise = AmericanCall::new(strike).map_err(to_js_err)?;
    price_lsmc_gbm_unbiased(
        spot,
        strike,
        rate,
        div_yield,
        vol,
        expiry,
        num_paths,
        seed,
        pricing_seed,
        num_steps,
        currency,
        use_parallel,
        basis,
        basis_degree,
        &exercise,
    )
}

#[allow(clippy::too_many_arguments)]
fn price_lsmc_gbm<E>(
    spot: f64,
    strike: f64,
    rate: f64,
    div_yield: f64,
    vol: f64,
    expiry: f64,
    num_paths: usize,
    seed: u64,
    num_steps: Option<usize>,
    currency: Option<String>,
    use_parallel: Option<bool>,
    basis: Option<String>,
    basis_degree: Option<usize>,
    exercise: &E,
) -> Result<JsValue, JsValue>
where
    E: finstack_monte_carlo::pricer::lsmc::ImmediateExercise,
{
    let run = prepare_lsmc_gbm(
        strike,
        rate,
        div_yield,
        vol,
        num_paths,
        seed,
        num_steps,
        currency,
        use_parallel,
        basis,
        basis_degree,
    )?;

    let est = run
        .pricer
        .price(
            &run.process,
            spot,
            expiry,
            run.num_steps,
            exercise,
            &run.basis,
            run.currency,
            rate,
        )
        .map_err(to_js_err)?;
    estimate_to_js(&est)
}

#[allow(clippy::too_many_arguments)]
fn price_lsmc_gbm_unbiased<E>(
    spot: f64,
    strike: f64,
    rate: f64,
    div_yield: f64,
    vol: f64,
    expiry: f64,
    num_paths: usize,
    seed: u64,
    pricing_seed: u64,
    num_steps: Option<usize>,
    currency: Option<String>,
    use_parallel: Option<bool>,
    basis: Option<String>,
    basis_degree: Option<usize>,
    exercise: &E,
) -> Result<JsValue, JsValue>
where
    E: finstack_monte_carlo::pricer::lsmc::ImmediateExercise,
{
    let run = prepare_lsmc_gbm(
        strike,
        rate,
        div_yield,
        vol,
        num_paths,
        seed,
        num_steps,
        currency,
        use_parallel,
        basis,
        basis_degree,
    )?;

    let est = run
        .pricer
        .price_unbiased(
            &run.process,
            spot,
            expiry,
            run.num_steps,
            exercise,
            &run.basis,
            run.currency,
            rate,
            pricing_seed,
        )
        .map_err(to_js_err)?;
    estimate_to_js(&est)
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

struct LsmcGbmRun {
    pricer: LsmcPricer,
    process: GbmProcess,
    basis: LsmcBasis,
    currency: Currency,
    num_steps: usize,
}

#[allow(clippy::too_many_arguments)]
fn prepare_lsmc_gbm(
    strike: f64,
    rate: f64,
    div_yield: f64,
    vol: f64,
    num_paths: usize,
    seed: u64,
    num_steps: Option<usize>,
    currency: Option<String>,
    use_parallel: Option<bool>,
    basis: Option<String>,
    basis_degree: Option<usize>,
) -> Result<LsmcGbmRun, JsValue> {
    use finstack_monte_carlo::pricer::basis::build_lsmc_basis_from_name;
    use finstack_monte_carlo::pricer::lsmc::LsmcConfig;

    let defaults = &binding_defaults()?.lsmc;
    let currency = resolve_currency(currency.as_deref())?;
    let num_steps = num_steps.unwrap_or(defaults.num_steps);
    let exercise_dates: Vec<usize> = (1..=num_steps).collect();
    let config = LsmcConfig::new(num_paths, exercise_dates, num_steps)
        .map_err(to_js_err)?
        .with_seed(seed)
        .with_parallel(use_parallel.unwrap_or(false));
    let process = GbmProcess::with_params(rate, div_yield, vol).map_err(to_js_err)?;

    let degree = basis_degree.unwrap_or(defaults.basis_degree);
    let basis_name = basis
        .as_deref()
        .unwrap_or(defaults.basis.as_str())
        .to_ascii_lowercase();
    let basis = build_lsmc_basis_from_name(&basis_name, degree, strike).map_err(to_js_err)?;

    Ok(LsmcGbmRun {
        pricer: LsmcPricer::new(config),
        process,
        basis,
        currency,
        num_steps,
    })
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
    use finstack_monte_carlo::discretization::QeHeston;
    use finstack_monte_carlo::engine::{McEngine, McEngineConfig};
    use finstack_monte_carlo::payoff::vanilla::{EuropeanCall, EuropeanPut};
    use finstack_monte_carlo::process::heston::HestonProcess;
    use finstack_monte_carlo::rng::philox::PhiloxRng;
    use finstack_monte_carlo::time_grid::TimeGrid;

    let ccy = resolve_currency(currency.as_deref())?;
    let steps = match num_steps {
        Some(steps) => steps,
        None => binding_defaults()?.european_pricer.num_steps,
    };
    let time_grid = TimeGrid::uniform(expiry, steps).map_err(to_js_err)?;
    let engine = McEngine::new(McEngineConfig::new(num_paths, time_grid).with_parallel(false));
    let rng = PhiloxRng::new(seed);
    let process = HestonProcess::with_params(rate, div_yield, kappa, theta, vol_of_vol, rho, v0)
        .map_err(to_js_err)?;
    let disc = QeHeston::new();
    let initial_state = [spot, v0];
    let discount_factor = (-rate * expiry).exp();
    let est = if is_call {
        let payoff = EuropeanCall::new(strike, 1.0, steps);
        engine.price(
            &rng,
            &process,
            &disc,
            &initial_state,
            &payoff,
            ccy,
            discount_factor,
        )
    } else {
        let payoff = EuropeanPut::new(strike, 1.0, steps);
        engine.price(
            &rng,
            &process,
            &disc,
            &initial_state,
            &payoff,
            ccy,
            discount_factor,
        )
    }
    .map_err(to_js_err)?;
    estimate_to_js(&est)
}

/// Resolve an optional currency string, defaulting to USD.
fn resolve_currency(code: Option<&str>) -> Result<Currency, JsValue> {
    let s = code.unwrap_or("USD");
    Currency::from_str(s).map_err(to_js_err)
}

/// Shared European pricer builder.
fn build_pricer(num_paths: usize, seed: u64) -> EuropeanPricer {
    EuropeanPricer::new(num_paths)
        .with_seed(seed)
        .with_parallel(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_core::currency::Currency;
    use finstack_core::money::Money;

    #[test]
    fn black_scholes_call_atm_reasonable() {
        let price = black_scholes_call(100.0, 100.0, 0.05, 0.0, 0.2, 1.0);
        assert!(price > 5.0 && price < 15.0, "ATM call price={price}");
    }

    #[test]
    fn black_scholes_put_atm_positive() {
        let price = black_scholes_put(100.0, 100.0, 0.05, 0.0, 0.2, 1.0);
        assert!(price > 0.0);
    }

    #[test]
    fn resolve_currency_defaults_and_parses_eur() {
        let Ok(usd) = resolve_currency(None) else {
            panic!("resolve_currency(None) should default to USD");
        };
        assert_eq!(usd, Currency::USD);

        let Ok(eur) = resolve_currency(Some("EUR")) else {
            panic!("resolve_currency EUR should succeed");
        };
        assert_eq!(eur, Currency::EUR);
    }

    #[test]
    fn mc_result_js_from_estimate_maps_fields() {
        let est = MoneyEstimate {
            mean: Money::new(10.0, Currency::USD),
            stderr: 0.25,
            ci_95: (
                Money::new(9.0, Currency::USD),
                Money::new(11.0, Currency::USD),
            ),
            num_paths: 1000,
            num_simulated_paths: 2000,
            std_dev: Some(5.0),
            median: None,
            percentile_25: None,
            percentile_75: None,
            min: None,
            max: None,
            num_skipped: 0,
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

    #[test]
    fn build_pricer_european_call_positive_mean() {
        let Ok(est) = build_pricer(1000, 42).price_gbm_call(
            100.0,
            100.0,
            0.05,
            0.0,
            0.2,
            1.0,
            252,
            Currency::USD,
        ) else {
            panic!("price_gbm_call should succeed");
        };
        assert!(est.mean.amount() > 0.0);
    }
}
