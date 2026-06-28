//! McEngine binding (configured via `PyTimeGrid`) plus module-level
//! convenience pricing functions.

use super::results::PyMoneyEstimate;
use super::time_grid::PyTimeGrid;
use crate::bindings::core::currency::extract_currency;
use crate::errors::core_to_py;
use finstack_quant_monte_carlo::engine::{McEngine, McEngineConfig};
use finstack_quant_monte_carlo::pricer::european::EuropeanPricer;
use finstack_quant_monte_carlo::registry::{self, PythonBindingDefaults};
use pyo3::prelude::*;
use std::str::FromStr;

/// Resolve the embedded Python-binding defaults, mapping registry errors to
/// Python exceptions.
pub(super) fn py_mc_defaults() -> PyResult<&'static PythonBindingDefaults> {
    registry::embedded_defaults()
        .map(|defaults| &defaults.python_bindings)
        .map_err(core_to_py)
}

/// The core Monte Carlo engine for full control over simulation.
#[pyclass(name = "McEngine", module = "finstack_quant.monte_carlo", frozen)]
pub struct PyMcEngine {
    inner: McEngine,
    seed: u64,
}

#[pymethods]
impl PyMcEngine {
    /// Build an engine from a time grid configuration.
    ///
    /// European call/put pricing runs through the GBM `EuropeanPricer`, whose
    /// simulation vocabulary is `num_paths`, `seed`, and `use_parallel`;
    /// antithetic variates are not part of that path, so no antithetic knob is
    /// exposed here.
    #[new]
    #[pyo3(signature = (num_paths, time_grid, seed=None, use_parallel=None))]
    fn new(
        num_paths: usize,
        time_grid: &PyTimeGrid,
        seed: Option<u64>,
        use_parallel: Option<bool>,
    ) -> PyResult<Self> {
        let defaults = &py_mc_defaults()?.engine;
        let seed = seed.unwrap_or(defaults.seed);
        let use_parallel = use_parallel.unwrap_or(defaults.use_parallel);
        let config = McEngineConfig::new(num_paths, time_grid.inner.clone()).parallel(use_parallel);
        Ok(Self {
            inner: McEngine::new(config),
            seed,
        })
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
    ) -> PyResult<PyMoneyEstimate> {
        let ccy = resolve_currency(currency)?;
        py.detach(|| {
            price_european_gbm(
                &self.inner,
                self.seed,
                true,
                spot,
                strike,
                rate,
                div_yield,
                vol,
                ccy,
            )
        })
        .map(PyMoneyEstimate::from_inner)
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
    ) -> PyResult<PyMoneyEstimate> {
        let ccy = resolve_currency(currency)?;
        py.detach(|| {
            price_european_gbm(
                &self.inner,
                self.seed,
                false,
                spot,
                strike,
                rate,
                div_yield,
                vol,
                ccy,
            )
        })
        .map(PyMoneyEstimate::from_inner)
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

#[allow(clippy::too_many_arguments)]
fn price_european_gbm(
    engine: &McEngine,
    seed: u64,
    is_call: bool,
    spot: f64,
    strike: f64,
    rate: f64,
    div_yield: f64,
    vol: f64,
    ccy: finstack_quant_core::currency::Currency,
) -> finstack_quant_core::Result<finstack_quant_monte_carlo::results::MoneyEstimate> {
    use finstack_quant_monte_carlo::discretization::exact::ExactGbm;
    use finstack_quant_monte_carlo::payoff::vanilla::{EuropeanCall, EuropeanPut};
    use finstack_quant_monte_carlo::process::gbm::GbmProcess;
    use finstack_quant_monte_carlo::rng::philox::PhiloxRng;

    let t_max = engine.config().time_grid.t_max();
    let num_steps = engine.config().time_grid.num_steps();
    let rng = PhiloxRng::new(seed);
    let process = GbmProcess::with_params(rate, div_yield, vol)?;
    let disc = ExactGbm::new();
    let initial_state = vec![spot];
    let discount_factor = (-rate * t_max).exp();

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
}

/// Resolve an optional currency argument, defaulting to USD.
pub(super) fn resolve_currency(
    currency: Option<&Bound<'_, PyAny>>,
) -> PyResult<finstack_quant_core::currency::Currency> {
    match currency {
        Some(obj) => extract_currency(obj),
        None => {
            let default_currency = &py_mc_defaults()?.default_currency;
            finstack_quant_core::currency::Currency::from_str(default_currency).map_err(|e| {
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
) -> PyResult<PyMoneyEstimate> {
    let defaults = &py_mc_defaults()?.european_pricer;
    let num_paths = num_paths.unwrap_or(defaults.num_paths);
    let seed = seed.unwrap_or(defaults.seed);
    let num_steps = num_steps.unwrap_or(defaults.num_steps);
    let ccy = resolve_currency(currency)?;
    let pricer = EuropeanPricer::new(num_paths)
        .with_seed(seed)
        .with_parallel(defaults.use_parallel);
    py.detach(|| pricer.price_gbm_call(spot, strike, rate, div_yield, vol, expiry, num_steps, ccy))
        .map(PyMoneyEstimate::from_inner)
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
) -> PyResult<PyMoneyEstimate> {
    let defaults = &py_mc_defaults()?.european_pricer;
    let num_paths = num_paths.unwrap_or(defaults.num_paths);
    let seed = seed.unwrap_or(defaults.seed);
    let num_steps = num_steps.unwrap_or(defaults.num_steps);
    let ccy = resolve_currency(currency)?;
    let pricer = EuropeanPricer::new(num_paths)
        .with_seed(seed)
        .with_parallel(defaults.use_parallel);
    py.detach(|| pricer.price_gbm_put(spot, strike, rate, div_yield, vol, expiry, num_steps, ccy))
        .map(PyMoneyEstimate::from_inner)
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
) -> PyResult<PyMoneyEstimate> {
    use finstack_quant_monte_carlo::discretization::QeHeston;
    use finstack_quant_monte_carlo::payoff::vanilla::{EuropeanCall, EuropeanPut};
    use finstack_quant_monte_carlo::process::heston::HestonProcess;
    use finstack_quant_monte_carlo::rng::philox::PhiloxRng;
    use finstack_quant_monte_carlo::time_grid::TimeGrid;

    let defaults = &py_mc_defaults()?.european_pricer;
    let num_paths = num_paths.unwrap_or(defaults.num_paths);
    let seed = seed.unwrap_or(defaults.seed);
    let num_steps = num_steps.unwrap_or(defaults.num_steps);
    let ccy = resolve_currency(currency)?;
    let time_grid = TimeGrid::uniform(expiry, num_steps).map_err(core_to_py)?;
    let config = McEngineConfig::new(num_paths, time_grid).parallel(defaults.use_parallel);
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
    .map(PyMoneyEstimate::from_inner)
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
) -> PyResult<PyMoneyEstimate> {
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
) -> PyResult<PyMoneyEstimate> {
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
