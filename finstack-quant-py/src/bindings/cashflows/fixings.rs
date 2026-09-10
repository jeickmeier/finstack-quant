//! Raw fixing observations and atomic realized-forward materialization.

use finstack_quant_cashflows::fixings::ProjectedFixing;
use pyo3::prelude::*;
use pyo3::types::PyList;

use super::builder::schedule::PyCashFlowSchedule;
use crate::bindings::core::market_data::context::PyMarketContext;
use crate::bindings::date_utils::{date_to_py, extract_date};
use crate::errors::core_to_py;

/// Raw rate or FX observation retained by canonical coupon projection.
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
    /// Construct a raw observation; index rates exclude coupon spread and gearing.
    #[new]
    #[pyo3(signature = (series_id, date, value=None))]
    fn new(series_id: String, date: &Bound<'_, PyAny>, value: Option<f64>) -> PyResult<Self> {
        Ok(Self {
            inner: ProjectedFixing {
                series_id,
                date: extract_date(date)?,
                value,
            },
        })
    }

    /// Canonical FIXING-prefixed series identifier, including FX quote orientation.
    #[getter]
    fn series_id(&self) -> &str {
        &self.inner.series_id
    }

    /// Contractual observation date after fixing-calendar adjustments.
    #[getter]
    fn date<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.date)
    }

    /// Raw annualized decimal rate or oriented FX rate; None means unavailable.
    #[getter]
    fn value(&self) -> Option<f64> {
        self.inner.value
    }
}

/// Materialize crossed observations in a new market, preserving existing fixings.
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
        extract_date(old_date)?,
        extract_date(new_date)?,
    )
    .map(PyMarketContext::from_inner)
    .map_err(core_to_py)
}

pub(crate) fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let module = PyModule::new(py, "fixings")?;
    module.setattr("__package__", "finstack_quant.cashflows.fixings")?;
    module.add_class::<PyProjectedFixing>()?;
    module.add_function(wrap_pyfunction!(materialize_fixings, &module)?)?;
    module.setattr(
        "__all__",
        PyList::new(py, ["ProjectedFixing", "materialize_fixings"])?,
    )?;
    parent.add_submodule(&module)
}
