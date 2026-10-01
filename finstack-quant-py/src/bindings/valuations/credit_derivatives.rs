//! CDS-family instrument example payloads.
//!
//! Mirrors `finstack-quant-wasm/src/api/valuations/credit_derivatives.rs`
//! (exposed on the JS facade as `valuations.creditDerivatives`).
//!
//! Pricing / validation / serialization for CDS instruments is provided by
//! the generic `price_instrument` and
//! `validate_instrument_json` entry points under
//! `finstack_quant.valuations.instruments`; this module only owns the
//! example-payload factories that produce canonical v1 instrument envelopes.

use crate::errors::core_to_py;
use finstack_quant_valuations::instruments::credit_derivatives::cds::CreditDefaultSwap;
use finstack_quant_valuations::instruments::credit_derivatives::cds_index::CdsIndex;
use finstack_quant_valuations::instruments::credit_derivatives::cds_option::CdsOption;
use finstack_quant_valuations::instruments::credit_derivatives::cds_tranche::CdsTranche;
use finstack_quant_valuations::instruments::InstrumentJson;
use pyo3::prelude::*;
use pyo3::types::{PyList, PyModule};

use super::instruments::serialize_typed_instrument_json;

/// Example ``CreditDefaultSwap`` instrument envelope.
///
/// Returns
/// -------
/// str
///     Canonical ``finstack_quant.instrument/1`` envelope accepted by
///     ``validate_instrument_json`` and ``price_instrument``.
#[pyfunction]
#[pyo3(text_signature = "()")]
fn credit_default_swap_example_json() -> PyResult<String> {
    serialize_typed_instrument_json(
        InstrumentJson::CreditDefaultSwap(CreditDefaultSwap::example().map_err(core_to_py)?),
        "CreditDefaultSwap example",
    )
}

/// Example ``CdsIndex`` instrument envelope.
///
/// Returns
/// -------
/// str
///     Canonical ``finstack_quant.instrument/1`` envelope accepted by
///     ``validate_instrument_json`` and ``price_instrument``.
#[pyfunction]
#[pyo3(text_signature = "()")]
fn cds_index_example_json() -> PyResult<String> {
    serialize_typed_instrument_json(
        InstrumentJson::CdsIndex(CdsIndex::example().map_err(core_to_py)?),
        "CdsIndex example",
    )
}

/// Example ``CdsTranche`` instrument envelope.
///
/// Returns
/// -------
/// str
///     Canonical ``finstack_quant.instrument/1`` envelope accepted by
///     ``validate_instrument_json`` and ``price_instrument``.
#[pyfunction]
#[pyo3(text_signature = "()")]
fn cds_tranche_example_json() -> PyResult<String> {
    serialize_typed_instrument_json(
        InstrumentJson::CdsTranche(CdsTranche::example().map_err(core_to_py)?),
        "CdsTranche example",
    )
}

/// Example ``CdsOption`` instrument envelope.
///
/// Returns
/// -------
/// str
///     Canonical ``finstack_quant.instrument/1`` envelope accepted by
///     ``validate_instrument_json`` and ``price_instrument``.
#[pyfunction]
#[pyo3(text_signature = "()")]
fn cds_option_example_json() -> PyResult<String> {
    let option = CdsOption::example().map_err(core_to_py)?;
    serialize_typed_instrument_json(InstrumentJson::CdsOption(option), "CdsOption example")
}

pub(super) fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = crate::bindings::module_utils::new_submodule(parent, "credit_derivatives")?;
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
    crate::bindings::module_utils::attach_submodule(
        parent,
        &m,
        crate::bindings::module_utils::Exposure::Python,
    )?;
    Ok(())
}
