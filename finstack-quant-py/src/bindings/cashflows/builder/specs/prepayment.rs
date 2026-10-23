//! Spec-type bindings for `finstack_quant_cashflows::builder`.

use crate::bindings::macros::wire_methods;
use crate::bindings::pydoc::pydoc;
use finstack_quant_cashflows::builder::PrepaymentModelSpec;
use pyo3::prelude::*;

use crate::bindings::pandas_utils::serde_to_py;
use crate::bindings::repr_support::repr_from_serde;
use crate::errors::core_to_py;

#[doc = pydoc!("finstack_quant.cashflows.builder", "PrepaymentModelSpec")]
#[pyclass(
    name = "PrepaymentModelSpec",
    module = "finstack_quant.cashflows.builder",
    frozen,
    skip_from_py_object
)]
#[derive(Clone, Debug)]
pub struct PyPrepaymentModelSpec {
    /// Inner prepayment model.
    pub(crate) inner: PrepaymentModelSpec,
}

wire_methods!(
    PyPrepaymentModelSpec,
    PrepaymentModelSpec,
    "PrepaymentModelSpec",
    module = "finstack_quant.cashflows.builder",
    validate
);

#[pymethods]
impl PyPrepaymentModelSpec {
    #[doc = pydoc!("finstack_quant.cashflows.builder", "PrepaymentModelSpec.constant_cpr")]
    #[staticmethod]
    #[pyo3(text_signature = "(cpr)")]
    fn constant_cpr(cpr: f64) -> PyResult<Self> {
        let inner = PrepaymentModelSpec::constant_cpr(cpr);
        inner.validate().map_err(core_to_py)?;
        Ok(Self { inner })
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder", "PrepaymentModelSpec.psa")]
    #[staticmethod]
    #[pyo3(text_signature = "(speed_multiplier)")]
    fn psa(speed_multiplier: f64) -> PyResult<Self> {
        let inner = PrepaymentModelSpec::psa(speed_multiplier);
        inner.validate().map_err(core_to_py)?;
        Ok(Self { inner })
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder", "PrepaymentModelSpec.cmbs_with_lockout")]
    #[staticmethod]
    #[pyo3(text_signature = "(lockout_months, post_lockout_cpr)")]
    fn cmbs_with_lockout(lockout_months: u32, post_lockout_cpr: f64) -> PyResult<Self> {
        let inner = PrepaymentModelSpec::cmbs_with_lockout(lockout_months, post_lockout_cpr);
        inner.validate().map_err(core_to_py)?;
        Ok(Self { inner })
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder", "PrepaymentModelSpec.abs")]
    #[staticmethod]
    #[pyo3(text_signature = "(speed)")]
    fn abs(speed: f64) -> PyResult<Self> {
        let inner = PrepaymentModelSpec::abs(speed);
        inner.validate().map_err(core_to_py)?;
        Ok(Self { inner })
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder", "PrepaymentModelSpec.vector")]
    #[staticmethod]
    #[pyo3(text_signature = "(monthly_cpr)")]
    fn vector(monthly_cpr: Vec<f64>) -> PyResult<Self> {
        let inner = PrepaymentModelSpec::vector(monthly_cpr);
        inner.validate().map_err(core_to_py)?;
        Ok(Self { inner })
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder", "PrepaymentModelSpec.validate")]
    #[pyo3(text_signature = "(self)")]
    fn validate(&self) -> PyResult<()> {
        self.inner.validate().map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder", "PrepaymentModelSpec.to_dict")]
    fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner)
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder", "PrepaymentModelSpec.smm")]
    #[pyo3(text_signature = "(self, seasoning_months)")]
    fn smm(&self, seasoning_months: u32) -> PyResult<f64> {
        self.inner.smm(seasoning_months).map_err(core_to_py)
    }

    /// Python-style field summary.
    fn __repr__(&self) -> String {
        repr_from_serde("PrepaymentModelSpec", &self.inner)
    }
}
