//! Python bindings for the `finstack-monte-carlo` crate.
//!
//! Exposes convenience pricing over the Rust Monte Carlo crate: engine,
//! European/Asian/LSMC pricers, Black-Scholes formulas, and selected non-GBM
//! process wrappers. Advanced Rust process, discretization, RNG, payoff, and
//! Greeks types are intentionally not standalone Python types yet; their
//! parameters are passed directly as numeric arguments to the exposed pricer
//! constructors and methods.

mod analytical;
mod engine;
mod greeks;
mod pricers;
mod results;
mod time_grid;

use pyo3::prelude::*;
use pyo3::types::PyList;

/// Register the `finstack.monte_carlo` submodule.
pub fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(py, "monte_carlo")?;
    m.setattr(
        "__doc__",
        "Monte Carlo convenience bindings (finstack-monte-carlo).",
    )?;

    results::register(py, &m)?;
    time_grid::register(py, &m)?;
    engine::register(py, &m)?;
    pricers::register(py, &m)?;
    analytical::register(py, &m)?;
    greeks::register(py, &m)?;

    let all = PyList::new(
        py,
        [
            "MoneyEstimate",
            "Estimate",
            "TimeGrid",
            "McEngine",
            "EuropeanPricer",
            "PathDependentPricer",
            "LsmcPricer",
            "black_scholes_call",
            "black_scholes_put",
            "price_european_call",
            "price_european_put",
            "price_heston_call",
            "price_heston_put",
            "finite_diff_delta",
            "finite_diff_delta_crn",
            "finite_diff_gamma",
            "finite_diff_gamma_crn",
        ],
    )?;
    m.setattr("__all__", all)?;
    crate::bindings::module_utils::register_submodule(
        py,
        parent,
        &m,
        "monte_carlo",
        crate::bindings::module_utils::ROOT_PACKAGE,
        crate::bindings::module_utils::ParentNameSource::Name,
    )?;

    Ok(())
}
