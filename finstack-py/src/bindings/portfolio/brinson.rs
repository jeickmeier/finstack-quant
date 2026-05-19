//! Python bindings for Brinson-Fachler attribution.
//!
//! Inputs and outputs are JSON strings matching the Rust `serde` shapes so the
//! binding layer stays a conversion shim around the canonical Rust analytics.

use pyo3::prelude::*;
use pyo3::types::PyModule;

use crate::errors::{portfolio_to_py, serde_json_to_py};

/// Compute a single-period Brinson-Fachler attribution from sector JSON.
///
/// Parameters
/// ----------
/// sectors_json : str
///     JSON array of ``SectorPeriod`` objects with ``sector``,
///     ``portfolio_weight``, ``benchmark_weight``, ``portfolio_return``, and
///     ``benchmark_return`` fields.
///
/// Returns
/// -------
/// str
///     JSON-serialized ``BrinsonPeriodResult``.
#[pyfunction]
#[pyo3(text_signature = "(sectors_json)")]
fn brinson_fachler(sectors_json: &str) -> PyResult<String> {
    let sectors: Vec<finstack_portfolio::SectorPeriod> = serde_json::from_str(sectors_json)
        .map_err(|err| serde_json_to_py(err, "invalid Brinson sectors JSON"))?;
    let result = finstack_portfolio::brinson_fachler(&sectors).map_err(portfolio_to_py)?;
    serde_json::to_string(&result).map_err(|err| serde_json_to_py(err, "serialize Brinson result"))
}

/// Compute Carino-linked multi-period Brinson attribution from period JSON.
///
/// Parameters
/// ----------
/// periods_json : str
///     JSON array of periods, where each period is an array of ``SectorPeriod``
///     objects.
///
/// Returns
/// -------
/// str
///     JSON-serialized ``CarinoLinkedAttribution``.
#[pyfunction]
#[pyo3(text_signature = "(periods_json)")]
fn carino_link(periods_json: &str) -> PyResult<String> {
    let periods: Vec<Vec<finstack_portfolio::SectorPeriod>> = serde_json::from_str(periods_json)
        .map_err(|err| serde_json_to_py(err, "invalid Carino periods JSON"))?;
    let result =
        finstack_portfolio::carino_link_from_sector_periods(&periods).map_err(portfolio_to_py)?;
    serde_json::to_string(&result).map_err(|err| serde_json_to_py(err, "serialize Carino result"))
}

/// Register Brinson attribution functions on the portfolio submodule.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(brinson_fachler, m)?)?;
    m.add_function(wrap_pyfunction!(carino_link, m)?)?;
    Ok(())
}
