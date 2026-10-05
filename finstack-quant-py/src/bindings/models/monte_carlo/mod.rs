//! Python bindings for the `finstack-quant-models` crate.
//!
//! Exposes path simulation for the built-in processes (Markov diffusions, the
//! LIBOR market model and the rough-volatility models) and canonical
//! European, Asian, LSMC, Heston, and Greek workflows; closed-form
//! Black-Scholes references live at `finstack_quant.models`. `simulate_paths`
//! selects a process and discretization scheme by spec; the process,
//! discretization, RNG, and payoff types themselves remain Rust-only.
//!
//! Simulated paths retain at most 100,000 random streams and 64 million scalar
//! values across paths and shared time grids. LSMC accepts at most 10 million independent
//! paths and 100,000 steps; each pricing pass limits retained spots to 64 million
//! values, including time zero and antithetic partners.

mod engine;
mod greeks;
mod pricers;
mod results;

use pyo3::prelude::*;
use pyo3::types::PyList;

/// Register the `finstack_quant.models.monte_carlo` submodule.
pub fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = crate::bindings::module_utils::new_submodule(parent, "monte_carlo")?;
    m.setattr(
        "__doc__",
        "Monte Carlo convenience bindings (finstack-quant-models).",
    )?;

    results::register(py, &m)?;
    engine::register(py, &m)?;
    pricers::register(py, &m)?;
    greeks::register(py, &m)?;

    let all = PyList::new(
        py,
        [
            "MoneyEstimate",
            "Estimate",
            "PathSummary",
            "LrmGreeks",
            "simulate_paths",
            "heston_satisfies_feller",
            "EuropeanPricer",
            "PathDependentPricer",
            "LsmcPricer",
            "price_heston_call",
            "price_heston_put",
            "finite_diff_delta",
            "finite_diff_gamma",
        ],
    )?;
    m.setattr("__all__", all)?;
    crate::bindings::module_utils::attach_submodule(
        parent,
        &m,
        crate::bindings::module_utils::Exposure::Python,
    )?;

    Ok(())
}
