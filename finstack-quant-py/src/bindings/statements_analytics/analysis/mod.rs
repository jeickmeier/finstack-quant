//! Python wrappers for statement analysis functions.
//!
//! Covers: sensitivity, variance, scenario sets, backtesting, goal seek, and
//! introspection (dependency tracing, formula explanation). DCF/LBO live in
//! `valuation.rs`, check suites in `checks.rs`, reports in `reports.rs`.
//!
//! All functions that accept a financial model or statement result support
//! both JSON strings and typed Python objects (`FinancialModelSpec`,
//! `StatementResult`) for zero-overhead calls when the caller already has
//! a parsed object. Errors from the `finstack-quant-statements` crate map to
//! `KeyError` for missing nodes, `RuntimeError` for capital-structure
//! failures and `ValueError` otherwise.

mod explain;
mod runners;

use explain::{PyDependencyTracer, explain_formula};
pub use explain::{PyDependencyTree, PyExplanation, PyExplanationStep};
use pyo3::prelude::*;
pub use runners::{PyForecastMetrics, PyGoalSeekResult};
use runners::{
    PyGoalSeekSpec, backtest_forecast, evaluate_scenario_set, evaluate_scenario_set_with_market,
    generate_tornado_entries, goal_seek, run_sensitivity, run_variance, scenario_diff,
    variance_bridge,
};

/// Register analysis functions and classes.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyDependencyTracer>()?;
    m.add_class::<PyDependencyTree>()?;
    m.add_class::<PyForecastMetrics>()?;
    m.add_class::<PyGoalSeekSpec>()?;
    m.add_class::<PyGoalSeekResult>()?;
    m.add_class::<PyExplanationStep>()?;
    m.add_class::<PyExplanation>()?;
    m.add_function(pyo3::wrap_pyfunction!(run_sensitivity, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(generate_tornado_entries, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(run_variance, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(evaluate_scenario_set, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(
        evaluate_scenario_set_with_market,
        m
    )?)?;
    m.add_function(pyo3::wrap_pyfunction!(scenario_diff, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(variance_bridge, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(backtest_forecast, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(goal_seek, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(explain_formula, m)?)?;
    Ok(())
}
