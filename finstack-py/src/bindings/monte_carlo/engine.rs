//! McEngine binding (configured via `PyTimeGrid`) plus module-level
//! convenience pricing functions.

use super::results::PyMonteCarloResult;
use super::time_grid::PyTimeGrid;
use crate::bindings::core::currency::extract_currency;
use crate::errors::core_to_py;
use finstack_monte_carlo::engine::{McEngine, McEngineConfig};
use finstack_monte_carlo::pricer::european::EuropeanPricer;
use finstack_monte_carlo::registry;
use pyo3::prelude::*;
use std::str::FromStr;

/// The core Monte Carlo engine for full control over simulation.
#[pyclass(name = "McEngine", module = "finstack.monte_carlo", frozen)]
pub struct PyMcEngine {
    inner: McEngine,
    seed: u64,
}

#[pymethods]
impl PyMcEngine {
    /// Build an engine from a time grid configuration.
    #[new]
    #[pyo3(signature = (num_paths, time_grid, seed=None, use_parallel=None, antithetic=None))]
    fn new(
        num_paths: usize,
        time_grid: &PyTimeGrid,
        seed: Option<u64>,
        use_parallel: Option<bool>,
        antithetic: Option<bool>,
    ) -> Self {
        let defaults = &registry::embedded_defaults_or_panic()
            .python_bindings
            .engine;
        let seed = seed.unwrap_or(defaults.seed);
        let use_parallel = use_parallel.unwrap_or(defaults.use_parallel);
        let antithetic = antithetic.unwrap_or(defaults.antithetic);
        let config = McEngineConfig::new(num_paths, time_grid.inner.clone())
            .with_parallel(use_parallel)
            .with_antithetic(antithetic);
        Self {
            inner: McEngine::new(config),
            seed,
        }
    }

    /// Price a European call under GBM.
    #[allow(clippy::too_many_arguments)]
    #[pyo3(signature = (spot, strike, rate, div_yield, vol, currency=None))]
    fn price_european_call(
        &self,
        py: Python<'_>,
        spot: f64,
        strike: f64,
        rate: f64,
        div_yield: f64,
        vol: f64,
        currency: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<PyMonteCarloResult> {
        let ccy = resolve_currency(currency)?;
        let t_max = self.inner.config().time_grid.t_max();
        let num_steps = self.inner.config().time_grid.num_steps();
        let pricer = EuropeanPricer::new(self.inner.config().num_paths)
            .with_seed(self.seed)
            .with_parallel(self.inner.config().use_parallel);
        py.detach(|| {
            pricer.price_gbm_call(spot, strike, rate, div_yield, vol, t_max, num_steps, ccy)
        })
        .map(PyMonteCarloResult::from_inner)
        .map_err(core_to_py)
    }

    /// Price a European put under GBM.
    #[allow(clippy::too_many_arguments)]
    #[pyo3(signature = (spot, strike, rate, div_yield, vol, currency=None))]
    fn price_european_put(
        &self,
        py: Python<'_>,
        spot: f64,
        strike: f64,
        rate: f64,
        div_yield: f64,
        vol: f64,
        currency: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<PyMonteCarloResult> {
        let ccy = resolve_currency(currency)?;
        let t_max = self.inner.config().time_grid.t_max();
        let num_steps = self.inner.config().time_grid.num_steps();
        let pricer = EuropeanPricer::new(self.inner.config().num_paths)
            .with_seed(self.seed)
            .with_parallel(self.inner.config().use_parallel);
        py.detach(|| {
            pricer.price_gbm_put(spot, strike, rate, div_yield, vol, t_max, num_steps, ccy)
        })
        .map(PyMonteCarloResult::from_inner)
        .map_err(core_to_py)
    }

    fn __repr__(&self) -> String {
        let c = self.inner.config();
        format!(
            "McEngine(paths={}, steps={}, T={:.4})",
            c.num_paths,
            c.time_grid.num_steps(),
            c.time_grid.t_max()
        )
    }
}

// ---------------------------------------------------------------------------
// Module-level convenience functions
// ---------------------------------------------------------------------------

/// Resolve an optional currency argument, defaulting to USD.
pub(super) fn resolve_currency(
    currency: Option<&Bound<'_, PyAny>>,
) -> PyResult<finstack_core::currency::Currency> {
    match currency {
        Some(obj) => extract_currency(obj),
        None => {
            let default_currency = &registry::embedded_defaults_or_panic()
                .python_bindings
                .default_currency;
            finstack_core::currency::Currency::from_str(default_currency).map_err(|e| {
                crate::errors::value_error(format!("Failed to resolve default currency: {e}"))
            })
        }
    }
}

/// Price a European call option via Monte Carlo under GBM dynamics.
#[pyfunction]
#[allow(clippy::too_many_arguments)]
#[pyo3(signature = (spot, strike, rate, div_yield, vol, expiry, num_paths=None, seed=None, num_steps=None, currency=None))]
fn price_european_call(
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
    currency: Option<&Bound<'_, PyAny>>,
) -> PyResult<PyMonteCarloResult> {
    let defaults = &registry::embedded_defaults_or_panic()
        .python_bindings
        .european_pricer;
    let num_paths = num_paths.unwrap_or(defaults.num_paths);
    let seed = seed.unwrap_or(defaults.seed);
    let num_steps = num_steps.unwrap_or(defaults.num_steps);
    let ccy = resolve_currency(currency)?;
    let pricer = EuropeanPricer::new(num_paths)
        .with_seed(seed)
        .with_parallel(defaults.use_parallel);
    py.detach(|| pricer.price_gbm_call(spot, strike, rate, div_yield, vol, expiry, num_steps, ccy))
        .map(PyMonteCarloResult::from_inner)
        .map_err(core_to_py)
}

/// Price a European put option via Monte Carlo under GBM dynamics.
#[pyfunction]
#[allow(clippy::too_many_arguments)]
#[pyo3(signature = (spot, strike, rate, div_yield, vol, expiry, num_paths=None, seed=None, num_steps=None, currency=None))]
fn price_european_put(
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
    currency: Option<&Bound<'_, PyAny>>,
) -> PyResult<PyMonteCarloResult> {
    let defaults = &registry::embedded_defaults_or_panic()
        .python_bindings
        .european_pricer;
    let num_paths = num_paths.unwrap_or(defaults.num_paths);
    let seed = seed.unwrap_or(defaults.seed);
    let num_steps = num_steps.unwrap_or(defaults.num_steps);
    let ccy = resolve_currency(currency)?;
    let pricer = EuropeanPricer::new(num_paths)
        .with_seed(seed)
        .with_parallel(defaults.use_parallel);
    py.detach(|| pricer.price_gbm_put(spot, strike, rate, div_yield, vol, expiry, num_steps, ccy))
        .map(PyMonteCarloResult::from_inner)
        .map_err(core_to_py)
}

#[allow(clippy::too_many_arguments)]
fn price_heston(
    py: Python<'_>,
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
    currency: Option<&Bound<'_, PyAny>>,
) -> PyResult<PyMonteCarloResult> {
    use finstack_monte_carlo::discretization::QeHeston;
    use finstack_monte_carlo::payoff::vanilla::{EuropeanCall, EuropeanPut};
    use finstack_monte_carlo::process::heston::HestonProcess;
    use finstack_monte_carlo::rng::philox::PhiloxRng;
    use finstack_monte_carlo::time_grid::TimeGrid;

    let defaults = &registry::embedded_defaults_or_panic()
        .python_bindings
        .european_pricer;
    let num_paths = num_paths.unwrap_or(defaults.num_paths);
    let seed = seed.unwrap_or(defaults.seed);
    let num_steps = num_steps.unwrap_or(defaults.num_steps);
    let ccy = resolve_currency(currency)?;
    let time_grid = TimeGrid::uniform(expiry, num_steps).map_err(core_to_py)?;
    let config = McEngineConfig::new(num_paths, time_grid).with_parallel(defaults.use_parallel);
    let engine = McEngine::new(config);
    let rng = PhiloxRng::new(seed);
    let process = HestonProcess::with_params(rate, div_yield, kappa, theta, vol_of_vol, rho, v0)
        .map_err(core_to_py)?;
    let disc = QeHeston::new();
    let initial_state = vec![spot, v0];
    let discount_factor = (-rate * expiry).exp();

    py.detach(|| {
        if is_call {
            let payoff = EuropeanCall::new(strike, 1.0, num_steps);
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
            let payoff = EuropeanPut::new(strike, 1.0, num_steps);
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
    })
    .map(PyMonteCarloResult::from_inner)
    .map_err(core_to_py)
}

#[pyfunction]
#[allow(clippy::too_many_arguments)]
#[pyo3(signature = (spot, strike, rate, div_yield, kappa, theta, vol_of_vol, rho, v0, expiry, num_paths=None, seed=None, num_steps=None, currency=None))]
fn price_heston_call(
    py: Python<'_>,
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
    currency: Option<&Bound<'_, PyAny>>,
) -> PyResult<PyMonteCarloResult> {
    price_heston(
        py, true, spot, strike, rate, div_yield, kappa, theta, vol_of_vol, rho, v0, expiry,
        num_paths, seed, num_steps, currency,
    )
}

#[pyfunction]
#[allow(clippy::too_many_arguments)]
#[pyo3(signature = (spot, strike, rate, div_yield, kappa, theta, vol_of_vol, rho, v0, expiry, num_paths=None, seed=None, num_steps=None, currency=None))]
fn price_heston_put(
    py: Python<'_>,
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
    currency: Option<&Bound<'_, PyAny>>,
) -> PyResult<PyMonteCarloResult> {
    price_heston(
        py, false, spot, strike, rate, div_yield, kappa, theta, vol_of_vol, rho, v0, expiry,
        num_paths, seed, num_steps, currency,
    )
}

pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyMcEngine>()?;
    m.add_function(wrap_pyfunction!(price_european_call, m)?)?;
    m.add_function(wrap_pyfunction!(price_european_put, m)?)?;
    m.add_function(wrap_pyfunction!(price_heston_call, m)?)?;
    m.add_function(wrap_pyfunction!(price_heston_put, m)?)?;
    Ok(())
}
