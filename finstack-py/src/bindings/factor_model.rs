//! Python bindings for the `finstack-factor-model` crate.
//!
//! The module mirrors the Rust crate boundary. Credit hierarchy bindings are
//! registered under `finstack.factor_model.credit`.

use pyo3::prelude::*;
use pyo3::types::PyList;

use crate::bindings::valuations::credit_factor_model;

/// Register the `factor_model` Python domain.
pub fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(py, "factor_model")?;
    m.setattr(
        "__doc__",
        "Factor-model primitives, credit calibration, and decomposition.",
    )?;

    let credit = PyModule::new(py, "credit")?;
    credit.setattr(
        "__doc__",
        "Credit factor hierarchy artifacts, calibration, and decomposition.",
    )?;
    credit_factor_model::register(py, &credit)?;

    let credit_all = PyList::new(
        py,
        [
            "CreditFactorModel",
            "CreditCalibrator",
            "LevelsAtDate",
            "PeriodDecomposition",
            "FactorCovarianceForecast",
            "decompose_levels",
            "decompose_period",
        ],
    )?;
    credit.setattr("__all__", credit_all)?;
    m.add_submodule(&credit)?;
    m.setattr("credit", &credit)?;

    let all = PyList::new(py, ["credit"])?;
    m.setattr("__all__", all)?;
    parent.add_submodule(&m)?;

    let parent_name: String = match parent.getattr("__name__") {
        Ok(attr) => match attr.extract::<String>() {
            Ok(s) => s,
            Err(_) => "finstack.finstack".to_string(),
        },
        Err(_) => "finstack.finstack".to_string(),
    };
    let qual = format!("{parent_name}.factor_model");
    let credit_qual = format!("{qual}.credit");
    m.setattr("__package__", &qual)?;
    credit.setattr("__package__", &credit_qual)?;
    let sys = PyModule::import(py, "sys")?;
    let modules = sys.getattr("modules")?;
    modules.set_item(&qual, &m)?;
    modules.set_item(&credit_qual, &credit)?;

    Ok(())
}
