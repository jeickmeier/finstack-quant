//! Python bindings for the `finstack-quant-cashflows` crate.

pub(crate) mod accrual;
pub(crate) mod aggregation;
pub(crate) mod builder;
pub(crate) mod fixings;
mod schedule_json;
mod schema;

use crate::bindings::pydoc::pydoc;
use pyo3::prelude::*;
use pyo3::types::{PyList, PyModule};
use schedule_json::{
    PyScheduleBuildOpts, build_cashflow_schedule, dated_flows, schedule_from_classified_flows,
    schedule_from_dated_flows, validate_cashflow_schedule_json,
};

/// Register the `finstack_quant.cashflows` Python namespace.
pub fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = crate::bindings::module_utils::new_submodule(parent, "cashflows")?;
    m.setattr("__doc__", pydoc!("finstack_quant.cashflows", "__module__"))?;

    builder::register(py, &m)?;
    accrual::register(py, &m)?;
    aggregation::register(py, &m)?;
    schema::register(py, &m)?;
    fixings::register(py, &m)?;

    m.add_class::<PyScheduleBuildOpts>()?;
    m.add_function(wrap_pyfunction!(build_cashflow_schedule, &m)?)?;
    m.add_function(wrap_pyfunction!(dated_flows, &m)?)?;
    m.add_function(wrap_pyfunction!(schedule_from_classified_flows, &m)?)?;
    m.add_function(wrap_pyfunction!(schedule_from_dated_flows, &m)?)?;
    m.add_function(wrap_pyfunction!(validate_cashflow_schedule_json, &m)?)?;

    let all = PyList::new(
        py,
        [
            "ScheduleBuildOpts",
            "accrual",
            "aggregation",
            "build_cashflow_schedule",
            "builder",
            "dated_flows",
            "fixings",
            "schedule_from_classified_flows",
            "schedule_from_dated_flows",
            "schema",
            "validate_cashflow_schedule_json",
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
