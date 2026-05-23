//! Python bindings for the `finstack-covenants` crate.

use pyo3::prelude::*;
use pyo3::types::{PyList, PyModule};

/// Validate and canonicalize a covenant spec JSON string.
#[pyfunction]
#[pyo3(text_signature = "(spec_json)")]
fn validate_covenant_spec(py: Python<'_>, spec_json: &str) -> PyResult<String> {
    py.detach(|| {
        finstack_covenants::validate_covenant_spec_json(spec_json)
            .map_err(crate::errors::core_to_py)
    })
}

/// Validate and canonicalize a covenant report JSON string.
#[pyfunction]
#[pyo3(text_signature = "(report_json)")]
fn validate_covenant_report(py: Python<'_>, report_json: &str) -> PyResult<String> {
    py.detach(|| {
        finstack_covenants::validate_covenant_report_json(report_json)
            .map_err(crate::errors::core_to_py)
    })
}

/// Validate and canonicalize a covenant engine JSON string.
#[pyfunction]
#[pyo3(text_signature = "(engine_json)")]
fn validate_covenant_engine(py: Python<'_>, engine_json: &str) -> PyResult<String> {
    py.detach(|| {
        finstack_covenants::validate_covenant_engine_json(engine_json)
            .map_err(crate::errors::core_to_py)
    })
}

/// Evaluate a covenant engine JSON string against a JSON metric map.
#[pyfunction]
#[pyo3(text_signature = "(engine_json, metrics_json, as_of)")]
fn evaluate_engine(
    py: Python<'_>,
    engine_json: &str,
    metrics_json: &str,
    as_of: &str,
) -> PyResult<String> {
    py.detach(|| {
        finstack_covenants::evaluate_engine_json(engine_json, metrics_json, as_of)
            .map_err(crate::errors::core_to_py)
    })
}

/// Standard leveraged-buyout covenant package as JSON.
#[pyfunction]
#[pyo3(text_signature = "(initial_leverage, interest_coverage, fixed_charge_coverage, max_capex)")]
fn lbo_standard(
    py: Python<'_>,
    initial_leverage: f64,
    interest_coverage: f64,
    fixed_charge_coverage: f64,
    max_capex: f64,
) -> PyResult<String> {
    py.detach(|| {
        finstack_covenants::lbo_standard_json(
            initial_leverage,
            interest_coverage,
            fixed_charge_coverage,
            max_capex,
        )
        .map_err(crate::errors::core_to_py)
    })
}

/// Covenant-lite package as JSON.
#[pyfunction]
#[pyo3(text_signature = "(max_leverage, max_senior_leverage)")]
fn cov_lite(py: Python<'_>, max_leverage: f64, max_senior_leverage: f64) -> PyResult<String> {
    py.detach(|| {
        finstack_covenants::cov_lite_json(max_leverage, max_senior_leverage)
            .map_err(crate::errors::core_to_py)
    })
}

/// Real-estate covenant package as JSON.
#[pyfunction]
#[pyo3(text_signature = "(min_dscr, min_debt_yield, max_ltv)")]
fn real_estate(
    py: Python<'_>,
    min_dscr: f64,
    min_debt_yield: f64,
    max_ltv: f64,
) -> PyResult<String> {
    py.detach(|| {
        finstack_covenants::real_estate_json(min_dscr, min_debt_yield, max_ltv)
            .map_err(crate::errors::core_to_py)
    })
}

/// Project-finance covenant package as JSON.
#[pyfunction]
#[pyo3(text_signature = "(min_dscr, distribution_lockup_dscr, min_liquidity, max_net_leverage)")]
fn project_finance(
    py: Python<'_>,
    min_dscr: f64,
    distribution_lockup_dscr: f64,
    min_liquidity: f64,
    max_net_leverage: f64,
) -> PyResult<String> {
    py.detach(|| {
        finstack_covenants::project_finance_json(
            min_dscr,
            distribution_lockup_dscr,
            min_liquidity,
            max_net_leverage,
        )
        .map_err(crate::errors::core_to_py)
    })
}

/// Register the `finstack.covenants` Python namespace.
pub fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(py, "covenants")?;
    m.setattr(
        "__doc__",
        "Covenant package JSON validation, templates, and map-backed evaluation.",
    )?;

    m.add_function(wrap_pyfunction!(validate_covenant_spec, &m)?)?;
    m.add_function(wrap_pyfunction!(validate_covenant_report, &m)?)?;
    m.add_function(wrap_pyfunction!(validate_covenant_engine, &m)?)?;
    m.add_function(wrap_pyfunction!(evaluate_engine, &m)?)?;
    m.add_function(wrap_pyfunction!(lbo_standard, &m)?)?;
    m.add_function(wrap_pyfunction!(cov_lite, &m)?)?;
    m.add_function(wrap_pyfunction!(real_estate, &m)?)?;
    m.add_function(wrap_pyfunction!(project_finance, &m)?)?;

    let all = PyList::new(
        py,
        [
            "validate_covenant_spec",
            "validate_covenant_report",
            "validate_covenant_engine",
            "evaluate_engine",
            "lbo_standard",
            "cov_lite",
            "real_estate",
            "project_finance",
        ],
    )?;
    m.setattr("__all__", all)?;
    crate::bindings::module_utils::register_submodule_by_parent_name(
        py,
        parent,
        &m,
        "covenants",
        "finstack.finstack",
    )?;

    Ok(())
}
