//! Raw fixing observations and atomic realized-forward materialization.

use crate::bindings::pydoc::pydoc;
use finstack_quant_cashflows::fixings::ProjectedFixing;
use pyo3::prelude::*;
use pyo3::types::PyList;

use super::builder::schedule::PyCashFlowSchedule;
use crate::bindings::core::market_data::context::PyMarketContext;
use crate::bindings::date_utils::{date_to_py, py_to_date};
use crate::errors::core_to_py;

#[doc = pydoc!("finstack_quant.cashflows.fixings", "ProjectedFixing")]
#[pyclass(
    name = "ProjectedFixing",
    module = "finstack_quant.cashflows.fixings",
    frozen,
    from_py_object
)]
#[derive(Clone)]
pub struct PyProjectedFixing {
    pub(crate) inner: ProjectedFixing,
}

#[pymethods]
impl PyProjectedFixing {
    #[doc = pydoc!("finstack_quant.cashflows.fixings", "ProjectedFixing.__init__")]
    #[new]
    #[pyo3(signature = (series_id, date, value=None))]
    fn new(series_id: String, date: &Bound<'_, PyAny>, value: Option<f64>) -> PyResult<Self> {
        Ok(Self {
            inner: ProjectedFixing {
                series_id,
                date: py_to_date(date)?,
                value,
            },
        })
    }

    #[doc = pydoc!("finstack_quant.cashflows.fixings", "ProjectedFixing.series_id")]
    #[getter]
    fn series_id(&self) -> &str {
        &self.inner.series_id
    }

    #[doc = pydoc!("finstack_quant.cashflows.fixings", "ProjectedFixing.date")]
    #[getter]
    fn date<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.date)
    }

    #[doc = pydoc!("finstack_quant.cashflows.fixings", "ProjectedFixing.value")]
    #[getter]
    fn value(&self) -> Option<f64> {
        self.inner.value
    }
}

#[doc = pydoc!("finstack_quant.cashflows.fixings", "materialize_fixings")]
#[pyfunction]
fn materialize_fixings(
    market: &PyMarketContext,
    schedules: Vec<PyRef<'_, PyCashFlowSchedule>>,
    old_date: &Bound<'_, PyAny>,
    new_date: &Bound<'_, PyAny>,
) -> PyResult<PyMarketContext> {
    finstack_quant_cashflows::fixings::materialize_fixings(
        &market.inner,
        schedules.iter().map(|schedule| &schedule.inner),
        py_to_date(old_date)?,
        py_to_date(new_date)?,
    )
    .map(PyMarketContext::from_inner)
    .map_err(core_to_py)
}

pub(crate) fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let module = crate::bindings::module_utils::new_submodule(parent, "fixings")?;
    module.setattr(
        "__doc__",
        "Projected index fixings recorded on schedules and their materialization on a market roll.",
    )?;
    module.add_class::<PyProjectedFixing>()?;
    module.add_function(wrap_pyfunction!(materialize_fixings, &module)?)?;
    module.setattr(
        "__all__",
        PyList::new(py, ["ProjectedFixing", "materialize_fixings"])?,
    )?;
    crate::bindings::module_utils::attach_submodule(
        parent,
        &module,
        crate::bindings::module_utils::Exposure::Compiled,
    )
}
