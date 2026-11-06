//! Inputs for the canonical standard-deal constructor.
use super::{PyAssetPool, PyTrancheStructure};
use crate::bindings::date_utils::date_to_py;
use finstack_quant_valuations::instruments::fixed_income::structured_credit::StandardDealParams;
use pyo3::prelude::*;

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.standard_deal_params", "StandardDealParams")]
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
    module = "finstack_quant.valuations.instruments._typed_structured_credit.standard_deal_params"
);
#[pymethods]
impl PyStandardDealParams {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.standard_deal_params", "StandardDealParams.to_dict")]
    fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::pandas_utils::serde_to_py(py, &self.inner)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.standard_deal_params", "StandardDealParams.__init__")]
    #[new]
    #[pyo3(
        signature = (**fields),
        text_signature = "(*, id, pool, tranches, closing_date, maturity, discount_curve_id, calendar_id=None)"
    )]
    fn new(py: Python<'_>, fields: Option<&Bound<'_, pyo3::types::PyDict>>) -> PyResult<Self> {
        let inner: StandardDealParams =
            crate::bindings::module_utils::fields_from_py(py, fields, "StandardDealParams")?;
        Ok(Self { inner })
    }
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.standard_deal_params", "StandardDealParams.id")]
    #[getter]
    fn id(&self) -> String {
        self.inner.id.to_string()
    }
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.standard_deal_params", "StandardDealParams.pool")]
    #[getter]
    fn pool(&self) -> PyAssetPool {
        PyAssetPool {
            inner: self.inner.pool.clone(),
        }
    }
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.standard_deal_params", "StandardDealParams.tranches")]
    #[getter]
    fn tranches(&self) -> PyTrancheStructure {
        PyTrancheStructure {
            inner: self.inner.tranches.clone(),
        }
    }
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.standard_deal_params", "StandardDealParams.closing_date")]
    #[getter]
    fn closing_date<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.closing_date)
    }
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.standard_deal_params", "StandardDealParams.maturity")]
    #[getter]
    fn maturity<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.maturity)
    }
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.standard_deal_params", "StandardDealParams.discount_curve_id")]
    #[getter]
    fn discount_curve_id(&self) -> String {
        self.inner.discount_curve_id.to_string()
    }
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.standard_deal_params", "StandardDealParams.calendar_id")]
    #[getter]
    fn calendar_id(&self) -> Option<String> {
        self.inner.calendar_id.as_ref().map(ToString::to_string)
    }
}
