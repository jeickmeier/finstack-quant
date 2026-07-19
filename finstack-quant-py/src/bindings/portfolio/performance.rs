//! Python bindings for portfolio performance measurement.
//!
//! The functions accept JSON inputs matching the Rust `serde` shapes and
//! delegate all calculations to `finstack_quant_portfolio::performance`.

use pyo3::prelude::*;
use pyo3::types::PyModule;

use crate::errors::{core_to_py, serde_json_to_py};

/// Compute a Modified-Dietz TWRR sub-period return.
///
/// Returns ``None`` when the Rust engine determines the return is undefined
/// (for example, non-positive adjusted denominator).
#[pyfunction]
#[pyo3(text_signature = "(period_json)")]
fn twrr_modified_dietz(py: Python<'_>, period_json: &str) -> PyResult<Option<f64>> {
    let period_json = period_json.to_owned();
    py.detach(move || {
        let period: finstack_quant_portfolio::TwrrPeriod = serde_json::from_str(&period_json)
            .map_err(|err| serde_json_to_py(err, "invalid TWRR period JSON"))?;
        Ok(finstack_quant_portfolio::twrr_modified_dietz(&period))
    })
}

/// Geometrically link TWRR sub-period returns.
#[pyfunction]
#[pyo3(text_signature = "(returns_json, horizon_years)")]
fn twrr_linked(py: Python<'_>, returns_json: &str, horizon_years: f64) -> PyResult<Option<String>> {
    let returns_json = returns_json.to_owned();
    py.detach(move || {
        let returns: Vec<f64> = serde_json::from_str(&returns_json)
            .map_err(|err| serde_json_to_py(err, "invalid TWRR returns JSON"))?;
        finstack_quant_portfolio::twrr_linked(&returns, horizon_years)
            .map(|result| {
                serde_json::to_string(&result)
                    .map_err(|err| serde_json_to_py(err, "serialize linked return"))
            })
            .transpose()
    })
}

/// Compute money-weighted return via XIRR from dated cashflow JSON.
#[pyfunction]
#[pyo3(text_signature = "(cashflows_json)")]
fn mwr_xirr(py: Python<'_>, cashflows_json: &str) -> PyResult<f64> {
    let cashflows_json = cashflows_json.to_owned();
    py.detach(move || {
        let cashflows: Vec<finstack_quant_portfolio::DatedCashflow> =
            serde_json::from_str(&cashflows_json)
                .map_err(|err| serde_json_to_py(err, "invalid MWR cashflows JSON"))?;
        finstack_quant_portfolio::mwr_xirr_from_cashflows(&cashflows).map_err(core_to_py)
    })
}

/// Register performance measurement functions on the portfolio submodule.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(twrr_modified_dietz, m)?)?;
    m.add_function(wrap_pyfunction!(twrr_linked, m)?)?;
    m.add_function(wrap_pyfunction!(mwr_xirr, m)?)?;
    Ok(())
}
