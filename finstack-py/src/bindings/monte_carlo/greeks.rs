//! Python bindings for Monte Carlo Greek estimators.
//!
//! Exposes finite-difference deltas and gammas with independent-path and
//! common-random-number standard errors.
//!
//! All functions release the GIL during the underlying Monte Carlo runs.

use super::engine::resolve_currency;
use crate::errors::core_to_py;
use finstack_monte_carlo::discretization::exact::ExactGbm;
use finstack_monte_carlo::engine::{McEngine, McEngineConfig};
use finstack_monte_carlo::greeks::finite_diff::{
    finite_diff_delta, finite_diff_delta_crn, finite_diff_gamma, finite_diff_gamma_crn,
};
use finstack_monte_carlo::payoff::vanilla::{EuropeanCall, EuropeanPut};
use finstack_monte_carlo::process::gbm::GbmProcess;
use finstack_monte_carlo::registry;
use finstack_monte_carlo::rng::philox::PhiloxRng;
use finstack_monte_carlo::time_grid::TimeGrid;
use pyo3::prelude::*;

type Currency = finstack_core::currency::Currency;
type GreekResult = finstack_core::Result<(f64, f64)>;
type CallGreekEstimator = fn(
    &McEngine,
    &PhiloxRng,
    &GbmProcess,
    &ExactGbm,
    f64,
    &EuropeanCall,
    Currency,
    f64,
    f64,
) -> GreekResult;
type PutGreekEstimator = fn(
    &McEngine,
    &PhiloxRng,
    &GbmProcess,
    &ExactGbm,
    f64,
    &EuropeanPut,
    Currency,
    f64,
    f64,
) -> GreekResult;

#[derive(Debug, Clone, Copy)]
enum OptionType {
    Call,
    Put,
}

struct GreekSetup {
    engine: McEngine,
    rng: PhiloxRng,
    gbm: GbmProcess,
    disc: ExactGbm,
    currency: Currency,
    num_steps: usize,
    bump_size: f64,
    option_type: OptionType,
    discount_factor: f64,
}

fn parse_option(name: &str) -> PyResult<OptionType> {
    match name.to_ascii_lowercase().as_str() {
        "call" | "c" => Ok(OptionType::Call),
        "put" | "p" => Ok(OptionType::Put),
        other => Err(crate::errors::value_error(format!(
            "unknown option_type '{other}'; expected 'call' or 'put'"
        ))),
    }
}

fn build_engine(num_paths: usize, expiry: f64, num_steps: usize) -> PyResult<McEngine> {
    let time_grid = TimeGrid::uniform(expiry, num_steps).map_err(core_to_py)?;
    let defaults = &registry::embedded_defaults_or_panic()
        .python_bindings
        .greeks;
    let config = McEngineConfig::new(num_paths, time_grid)
        .with_parallel(defaults.use_parallel)
        .with_chunk_size(defaults.chunk_size)
        .with_antithetic(defaults.antithetic);
    Ok(McEngine::new(config))
}

fn greek_defaults() -> &'static registry::PythonGreekDefaults {
    &registry::embedded_defaults_or_panic()
        .python_bindings
        .greeks
}

#[allow(clippy::too_many_arguments)]
fn build_greek_setup(
    rate: f64,
    div_yield: f64,
    vol: f64,
    expiry: f64,
    num_paths: Option<usize>,
    seed: Option<u64>,
    num_steps: Option<usize>,
    bump_size: Option<f64>,
    option_type: Option<&str>,
    currency: Option<&Bound<'_, PyAny>>,
) -> PyResult<GreekSetup> {
    let defaults = greek_defaults();
    let num_paths = num_paths.unwrap_or(defaults.num_paths);
    let seed = seed.unwrap_or(defaults.seed);
    let num_steps = num_steps.unwrap_or(defaults.num_steps);
    let bump_size = bump_size.unwrap_or(defaults.bump_size);
    let option_type = option_type.unwrap_or(&defaults.option_type);

    Ok(GreekSetup {
        engine: build_engine(num_paths, expiry, num_steps)?,
        rng: PhiloxRng::new(seed),
        gbm: GbmProcess::with_params(rate, div_yield, vol).map_err(core_to_py)?,
        disc: ExactGbm::new(),
        currency: resolve_currency(currency)?,
        num_steps,
        bump_size,
        option_type: parse_option(option_type)?,
        discount_factor: (-rate * expiry).exp(),
    })
}

#[allow(clippy::too_many_arguments)]
fn run_greek(
    py: Python<'_>,
    spot: f64,
    strike: f64,
    rate: f64,
    div_yield: f64,
    vol: f64,
    expiry: f64,
    num_paths: Option<usize>,
    seed: Option<u64>,
    num_steps: Option<usize>,
    bump_size: Option<f64>,
    option_type: Option<&str>,
    currency: Option<&Bound<'_, PyAny>>,
    call_estimator: CallGreekEstimator,
    put_estimator: PutGreekEstimator,
) -> PyResult<(f64, f64)> {
    let setup = build_greek_setup(
        rate,
        div_yield,
        vol,
        expiry,
        num_paths,
        seed,
        num_steps,
        bump_size,
        option_type,
        currency,
    )?;
    py.detach(|| match setup.option_type {
        OptionType::Call => {
            let payoff = EuropeanCall::new(strike, expiry, setup.num_steps);
            call_estimator(
                &setup.engine,
                &setup.rng,
                &setup.gbm,
                &setup.disc,
                spot,
                &payoff,
                setup.currency,
                setup.discount_factor,
                setup.bump_size,
            )
        }
        OptionType::Put => {
            let payoff = EuropeanPut::new(strike, expiry, setup.num_steps);
            put_estimator(
                &setup.engine,
                &setup.rng,
                &setup.gbm,
                &setup.disc,
                spot,
                &payoff,
                setup.currency,
                setup.discount_factor,
                setup.bump_size,
            )
        }
    })
    .map_err(core_to_py)
}

/// Finite-difference delta for a vanilla European option under GBM.
///
/// Reports the conservative independence-bound stderr. Use [`fd_delta_crn`]
/// for paired common-random-number stderr.
///
/// Returns `(delta, stderr)`.
#[pyfunction]
#[allow(clippy::too_many_arguments)]
#[pyo3(signature = (
    spot, strike, rate, div_yield, vol, expiry,
    num_paths=None, seed=None, num_steps=None,
    bump_size=None, option_type=None, currency=None,
))]
fn fd_delta(
    py: Python<'_>,
    spot: f64,
    strike: f64,
    rate: f64,
    div_yield: f64,
    vol: f64,
    expiry: f64,
    num_paths: Option<usize>,
    seed: Option<u64>,
    num_steps: Option<usize>,
    bump_size: Option<f64>,
    option_type: Option<&str>,
    currency: Option<&Bound<'_, PyAny>>,
) -> PyResult<(f64, f64)> {
    run_greek(
        py,
        spot,
        strike,
        rate,
        div_yield,
        vol,
        expiry,
        num_paths,
        seed,
        num_steps,
        bump_size,
        option_type,
        currency,
        finite_diff_delta,
        finite_diff_delta,
    )
}

/// Finite-difference delta with paired common-random-number stderr.
///
/// Returns `(delta, stderr)`.
#[pyfunction]
#[allow(clippy::too_many_arguments)]
#[pyo3(signature = (
    spot, strike, rate, div_yield, vol, expiry,
    num_paths=None, seed=None, num_steps=None,
    bump_size=None, option_type=None, currency=None,
))]
fn fd_delta_crn(
    py: Python<'_>,
    spot: f64,
    strike: f64,
    rate: f64,
    div_yield: f64,
    vol: f64,
    expiry: f64,
    num_paths: Option<usize>,
    seed: Option<u64>,
    num_steps: Option<usize>,
    bump_size: Option<f64>,
    option_type: Option<&str>,
    currency: Option<&Bound<'_, PyAny>>,
) -> PyResult<(f64, f64)> {
    run_greek(
        py,
        spot,
        strike,
        rate,
        div_yield,
        vol,
        expiry,
        num_paths,
        seed,
        num_steps,
        bump_size,
        option_type,
        currency,
        finite_diff_delta_crn,
        finite_diff_delta_crn,
    )
}

/// Finite-difference gamma (independence-bound stderr).
///
/// Returns `(gamma, stderr)`.
#[pyfunction]
#[allow(clippy::too_many_arguments)]
#[pyo3(signature = (
    spot, strike, rate, div_yield, vol, expiry,
    num_paths=None, seed=None, num_steps=None,
    bump_size=None, option_type=None, currency=None,
))]
fn fd_gamma(
    py: Python<'_>,
    spot: f64,
    strike: f64,
    rate: f64,
    div_yield: f64,
    vol: f64,
    expiry: f64,
    num_paths: Option<usize>,
    seed: Option<u64>,
    num_steps: Option<usize>,
    bump_size: Option<f64>,
    option_type: Option<&str>,
    currency: Option<&Bound<'_, PyAny>>,
) -> PyResult<(f64, f64)> {
    run_greek(
        py,
        spot,
        strike,
        rate,
        div_yield,
        vol,
        expiry,
        num_paths,
        seed,
        num_steps,
        bump_size,
        option_type,
        currency,
        finite_diff_gamma,
        finite_diff_gamma,
    )
}

/// Finite-difference gamma with paired common-random-number stderr.
///
/// Returns `(gamma, stderr)`.
#[pyfunction]
#[allow(clippy::too_many_arguments)]
#[pyo3(signature = (
    spot, strike, rate, div_yield, vol, expiry,
    num_paths=None, seed=None, num_steps=None,
    bump_size=None, option_type=None, currency=None,
))]
fn fd_gamma_crn(
    py: Python<'_>,
    spot: f64,
    strike: f64,
    rate: f64,
    div_yield: f64,
    vol: f64,
    expiry: f64,
    num_paths: Option<usize>,
    seed: Option<u64>,
    num_steps: Option<usize>,
    bump_size: Option<f64>,
    option_type: Option<&str>,
    currency: Option<&Bound<'_, PyAny>>,
) -> PyResult<(f64, f64)> {
    run_greek(
        py,
        spot,
        strike,
        rate,
        div_yield,
        vol,
        expiry,
        num_paths,
        seed,
        num_steps,
        bump_size,
        option_type,
        currency,
        finite_diff_gamma_crn,
        finite_diff_gamma_crn,
    )
}

pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(fd_delta, m)?)?;
    m.add_function(wrap_pyfunction!(fd_delta_crn, m)?)?;
    m.add_function(wrap_pyfunction!(fd_gamma, m)?)?;
    m.add_function(wrap_pyfunction!(fd_gamma_crn, m)?)?;
    Ok(())
}
