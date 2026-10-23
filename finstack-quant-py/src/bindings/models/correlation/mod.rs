//! Python bindings for the credit-correlation module.
//!
//! Exposes copula models, recovery models, factor models, and joint
//! probability utilities to Python under `finstack_quant.models.correlation`,
//! mirroring the Rust module [`finstack_quant_models::correlation`].

use crate::bindings::macros::{impl_repr_html_via_dataframe, wire_methods};
use crate::bindings::pandas_utils::{
    dict_to_dataframe, serde_object_to_single_row_dataframe_with_schema,
};
use crate::bindings::pydoc::pydoc;
use crate::errors::{core_to_py, correlation_to_py};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PyModule, PyType};

use finstack_quant_core::math::linalg;
use finstack_quant_core::math::probability::{self, CorrelatedBernoulli};
use finstack_quant_models::correlation::CreditExposure;
use finstack_quant_models::correlation::{
    self as corr, Copula, CopulaSpec, LatentFactorKind, LatentFactorSpec, LatentMultiFactor,
    LatentSingleFactor, LatentTwoFactor, PortfolioLossConfig, PortfolioLossResult, RecoveryModel,
    RecoverySpec, TrancheLossStatistics,
};

#[doc = pydoc!("finstack_quant.models.correlation", "CopulaSpec")]
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
    #[doc = pydoc!("finstack_quant.models.correlation", "CopulaSpec.gaussian")]
    #[classmethod]
    fn gaussian(_cls: &Bound<'_, PyType>) -> Self {
        Self::from_inner(CopulaSpec::gaussian())
    }

    #[doc = pydoc!("finstack_quant.models.correlation", "CopulaSpec.student_t")]
    #[classmethod]
    #[pyo3(text_signature = "(cls, degrees_of_freedom)")]
    fn student_t(_cls: &Bound<'_, PyType>, degrees_of_freedom: f64) -> PyResult<Self> {
        CopulaSpec::student_t(degrees_of_freedom)
            .map(Self::from_inner)
            .map_err(correlation_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.correlation", "CopulaSpec.random_factor_loading")]
    #[classmethod]
    #[pyo3(text_signature = "(cls, loading_vol)")]
    fn random_factor_loading(_cls: &Bound<'_, PyType>, loading_vol: f64) -> Self {
        Self::from_inner(CopulaSpec::random_factor_loading(loading_vol))
    }

    #[doc = pydoc!("finstack_quant.models.correlation", "CopulaSpec.multi_factor")]
    #[classmethod]
    #[pyo3(text_signature = "(cls)")]
    fn multi_factor(_cls: &Bound<'_, PyType>) -> Self {
        Self::from_inner(CopulaSpec::multi_factor())
    }

    #[doc = pydoc!("finstack_quant.models.correlation", "CopulaSpec.build")]
    fn build(&self) -> PyResult<PyCopula> {
        self.inner
            .build()
            .map(|inner| PyCopula { inner })
            .map_err(correlation_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.correlation", "CopulaSpec.is_gaussian")]
    #[getter]
    fn is_gaussian(&self) -> bool {
        self.inner.is_gaussian()
    }

    #[doc = pydoc!("finstack_quant.models.correlation", "CopulaSpec.is_student_t")]
    #[getter]
    fn is_student_t(&self) -> bool {
        self.inner.is_student_t()
    }

    #[doc = pydoc!("finstack_quant.models.correlation", "CopulaSpec.is_rfl")]
    #[getter]
    fn is_rfl(&self) -> bool {
        self.inner.is_rfl()
    }

    #[doc = pydoc!("finstack_quant.models.correlation", "CopulaSpec.is_multi_factor")]
    #[getter]
    fn is_multi_factor(&self) -> bool {
        self.inner.is_multi_factor()
    }

    #[doc = pydoc!("finstack_quant.models.correlation", "CopulaSpec.__repr__")]
    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("CopulaSpec", &self.inner)
    }
}

wire_methods!(
    PyCopulaSpec,
    CopulaSpec,
    "CopulaSpec",
    module = "finstack_quant.models.correlation"
);

#[doc = pydoc!("finstack_quant.models.correlation", "Copula")]
#[pyclass(name = "Copula", module = "finstack_quant.models.correlation", frozen)]
pub struct PyCopula {
    /// Boxed trait object.
    pub(crate) inner: Box<dyn Copula + Send + Sync>,
}

#[pymethods]
impl PyCopula {
    #[doc = pydoc!("finstack_quant.models.correlation", "Copula.conditional_default_prob")]
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

    #[doc = pydoc!("finstack_quant.models.correlation", "Copula.num_factors")]
    #[getter]
    fn num_factors(&self) -> usize {
        self.inner.num_factors()
    }

    #[doc = pydoc!("finstack_quant.models.correlation", "Copula.model_name")]
    #[getter]
    fn model_name(&self) -> &'static str {
        self.inner.model_name()
    }

    #[doc = pydoc!("finstack_quant.models.correlation", "Copula.tail_dependence")]
    #[pyo3(text_signature = "(self, correlation)")]
    fn tail_dependence(&self, correlation: f64) -> f64 {
        self.inner.tail_dependence(correlation)
    }

    #[doc = pydoc!("finstack_quant.models.correlation", "Copula.stress_correlation_proxy")]
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

#[doc = pydoc!("finstack_quant.models.correlation", "RecoverySpec")]
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
    #[doc = pydoc!("finstack_quant.models.correlation", "RecoverySpec.constant")]
    #[classmethod]
    #[pyo3(text_signature = "(cls, rate)")]
    fn constant(_cls: &Bound<'_, PyType>, rate: f64) -> PyResult<Self> {
        RecoverySpec::constant(rate)
            .map(Self::from_inner)
            .map_err(correlation_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.correlation", "RecoverySpec.market_correlated")]
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

    #[doc = pydoc!("finstack_quant.models.correlation", "RecoverySpec.market_standard_stochastic")]
    #[classmethod]
    fn market_standard_stochastic(_cls: &Bound<'_, PyType>) -> PyResult<Self> {
        RecoverySpec::market_standard_stochastic()
            .map(Self::from_inner)
            .map_err(correlation_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.correlation", "RecoverySpec.build")]
    fn build(&self) -> PyResult<PyRecoveryModel> {
        self.inner
            .build()
            .map(|inner| PyRecoveryModel { inner })
            .map_err(correlation_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.correlation", "RecoverySpec.__repr__")]
    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("RecoverySpec", &self.inner)
    }
}

wire_methods!(
    PyRecoverySpec,
    RecoverySpec,
    "RecoverySpec",
    module = "finstack_quant.models.correlation"
);

#[doc = pydoc!("finstack_quant.models.correlation", "RecoveryModel")]
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
    #[doc = pydoc!("finstack_quant.models.correlation", "RecoveryModel.expected_recovery")]
    #[getter]
    fn expected_recovery(&self) -> f64 {
        self.inner.expected_recovery()
    }

    #[doc = pydoc!("finstack_quant.models.correlation", "RecoveryModel.conditional_recovery")]
    #[pyo3(text_signature = "(self, market_factor)")]
    fn conditional_recovery(&self, market_factor: f64) -> PyResult<f64> {
        self.inner
            .conditional_recovery(market_factor)
            .map_err(correlation_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.correlation", "RecoveryModel.lgd")]
    #[getter]
    fn lgd(&self) -> f64 {
        self.inner.lgd()
    }

    #[doc = pydoc!("finstack_quant.models.correlation", "RecoveryModel.conditional_lgd")]
    #[pyo3(text_signature = "(self, market_factor)")]
    fn conditional_lgd(&self, market_factor: f64) -> PyResult<f64> {
        self.inner
            .conditional_lgd(market_factor)
            .map_err(correlation_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.correlation", "RecoveryModel.sample_recovery")]
    #[pyo3(text_signature = "(self, market_factor, idiosyncratic_normal)")]
    fn sample_recovery(&self, market_factor: f64, idiosyncratic_normal: f64) -> PyResult<f64> {
        self.inner
            .sample_recovery(market_factor, idiosyncratic_normal)
            .map_err(correlation_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.correlation", "RecoveryModel.has_idiosyncratic_dispersion")]
    #[getter]
    fn has_idiosyncratic_dispersion(&self) -> bool {
        self.inner.has_idiosyncratic_dispersion()
    }

    #[doc = pydoc!("finstack_quant.models.correlation", "RecoveryModel.recovery_volatility")]
    #[getter]
    fn recovery_volatility(&self) -> f64 {
        self.inner.recovery_volatility()
    }

    #[doc = pydoc!("finstack_quant.models.correlation", "RecoveryModel.is_stochastic")]
    #[getter]
    fn is_stochastic(&self) -> bool {
        self.inner.is_stochastic()
    }

    #[doc = pydoc!("finstack_quant.models.correlation", "RecoveryModel.model_name")]
    #[getter]
    fn model_name(&self) -> &'static str {
        self.inner.model_name()
    }

    fn __repr__(&self) -> String {
        format!(
            "RecoveryModel('{}', expected={:.4})",
            self.inner.model_name(),
            self.inner.expected_recovery()
        )
    }
}

#[doc = pydoc!("finstack_quant.models.correlation", "LatentFactorSpec")]
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
    #[doc = pydoc!("finstack_quant.models.correlation", "LatentFactorSpec.single_factor")]
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

    #[doc = pydoc!("finstack_quant.models.correlation", "LatentFactorSpec.two_factor")]
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

    #[doc = pydoc!("finstack_quant.models.correlation", "LatentFactorSpec.num_factors")]
    #[getter]
    fn num_factors(&self) -> usize {
        self.inner.num_factors()
    }

    #[doc = pydoc!("finstack_quant.models.correlation", "LatentFactorSpec.build")]
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

#[doc = pydoc!("finstack_quant.models.correlation", "LatentFactorKind")]
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
    #[doc = pydoc!("finstack_quant.models.correlation", "LatentFactorKind.num_factors")]
    #[getter]
    fn num_factors(&self) -> usize {
        self.inner.num_factors()
    }

    #[doc = pydoc!("finstack_quant.models.correlation", "LatentFactorKind.correlation_matrix")]
    #[getter]
    fn correlation_matrix(&self) -> Vec<f64> {
        self.inner.correlation_matrix().to_vec()
    }

    #[doc = pydoc!("finstack_quant.models.correlation", "LatentFactorKind.volatilities")]
    #[getter]
    fn volatilities(&self) -> Vec<f64> {
        self.inner.volatilities().to_vec()
    }

    #[doc = pydoc!("finstack_quant.models.correlation", "LatentFactorKind.factor_names")]
    #[getter]
    fn factor_names(&self) -> Vec<&'static str> {
        self.inner.factor_names()
    }

    #[doc = pydoc!("finstack_quant.models.correlation", "LatentFactorKind.model_name")]
    #[getter]
    fn model_name(&self) -> &'static str {
        self.inner.model_name()
    }

    #[doc = pydoc!("finstack_quant.models.correlation", "LatentFactorKind.diagonal_factor_contribution")]
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

#[doc = pydoc!("finstack_quant.models.correlation", "LatentSingleFactor")]
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
    #[doc = pydoc!("finstack_quant.models.correlation", "LatentSingleFactor.__init__")]
    #[new]
    #[pyo3(text_signature = "(volatility, mean_reversion)")]
    fn new(volatility: f64, mean_reversion: f64) -> PyResult<Self> {
        LatentSingleFactor::new(volatility, mean_reversion)
            .map(|inner| Self { inner })
            .map_err(correlation_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.correlation", "LatentSingleFactor.volatility")]
    #[getter]
    fn volatility(&self) -> f64 {
        self.inner.volatility()
    }

    #[doc = pydoc!("finstack_quant.models.correlation", "LatentSingleFactor.mean_reversion")]
    #[getter]
    fn mean_reversion(&self) -> f64 {
        self.inner.mean_reversion()
    }

    #[doc = pydoc!("finstack_quant.models.correlation", "LatentSingleFactor.num_factors")]
    #[getter]
    fn num_factors(&self) -> usize {
        self.inner.num_factors()
    }

    fn __repr__(&self) -> String {
        format!(
            "LatentSingleFactor(vol={:.4}, mr={:.4})",
            self.inner.volatility(),
            self.inner.mean_reversion()
        )
    }
}

#[doc = pydoc!("finstack_quant.models.correlation", "LatentTwoFactor")]
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
    #[doc = pydoc!("finstack_quant.models.correlation", "LatentTwoFactor.__init__")]
    #[new]
    #[pyo3(text_signature = "(prepay_vol, credit_vol, correlation)")]
    fn new(prepay_vol: f64, credit_vol: f64, correlation: f64) -> PyResult<Self> {
        LatentTwoFactor::new(prepay_vol, credit_vol, correlation)
            .map(|inner| Self { inner })
            .map_err(correlation_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.correlation", "LatentTwoFactor.rmbs_standard")]
    #[classmethod]
    fn rmbs_standard(_cls: &Bound<'_, PyType>) -> PyResult<Self> {
        LatentTwoFactor::rmbs_standard()
            .map(|inner| Self { inner })
            .map_err(correlation_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.correlation", "LatentTwoFactor.clo_standard")]
    #[classmethod]
    fn clo_standard(_cls: &Bound<'_, PyType>) -> PyResult<Self> {
        LatentTwoFactor::clo_standard()
            .map(|inner| Self { inner })
            .map_err(correlation_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.correlation", "LatentTwoFactor.prepay_vol")]
    #[getter]
    fn prepay_vol(&self) -> f64 {
        self.inner.prepay_vol()
    }

    #[doc = pydoc!("finstack_quant.models.correlation", "LatentTwoFactor.credit_vol")]
    #[getter]
    fn credit_vol(&self) -> f64 {
        self.inner.credit_vol()
    }

    #[doc = pydoc!("finstack_quant.models.correlation", "LatentTwoFactor.correlation")]
    #[getter]
    fn correlation(&self) -> f64 {
        self.inner.correlation()
    }

    #[doc = pydoc!("finstack_quant.models.correlation", "LatentTwoFactor.num_factors")]
    #[getter]
    fn num_factors(&self) -> usize {
        self.inner.num_factors()
    }

    #[doc = pydoc!("finstack_quant.models.correlation", "LatentTwoFactor.cholesky_l10")]
    #[getter]
    fn cholesky_l10(&self) -> f64 {
        self.inner.cholesky_l10()
    }

    #[doc = pydoc!("finstack_quant.models.correlation", "LatentTwoFactor.cholesky_l11")]
    #[getter]
    fn cholesky_l11(&self) -> f64 {
        self.inner.cholesky_l11()
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

#[doc = pydoc!("finstack_quant.models.correlation", "LatentMultiFactor")]
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
    #[doc = pydoc!("finstack_quant.models.correlation", "LatentMultiFactor.__init__")]
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

    #[doc = pydoc!("finstack_quant.models.correlation", "LatentMultiFactor.uncorrelated")]
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

    #[doc = pydoc!("finstack_quant.models.correlation", "LatentMultiFactor.num_factors")]
    #[getter]
    fn num_factors(&self) -> usize {
        self.inner.num_factors()
    }

    #[doc = pydoc!("finstack_quant.models.correlation", "LatentMultiFactor.correlation_matrix")]
    #[getter]
    fn correlation_matrix(&self) -> Vec<f64> {
        self.inner.correlation_matrix().to_vec()
    }

    #[doc = pydoc!("finstack_quant.models.correlation", "LatentMultiFactor.volatilities")]
    #[getter]
    fn volatilities(&self) -> Vec<f64> {
        self.inner.volatilities().to_vec()
    }

    #[doc = pydoc!("finstack_quant.models.correlation", "LatentMultiFactor.generate_correlated_factors")]
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

#[doc = pydoc!("finstack_quant.models.correlation", "CorrelatedBernoulli")]
#[pyclass(
    name = "CorrelatedBernoulli",
    module = "finstack_quant.models.correlation",
    frozen,
    from_py_object
)]
#[derive(Clone, Debug)]
pub struct PyCorrelatedBernoulli {
    /// Inner Rust struct.
    pub(crate) inner: CorrelatedBernoulli,
}

#[pymethods]
impl PyCorrelatedBernoulli {
    #[doc = pydoc!("finstack_quant.models.correlation", "CorrelatedBernoulli.__init__")]
    #[new]
    #[pyo3(text_signature = "(p1, p2, correlation)")]
    fn new(p1: f64, p2: f64, correlation: f64) -> PyResult<Self> {
        Ok(Self {
            inner: CorrelatedBernoulli::new(p1, p2, correlation).map_err(core_to_py)?,
        })
    }

    #[doc = pydoc!("finstack_quant.models.correlation", "CorrelatedBernoulli.p1")]
    #[getter]
    fn p1(&self) -> f64 {
        self.inner.p1()
    }

    #[doc = pydoc!("finstack_quant.models.correlation", "CorrelatedBernoulli.p2")]
    #[getter]
    fn p2(&self) -> f64 {
        self.inner.p2()
    }

    #[doc = pydoc!("finstack_quant.models.correlation", "CorrelatedBernoulli.correlation")]
    #[getter]
    fn correlation(&self) -> f64 {
        self.inner.correlation()
    }

    #[doc = pydoc!("finstack_quant.models.correlation", "CorrelatedBernoulli.requested_correlation")]
    #[getter]
    fn requested_correlation(&self) -> f64 {
        self.inner.requested_correlation()
    }

    #[doc = pydoc!("finstack_quant.models.correlation", "CorrelatedBernoulli.joint_p11")]
    #[getter]
    fn joint_p11(&self) -> f64 {
        self.inner.joint_p11()
    }

    #[doc = pydoc!("finstack_quant.models.correlation", "CorrelatedBernoulli.joint_p10")]
    #[getter]
    fn joint_p10(&self) -> f64 {
        self.inner.joint_p10()
    }

    #[doc = pydoc!("finstack_quant.models.correlation", "CorrelatedBernoulli.joint_p01")]
    #[getter]
    fn joint_p01(&self) -> f64 {
        self.inner.joint_p01()
    }

    #[doc = pydoc!("finstack_quant.models.correlation", "CorrelatedBernoulli.joint_p00")]
    #[getter]
    fn joint_p00(&self) -> f64 {
        self.inner.joint_p00()
    }

    #[doc = pydoc!("finstack_quant.models.correlation", "CorrelatedBernoulli.joint_probabilities")]
    fn joint_probabilities(&self) -> (f64, f64, f64, f64) {
        self.inner.joint_probabilities()
    }

    #[doc = pydoc!("finstack_quant.models.correlation", "CorrelatedBernoulli.conditional_p2_given_x1")]
    fn conditional_p2_given_x1(&self) -> f64 {
        self.inner.conditional_p2_given_x1()
    }

    #[doc = pydoc!("finstack_quant.models.correlation", "CorrelatedBernoulli.conditional_p1_given_x2")]
    fn conditional_p1_given_x2(&self) -> f64 {
        self.inner.conditional_p1_given_x2()
    }

    #[doc = pydoc!("finstack_quant.models.correlation", "CorrelatedBernoulli.sample_from_uniform")]
    #[pyo3(text_signature = "(self, u)")]
    fn sample_from_uniform(&self, u: f64) -> PyResult<(u8, u8)> {
        self.inner.sample_from_uniform(u).map_err(core_to_py)
    }

    fn __repr__(&self) -> String {
        format!(
            "CorrelatedBernoulli(p1={:.4}, p2={:.4}, corr={:.4})",
            self.inner.p1(),
            self.inner.p2(),
            self.inner.correlation()
        )
    }
}

#[doc = pydoc!("finstack_quant.models.correlation", "CreditExposure")]
#[pyclass(
    name = "CreditExposure",
    module = "finstack_quant.models.correlation",
    frozen,
    from_py_object
)]
#[derive(Clone, Debug)]
pub struct PyCreditExposure {
    inner: CreditExposure,
}

#[pymethods]
impl PyCreditExposure {
    #[doc = pydoc!("finstack_quant.models.correlation", "CreditExposure.__init__")]
    #[new]
    #[pyo3(text_signature = "(id, notional, default_probability, lgd, factor_loadings)")]
    fn new(
        id: String,
        notional: f64,
        default_probability: f64,
        lgd: f64,
        factor_loadings: Vec<f64>,
    ) -> Self {
        Self {
            inner: CreditExposure {
                id,
                notional,
                default_probability,
                lgd,
                factor_loadings,
            },
        }
    }

    #[doc = pydoc!("finstack_quant.models.correlation", "CreditExposure.id")]
    #[getter]
    fn id(&self) -> String {
        self.inner.id.clone()
    }

    #[doc = pydoc!("finstack_quant.models.correlation", "CreditExposure.notional")]
    #[getter]
    fn notional(&self) -> f64 {
        self.inner.notional
    }

    #[doc = pydoc!("finstack_quant.models.correlation", "CreditExposure.default_probability")]
    #[getter]
    fn default_probability(&self) -> f64 {
        self.inner.default_probability
    }

    #[doc = pydoc!("finstack_quant.models.correlation", "CreditExposure.lgd")]
    #[getter]
    fn lgd(&self) -> f64 {
        self.inner.lgd
    }

    #[doc = pydoc!("finstack_quant.models.correlation", "CreditExposure.factor_loadings")]
    #[getter]
    fn factor_loadings(&self) -> Vec<f64> {
        self.inner.factor_loadings.clone()
    }

    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("CreditExposure", &self.inner)
    }
}

wire_methods!(
    PyCreditExposure,
    CreditExposure,
    "CreditExposure",
    module = "finstack_quant.models.correlation"
);

#[doc = pydoc!("finstack_quant.models.correlation", "PortfolioLossConfig")]
#[pyclass(
    name = "PortfolioLossConfig",
    module = "finstack_quant.models.correlation",
    frozen,
    from_py_object
)]
#[derive(Clone, Debug)]
pub struct PyPortfolioLossConfig {
    inner: PortfolioLossConfig,
}

#[pymethods]
impl PyPortfolioLossConfig {
    #[doc = pydoc!("finstack_quant.models.correlation", "PortfolioLossConfig.__init__")]
    #[new]
    #[pyo3(text_signature = "(num_paths, seed, confidence, copula)")]
    fn new(num_paths: usize, seed: u64, confidence: f64, copula: PyCopulaSpec) -> Self {
        Self {
            inner: PortfolioLossConfig {
                num_paths,
                seed,
                confidence,
                copula: copula.inner,
            },
        }
    }

    #[doc = pydoc!("finstack_quant.models.correlation", "PortfolioLossConfig.num_paths")]
    #[getter]
    fn num_paths(&self) -> usize {
        self.inner.num_paths
    }

    #[doc = pydoc!("finstack_quant.models.correlation", "PortfolioLossConfig.seed")]
    #[getter]
    fn seed(&self) -> u64 {
        self.inner.seed
    }

    #[doc = pydoc!("finstack_quant.models.correlation", "PortfolioLossConfig.confidence")]
    #[getter]
    fn confidence(&self) -> f64 {
        self.inner.confidence
    }

    #[doc = pydoc!("finstack_quant.models.correlation", "PortfolioLossConfig.copula")]
    #[getter]
    fn copula(&self) -> PyCopulaSpec {
        PyCopulaSpec::from_inner(self.inner.copula.clone())
    }

    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("PortfolioLossConfig", &self.inner)
    }
}

wire_methods!(
    PyPortfolioLossConfig,
    PortfolioLossConfig,
    "PortfolioLossConfig",
    module = "finstack_quant.models.correlation"
);

#[doc = pydoc!("finstack_quant.models.correlation", "PortfolioLossResult")]
#[pyclass(
    name = "PortfolioLossResult",
    module = "finstack_quant.models.correlation",
    frozen,
    skip_from_py_object
)]
#[derive(Clone, Debug)]
pub struct PyPortfolioLossResult {
    inner: PortfolioLossResult,
}

#[pymethods]
impl PyPortfolioLossResult {
    #[doc = pydoc!("finstack_quant.models.correlation", "PortfolioLossResult.from_losses")]
    #[staticmethod]
    #[pyo3(text_signature = "(losses, confidence)")]
    fn from_losses(losses: Vec<f64>, confidence: f64) -> PyResult<Self> {
        PortfolioLossResult::from_losses(losses, confidence)
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.correlation", "PortfolioLossResult.losses")]
    #[getter]
    fn losses(&self) -> Vec<f64> {
        self.inner.losses.clone()
    }

    #[doc = pydoc!("finstack_quant.models.correlation", "PortfolioLossResult.expected_loss")]
    #[getter]
    fn expected_loss(&self) -> f64 {
        self.inner.expected_loss
    }

    #[doc = pydoc!("finstack_quant.models.correlation", "PortfolioLossResult.var")]
    #[getter]
    fn var(&self) -> f64 {
        self.inner.var
    }

    #[doc = pydoc!("finstack_quant.models.correlation", "PortfolioLossResult.expected_shortfall")]
    #[getter]
    fn expected_shortfall(&self) -> f64 {
        self.inner.expected_shortfall
    }

    #[doc = pydoc!("finstack_quant.models.correlation", "PortfolioLossResult.confidence")]
    #[getter]
    fn confidence(&self) -> f64 {
        self.inner.confidence
    }

    #[doc = pydoc!("finstack_quant.models.correlation", "PortfolioLossResult.tranche_loss_statistics")]
    #[pyo3(text_signature = "(attachment, detachment, pool_notional)")]
    fn tranche_loss_statistics(
        &self,
        py: Python<'_>,
        attachment: f64,
        detachment: f64,
        pool_notional: f64,
    ) -> PyResult<PyTrancheLossStatistics> {
        py.detach(|| {
            self.inner
                .tranche_loss_statistics(attachment, detachment, pool_notional)
        })
        .map(|inner| PyTrancheLossStatistics { inner })
        .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.correlation", "PortfolioLossResult.to_dataframe")]
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let data = PyDict::new(py);
        data.set_item("loss", self.inner.losses.clone())?;
        dict_to_dataframe(py, &data, None)
    }

    #[doc = pydoc!("finstack_quant.models.correlation", "PortfolioLossResult.to_summary_dataframe")]
    fn to_summary_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let row = serde_json::json!({
            "expected_loss": self.inner.expected_loss,
            "var": self.inner.var,
            "expected_shortfall": self.inner.expected_shortfall,
            "confidence": self.inner.confidence,
            "num_paths": self.inner.losses.len(),
        });
        serde_object_to_single_row_dataframe_with_schema(
            py,
            &row,
            &[
                "expected_loss",
                "var",
                "expected_shortfall",
                "confidence",
                "num_paths",
            ],
        )
    }

    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("PortfolioLossResult", &self.inner)
    }
}

wire_methods!(
    PyPortfolioLossResult,
    PortfolioLossResult,
    "PortfolioLossResult",
    module = "finstack_quant.models.correlation"
);

#[doc = pydoc!("finstack_quant.models.correlation", "TrancheLossStatistics")]
#[pyclass(
    name = "TrancheLossStatistics",
    module = "finstack_quant.models.correlation",
    frozen,
    skip_from_py_object
)]
#[derive(Clone, Debug)]
pub struct PyTrancheLossStatistics {
    inner: TrancheLossStatistics,
}

#[pymethods]
impl PyTrancheLossStatistics {
    #[doc = pydoc!("finstack_quant.models.correlation", "TrancheLossStatistics.attachment")]
    #[getter]
    fn attachment(&self) -> f64 {
        self.inner.attachment
    }

    #[doc = pydoc!("finstack_quant.models.correlation", "TrancheLossStatistics.detachment")]
    #[getter]
    fn detachment(&self) -> f64 {
        self.inner.detachment
    }

    #[doc = pydoc!("finstack_quant.models.correlation", "TrancheLossStatistics.tranche_notional")]
    #[getter]
    fn tranche_notional(&self) -> f64 {
        self.inner.tranche_notional
    }

    #[doc = pydoc!("finstack_quant.models.correlation", "TrancheLossStatistics.expected_loss_fraction")]
    #[getter]
    fn expected_loss_fraction(&self) -> f64 {
        self.inner.expected_loss_fraction
    }

    #[doc = pydoc!("finstack_quant.models.correlation", "TrancheLossStatistics.expected_loss_amount")]
    #[getter]
    fn expected_loss_amount(&self) -> f64 {
        self.inner.expected_loss_amount
    }

    #[doc = pydoc!("finstack_quant.models.correlation", "TrancheLossStatistics.var_fraction")]
    #[getter]
    fn var_fraction(&self) -> f64 {
        self.inner.var_fraction
    }

    #[doc = pydoc!("finstack_quant.models.correlation", "TrancheLossStatistics.var_amount")]
    #[getter]
    fn var_amount(&self) -> f64 {
        self.inner.var_amount
    }

    #[doc = pydoc!("finstack_quant.models.correlation", "TrancheLossStatistics.expected_shortfall_fraction")]
    #[getter]
    fn expected_shortfall_fraction(&self) -> f64 {
        self.inner.expected_shortfall_fraction
    }

    #[doc = pydoc!("finstack_quant.models.correlation", "TrancheLossStatistics.expected_shortfall_amount")]
    #[getter]
    fn expected_shortfall_amount(&self) -> f64 {
        self.inner.expected_shortfall_amount
    }

    #[doc = pydoc!("finstack_quant.models.correlation", "TrancheLossStatistics.prob_attachment_breached")]
    #[getter]
    fn prob_attachment_breached(&self) -> f64 {
        self.inner.prob_attachment_breached
    }

    #[doc = pydoc!("finstack_quant.models.correlation", "TrancheLossStatistics.prob_full_writedown")]
    #[getter]
    fn prob_full_writedown(&self) -> f64 {
        self.inner.prob_full_writedown
    }

    #[doc = pydoc!("finstack_quant.models.correlation", "TrancheLossStatistics.to_dataframe")]
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let row = serde_json::json!({
            "attachment": self.inner.attachment,
            "detachment": self.inner.detachment,
            "tranche_notional": self.inner.tranche_notional,
            "expected_loss_fraction": self.inner.expected_loss_fraction,
            "expected_loss_amount": self.inner.expected_loss_amount,
            "var_fraction": self.inner.var_fraction,
            "var_amount": self.inner.var_amount,
            "expected_shortfall_fraction": self.inner.expected_shortfall_fraction,
            "expected_shortfall_amount": self.inner.expected_shortfall_amount,
            "prob_attachment_breached": self.inner.prob_attachment_breached,
            "prob_full_writedown": self.inner.prob_full_writedown,
        });
        serde_object_to_single_row_dataframe_with_schema(
            py,
            &row,
            &[
                "attachment",
                "detachment",
                "tranche_notional",
                "expected_loss_fraction",
                "expected_loss_amount",
                "var_fraction",
                "var_amount",
                "expected_shortfall_fraction",
                "expected_shortfall_amount",
                "prob_attachment_breached",
                "prob_full_writedown",
            ],
        )
    }

    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("TrancheLossStatistics", &self.inner)
    }
}

wire_methods!(
    PyTrancheLossStatistics,
    TrancheLossStatistics,
    "TrancheLossStatistics",
    module = "finstack_quant.models.correlation"
);
impl_repr_html_via_dataframe!(PyTrancheLossStatistics);

/// Read exposures from a ``list[CreditExposure]`` or a ``pandas.DataFrame``.
///
/// A DataFrame's columns are the ``CreditExposure`` serde field names
/// (``id``, ``notional``, ``default_probability``, ``lgd`` and
/// ``factor_loadings`` holding one list per name); each record is
/// deserialized through the Rust type.
fn extract_exposures(exposures: &Bound<'_, PyAny>) -> PyResult<Vec<CreditExposure>> {
    if let Ok(typed) = exposures.extract::<Vec<PyCreditExposure>>() {
        return Ok(typed.into_iter().map(|exposure| exposure.inner).collect());
    }
    let py = exposures.py();
    let df_type = py.import("pandas")?.getattr("DataFrame")?;
    if !exposures.is_instance(&df_type)? {
        return Err(pyo3::exceptions::PyTypeError::new_err(
            "exposures must be a list of CreditExposure or a pandas DataFrame with columns \
             id, notional, default_probability, lgd and factor_loadings",
        ));
    }
    let records = exposures.call_method1("to_dict", ("records",))?;
    crate::bindings::module_utils::py_to_serde(py, &records, "exposures")
}

#[doc = pydoc!("finstack_quant.models.correlation", "simulate_portfolio_loss")]
#[pyfunction]
#[pyo3(signature = (exposures, config, recovery=None))]
#[pyo3(text_signature = "(exposures, config, recovery=None)")]
fn simulate_portfolio_loss(
    py: Python<'_>,
    exposures: &Bound<'_, PyAny>,
    config: PyPortfolioLossConfig,
    recovery: Option<PyRecoverySpec>,
) -> PyResult<PyPortfolioLossResult> {
    let exposures = extract_exposures(exposures)?;
    let recovery = recovery.map(|spec| spec.inner);
    py.detach(|| corr::simulate_portfolio_loss(&exposures, &config.inner, recovery.as_ref()))
        .map(|inner| PyPortfolioLossResult { inner })
        .map_err(core_to_py)
}

/// Read a correlation matrix given flat row-major, or as nested rows / a 2-D
/// NumPy array flattened by the Rust `flatten_square_matrix` (which owns the
/// `n x n` shape check). A flat input's length is checked by the Rust
/// correlation routine itself.
fn extract_square_matrix(
    py: Python<'_>,
    matrix: &Bound<'_, PyAny>,
    n: usize,
) -> PyResult<Vec<f64>> {
    if let Ok(flat) = matrix.extract::<Vec<f64>>() {
        return Ok(flat);
    }
    crate::bindings::matrix_input::extract_square_matrix(py, matrix, n, "matrix")
}

#[doc = pydoc!("finstack_quant.models.correlation", "correlation_bounds")]
#[pyfunction]
#[pyo3(text_signature = "(p1, p2)")]
fn correlation_bounds(p1: f64, p2: f64) -> PyResult<(f64, f64)> {
    probability::correlation_bounds(p1, p2).map_err(core_to_py)
}

#[doc = pydoc!("finstack_quant.models.correlation", "validate_correlation_matrix")]
#[pyfunction]
#[pyo3(text_signature = "(matrix, n)")]
fn validate_correlation_matrix(
    py: Python<'_>,
    matrix: &Bound<'_, PyAny>,
    n: usize,
) -> PyResult<()> {
    let matrix = extract_square_matrix(py, matrix, n)?;
    py.detach(|| linalg::validate_correlation_matrix(&matrix, n))
        .map_err(|err| correlation_to_py(corr::Error::from(err)))
}

#[doc = pydoc!("finstack_quant.models.correlation", "nearest_correlation")]
#[pyfunction]
#[pyo3(signature = (matrix, n, max_iter=None, tol=None))]
fn nearest_correlation(
    py: Python<'_>,
    matrix: &Bound<'_, PyAny>,
    n: usize,
    max_iter: Option<usize>,
    tol: Option<f64>,
) -> PyResult<Vec<f64>> {
    let matrix = extract_square_matrix(py, matrix, n)?;
    // Single source of truth for the defaults: the Rust
    // `NearestCorrelationOpts::default()` (max_iter = 200, tol = 1e-10).
    let defaults = linalg::NearestCorrelationOpts::default();
    let opts = linalg::NearestCorrelationOpts {
        max_iter: max_iter.unwrap_or(defaults.max_iter),
        tol: tol.unwrap_or(defaults.tol),
    };
    py.detach(|| linalg::nearest_correlation(&matrix, n, opts))
        .map_err(|err| correlation_to_py(corr::Error::from(err)))
}

#[doc = pydoc!("finstack_quant.models.correlation", "cholesky_correlation")]
#[pyfunction]
#[pyo3(text_signature = "(matrix, n)")]
fn cholesky_correlation(py: Python<'_>, matrix: &Bound<'_, PyAny>, n: usize) -> PyResult<Vec<f64>> {
    let matrix = extract_square_matrix(py, matrix, n)?;
    py.detach(|| {
        finstack_quant_core::math::linalg::cholesky_correlation(&matrix, n)
            .map(|f| f.factor_matrix().to_vec())
    })
    .map_err(|e| correlation_to_py(corr::Error::from(e)))
}

/// Register the `correlation` submodule on the parent module.
pub fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = crate::bindings::module_utils::new_submodule(parent, "correlation")?;
    m.setattr(
        "__doc__",
        pydoc!("finstack_quant.models.correlation", "__module__"),
    )?;

    m.add_class::<PyCopulaSpec>()?;
    m.add_class::<PyCopula>()?;
    m.add_class::<PyRecoverySpec>()?;
    m.add_class::<PyRecoveryModel>()?;
    m.add_class::<PyLatentFactorSpec>()?;
    m.add_class::<PyLatentFactorKind>()?;
    m.add_class::<PyLatentSingleFactor>()?;
    m.add_class::<PyLatentTwoFactor>()?;
    m.add_class::<PyLatentMultiFactor>()?;
    m.add_class::<PyCorrelatedBernoulli>()?;
    m.add_class::<PyCreditExposure>()?;
    m.add_class::<PyPortfolioLossConfig>()?;
    m.add_class::<PyPortfolioLossResult>()?;
    m.add_class::<PyTrancheLossStatistics>()?;
    m.add("MAX_PORTFOLIO_LOSS_PATHS", corr::MAX_PORTFOLIO_LOSS_PATHS)?;
    m.add_function(wrap_pyfunction!(correlation_bounds, &m)?)?;
    m.add_function(wrap_pyfunction!(validate_correlation_matrix, &m)?)?;
    m.add_function(wrap_pyfunction!(nearest_correlation, &m)?)?;
    m.add_function(wrap_pyfunction!(cholesky_correlation, &m)?)?;
    m.add_function(wrap_pyfunction!(simulate_portfolio_loss, &m)?)?;

    let all = PyList::new(
        py,
        [
            "Copula",
            "CopulaSpec",
            "CorrelatedBernoulli",
            "CreditExposure",
            "LatentFactorKind",
            "LatentFactorSpec",
            "LatentMultiFactor",
            "LatentSingleFactor",
            "LatentTwoFactor",
            "MAX_PORTFOLIO_LOSS_PATHS",
            "PortfolioLossConfig",
            "PortfolioLossResult",
            "RecoveryModel",
            "RecoverySpec",
            "TrancheLossStatistics",
            "cholesky_correlation",
            "correlation_bounds",
            "nearest_correlation",
            "simulate_portfolio_loss",
            "validate_correlation_matrix",
        ],
    )?;
    m.setattr("__all__", all)?;

    crate::bindings::module_utils::attach_submodule(
        parent,
        &m,
        crate::bindings::module_utils::Exposure::Python,
    )?;

    Ok(())
}
