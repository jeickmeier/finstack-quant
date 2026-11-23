//! Python bindings for structural credit model specifications.

mod lgd;
mod liability_management;
mod migration;
mod pd;
mod recovery_waterfall;
mod scoring;
pub(crate) use scoring::PyScoringResult;
mod merton;
mod paths;
mod rating;
mod recovery_hazard;
mod toggle;

use crate::bindings::pydoc::pydoc;
pub(crate) use merton::PyMertonModel;
use merton::{PyAssetDynamics, PyBarrierType, PyMertonReport, merton_report};
use paths::PySimulatedPaths;
use pyo3::prelude::*;
use pyo3::types::{PyList, PyModule};
use rating::{PyRatingFactorTable, moodys_warf_factor};
pub(crate) use recovery_hazard::{PyDynamicRecoverySpec, PyEndogenousHazardSpec};
use toggle::PyCreditState;
pub(crate) use toggle::PyToggleExerciseModel;

pub(crate) fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let module = crate::bindings::module_utils::new_submodule(parent, "credit")?;
    module.setattr(
        "__doc__",
        pydoc!("finstack_quant.models.credit", "__module__"),
    )?;
    module.add_class::<PyAssetDynamics>()?;
    module.add_class::<PyBarrierType>()?;
    module.add_class::<PyMertonModel>()?;
    module.add_class::<PyMertonReport>()?;
    module.add_class::<PySimulatedPaths>()?;
    module.add_class::<PyDynamicRecoverySpec>()?;
    module.add_class::<PyEndogenousHazardSpec>()?;
    module.add_class::<PyCreditState>()?;
    module.add_class::<PyToggleExerciseModel>()?;
    module.add_class::<PyRatingFactorTable>()?;
    module.add_function(wrap_pyfunction!(merton_report, &module)?)?;
    module.add_function(wrap_pyfunction!(moodys_warf_factor, &module)?)?;
    scoring::register(py, &module)?;
    pd::register(py, &module)?;
    lgd::register(py, &module)?;
    migration::register(py, &module)?;
    recovery_waterfall::register(py, &module)?;
    liability_management::register(py, &module)?;
    let all = PyList::new(
        py,
        [
            "AssetDynamics",
            "CreditState",
            "DynamicRecoverySpec",
            "EndogenousHazardSpec",
            "MertonBarrierType",
            "MertonModel",
            "MertonReport",
            "RatingFactorTable",
            "SimulatedPaths",
            "ToggleExerciseModel",
            "lgd",
            "liability_management",
            "migration",
            "merton_report",
            "moodys_warf_factor",
            "pd",
            "recovery_waterfall",
            "scoring",
        ],
    )?;
    module.setattr("__all__", all)?;
    crate::bindings::module_utils::attach_submodule(
        parent,
        &module,
        crate::bindings::module_utils::Exposure::Python,
    )?;
    Ok(())
}
