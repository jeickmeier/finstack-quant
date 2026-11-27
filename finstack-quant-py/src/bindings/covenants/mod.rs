//! Python bindings for the `finstack-quant-covenants` crate.
//!
//! Typed covenant definitions (`Covenant`, `CovenantType`, `CovenantSpec`,
//! `ThresholdSchedule`, `CovenantWaiver`, ...), the `CovenantEngine`
//! evaluator, template packages, DataFrame-backed breach forecasting, and the
//! JSON validators / `_json` template twins shared with WASM.

mod engine;
mod forecast;
mod report;
mod spec;

pub(crate) use engine::{PyCovenantBreach, PyCovenantEngine, dated_reports_table};
pub(crate) use forecast::{PyCovenantForecast, PyCovenantForecastConfig, PyFutureBreach};
pub(crate) use report::PyCovenantReport;
pub(crate) use spec::{
    PyCovenant, PyCovenantConsequence, PyCovenantSpec, PyCovenantType, PyCovenantWaiver,
    PySpringingCondition, PyThresholdSchedule,
};

use pyo3::prelude::*;
use pyo3::types::{PyList, PyModule};
mod operations;

use operations::{
    validate_covenant_engine_json, validate_covenant_report_json, validate_covenant_spec_json,
};

/// Register the `finstack_quant.covenants` Python namespace.
pub fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = crate::bindings::module_utils::new_submodule(parent, "covenants")?;
    m.setattr(
        "__doc__",
        "Typed covenant definitions, the CovenantEngine evaluator, CovenantTemplate packages, \
         DataFrame-backed breach forecasting, and the JSON validators.",
    )?;

    m.add_class::<PyCovenant>()?;
    m.add_class::<PyCovenantBreach>()?;
    m.add_class::<PyCovenantConsequence>()?;
    m.add_class::<PyCovenantEngine>()?;
    m.add_class::<PyCovenantForecast>()?;
    m.add_class::<PyCovenantForecastConfig>()?;
    m.add_class::<PyCovenantReport>()?;
    m.add_class::<PyCovenantSpec>()?;
    m.add_class::<engine::PyCovenantTemplate>()?;
    m.add_class::<PyCovenantType>()?;
    m.add_class::<PyCovenantWaiver>()?;
    m.add_class::<PyFutureBreach>()?;
    m.add_class::<PySpringingCondition>()?;
    m.add_class::<PyThresholdSchedule>()?;
    m.add_function(wrap_pyfunction!(forecast::breaches_table, &m)?)?;
    m.add_function(wrap_pyfunction!(engine::covenant_template, &m)?)?;
    m.add_function(wrap_pyfunction!(forecast::forecast_breaches, &m)?)?;
    m.add_function(wrap_pyfunction!(forecast::forecast_covenant, &m)?)?;
    m.add_function(wrap_pyfunction!(engine::reports_table, &m)?)?;
    m.add_function(wrap_pyfunction!(validate_covenant_engine_json, &m)?)?;
    m.add_function(wrap_pyfunction!(validate_covenant_report_json, &m)?)?;
    m.add_function(wrap_pyfunction!(validate_covenant_spec_json, &m)?)?;

    let all = PyList::new(
        py,
        [
            "Covenant",
            "CovenantBreach",
            "CovenantConsequence",
            "CovenantEngine",
            "CovenantForecast",
            "CovenantForecastConfig",
            "CovenantReport",
            "CovenantSpec",
            "CovenantTemplate",
            "CovenantType",
            "CovenantWaiver",
            "FutureBreach",
            "SpringingCondition",
            "ThresholdSchedule",
            "breaches_table",
            "covenant_template",
            "forecast_breaches",
            "forecast_covenant",
            "reports_table",
            "validate_covenant_engine_json",
            "validate_covenant_report_json",
            "validate_covenant_spec_json",
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
