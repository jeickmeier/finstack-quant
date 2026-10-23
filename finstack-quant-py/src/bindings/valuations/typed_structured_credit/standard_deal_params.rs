//! Inputs for the canonical standard-deal constructor.
use super::{PyAssetPool, PyTrancheStructure};
use crate::bindings::date_utils::{date_to_py, py_to_date};
use finstack_quant_valuations::instruments::fixed_income::structured_credit::StandardDealParams;
use pyo3::prelude::*;

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "StandardDealParams")]
#[pyclass(
    module = "finstack_quant.valuations.instruments",
    name = "StandardDealParams",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyStandardDealParams {
    /// Canonical Rust inputs.
    pub(crate) inner: StandardDealParams,
}
crate::bindings::macros::wire_methods!(
    PyStandardDealParams,
    StandardDealParams,
    "StandardDealParams",
    module = "finstack_quant.valuations.instruments"
);
#[pymethods]
impl PyStandardDealParams {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "StandardDealParams.to_dict")]
    fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::pandas_utils::serde_to_py(py, &self.inner)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "StandardDealParams.__init__")]
    #[new]
    #[pyo3(signature = (id, pool, tranches, closing_date, maturity, discount_curve_id, calendar_id=None))]
    #[allow(clippy::too_many_arguments)]
    fn new(
        id: &str,
        pool: PyRef<'_, PyAssetPool>,
        tranches: PyRef<'_, PyTrancheStructure>,
        closing_date: &Bound<'_, PyAny>,
        maturity: &Bound<'_, PyAny>,
        discount_curve_id: &str,
        calendar_id: Option<&str>,
    ) -> PyResult<Self> {
        Ok(Self {
            inner: StandardDealParams {
                id: id.into(),
                pool: pool.inner.clone(),
                tranches: tranches.inner.clone(),
                closing_date: py_to_date(closing_date)?,
                maturity: py_to_date(maturity)?,
                discount_curve_id: discount_curve_id.into(),
                calendar_id: calendar_id.map(Into::into),
            },
        })
    }
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "StandardDealParams.id")]
    #[getter]
    fn id(&self) -> String {
        self.inner.id.to_string()
    }
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "StandardDealParams.pool")]
    #[getter]
    fn pool(&self) -> PyAssetPool {
        PyAssetPool {
            inner: self.inner.pool.clone(),
        }
    }
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "StandardDealParams.tranches")]
    #[getter]
    fn tranches(&self) -> PyTrancheStructure {
        PyTrancheStructure {
            inner: self.inner.tranches.clone(),
        }
    }
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "StandardDealParams.closing_date")]
    #[getter]
    fn closing_date<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.closing_date)
    }
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "StandardDealParams.maturity")]
    #[getter]
    fn maturity<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.maturity)
    }
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "StandardDealParams.discount_curve_id")]
    #[getter]
    fn discount_curve_id(&self) -> String {
        self.inner.discount_curve_id.to_string()
    }
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "StandardDealParams.calendar_id")]
    #[getter]
    fn calendar_id(&self) -> Option<String> {
        self.inner.calendar_id.as_ref().map(ToString::to_string)
    }
}
