//! Correlated recovery specification and model Python handles.

use crate::bindings::macros::{py_getters, wire_methods};
use crate::bindings::pydoc::pydoc;
use crate::errors::correlation_to_py;
use pyo3::prelude::*;
use pyo3::types::PyType;

use finstack_quant_models::correlation::{RecoveryModel, RecoverySpec};

#[doc = pydoc!("finstack_quant.models.correlation._recovery", "RecoverySpec")]
#[pyclass(
    name = "RecoverySpec",
    module = "finstack_quant.models.correlation",
    frozen,
    eq,
    from_py_object
)]
#[derive(Clone, Debug, PartialEq)]
pub struct PyRecoverySpec {
    /// Inner Rust spec.
    pub(crate) inner: RecoverySpec,
}

impl PyRecoverySpec {
    /// Construct from an existing [`RecoverySpec`].
    pub(crate) fn from_inner(inner: RecoverySpec) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyRecoverySpec {
    #[doc = pydoc!("finstack_quant.models.correlation._recovery", "RecoverySpec.constant")]
    #[classmethod]
    #[pyo3(text_signature = "(cls, rate)")]
    fn constant(_cls: &Bound<'_, PyType>, rate: f64) -> PyResult<Self> {
        RecoverySpec::constant(rate)
            .map(Self::from_inner)
            .map_err(correlation_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.correlation._recovery", "RecoverySpec.market_correlated")]
    #[classmethod]
    #[pyo3(text_signature = "(cls, mean, vol, correlation)")]
    fn market_correlated(
        _cls: &Bound<'_, PyType>,
        mean: f64,
        vol: f64,
        correlation: f64,
    ) -> PyResult<Self> {
        RecoverySpec::market_correlated(mean, vol, correlation)
            .map(Self::from_inner)
            .map_err(correlation_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.correlation._recovery", "RecoverySpec.market_standard_stochastic")]
    #[classmethod]
    fn market_standard_stochastic(_cls: &Bound<'_, PyType>) -> PyResult<Self> {
        RecoverySpec::market_standard_stochastic()
            .map(Self::from_inner)
            .map_err(correlation_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.correlation._recovery", "RecoverySpec.build")]
    fn build(&self) -> PyResult<PyRecoveryModel> {
        self.inner
            .build()
            .map(|inner| PyRecoveryModel { inner })
            .map_err(correlation_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.correlation._recovery", "RecoverySpec.__repr__")]
    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("RecoverySpec", &self.inner)
    }
}

wire_methods!(
    PyRecoverySpec,
    RecoverySpec,
    "RecoverySpec",
    module = "finstack_quant.models.correlation._recovery"
);

#[doc = pydoc!("finstack_quant.models.correlation._recovery", "RecoveryModel")]
#[pyclass(
    name = "RecoveryModel",
    module = "finstack_quant.models.correlation",
    frozen
)]
pub struct PyRecoveryModel {
    /// Boxed trait object.
    pub(crate) inner: Box<dyn RecoveryModel + Send + Sync>,
}

#[pymethods]
impl PyRecoveryModel {
    #[doc = pydoc!("finstack_quant.models.correlation._recovery", "RecoveryModel.conditional_recovery")]
    #[pyo3(text_signature = "(self, market_factor)")]
    fn conditional_recovery(&self, market_factor: f64) -> PyResult<f64> {
        self.inner
            .conditional_recovery(market_factor)
            .map_err(correlation_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.correlation._recovery", "RecoveryModel.conditional_lgd")]
    #[pyo3(text_signature = "(self, market_factor)")]
    fn conditional_lgd(&self, market_factor: f64) -> PyResult<f64> {
        self.inner
            .conditional_lgd(market_factor)
            .map_err(correlation_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.correlation._recovery", "RecoveryModel.sample_recovery")]
    #[pyo3(text_signature = "(self, market_factor, idiosyncratic_normal)")]
    fn sample_recovery(&self, market_factor: f64, idiosyncratic_normal: f64) -> PyResult<f64> {
        self.inner
            .sample_recovery(market_factor, idiosyncratic_normal)
            .map_err(correlation_to_py)
    }

    fn __repr__(&self) -> String {
        format!(
            "RecoveryModel('{}', expected={:.4})",
            self.inner.model_name(),
            self.inner.expected_recovery()
        )
    }
}

py_getters!(PyRecoveryModel |s| {
    #[doc = pydoc!("finstack_quant.models.correlation._recovery", "RecoveryModel.expected_recovery")]
    expected_recovery -> f64 = s.inner.expected_recovery();

    #[doc = pydoc!("finstack_quant.models.correlation._recovery", "RecoveryModel.lgd")]
    lgd -> f64 = s.inner.lgd();

    #[doc = pydoc!("finstack_quant.models.correlation._recovery", "RecoveryModel.has_idiosyncratic_dispersion")]
    has_idiosyncratic_dispersion -> bool = s.inner.has_idiosyncratic_dispersion();

    #[doc = pydoc!("finstack_quant.models.correlation._recovery", "RecoveryModel.recovery_volatility")]
    recovery_volatility -> f64 = s.inner.recovery_volatility();

    #[doc = pydoc!("finstack_quant.models.correlation._recovery", "RecoveryModel.is_stochastic")]
    is_stochastic -> bool = s.inner.is_stochastic();

    #[doc = pydoc!("finstack_quant.models.correlation._recovery", "RecoveryModel.model_name")]
    model_name -> &'static str = s.inner.model_name();
});
