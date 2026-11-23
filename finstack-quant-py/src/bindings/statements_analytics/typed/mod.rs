//! Typed Python wrappers for statement-analysis configs and root results.

mod config;
mod results;

pub use config::{PyParameterSpec, PyScenarioSet, PySensitivityConfig, PyVarianceConfig};
use pyo3::prelude::*;
pub use results::{
    PyBridgeChart, PyBridgeStep, PyScenarioDiff, PyScenarioResults, PySensitivityResult,
    PyTornadoEntry, PyVarianceReport, PyVarianceRow,
};

pub fn register(_py: Python<'_>, module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyParameterSpec>()?;
    module.add_class::<PySensitivityConfig>()?;
    module.add_class::<PyVarianceConfig>()?;
    module.add_class::<PyScenarioSet>()?;
    module.add_class::<PySensitivityResult>()?;
    module.add_class::<PyTornadoEntry>()?;
    module.add_class::<PyVarianceRow>()?;
    module.add_class::<PyVarianceReport>()?;
    module.add_class::<PyScenarioResults>()?;
    module.add_class::<PyScenarioDiff>()?;
    module.add_class::<PyBridgeStep>()?;
    module.add_class::<PyBridgeChart>()?;
    Ok(())
}
