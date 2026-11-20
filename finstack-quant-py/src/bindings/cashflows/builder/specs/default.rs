//! Spec-type bindings for `finstack_quant_cashflows::builder`.

use crate::bindings::macros::wire_methods;
use crate::bindings::pydoc::pydoc;
use finstack_quant_cashflows::builder::DefaultModelSpec;
use pyo3::prelude::*;

use crate::bindings::repr_support::repr_from_serde;
use crate::errors::core_to_py;

#[doc = pydoc!("finstack_quant.cashflows.builder._specs.default", "DefaultModelSpec")]
#[pyclass(
    name = "DefaultModelSpec",
    module = "finstack_quant.cashflows.builder",
    frozen,
    skip_from_py_object
)]
#[derive(Clone, Debug)]
pub struct PyDefaultModelSpec {
    /// Inner default model.
    pub(crate) inner: DefaultModelSpec,
}

wire_methods!(
    PyDefaultModelSpec,
    DefaultModelSpec,
    "DefaultModelSpec",
    module = "finstack_quant.cashflows.builder._specs.default",
    validate,
    to_dict
);

impl PyDefaultModelSpec {
    fn validated(inner: DefaultModelSpec) -> PyResult<Self> {
        inner.validate().map_err(core_to_py)?;
        Ok(Self { inner })
    }
}

#[pymethods]
impl PyDefaultModelSpec {
    #[doc = pydoc!("finstack_quant.cashflows.builder._specs.default", "DefaultModelSpec.constant_cdr")]
    #[staticmethod]
    #[pyo3(text_signature = "(cdr)")]
    fn constant_cdr(cdr: f64) -> PyResult<Self> {
        Self::validated(DefaultModelSpec::constant_cdr(cdr))
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder._specs.default", "DefaultModelSpec.sda")]
    #[staticmethod]
    #[pyo3(text_signature = "(speed_multiplier)")]
    fn sda(speed_multiplier: f64) -> PyResult<Self> {
        Self::validated(DefaultModelSpec::sda(speed_multiplier))
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder._specs.default", "DefaultModelSpec.vector")]
    #[staticmethod]
    #[pyo3(text_signature = "(monthly_cdr)")]
    fn vector(monthly_cdr: Vec<f64>) -> PyResult<Self> {
        Self::validated(DefaultModelSpec::vector(monthly_cdr))
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder._specs.default", "DefaultModelSpec.cumulative_loss")]
    #[staticmethod]
    #[pyo3(text_signature = "(cumulative_net_loss_pct, severity)")]
    fn cumulative_loss(cumulative_net_loss_pct: Vec<f64>, severity: f64) -> PyResult<Self> {
        Self::validated(DefaultModelSpec::cumulative_loss(
            cumulative_net_loss_pct,
            severity,
        ))
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder._specs.default", "DefaultModelSpec.timing")]
    #[staticmethod]
    #[pyo3(text_signature = "(cumulative_default_rate, annual_pct)")]
    fn timing(cumulative_default_rate: f64, annual_pct: Vec<f64>) -> PyResult<Self> {
        Self::validated(DefaultModelSpec::timing(
            cumulative_default_rate,
            annual_pct,
        ))
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder._specs.default", "DefaultModelSpec.validate")]
    #[pyo3(text_signature = "(self)")]
    fn validate(&self) -> PyResult<()> {
        self.inner.validate().map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder._specs.default", "DefaultModelSpec.mdr")]
    #[pyo3(text_signature = "(self, seasoning_months)")]
    fn mdr(&self, seasoning_months: u32) -> PyResult<f64> {
        self.inner.mdr(seasoning_months).map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder._specs.default", "DefaultModelSpec.mdr_with_survival")]
    #[pyo3(text_signature = "(self, seasoning_months, surviving_balance_fraction)")]
    fn mdr_with_survival(
        &self,
        seasoning_months: u32,
        surviving_balance_fraction: f64,
    ) -> PyResult<f64> {
        self.inner
            .mdr_with_survival(seasoning_months, surviving_balance_fraction)
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.cashflows.builder._specs.default", "DefaultModelSpec.cumulative_default_fraction")]
    #[pyo3(text_signature = "(self, seasoning_months)")]
    fn cumulative_default_fraction(&self, seasoning_months: u32) -> PyResult<Option<f64>> {
        self.inner
            .cumulative_default_fraction(seasoning_months)
            .map_err(core_to_py)
    }

    /// Python-style field summary.
    fn __repr__(&self) -> String {
        repr_from_serde("DefaultModelSpec", &self.inner)
    }
}
