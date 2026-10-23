//! Spec-type bindings for `finstack_quant_cashflows::builder`.

use crate::bindings::macros::wire_methods;
use crate::bindings::pydoc::pydoc;
use finstack_quant_cashflows::builder::RecoveryModelSpec;
use pyo3::prelude::*;

use crate::bindings::repr_support::repr_from_serde;
use crate::errors::core_to_py;

#[doc = pydoc!("finstack_quant.cashflows.builder", "RecoveryModelSpec")]
#[pyclass(
    name = "RecoveryModelSpec",
    module = "finstack_quant.cashflows.builder",
    frozen,
    skip_from_py_object
)]
#[derive(Clone, Debug)]
pub struct PyRecoveryModelSpec {
    /// Inner recovery model.
    pub(crate) inner: RecoveryModelSpec,
}

wire_methods!(
    PyRecoveryModelSpec,
    RecoveryModelSpec,
    "RecoveryModelSpec",
    module = "finstack_quant.cashflows.builder",
    validate
);

#[pymethods]
impl PyRecoveryModelSpec {
    #[doc = pydoc!("finstack_quant.cashflows.builder", "RecoveryModelSpec.__init__")]
    #[new]
    #[pyo3(text_signature = "(rate, recovery_lag)")]
    fn new(rate: f64, recovery_lag: u32) -> PyResult<Self> {
        let inner = RecoveryModelSpec::with_lag(rate, recovery_lag);
        inner.validate().map_err(core_to_py)?;
        Ok(Self { inner })
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder", "RecoveryModelSpec.rate")]
    #[getter]
    fn rate(&self) -> f64 {
        self.inner.rate
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder", "RecoveryModelSpec.recovery_lag")]
    #[getter]
    fn recovery_lag(&self) -> u32 {
        self.inner.recovery_lag
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder", "RecoveryModelSpec.with_severity_vector")]
    #[pyo3(text_signature = "(self, severity_vector)")]
    fn with_severity_vector(&self, severity_vector: Vec<f64>) -> PyResult<Self> {
        let inner = self.inner.clone().with_severity_vector(severity_vector);
        inner.validate().map_err(core_to_py)?;
        Ok(Self { inner })
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder", "RecoveryModelSpec.severity_vector")]
    #[getter]
    fn severity_vector(&self) -> Option<Vec<f64>> {
        self.inner.severity_vector.clone()
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder", "RecoveryModelSpec.recovery_rate")]
    #[pyo3(text_signature = "(self, seasoning_months)")]
    fn recovery_rate(&self, seasoning_months: u32) -> f64 {
        self.inner.recovery_rate(seasoning_months)
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder", "RecoveryModelSpec.validate")]
    #[pyo3(text_signature = "(self)")]
    fn validate(&self) -> PyResult<()> {
        self.inner.validate().map_err(core_to_py)
    }

    /// Python-style field summary.
    fn __repr__(&self) -> String {
        repr_from_serde("RecoveryModelSpec", &self.inner)
    }
}
