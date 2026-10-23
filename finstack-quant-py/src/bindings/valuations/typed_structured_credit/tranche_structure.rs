use pyo3::prelude::*;

use crate::bindings::core::money::PyMoney;
use crate::bindings::valuations::convert::money_to_py;
use crate::errors::core_to_py;
use finstack_quant_valuations::instruments::fixed_income::structured_credit::{
    Tranche, TrancheStructure,
};

use super::PyTranche;

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "TrancheStructure")]
#[pyclass(
    module = "finstack_quant.valuations.instruments",
    name = "TrancheStructure",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyTrancheStructure {
    /// Inner canonical Rust tranche structure.
    pub(crate) inner: TrancheStructure,
}

#[pymethods]
impl PyTrancheStructure {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "TrancheStructure.__init__")]
    #[new]
    #[pyo3(text_signature = "(tranches)")]
    fn new(tranches: Vec<PyRef<'_, PyTranche>>) -> PyResult<Self> {
        let tranches: Vec<Tranche> = tranches.iter().map(|t| t.inner.clone()).collect();
        let inner = TrancheStructure::new(tranches).map_err(core_to_py)?;
        Ok(Self { inner })
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "TrancheStructure.from_balances")]
    #[staticmethod]
    #[pyo3(text_signature = "(tranches)")]
    fn from_balances(tranches: Vec<PyRef<'_, PyTranche>>) -> PyResult<Self> {
        let inner = TrancheStructure::from_balances(
            tranches
                .iter()
                .map(|t| t.inner.clone())
                .collect::<Vec<Tranche>>(),
        )
        .map_err(core_to_py)?;
        Ok(Self { inner })
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "TrancheStructure.from_json")]
    #[staticmethod]
    #[pyo3(text_signature = "(json)")]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner = serde_json::from_str(json)
            .map_err(|err| crate::errors::serde_json_to_py(err, "invalid TrancheStructure JSON"))?;
        Ok(Self { inner })
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "TrancheStructure.to_json")]
    #[pyo3(text_signature = "($self)")]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(crate::errors::display_to_py)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "TrancheStructure.to_dict")]
    #[pyo3(text_signature = "($self)")]
    fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::pandas_utils::serde_to_py(py, &self.inner)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "TrancheStructure.__reduce__")]
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "TrancheStructure.tranches")]
    #[getter]
    fn tranches(&self) -> Vec<PyTranche> {
        self.inner
            .tranches
            .iter()
            .map(|t| PyTranche { inner: t.clone() })
            .collect()
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> PyResult<String> {
        Ok(format!(
            "TrancheStructure(tranches={}, total_size={})",
            self.inner.tranches.len(),
            self.total_size()?.inner.amount()
        ))
    }
}

crate::bindings::macros::py_getters!(PyTrancheStructure |s| {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "TrancheStructure.total_size")]
    total_size -> PyResult<PyMoney> =
        s.inner.total_size().map(money_to_py).map_err(crate::errors::core_to_py);
});
