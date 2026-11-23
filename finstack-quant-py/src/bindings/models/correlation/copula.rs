//! Copula specification and copula Python handles.

use crate::bindings::macros::{py_getters, wire_methods};
use crate::bindings::pydoc::pydoc;
use crate::errors::{core_to_py, correlation_to_py};
use pyo3::prelude::*;
use pyo3::types::PyType;

use finstack_quant_models::correlation::{Copula, CopulaSpec};

#[doc = pydoc!("finstack_quant.models.correlation._copula", "CopulaSpec")]
#[pyclass(
    name = "CopulaSpec",
    module = "finstack_quant.models.correlation",
    frozen,
    eq,
    from_py_object
)]
#[derive(Clone, Debug, PartialEq)]
pub struct PyCopulaSpec {
    /// Inner Rust spec.
    pub(crate) inner: CopulaSpec,
}

impl PyCopulaSpec {
    /// Construct from an existing [`CopulaSpec`].
    pub(crate) fn from_inner(inner: CopulaSpec) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyCopulaSpec {
    #[doc = pydoc!("finstack_quant.models.correlation._copula", "CopulaSpec.gaussian")]
    #[classmethod]
    fn gaussian(_cls: &Bound<'_, PyType>) -> Self {
        Self::from_inner(CopulaSpec::gaussian())
    }

    #[doc = pydoc!("finstack_quant.models.correlation._copula", "CopulaSpec.student_t")]
    #[classmethod]
    #[pyo3(text_signature = "(cls, degrees_of_freedom)")]
    fn student_t(_cls: &Bound<'_, PyType>, degrees_of_freedom: f64) -> PyResult<Self> {
        CopulaSpec::student_t(degrees_of_freedom)
            .map(Self::from_inner)
            .map_err(correlation_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.correlation._copula", "CopulaSpec.random_factor_loading")]
    #[classmethod]
    #[pyo3(text_signature = "(cls, loading_vol)")]
    fn random_factor_loading(_cls: &Bound<'_, PyType>, loading_vol: f64) -> Self {
        Self::from_inner(CopulaSpec::random_factor_loading(loading_vol))
    }

    #[doc = pydoc!("finstack_quant.models.correlation._copula", "CopulaSpec.multi_factor")]
    #[classmethod]
    #[pyo3(text_signature = "(cls)")]
    fn multi_factor(_cls: &Bound<'_, PyType>) -> Self {
        Self::from_inner(CopulaSpec::multi_factor())
    }

    #[doc = pydoc!("finstack_quant.models.correlation._copula", "CopulaSpec.build")]
    fn build(&self) -> PyResult<PyCopula> {
        self.inner
            .build()
            .map(|inner| PyCopula { inner })
            .map_err(correlation_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.correlation._copula", "CopulaSpec.__repr__")]
    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("CopulaSpec", &self.inner)
    }
}

py_getters!(PyCopulaSpec |s| {
    #[doc = pydoc!("finstack_quant.models.correlation._copula", "CopulaSpec.is_gaussian")]
    is_gaussian -> bool = s.inner.is_gaussian();

    #[doc = pydoc!("finstack_quant.models.correlation._copula", "CopulaSpec.is_student_t")]
    is_student_t -> bool = s.inner.is_student_t();

    #[doc = pydoc!("finstack_quant.models.correlation._copula", "CopulaSpec.is_rfl")]
    is_rfl -> bool = s.inner.is_rfl();

    #[doc = pydoc!("finstack_quant.models.correlation._copula", "CopulaSpec.is_multi_factor")]
    is_multi_factor -> bool = s.inner.is_multi_factor();
});

wire_methods!(
    PyCopulaSpec,
    CopulaSpec,
    "CopulaSpec",
    module = "finstack_quant.models.correlation._copula"
);

#[doc = pydoc!("finstack_quant.models.correlation._copula", "Copula")]
#[pyclass(name = "Copula", module = "finstack_quant.models.correlation", frozen)]
pub struct PyCopula {
    /// Boxed trait object.
    pub(crate) inner: Box<dyn Copula + Send + Sync>,
}

#[pymethods]
impl PyCopula {
    #[doc = pydoc!("finstack_quant.models.correlation._copula", "Copula.conditional_default_prob")]
    #[pyo3(text_signature = "(self, default_threshold, factor_realization, correlation)")]
    fn conditional_default_prob(
        &self,
        default_threshold: f64,
        factor_realization: Vec<f64>,
        correlation: f64,
    ) -> PyResult<f64> {
        self.inner
            .conditional_default_prob_checked(default_threshold, &factor_realization, correlation)
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.correlation._copula", "Copula.tail_dependence")]
    #[pyo3(text_signature = "(self, correlation)")]
    fn tail_dependence(&self, correlation: f64) -> f64 {
        self.inner.tail_dependence(correlation)
    }

    #[doc = pydoc!("finstack_quant.models.correlation._copula", "Copula.stress_correlation_proxy")]
    #[pyo3(text_signature = "(self, correlation)")]
    fn stress_correlation_proxy(&self, correlation: f64) -> PyResult<f64> {
        self.inner
            .stress_correlation_proxy(correlation)
            .map_err(core_to_py)
    }

    fn __repr__(&self) -> String {
        format!("Copula('{}')", self.inner.model_name())
    }
}

py_getters!(PyCopula |s| {
    #[doc = pydoc!("finstack_quant.models.correlation._copula", "Copula.num_factors")]
    num_factors -> usize = s.inner.num_factors();

    #[doc = pydoc!("finstack_quant.models.correlation._copula", "Copula.model_name")]
    model_name -> &'static str = s.inner.model_name();
});
