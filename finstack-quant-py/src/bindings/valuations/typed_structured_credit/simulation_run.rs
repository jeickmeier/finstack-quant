//! Complete result of a deterministic structured-credit simulation.
use super::{PySimulationDiagnostics, PyTrancheCashflows};
use finstack_quant_valuations::instruments::fixed_income::structured_credit::SimulationRun;
use pyo3::prelude::*;

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.simulation_run", "SimulationRun")]
#[pyclass(
    module = "finstack_quant.valuations.instruments",
    name = "SimulationRun",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PySimulationRun {
    /// Canonical Rust simulation result.
    pub(crate) inner: SimulationRun,
}
crate::bindings::macros::wire_methods!(
    PySimulationRun,
    SimulationRun,
    "SimulationRun",
    module = "finstack_quant.valuations.instruments._typed_structured_credit.simulation_run"
);
#[pymethods]
impl PySimulationRun {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.simulation_run", "SimulationRun.tranches")]
    #[getter]
    fn tranches(&self) -> std::collections::HashMap<String, PyTrancheCashflows> {
        self.inner
            .tranches
            .iter()
            .map(|(id, flows)| {
                (
                    id.clone(),
                    PyTrancheCashflows {
                        inner: flows.clone(),
                    },
                )
            })
            .collect()
    }
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.simulation_run", "SimulationRun.diagnostics")]
    #[getter]
    fn diagnostics(&self) -> PySimulationDiagnostics {
        PySimulationDiagnostics {
            inner: self.inner.diagnostics.clone(),
        }
    }
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.simulation_run", "SimulationRun.to_dict")]
    fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::pandas_utils::serde_to_py(py, &self.inner)
    }
}
