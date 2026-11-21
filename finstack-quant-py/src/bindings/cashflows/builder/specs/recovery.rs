//! Spec-type bindings for `finstack_quant_cashflows::builder`.

use crate::bindings::macros::{py_getters, wire_methods};
use crate::bindings::pydoc::pydoc;
use finstack_quant_cashflows::builder::RecoveryModelSpec;
use pyo3::prelude::*;

use crate::bindings::repr_support::repr_from_serde;
use crate::errors::core_to_py;

#[doc = pydoc!("finstack_quant.cashflows.builder._specs.recovery", "RecoveryModelSpec")]
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
    module = "finstack_quant.cashflows.builder._specs.recovery",
    validate
);

#[pymethods]
impl PyRecoveryModelSpec {
    #[doc = pydoc!("finstack_quant.cashflows.builder._specs.recovery", "RecoveryModelSpec.__init__")]
    #[new]
    #[pyo3(text_signature = "(rate, recovery_lag)")]
    fn new(rate: f64, recovery_lag: u32) -> PyResult<Self> {
        let inner = RecoveryModelSpec::with_lag(rate, recovery_lag);
        inner.validate().map_err(core_to_py)?;
        Ok(Self { inner })
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder._specs.recovery", "RecoveryModelSpec.with_severity_vector")]
    #[pyo3(text_signature = "(self, severity_vector)")]
    fn with_severity_vector(&self, severity_vector: Vec<f64>) -> PyResult<Self> {
        let inner = self.inner.clone().with_severity_vector(severity_vector);
        inner.validate().map_err(core_to_py)?;
        Ok(Self { inner })
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder._specs.recovery", "RecoveryModelSpec.recovery_rate")]
    #[pyo3(text_signature = "(self, seasoning_months)")]
    fn recovery_rate(&self, seasoning_months: u32) -> PyResult<f64> {
        self.inner
            .recovery_rate(seasoning_months)
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder._specs.recovery", "RecoveryModelSpec.validate")]
    #[pyo3(text_signature = "(self)")]
    fn validate(&self) -> PyResult<()> {
        self.inner.validate().map_err(core_to_py)
    }

    /// Python-style field summary.
    fn __repr__(&self) -> String {
        repr_from_serde("RecoveryModelSpec", &self.inner)
    }
}

py_getters!(PyRecoveryModelSpec |s| {
    #[doc = pydoc!("finstack_quant.cashflows.builder._specs.recovery", "RecoveryModelSpec.rate")]
    rate -> f64 = s.inner.rate;

    #[doc = pydoc!("finstack_quant.cashflows.builder._specs.recovery", "RecoveryModelSpec.recovery_lag")]
    recovery_lag -> u32 = s.inner.recovery_lag;

    #[doc = pydoc!("finstack_quant.cashflows.builder._specs.recovery", "RecoveryModelSpec.severity_vector")]
    severity_vector -> Option<Vec<f64>> = s.inner.severity_vector.clone();
});
