//! Python bindings for the `finstack-quant-factor-model` crate.
//!
//! The module mirrors the Rust crate boundary. Credit hierarchy bindings are
//! registered under `finstack_quant.factor_model.credit`.

use pyo3::prelude::*;
use pyo3::types::PyList;

pub(crate) mod credit;

/// Register the `factor_model` Python domain.
pub fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(py, "factor_model")?;
    let qual = crate::bindings::module_utils::set_submodule_package(
        parent,
        &m,
        "factor_model",
        crate::bindings::module_utils::ROOT_PACKAGE,
        crate::bindings::module_utils::ParentNameSource::Name,
    )?;
    m.setattr(
        "__doc__",
        "Factor-model primitives, credit calibration, and decomposition.",
    )?;

    let credit = PyModule::new(py, "credit")?;
    let credit_qual = crate::bindings::module_utils::set_submodule_package_by_package(
        &m, &credit, "credit", &qual,
    )?;
    credit.setattr(
        "__doc__",
        "Credit factor hierarchy artifacts, calibration, and decomposition.",
    )?;
    credit::register(py, &credit)?;

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
    crate::bindings::module_utils::register_submodule_at(py, &m, &credit, &credit_qual)?;

    let all = PyList::new(py, ["credit"])?;
    m.setattr("__all__", all)?;
    crate::bindings::module_utils::register_submodule_at(py, parent, &m, &qual)?;

    Ok(())
}
