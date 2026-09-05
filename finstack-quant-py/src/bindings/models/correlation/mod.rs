//! Python bindings for the credit-correlation module.
//!
//! Exposes copula models, recovery models, factor models, and joint
//! probability utilities to Python under `finstack_quant.models.correlation`,
//! mirroring the Rust module [`finstack_quant_models::correlation`].

use crate::bindings::pandas_utils::{
    dict_to_dataframe, serde_object_to_single_row_dataframe_with_schema,
};
use crate::errors::{core_to_py, correlation_to_py, serde_json_to_py, value_error};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PyModule, PyType};

use finstack_quant_models::correlation::CreditExposure;
use finstack_quant_models::correlation::{
    self as corr, Copula, CopulaSpec, CorrelatedBernoulli, LatentFactorKind, LatentFactorSpec,
    LatentMultiFactor, LatentSingleFactor, LatentTwoFactor, PortfolioLossConfig,
    PortfolioLossResult, RecoveryModel, RecoverySpec, TrancheLossStatistics,
};

/// Copula model specification for configuration and deferred construction.
///
/// Use class methods to create a spec, then call `build()` to get a `Copula`.
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
    /// One-factor Gaussian copula (market standard).
    #[classmethod]
    fn gaussian(_cls: &Bound<'_, PyType>) -> Self {
        Self::from_inner(CopulaSpec::gaussian())
    }

    /// Student-t copula with specified degrees of freedom.
    ///
    /// ``degrees_of_freedom`` must be finite and greater than 2 (required for
    /// finite variance). Typical calibration range for CDX tranches is 4-10.
    ///
    /// Raises ``ValueError`` when ``degrees_of_freedom`` is not finite or
    /// ``<= 2``.
    #[classmethod]
    #[pyo3(text_signature = "(cls, degrees_of_freedom)")]
    fn student_t(_cls: &Bound<'_, PyType>, degrees_of_freedom: f64) -> PyResult<Self> {
        CopulaSpec::student_t(degrees_of_freedom)
            .map(Self::from_inner)
            .map_err(correlation_to_py)
    }

    /// Random Factor Loading copula with stochastic correlation.
    ///
    /// The ``loading_vol`` parameter controls the volatility of the factor
    /// loading and is clamped to ``[0, 0.5]``.
    #[classmethod]
    #[pyo3(text_signature = "(cls, loading_vol)")]
    fn random_factor_loading(_cls: &Bound<'_, PyType>, loading_vol: f64) -> Self {
        Self::from_inner(CopulaSpec::random_factor_loading(loading_vol))
    }

    /// Global-plus-sector two-factor Gaussian copula.
    #[classmethod]
    #[pyo3(text_signature = "(cls)")]
    fn multi_factor(_cls: &Bound<'_, PyType>) -> Self {
        Self::from_inner(CopulaSpec::multi_factor())
    }

    /// Build a concrete `Copula` from this specification.
    fn build(&self) -> PyResult<PyCopula> {
        self.inner
            .build()
            .map(|inner| PyCopula { inner })
            .map_err(correlation_to_py)
    }

    /// ``True`` if this is a Gaussian spec.
    #[getter]
    fn is_gaussian(&self) -> bool {
        self.inner.is_gaussian()
    }

    /// ``True`` if this is a Student-t spec.
    #[getter]
    fn is_student_t(&self) -> bool {
        self.inner.is_student_t()
    }

    /// ``True`` if this is a Random Factor Loading spec.
    #[getter]
    fn is_rfl(&self) -> bool {
        self.inner.is_rfl()
    }

    /// ``True`` if this is a Multi-factor spec.
    #[getter]
    fn is_multi_factor(&self) -> bool {
        self.inner.is_multi_factor()
    }

    /// Serialize to the canonical JSON wire format (``{"type": ...}``).
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|err| serde_json_to_py(err, "CopulaSpec serialization failed"))
    }

    /// Deserialize from JSON produced by ``to_json``.
    ///
    /// Raises ``ValueError`` when the payload is malformed.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        serde_json::from_str(json)
            .map(Self::from_inner)
            .map_err(|err| serde_json_to_py(err, "invalid CopulaSpec JSON"))
    }

    /// Support ``pickle``.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("CopulaSpec", &self.inner)
    }
}

/// Concrete copula model for portfolio default correlation.
///
/// Obtain an instance via ``CopulaSpec.build()``.
#[pyclass(name = "Copula", module = "finstack_quant.models.correlation", frozen)]
pub struct PyCopula {
    /// Boxed trait object.
    pub(crate) inner: Box<dyn Copula + Send + Sync>,
}

#[pymethods]
impl PyCopula {
    /// Conditional default probability given factor realization(s).
    ///
    /// P(default | Z) where the default threshold is typically Φ⁻¹(PD).
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

    /// Number of systematic factors in the model.
    #[getter]
    fn num_factors(&self) -> usize {
        self.inner.num_factors()
    }

    /// Model name for diagnostics.
    #[getter]
    fn model_name(&self) -> &'static str {
        self.inner.model_name()
    }

    /// Strict lower-tail dependence coefficient ``λ_L`` at the given
    /// correlation.
    ///
    /// Returns ``nan`` when the model has no closed-form ``λ_L`` (Random
    /// Factor Loading); check ``math.isnan()`` before using the result. For
    /// the RFL heuristic stress gauge use
    /// :meth:`stress_correlation_proxy` instead.
    #[pyo3(text_signature = "(self, correlation)")]
    fn tail_dependence(&self, correlation: f64) -> f64 {
        self.inner.tail_dependence(correlation)
    }

    /// Heuristic stress-correlation proxy for the Random Factor Loading
    /// copula.
    ///
    /// This is **not** the strict copula lower-tail-dependence coefficient
    /// ``λ_L`` (which has no closed form for RFL — ``tail_dependence``
    /// returns ``nan``). It gauges the extra correlation mass in the
    /// high-loading tail and vanishes in the Gaussian (``loading_vol = 0``)
    /// limit.
    ///
    /// Raises ``ValueError`` for non-RFL copulas.
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

/// Recovery model specification for configuration and deferred construction.
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
    /// Constant recovery rate.
    ///
    /// Raises ``ValueError`` if ``rate`` is not finite or lies outside
    /// ``[0, 1]``.
    #[classmethod]
    #[pyo3(text_signature = "(cls, rate)")]
    fn constant(_cls: &Bound<'_, PyType>, rate: f64) -> PyResult<Self> {
        RecoverySpec::constant(rate)
            .map(Self::from_inner)
            .map_err(correlation_to_py)
    }

    /// Market-correlated (Andersen-Sidenius) stochastic recovery.
    ///
    /// Raises ``ValueError`` if ``mean`` is not finite or lies outside
    /// ``[0, 1]``, or if ``vol`` / ``correlation`` are not finite.
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

    /// Market-standard stochastic recovery (40% mean, 25% vol, +40% corr —
    /// recovery falls in stress under the canonical low-factor-stress
    /// convention).
    #[classmethod]
    fn market_standard_stochastic(_cls: &Bound<'_, PyType>) -> Self {
        Self::from_inner(RecoverySpec::market_standard_stochastic())
    }

    /// Location-parameter recovery rate of this spec.
    ///
    /// For a constant spec this is the constant rate. For a
    /// market-correlated spec this returns the ``mean`` input — the target
    /// recovery at factor ``Z = 0`` — which differs from the
    /// Jensen-corrected unconditional mean ``E_Z[R(Z)]`` whenever the factor
    /// sensitivity is non-zero. For the true unconditional mean call
    /// ``build().expected_recovery``.
    #[getter]
    fn expected_recovery(&self) -> f64 {
        self.inner.expected_recovery()
    }

    /// Build a concrete `RecoveryModel` from this specification.
    fn build(&self) -> PyRecoveryModel {
        PyRecoveryModel {
            inner: self.inner.build(),
        }
    }

    /// Serialize to the canonical JSON wire format (``{"type": ...}``).
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|err| serde_json_to_py(err, "RecoverySpec serialization failed"))
    }

    /// Deserialize from JSON produced by ``to_json``.
    ///
    /// Raises ``ValueError`` when the payload is malformed.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        serde_json::from_str(json)
            .map(Self::from_inner)
            .map_err(|err| serde_json_to_py(err, "invalid RecoverySpec JSON"))
    }

    /// Support ``pickle``.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("RecoverySpec", &self.inner)
    }
}

/// Concrete recovery model for credit portfolio pricing.
///
/// Obtain an instance via ``RecoverySpec.build()``.
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
    /// Expected (unconditional) recovery rate.
    #[getter]
    fn expected_recovery(&self) -> f64 {
        self.inner.expected_recovery()
    }

    /// Recovery conditional on the systematic market factor.
    #[pyo3(text_signature = "(self, market_factor)")]
    fn conditional_recovery(&self, market_factor: f64) -> f64 {
        self.inner.conditional_recovery(market_factor)
    }

    /// Loss given default (1 − recovery).
    #[getter]
    fn lgd(&self) -> f64 {
        self.inner.lgd()
    }

    /// Conditional LGD given market factor.
    #[pyo3(text_signature = "(self, market_factor)")]
    fn conditional_lgd(&self, market_factor: f64) -> f64 {
        self.inner.conditional_lgd(market_factor)
    }

    /// Recovery-rate volatility scale (0 for constant models).
    #[getter]
    fn recovery_volatility(&self) -> f64 {
        self.inner.recovery_volatility()
    }

    /// Whether recovery varies with the market factor.
    #[getter]
    fn is_stochastic(&self) -> bool {
        self.inner.is_stochastic()
    }

    /// Model name for diagnostics.
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

/// Factor model specification for configuration and deferred construction.
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
    /// Single-factor model specification.
    #[classmethod]
    #[pyo3(text_signature = "(cls, volatility, mean_reversion)")]
    fn single_factor(_cls: &Bound<'_, PyType>, volatility: f64, mean_reversion: f64) -> Self {
        Self::from_inner(LatentFactorSpec::single_factor(volatility, mean_reversion))
    }

    /// Two-factor model (prepayment + credit) specification.
    #[classmethod]
    #[pyo3(text_signature = "(cls, prepay_vol, credit_vol, correlation)")]
    fn two_factor(
        _cls: &Bound<'_, PyType>,
        prepay_vol: f64,
        credit_vol: f64,
        correlation: f64,
    ) -> Self {
        Self::from_inner(LatentFactorSpec::two_factor(
            prepay_vol,
            credit_vol,
            correlation,
        ))
    }

    /// Number of factors implied by this specification.
    #[getter]
    fn num_factors(&self) -> usize {
        self.inner.num_factors()
    }

    /// Build a concrete factor model from this specification.
    ///
    /// Raises ``ValueError`` if a multi-factor specification contains an
    /// invalid volatility vector or correlation matrix.
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

/// Concrete factor model for correlated behavior.
///
/// Obtain an instance via ``LatentFactorSpec.build()``.
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
    /// Number of factors in the model.
    #[getter]
    fn num_factors(&self) -> usize {
        self.inner.num_factors()
    }

    /// Factor correlation matrix (flattened row-major).
    #[getter]
    fn correlation_matrix(&self) -> Vec<f64> {
        self.inner.correlation_matrix().to_vec()
    }

    /// Factor volatilities.
    #[getter]
    fn volatilities(&self) -> Vec<f64> {
        self.inner.volatilities().to_vec()
    }

    /// Factor names for reporting.
    #[getter]
    fn factor_names(&self) -> Vec<&'static str> {
        self.inner.factor_names()
    }

    /// Model name for diagnostics.
    #[getter]
    fn model_name(&self) -> &'static str {
        self.inner.model_name()
    }

    /// Diagonal factor contribution for a single standard-normal draw.
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

/// Single-factor model (common market factor).
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
    /// Create a single-factor model.
    #[new]
    #[pyo3(text_signature = "(volatility, mean_reversion)")]
    fn new(volatility: f64, mean_reversion: f64) -> Self {
        Self {
            inner: LatentSingleFactor::new(volatility, mean_reversion),
        }
    }

    /// Factor volatility.
    #[getter]
    fn volatility(&self) -> f64 {
        self.inner.volatility()
    }

    /// Mean reversion speed.
    #[getter]
    fn mean_reversion(&self) -> f64 {
        self.inner.mean_reversion()
    }

    /// Number of factors (always 1).
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

/// Two-factor model for prepayment and credit.
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
    /// Create a two-factor model.
    #[new]
    #[pyo3(text_signature = "(prepay_vol, credit_vol, correlation)")]
    fn new(prepay_vol: f64, credit_vol: f64, correlation: f64) -> Self {
        Self {
            inner: LatentTwoFactor::new(prepay_vol, credit_vol, correlation),
        }
    }

    /// Standard RMBS calibration.
    #[classmethod]
    fn rmbs_standard(_cls: &Bound<'_, PyType>) -> Self {
        Self {
            inner: LatentTwoFactor::rmbs_standard(),
        }
    }

    /// Standard CLO calibration.
    #[classmethod]
    fn clo_standard(_cls: &Bound<'_, PyType>) -> Self {
        Self {
            inner: LatentTwoFactor::clo_standard(),
        }
    }

    /// Prepayment factor volatility.
    #[getter]
    fn prepay_vol(&self) -> f64 {
        self.inner.prepay_vol()
    }

    /// Credit factor volatility.
    #[getter]
    fn credit_vol(&self) -> f64 {
        self.inner.credit_vol()
    }

    /// Factor correlation.
    #[getter]
    fn correlation(&self) -> f64 {
        self.inner.correlation()
    }

    /// Number of factors (always 2).
    #[getter]
    fn num_factors(&self) -> usize {
        self.inner.num_factors()
    }

    /// Cholesky ``L[1][0]`` for correlated factor generation.
    #[getter]
    fn cholesky_l10(&self) -> f64 {
        self.inner.cholesky_l10()
    }

    /// Cholesky ``L[1][1]`` for correlated factor generation.
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

/// Multi-factor model with custom correlation structure.
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
    /// Create a validated multi-factor model.
    ///
    /// Raises ``ValueError`` if the correlation matrix is invalid.
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

    /// Create an uncorrelated (identity) multi-factor model.
    #[classmethod]
    #[pyo3(text_signature = "(cls, num_factors, volatilities)")]
    fn uncorrelated(_cls: &Bound<'_, PyType>, num_factors: usize, volatilities: Vec<f64>) -> Self {
        Self {
            inner: LatentMultiFactor::uncorrelated(num_factors, volatilities),
        }
    }

    /// Number of factors.
    #[getter]
    fn num_factors(&self) -> usize {
        self.inner.num_factors()
    }

    /// Factor correlation matrix (flattened row-major).
    #[getter]
    fn correlation_matrix(&self) -> Vec<f64> {
        self.inner.correlation_matrix().to_vec()
    }

    /// Factor volatilities.
    #[getter]
    fn volatilities(&self) -> Vec<f64> {
        self.inner.volatilities().to_vec()
    }

    /// Generate correlated factor values from independent standard normal draws.
    ///
    /// Raises ``ValueError`` if ``independent_z`` does not contain exactly
    /// ``num_factors`` draws.
    #[pyo3(text_signature = "(self, independent_z)")]
    fn generate_correlated_factors(&self, independent_z: Vec<f64>) -> PyResult<Vec<f64>> {
        let expected = self.inner.num_factors();
        if independent_z.len() != expected {
            return Err(value_error(format!(
                "independent_z must contain exactly {expected} draws (one per factor), \
                 got {}",
                independent_z.len()
            )));
        }
        Ok(self.inner.generate_correlated_factors(&independent_z))
    }

    fn __repr__(&self) -> String {
        format!("LatentMultiFactor(n={})", self.inner.num_factors())
    }
}

/// Correlated Bernoulli distribution for two binary events.
///
/// Wraps ``finstack_quant_core::math::probability::CorrelatedBernoulli``.
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
    /// Create a correlated Bernoulli distribution.
    ///
    /// Correlation is clamped to the Fréchet-Hoeffding bounds for the
    /// given marginal probabilities.
    #[new]
    #[pyo3(text_signature = "(p1, p2, correlation)")]
    fn new(p1: f64, p2: f64, correlation: f64) -> PyResult<Self> {
        Ok(Self {
            inner: CorrelatedBernoulli::new(p1, p2, correlation).map_err(core_to_py)?,
        })
    }

    /// Marginal probability of event 1.
    #[getter]
    fn p1(&self) -> f64 {
        self.inner.p1()
    }

    /// Marginal probability of event 2.
    #[getter]
    fn p2(&self) -> f64 {
        self.inner.p2()
    }

    /// Correlation between events.
    #[getter]
    fn correlation(&self) -> f64 {
        self.inner.correlation()
    }

    /// Caller-requested correlation before Fréchet-Hoeffding clamping.
    #[getter]
    fn requested_correlation(&self) -> f64 {
        self.inner.requested_correlation()
    }

    /// P(X₁=1, X₂=1).
    #[getter]
    fn joint_p11(&self) -> f64 {
        self.inner.joint_p11()
    }

    /// P(X₁=1, X₂=0).
    #[getter]
    fn joint_p10(&self) -> f64 {
        self.inner.joint_p10()
    }

    /// P(X₁=0, X₂=1).
    #[getter]
    fn joint_p01(&self) -> f64 {
        self.inner.joint_p01()
    }

    /// P(X₁=0, X₂=0).
    #[getter]
    fn joint_p00(&self) -> f64 {
        self.inner.joint_p00()
    }

    /// All four joint probabilities ``(p11, p10, p01, p00)``.
    fn joint_probabilities(&self) -> (f64, f64, f64, f64) {
        self.inner.joint_probabilities()
    }

    /// Conditional probability P(X₂=1 | X₁=1).
    fn conditional_p2_given_x1(&self) -> f64 {
        self.inner.conditional_p2_given_x1()
    }

    /// Conditional probability P(X₁=1 | X₂=1).
    fn conditional_p1_given_x2(&self) -> f64 {
        self.inner.conditional_p1_given_x2()
    }

    /// Sample a pair of correlated binary outcomes from a uniform ``[0,1]`` draw.
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

/// One name in a finite credit portfolio.
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

    /// Stable identifier for this exposure.
    #[getter]
    fn id(&self) -> String {
        self.inner.id.clone()
    }

    /// Exposure at default, in the portfolio currency.
    #[getter]
    fn notional(&self) -> f64 {
        self.inner.notional
    }

    /// Marginal probability of default over the horizon, in [0, 1].
    #[getter]
    fn default_probability(&self) -> f64 {
        self.inner.default_probability
    }

    /// Loss given default, as a fraction of notional in [0, 1].
    #[getter]
    fn lgd(&self) -> f64 {
        self.inner.lgd
    }

    /// Systematic factor loadings driving correlated defaults.
    #[getter]
    fn factor_loadings(&self) -> Vec<f64> {
        self.inner.factor_loadings.clone()
    }

    /// Serialize to the canonical JSON wire format.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(|error| value_error(error.to_string()))
    }

    /// Deserialize from JSON produced by `to_json`.
    ///
    /// Completes the wire round-trip, which is also what makes this type
    /// picklable (see `__reduce__`).
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: finstack_quant_models::correlation::CreditExposure = serde_json::from_str(json)
            .map_err(|err| serde_json_to_py(err, "invalid CreditExposure JSON"))?;
        Ok(Self { inner })
    }

    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Identify this value in notebooks and logs.
    ///
    /// Rendered from the wire representation, so the fields shown are the
    /// fields `to_json()` names. Collections are summarised by length; use
    /// `to_json()` or a DataFrame exit when the contents matter.
    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("CreditExposure", &self.inner)
    }
}

/// Settings for deterministic portfolio credit-loss simulation.
///
/// ``num_paths`` must be between 1 and
/// ``MAX_PORTFOLIO_LOSS_PATHS`` (inclusive).
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

    /// Number of simulated paths.
    #[getter]
    fn num_paths(&self) -> usize {
        self.inner.num_paths
    }

    /// RNG seed; the same seed reproduces the same paths exactly.
    #[getter]
    fn seed(&self) -> u64 {
        self.inner.seed
    }

    /// Confidence level for VaR and expected shortfall, in (0, 1).
    #[getter]
    fn confidence(&self) -> f64 {
        self.inner.confidence
    }

    /// Dependence structure used to couple the marginal defaults.
    #[getter]
    fn copula(&self) -> PyCopulaSpec {
        PyCopulaSpec::from_inner(self.inner.copula.clone())
    }

    /// Serialize to the canonical JSON wire format.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(|error| value_error(error.to_string()))
    }

    /// Deserialize from JSON produced by `to_json`.
    ///
    /// Completes the wire round-trip, which is also what makes this type
    /// picklable (see `__reduce__`).
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: finstack_quant_models::correlation::PortfolioLossConfig =
            serde_json::from_str(json)
                .map_err(|err| serde_json_to_py(err, "invalid PortfolioLossConfig JSON"))?;
        Ok(Self { inner })
    }

    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Identify this value in notebooks and logs.
    ///
    /// Rendered from the wire representation, so the fields shown are the
    /// fields `to_json()` names. Collections are summarised by length; use
    /// `to_json()` or a DataFrame exit when the contents matter.
    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("PortfolioLossConfig", &self.inner)
    }
}

/// Simulated loss distribution and loss-positive VaR/expected shortfall.
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
    /// Simulated portfolio loss per path, in the ascending path order Rust produced.
    #[getter]
    fn losses(&self) -> Vec<f64> {
        self.inner.losses.clone()
    }

    /// Mean simulated loss.
    #[getter]
    fn expected_loss(&self) -> f64 {
        self.inner.expected_loss
    }

    /// Value at risk at `confidence`, loss-positive (larger is worse).
    #[getter]
    fn var(&self) -> f64 {
        self.inner.var
    }

    /// Probability-weighted mean loss in the worst `1 - confidence` tail, loss-positive.
    #[getter]
    fn expected_shortfall(&self) -> f64 {
        self.inner.expected_shortfall
    }

    /// Confidence level for VaR and expected shortfall, in (0, 1).
    #[getter]
    fn confidence(&self) -> f64 {
        self.inner.confidence
    }

    /// Tranche loss statistics for one attachment/detachment pair.
    ///
    /// ``attachment`` and ``detachment`` are fractions of pool notional in
    /// ``[0, 1]`` — a 0-3% equity tranche is ``(0.0, 0.03)``. Each path's
    /// pool loss fraction ``L`` maps to
    /// ``clamp(L - attachment, 0, width) / width``, and the resulting
    /// distribution is aggregated at this result's own ``confidence``.
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

    /// Primary table: the simulated loss distribution.
    ///
    /// Alias of :meth:`to_distribution_dataframe`. Every tabular result type
    /// in the library answers ``to_dataframe()``; the one-row aggregate view
    /// stays on :meth:`to_summary_dataframe`.
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        self.to_distribution_dataframe(py)
    }

    /// Export the simulated loss distribution as a pandas ``DataFrame``.
    ///
    /// Columns: ``loss``.
    ///
    /// One row per simulated path, indexed by path id (a ``RangeIndex``), in
    /// the ascending path order Rust produced — so repeated exports of the
    /// same result are identical. Feed it straight to ``df["loss"].hist()`` or
    /// ``df["loss"].quantile(...)``.
    ///
    /// The aggregate statistics are not repeated per row; see
    /// :meth:`to_summary_dataframe`.
    fn to_distribution_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let data = PyDict::new(py);
        data.set_item("loss", self.inner.losses.clone())?;
        dict_to_dataframe(py, &data, None)
    }

    /// Export the aggregate loss statistics as a single-row pandas ``DataFrame``.
    ///
    /// ``var`` must be read as ``df["var"]``: attribute access resolves to
    /// ``DataFrame.var`` (the variance method), not the column.
    ///
    /// Columns: ``expected_loss``, ``var``, ``expected_shortfall``,
    /// ``confidence``, ``num_paths``.
    ///
    /// One simulation is one flat record, so a one-row frame is the right
    /// shape: ``pd.concat`` over several correlation or recovery assumptions
    /// gives a comparison table directly.
    ///
    /// ``var`` and ``expected_shortfall`` are loss-positive, matching the Rust
    /// convention: a larger number is a worse outcome.
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

    /// Serialize to the canonical JSON wire format.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(|error| value_error(error.to_string()))
    }

    /// Deserialize from JSON produced by `to_json`.
    ///
    /// Completes the wire round-trip, which is also what makes this type
    /// picklable (see `__reduce__`).
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: finstack_quant_models::correlation::PortfolioLossResult =
            serde_json::from_str(json)
                .map_err(|err| serde_json_to_py(err, "invalid PortfolioLossResult JSON"))?;
        Ok(Self { inner })
    }

    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Identify this value in notebooks and logs.
    ///
    /// Rendered from the wire representation, so the fields shown are the
    /// fields `to_json()` names. Collections are summarised by length; use
    /// `to_json()` or a DataFrame exit when the contents matter.
    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("PortfolioLossResult", &self.inner)
    }
}

/// Expected loss, tail statistics, and breach probabilities for one tranche.
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
    /// Attachment point as a fraction of portfolio notional.
    #[getter]
    fn attachment(&self) -> f64 {
        self.inner.attachment
    }

    /// Detachment point as a fraction of portfolio notional.
    #[getter]
    fn detachment(&self) -> f64 {
        self.inner.detachment
    }

    /// Tranche width in currency: `(detachment - attachment)` x portfolio notional.
    #[getter]
    fn tranche_notional(&self) -> f64 {
        self.inner.tranche_notional
    }

    /// Mean tranche loss as a fraction of `tranche_notional`.
    #[getter]
    fn expected_loss_fraction(&self) -> f64 {
        self.inner.expected_loss_fraction
    }

    /// Mean tranche loss in currency.
    #[getter]
    fn expected_loss_amount(&self) -> f64 {
        self.inner.expected_loss_amount
    }

    /// Tranche value at risk as a fraction of `tranche_notional`.
    #[getter]
    fn var_fraction(&self) -> f64 {
        self.inner.var_fraction
    }

    /// Tranche value at risk in currency, loss-positive.
    #[getter]
    fn var_amount(&self) -> f64 {
        self.inner.var_amount
    }

    /// Tranche expected shortfall as a fraction of `tranche_notional`.
    #[getter]
    fn expected_shortfall_fraction(&self) -> f64 {
        self.inner.expected_shortfall_fraction
    }

    /// Tranche expected shortfall in currency, loss-positive.
    #[getter]
    fn expected_shortfall_amount(&self) -> f64 {
        self.inner.expected_shortfall_amount
    }

    /// Probability the tranche takes any loss at all.
    #[getter]
    fn prob_attachment_breached(&self) -> f64 {
        self.inner.prob_attachment_breached
    }

    /// Probability the tranche is written down in full.
    #[getter]
    fn prob_full_writedown(&self) -> f64 {
        self.inner.prob_full_writedown
    }

    /// Export as a single-row pandas ``DataFrame``.
    ///
    /// Columns: ``attachment``, ``detachment``, ``tranche_notional``,
    /// ``expected_loss_fraction``, ``expected_loss_amount``, ``var_fraction``,
    /// ``var_amount``, ``expected_shortfall_fraction``,
    /// ``expected_shortfall_amount``, ``prob_attachment_breached``,
    /// ``prob_full_writedown``.
    ///
    /// These statistics describe ONE tranche, so a one-row frame is the right
    /// shape. Build the capital-structure table by stacking tranches::
    ///
    ///     pd.concat(
    ///         [
    ///             result.tranche_loss_statistics(a, d, pool).to_dataframe()
    ///             for a, d in [(0.0, 0.03), (0.03, 0.07), (0.07, 1.0)]
    ///         ],
    ///         ignore_index=True,
    ///     )
    ///
    /// ``*_fraction`` columns are shares of the tranche's own notional;
    /// ``*_amount`` columns are in the pool-notional unit passed to
    /// :meth:`PortfolioLossResult.tranche_loss_statistics`.
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

    /// Serialize to the canonical JSON wire format.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(|error| value_error(error.to_string()))
    }

    /// Render as an HTML table in Jupyter notebooks.
    ///
    /// Delegates to the frame from `to_dataframe`, so pandas' own row/column
    /// truncation applies and a large result stays a small repr. Returns
    /// `None` if the frame cannot be built, which makes IPython fall back to
    /// `__repr__` instead of raising from the display hook.
    fn _repr_html_(&self, py: Python<'_>) -> Option<String> {
        let frame = self.to_dataframe(py).ok()?;
        frame.call_method0("_repr_html_").ok()?.extract().ok()
    }

    /// Deserialize from JSON produced by `to_json`.
    ///
    /// Completes the wire round-trip, which is also what makes this type
    /// picklable (see `__reduce__`).
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: finstack_quant_models::correlation::TrancheLossStatistics =
            serde_json::from_str(json)
                .map_err(|err| serde_json_to_py(err, "invalid TrancheLossStatistics JSON"))?;
        Ok(Self { inner })
    }

    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Identify this value in notebooks and logs.
    ///
    /// Rendered from the wire representation, so the fields shown are the
    /// fields `to_json()` names. Collections are summarised by length; use
    /// `to_json()` or a DataFrame exit when the contents matter.
    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("TrancheLossStatistics", &self.inner)
    }
}

fn record_item<'py>(record: &Bound<'py, PyDict>, key: &str) -> PyResult<Bound<'py, PyAny>> {
    record
        .get_item(key)?
        .ok_or_else(|| value_error(format!("exposure record is missing `{key}`")))
}

/// Read exposures from a ``list[CreditExposure]`` or a ``pandas.DataFrame``.
///
/// DataFrame records need ``id``, ``notional``, ``lgd``, a PD column named
/// ``pd`` or ``default_probability``, and loadings as either
/// ``factor_loading`` (one scalar per name) or ``factor_loadings`` (a list
/// per name).
fn extract_exposures(exposures: &Bound<'_, PyAny>) -> PyResult<Vec<CreditExposure>> {
    if let Ok(typed) = exposures.extract::<Vec<PyCreditExposure>>() {
        return Ok(typed.into_iter().map(|exposure| exposure.inner).collect());
    }
    let py = exposures.py();
    let df_type = py.import("pandas")?.getattr("DataFrame")?;
    if !exposures.is_instance(&df_type)? {
        return Err(pyo3::exceptions::PyTypeError::new_err(
            "exposures must be a list of CreditExposure or a pandas DataFrame with columns \
             id, notional, pd (or default_probability), lgd and factor_loading(s)",
        ));
    }
    let columns: Vec<String> = exposures
        .getattr("columns")?
        .call_method0("tolist")?
        .extract()?;
    let has = |name: &str| columns.iter().any(|column| column == name);
    let pd_column = if has("pd") {
        "pd"
    } else if has("default_probability") {
        "default_probability"
    } else {
        return Err(value_error(
            "exposures DataFrame needs a `pd` or `default_probability` column",
        ));
    };
    for required in ["id", "notional", "lgd"] {
        if !has(required) {
            return Err(value_error(format!(
                "exposures DataFrame is missing the `{required}` column"
            )));
        }
    }
    let loadings_column = if has("factor_loadings") {
        "factor_loadings"
    } else if has("factor_loading") {
        "factor_loading"
    } else {
        return Err(value_error(
            "exposures DataFrame needs a `factor_loading` (scalar) or `factor_loadings` (list) column",
        ));
    };
    let records: Vec<Bound<'_, PyDict>> =
        exposures.call_method1("to_dict", ("records",))?.extract()?;
    records
        .iter()
        .map(|record| {
            let get = |key: &str| record_item(record, key);
            let loadings_value = get(loadings_column)?;
            let factor_loadings = match loadings_value.extract::<f64>() {
                Ok(scalar) => vec![scalar],
                Err(_) => loadings_value.extract::<Vec<f64>>().map_err(|_| {
                    value_error(format!(
                        "`{loadings_column}` must hold a float or a list of floats per exposure"
                    ))
                })?,
            };
            Ok(CreditExposure {
                id: get("id")?.str()?.to_string(),
                notional: get("notional")?.extract()?,
                default_probability: get(pd_column)?.extract()?,
                lgd: get("lgd")?.extract()?,
                factor_loadings,
            })
        })
        .collect()
}

/// Simulate a finite portfolio's loss-positive credit-loss distribution.
///
/// ``exposures`` is a ``list[CreditExposure]`` or a ``pandas.DataFrame`` with
/// columns ``id``, ``notional``, ``pd`` (or ``default_probability``), ``lgd``
/// and ``factor_loading`` (scalar) or ``factor_loadings`` (list).
///
/// Raises ``TypeError`` for other exposure containers, ``ValueError`` for
/// missing columns or invalid simulation inputs.
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
    py.detach(|| match recovery {
        Some(recovery) => {
            corr::simulate_portfolio_loss_with_recovery(&exposures, &config.inner, &recovery.inner)
        }
        None => corr::simulate_portfolio_loss(&exposures, &config.inner),
    })
    .map(|inner| PyPortfolioLossResult { inner })
    .map_err(core_to_py)
}

/// Read a correlation matrix given either flat row-major or as nested rows /
/// a 2-D array, validating the shape against ``n``.
fn extract_square_matrix(matrix: &Bound<'_, PyAny>, n: usize) -> PyResult<Vec<f64>> {
    if let Ok(flat) = matrix.extract::<Vec<f64>>() {
        if flat.len() != n * n {
            return Err(value_error(format!(
                "matrix has {} entries but n={n} requires {} (flat row-major n*n)",
                flat.len(),
                n * n
            )));
        }
        return Ok(flat);
    }
    let rows: Vec<Vec<f64>> = matrix.extract().map_err(|_| {
        value_error("matrix must be a flat row-major list of floats or a 2-D list/array of rows")
    })?;
    if rows.len() != n || rows.iter().any(|row| row.len() != n) {
        let widths: Vec<usize> = rows.iter().map(Vec::len).collect();
        return Err(value_error(format!(
            "matrix must be {n}x{n} for n={n}; got {} rows with widths {widths:?}",
            rows.len()
        )));
    }
    Ok(rows.into_iter().flatten().collect())
}

/// Fréchet-Hoeffding correlation bounds for two Bernoulli marginals.
///
/// Returns ``(rho_min, rho_max)`` — the feasible correlation range.
#[pyfunction]
#[pyo3(text_signature = "(p1, p2)")]
fn correlation_bounds(p1: f64, p2: f64) -> PyResult<(f64, f64)> {
    corr::correlation_bounds(p1, p2).map_err(core_to_py)
}

/// Joint probabilities for two correlated Bernoulli variables.
///
/// Returns ``(p11, p10, p01, p00)`` that sums to 1 and exactly
/// preserves the marginals.
#[pyfunction]
#[pyo3(text_signature = "(p1, p2, correlation)")]
fn joint_probabilities(p1: f64, p2: f64, correlation: f64) -> PyResult<(f64, f64, f64, f64)> {
    corr::joint_probabilities(p1, p2, correlation).map_err(core_to_py)
}

/// Validate a correlation matrix.
///
/// ``matrix`` is either flat row-major (length ``n * n``) or a 2-D
/// ``n x n`` list/array of rows.
///
/// Raises ``ValueError`` if the shape does not match ``n`` or the matrix is
/// not a valid correlation matrix (diagonal, bounds, symmetry, PSD).
#[pyfunction]
#[pyo3(text_signature = "(matrix, n)")]
fn validate_correlation_matrix(
    py: Python<'_>,
    matrix: &Bound<'_, PyAny>,
    n: usize,
) -> PyResult<()> {
    let matrix = extract_square_matrix(matrix, n)?;
    py.detach(|| corr::validate_correlation_matrix(&matrix, n))
        .map_err(|err| correlation_to_py(corr::Error::from(err)))
}

/// Nearest correlation matrix (Higham 2002) for a near-PSD input.
///
/// Given a symmetric matrix ``matrix`` (flattened row-major, length ``n*n``)
/// that is approximately a correlation matrix but has small PSD violations,
/// returns the nearest valid correlation matrix (symmetric, unit diagonal,
/// PSD) in Frobenius norm using Higham's alternating-projection algorithm
/// with Dykstra's correction.
///
/// Typical use: repair a shrinkage or thresholded sample correlation that
/// fails Cholesky by a small margin. Gross violations (asymmetric by more
/// than ``1e-6``, diagonal further than ``1e-3`` from ``1.0``) raise rather
/// than being silently reshaped.
///
/// Parameters
/// ----------
/// matrix : list[float] | list[list[float]]
///     Flat row-major ``n x n`` input matrix, or a 2-D ``n x n`` list/array.
/// n : int
///     Matrix dimension.
/// max_iter : int, optional
///     Maximum alternating-projection iterations. Defaults to the Rust
///     ``NearestCorrelationOpts::default()`` value (currently ``200``).
/// tol : float, optional
///     Frobenius-norm tolerance between successive iterates. Defaults to
///     the Rust ``NearestCorrelationOpts::default()`` value (currently
///     ``1e-10``).
///
/// Returns
/// -------
/// list[float]
///     Flattened row-major ``n x n`` correlation matrix with unit diagonal
///     and PSD.
///
/// Raises
/// ------
/// ValueError
///     If the input shape does not match ``n``, is grossly asymmetric, or
///     the diagonal is far from 1.
/// RuntimeError
///     If the projection does not converge within ``max_iter`` iterations.
#[pyfunction]
#[pyo3(signature = (matrix, n, max_iter=None, tol=None))]
fn nearest_correlation(
    py: Python<'_>,
    matrix: &Bound<'_, PyAny>,
    n: usize,
    max_iter: Option<usize>,
    tol: Option<f64>,
) -> PyResult<Vec<f64>> {
    let matrix = extract_square_matrix(matrix, n)?;
    // Single source of truth for the defaults: the Rust
    // `NearestCorrelationOpts::default()` (max_iter = 200, tol = 1e-10).
    let defaults = corr::NearestCorrelationOpts::default();
    let opts = corr::NearestCorrelationOpts {
        max_iter: max_iter.unwrap_or(defaults.max_iter),
        tol: tol.unwrap_or(defaults.tol),
    };
    py.detach(|| corr::nearest_correlation_matrix(&matrix, n, opts))
        .map_err(|err| correlation_to_py(corr::Error::from(err)))
}

/// Pivoted Cholesky decomposition of a correlation matrix (flattened
/// row-major).
///
/// Returns a factor matrix ``L`` (flat ``list[float]``, row-major, original
/// variable order) satisfying ``L @ L.T == matrix``. Because diagonal
/// pivoting is used to handle near-singular and positive-semidefinite
/// matrices, the unpermuted factor is **not** guaranteed to be lower
/// triangular — it may contain non-zero entries above the diagonal. The
/// effective numerical rank is not surfaced through this function.
///
/// ``matrix`` is either flat row-major (length ``n * n``) or a 2-D
/// ``n x n`` list/array of rows.
///
/// Raises ``ValueError`` if the matrix shape is wrong, an entry is non-finite,
/// or the matrix is indefinite. The message preserves the core Cholesky
/// diagnostic, including dimensions or the offending position and value.
#[pyfunction]
#[pyo3(text_signature = "(matrix, n)")]
fn cholesky_decompose(py: Python<'_>, matrix: &Bound<'_, PyAny>, n: usize) -> PyResult<Vec<f64>> {
    let matrix = extract_square_matrix(matrix, n)?;
    py.detach(|| corr::cholesky_decompose(&matrix, n).map(|f| f.factor_matrix().to_vec()))
        .map_err(correlation_to_py)
}

/// Register the `correlation` submodule on the parent module.
pub fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(py, "correlation")?;
    m.setattr(
        "__doc__",
        "Correlation infrastructure: copulas, factor models, recovery models.",
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
    m.add_function(wrap_pyfunction!(joint_probabilities, &m)?)?;
    m.add_function(wrap_pyfunction!(validate_correlation_matrix, &m)?)?;
    m.add_function(wrap_pyfunction!(nearest_correlation, &m)?)?;
    m.add_function(wrap_pyfunction!(cholesky_decompose, &m)?)?;
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
            "cholesky_decompose",
            "correlation_bounds",
            "joint_probabilities",
            "nearest_correlation",
            "simulate_portfolio_loss",
            "validate_correlation_matrix",
        ],
    )?;
    m.setattr("__all__", all)?;

    crate::bindings::module_utils::register_submodule(
        py,
        parent,
        &m,
        "correlation",
        "finstack_quant.models",
        crate::bindings::module_utils::ParentNameSource::Package,
    )?;

    Ok(())
}
