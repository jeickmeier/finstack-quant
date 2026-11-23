//! Latent-factor correlation model Python handles.

use crate::bindings::macros::py_getters;
use crate::bindings::pydoc::pydoc;
use crate::errors::correlation_to_py;
use pyo3::prelude::*;
use pyo3::types::PyType;

use finstack_quant_models::correlation::{
    LatentFactorKind, LatentFactorSpec, LatentMultiFactor, LatentSingleFactor, LatentTwoFactor,
};

#[doc = pydoc!("finstack_quant.models.correlation._latent", "LatentFactorSpec")]
#[pyclass(
    name = "LatentFactorSpec",
    module = "finstack_quant.models.correlation",
    frozen,
    from_py_object
)]
#[derive(Clone, Debug)]
pub struct PyLatentFactorSpec {
    /// Inner Rust spec.
    pub(crate) inner: LatentFactorSpec,
}

impl PyLatentFactorSpec {
    /// Construct from an existing [`LatentFactorSpec`].
    pub(crate) fn from_inner(inner: LatentFactorSpec) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyLatentFactorSpec {
    #[doc = pydoc!("finstack_quant.models.correlation._latent", "LatentFactorSpec.single_factor")]
    #[classmethod]
    #[pyo3(text_signature = "(cls, volatility, mean_reversion)")]
    fn single_factor(
        _cls: &Bound<'_, PyType>,
        volatility: f64,
        mean_reversion: f64,
    ) -> PyResult<Self> {
        LatentFactorSpec::single_factor(volatility, mean_reversion)
            .map(Self::from_inner)
            .map_err(correlation_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.correlation._latent", "LatentFactorSpec.two_factor")]
    #[classmethod]
    #[pyo3(text_signature = "(cls, prepay_vol, credit_vol, correlation)")]
    fn two_factor(
        _cls: &Bound<'_, PyType>,
        prepay_vol: f64,
        credit_vol: f64,
        correlation: f64,
    ) -> PyResult<Self> {
        LatentFactorSpec::two_factor(prepay_vol, credit_vol, correlation)
            .map(Self::from_inner)
            .map_err(correlation_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.correlation._latent", "LatentFactorSpec.num_factors")]
    #[getter]
    fn num_factors(&self) -> usize {
        self.inner.num_factors()
    }

    #[doc = pydoc!("finstack_quant.models.correlation._latent", "LatentFactorSpec.build")]
    fn build(&self) -> PyResult<PyLatentFactorKind> {
        self.inner
            .build()
            .map(|inner| PyLatentFactorKind { inner })
            .map_err(correlation_to_py)
    }

    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("LatentFactorSpec", &self.inner)
    }
}

#[doc = pydoc!("finstack_quant.models.correlation._latent", "LatentFactorKind")]
#[pyclass(
    name = "LatentFactorKind",
    module = "finstack_quant.models.correlation",
    frozen
)]
pub struct PyLatentFactorKind {
    /// Concrete factor-model dispatch enum.
    pub(crate) inner: LatentFactorKind,
}

#[pymethods]
impl PyLatentFactorKind {
    #[doc = pydoc!("finstack_quant.models.correlation._latent", "LatentFactorKind.diagonal_factor_contribution")]
    #[pyo3(text_signature = "(self, factor_index, z)")]
    fn diagonal_factor_contribution(&self, factor_index: usize, z: f64) -> f64 {
        self.inner.diagonal_factor_contribution(factor_index, z)
    }

    fn __repr__(&self) -> String {
        format!(
            "LatentFactorKind('{}', n={})",
            self.inner.model_name(),
            self.inner.num_factors()
        )
    }
}

py_getters!(PyLatentFactorKind |s| {
    #[doc = pydoc!("finstack_quant.models.correlation._latent", "LatentFactorKind.num_factors")]
    num_factors -> usize = s.inner.num_factors();

    #[doc = pydoc!("finstack_quant.models.correlation._latent", "LatentFactorKind.correlation_matrix")]
    correlation_matrix -> Vec<f64> = s.inner.correlation_matrix().to_vec();

    #[doc = pydoc!("finstack_quant.models.correlation._latent", "LatentFactorKind.volatilities")]
    volatilities -> Vec<f64> = s.inner.volatilities().to_vec();

    #[doc = pydoc!("finstack_quant.models.correlation._latent", "LatentFactorKind.factor_names")]
    factor_names -> Vec<&'static str> = s.inner.factor_names();

    #[doc = pydoc!("finstack_quant.models.correlation._latent", "LatentFactorKind.model_name")]
    model_name -> &'static str = s.inner.model_name();
});

#[doc = pydoc!("finstack_quant.models.correlation._latent", "LatentSingleFactor")]
#[pyclass(
    name = "LatentSingleFactor",
    module = "finstack_quant.models.correlation",
    frozen,
    from_py_object
)]
#[derive(Clone, Debug)]
pub struct PyLatentSingleFactor {
    /// Inner Rust model.
    pub(crate) inner: LatentSingleFactor,
}

#[pymethods]
impl PyLatentSingleFactor {
    #[doc = pydoc!("finstack_quant.models.correlation._latent", "LatentSingleFactor.__init__")]
    #[new]
    #[pyo3(text_signature = "(volatility, mean_reversion)")]
    fn new(volatility: f64, mean_reversion: f64) -> PyResult<Self> {
        LatentSingleFactor::new(volatility, mean_reversion)
            .map(|inner| Self { inner })
            .map_err(correlation_to_py)
    }

    fn __repr__(&self) -> String {
        format!(
            "LatentSingleFactor(vol={:.4}, mr={:.4})",
            self.inner.volatility(),
            self.inner.mean_reversion()
        )
    }
}

py_getters!(PyLatentSingleFactor |s| {
    #[doc = pydoc!("finstack_quant.models.correlation._latent", "LatentSingleFactor.volatility")]
    volatility -> f64 = s.inner.volatility();

    #[doc = pydoc!("finstack_quant.models.correlation._latent", "LatentSingleFactor.mean_reversion")]
    mean_reversion -> f64 = s.inner.mean_reversion();

    #[doc = pydoc!("finstack_quant.models.correlation._latent", "LatentSingleFactor.num_factors")]
    num_factors -> usize = s.inner.num_factors();
});

#[doc = pydoc!("finstack_quant.models.correlation._latent", "LatentTwoFactor")]
#[pyclass(
    name = "LatentTwoFactor",
    module = "finstack_quant.models.correlation",
    frozen,
    from_py_object
)]
#[derive(Clone, Debug)]
pub struct PyLatentTwoFactor {
    /// Inner Rust model.
    pub(crate) inner: LatentTwoFactor,
}

#[pymethods]
impl PyLatentTwoFactor {
    #[doc = pydoc!("finstack_quant.models.correlation._latent", "LatentTwoFactor.__init__")]
    #[new]
    #[pyo3(text_signature = "(prepay_vol, credit_vol, correlation)")]
    fn new(prepay_vol: f64, credit_vol: f64, correlation: f64) -> PyResult<Self> {
        LatentTwoFactor::new(prepay_vol, credit_vol, correlation)
            .map(|inner| Self { inner })
            .map_err(correlation_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.correlation._latent", "LatentTwoFactor.rmbs_standard")]
    #[classmethod]
    fn rmbs_standard(_cls: &Bound<'_, PyType>) -> PyResult<Self> {
        LatentTwoFactor::rmbs_standard()
            .map(|inner| Self { inner })
            .map_err(correlation_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.correlation._latent", "LatentTwoFactor.clo_standard")]
    #[classmethod]
    fn clo_standard(_cls: &Bound<'_, PyType>) -> PyResult<Self> {
        LatentTwoFactor::clo_standard()
            .map(|inner| Self { inner })
            .map_err(correlation_to_py)
    }

    fn __repr__(&self) -> String {
        format!(
            "LatentTwoFactor(prepay={:.4}, credit={:.4}, corr={:.4})",
            self.inner.prepay_vol(),
            self.inner.credit_vol(),
            self.inner.correlation()
        )
    }
}

py_getters!(PyLatentTwoFactor |s| {
    #[doc = pydoc!("finstack_quant.models.correlation._latent", "LatentTwoFactor.prepay_vol")]
    prepay_vol -> f64 = s.inner.prepay_vol();

    #[doc = pydoc!("finstack_quant.models.correlation._latent", "LatentTwoFactor.credit_vol")]
    credit_vol -> f64 = s.inner.credit_vol();

    #[doc = pydoc!("finstack_quant.models.correlation._latent", "LatentTwoFactor.correlation")]
    correlation -> f64 = s.inner.correlation();

    #[doc = pydoc!("finstack_quant.models.correlation._latent", "LatentTwoFactor.num_factors")]
    num_factors -> usize = s.inner.num_factors();

    #[doc = pydoc!("finstack_quant.models.correlation._latent", "LatentTwoFactor.cholesky_l10")]
    cholesky_l10 -> f64 = s.inner.cholesky_l10();

    #[doc = pydoc!("finstack_quant.models.correlation._latent", "LatentTwoFactor.cholesky_l11")]
    cholesky_l11 -> f64 = s.inner.cholesky_l11();
});

#[doc = pydoc!("finstack_quant.models.correlation._latent", "LatentMultiFactor")]
#[pyclass(
    name = "LatentMultiFactor",
    module = "finstack_quant.models.correlation",
    frozen,
    from_py_object
)]
#[derive(Clone, Debug)]
pub struct PyLatentMultiFactor {
    /// Inner Rust model.
    pub(crate) inner: LatentMultiFactor,
}

#[pymethods]
impl PyLatentMultiFactor {
    #[doc = pydoc!("finstack_quant.models.correlation._latent", "LatentMultiFactor.__init__")]
    #[new]
    #[pyo3(text_signature = "(num_factors, volatilities, correlations)")]
    fn new(
        py: Python<'_>,
        num_factors: usize,
        volatilities: Vec<f64>,
        correlations: Vec<f64>,
    ) -> PyResult<Self> {
        py.detach(|| LatentMultiFactor::new(num_factors, volatilities, correlations))
            .map(|m| Self { inner: m })
            .map_err(correlation_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.correlation._latent", "LatentMultiFactor.uncorrelated")]
    #[classmethod]
    #[pyo3(text_signature = "(cls, num_factors, volatilities)")]
    fn uncorrelated(
        _cls: &Bound<'_, PyType>,
        num_factors: usize,
        volatilities: Vec<f64>,
    ) -> PyResult<Self> {
        LatentMultiFactor::uncorrelated(num_factors, volatilities)
            .map(|inner| Self { inner })
            .map_err(correlation_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.correlation._latent", "LatentMultiFactor.generate_correlated_factors")]
    #[pyo3(text_signature = "(self, independent_z)")]
    fn generate_correlated_factors(&self, independent_z: Vec<f64>) -> PyResult<Vec<f64>> {
        self.inner
            .generate_correlated_factors(&independent_z)
            .map_err(correlation_to_py)
    }

    fn __repr__(&self) -> String {
        format!("LatentMultiFactor(n={})", self.inner.num_factors())
    }
}

py_getters!(PyLatentMultiFactor |s| {
    #[doc = pydoc!("finstack_quant.models.correlation._latent", "LatentMultiFactor.num_factors")]
    num_factors -> usize = s.inner.num_factors();

    #[doc = pydoc!("finstack_quant.models.correlation._latent", "LatentMultiFactor.correlation_matrix")]
    correlation_matrix -> Vec<f64> = s.inner.correlation_matrix().to_vec();

    #[doc = pydoc!("finstack_quant.models.correlation._latent", "LatentMultiFactor.volatilities")]
    volatilities -> Vec<f64> = s.inner.volatilities().to_vec();
});
