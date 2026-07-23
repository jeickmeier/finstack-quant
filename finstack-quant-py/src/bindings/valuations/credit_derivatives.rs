//! CDS-family instrument example payloads.
//!
//! Mirrors `finstack-quant-wasm/src/api/valuations/credit_derivatives.rs`
//! (exposed on the JS facade as `valuations.creditDerivatives`).
//!
//! Pricing / validation / serialization for CDS instruments is provided by
//! the generic `price_instrument`, `price_instrument_with_metrics`, and
//! `validate_instrument_json` entry points under
//! `finstack_quant.valuations.instruments`; this module only owns the
//! example-payload factories that produce canonical tagged instrument JSON.

use crate::errors::display_to_py;
use finstack_quant_valuations::instruments::credit_derivatives::cds::CreditDefaultSwap;
use finstack_quant_valuations::instruments::credit_derivatives::cds_index::CDSIndex;
use finstack_quant_valuations::instruments::credit_derivatives::cds_option::CDSOption;
use finstack_quant_valuations::instruments::credit_derivatives::cds_tranche::CDSTranche;
use finstack_quant_valuations::instruments::InstrumentJson;
use pyo3::prelude::*;
use pyo3::types::{PyList, PyModule};

/// Example tagged ``CreditDefaultSwap`` instrument JSON.
///
/// Returns
/// -------
/// str
///     Tagged instrument JSON (``{"type": "credit_default_swap", ...}``)
///     accepted by ``validate_instrument_json`` and ``price_instrument``.
#[pyfunction]
#[pyo3(text_signature = "()")]
fn credit_default_swap_example_json() -> PyResult<String> {
    serde_json::to_string(&InstrumentJson::CreditDefaultSwap(
        CreditDefaultSwap::example(),
    ))
    .map_err(display_to_py)
}

/// Example tagged ``CDSIndex`` instrument JSON.
///
/// Returns
/// -------
/// str
///     Tagged instrument JSON (``{"type": "cds_index", ...}``) accepted by
///     ``validate_instrument_json`` and ``price_instrument``.
#[pyfunction]
#[pyo3(text_signature = "()")]
fn cds_index_example_json() -> PyResult<String> {
    serde_json::to_string(&InstrumentJson::CDSIndex(CDSIndex::example())).map_err(display_to_py)
}

/// Example tagged ``CDSTranche`` instrument JSON.
///
/// Returns
/// -------
/// str
///     Tagged instrument JSON (``{"type": "cds_tranche", ...}``) accepted by
///     ``validate_instrument_json`` and ``price_instrument``.
#[pyfunction]
#[pyo3(text_signature = "()")]
fn cds_tranche_example_json() -> PyResult<String> {
    serde_json::to_string(&InstrumentJson::CDSTranche(CDSTranche::example())).map_err(display_to_py)
}

/// Example tagged ``CDSOption`` instrument JSON.
///
/// Returns
/// -------
/// str
///     Tagged instrument JSON (``{"type": "cds_option", ...}``) accepted by
///     ``validate_instrument_json`` and ``price_instrument``.
#[pyfunction]
#[pyo3(text_signature = "()")]
fn cds_option_example_json() -> PyResult<String> {
    let option = CDSOption::example().map_err(display_to_py)?;
    serde_json::to_string(&InstrumentJson::CDSOption(option)).map_err(display_to_py)
}

pub(super) fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(py, "credit_derivatives")?;
    let qual = crate::bindings::module_utils::set_submodule_package_by_package(
        parent,
        &m,
        "credit_derivatives",
        "finstack_quant.finstack_quant.valuations",
    )?;
    m.setattr(
        "__doc__",
        "Canonical example payloads for CDS-family instruments (CDS, index, tranche, option).",
    )?;

    m.add_function(wrap_pyfunction!(credit_default_swap_example_json, &m)?)?;
    m.add_function(wrap_pyfunction!(cds_index_example_json, &m)?)?;
    m.add_function(wrap_pyfunction!(cds_tranche_example_json, &m)?)?;
    m.add_function(wrap_pyfunction!(cds_option_example_json, &m)?)?;

    let all = PyList::new(
        py,
        [
            "cds_index_example_json",
            "cds_option_example_json",
            "cds_tranche_example_json",
            "credit_default_swap_example_json",
        ],
    )?;
    m.setattr("__all__", all)?;
    crate::bindings::module_utils::register_submodule_at(py, parent, &m, &qual)?;
    Ok(())
}
