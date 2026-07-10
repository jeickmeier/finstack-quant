//! Python bindings for the credit-correlation module.
//!
//! Exposes copula models, recovery models, factor models, and joint
//! probability utilities to Python under `finstack_quant.valuations.correlation`,
//! mirroring the Rust module [`finstack_quant_valuations::correlation`].

use crate::errors::{display_to_py, value_error};
use pyo3::prelude::*;
use pyo3::types::{PyList, PyModule, PyType};

use finstack_quant_valuations::correlation::{
    self as corr, Copula, CopulaSpec, CorrelatedBernoulli, LatentFactorKind, LatentFactorSpec,
    LatentMultiFactor, LatentSingleFactor, LatentTwoFactor, RecoveryModel, RecoverySpec,
};

// ---------------------------------------------------------------------------
// CopulaSpec
// ---------------------------------------------------------------------------

/// Copula model specification for configuration and deferred construction.
///
/// Use class methods to create a spec, then call `build()` to get a `Copula`.
#[pyclass(
    name = "CopulaSpec",
    module = "finstack_quant.valuations.correlation",
    frozen,
    from_py_object
)]
#[derive(Clone, Debug)]
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
    /// The ``df`` parameter must be greater than 2 (required for finite
    /// variance).  Typical calibration range for CDX tranches is 4–10.
    #[classmethod]
    #[pyo3(text_signature = "(cls, df)")]
    fn student_t(_cls: &Bound<'_, PyType>, df: f64) -> PyResult<Self> {
        CopulaSpec::student_t(df)
            .map(Self::from_inner)
            .map_err(display_to_py)
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

    /// Multi-factor Gaussian copula with sector structure.
    #[classmethod]
    #[pyo3(text_signature = "(cls, num_factors)")]
    fn multi_factor(_cls: &Bound<'_, PyType>, num_factors: usize) -> Self {
        Self::from_inner(CopulaSpec::multi_factor(num_factors))
    }

    /// Build a concrete `Copula` from this specification.
    fn build(&self) -> PyResult<PyCopula> {
        self.inner
            .build()
            .map(|inner| PyCopula {
                inner,
                spec: self.inner.clone(),
            })
            .map_err(display_to_py)
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

    fn __repr__(&self) -> String {
        format!("CopulaSpec({:?})", self.inner)
    }
}

// ---------------------------------------------------------------------------
// Copula (trait object wrapper)
// ---------------------------------------------------------------------------

/// Concrete copula model for portfolio default correlation.
///
/// Obtain an instance via ``CopulaSpec.build()``.
#[pyclass(
    name = "Copula",
    module = "finstack_quant.valuations.correlation",
    frozen
)]
pub struct PyCopula {
    /// Boxed trait object.
    pub(crate) inner: Box<dyn Copula + Send + Sync>,
    /// Originating spec, retained so concrete-model-only diagnostics
    /// (`stress_correlation_proxy`) can be dispatched.
    pub(crate) spec: CopulaSpec,
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
            .map_err(display_to_py)
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
        match &self.spec {
            CopulaSpec::RandomFactorLoading { loading_volatility } => {
                Ok(corr::RandomFactorLoadingCopula::new(*loading_volatility)
                    .stress_correlation_proxy(correlation))
            }
            _ => Err(value_error(format!(
                "stress_correlation_proxy is only defined for the Random Factor Loading \
                 copula, got '{}'",
                self.inner.model_name()
            ))),
        }
    }

    fn __repr__(&self) -> String {
        format!("Copula('{}')", self.inner.model_name())
    }
}

// ---------------------------------------------------------------------------
// RecoverySpec
// ---------------------------------------------------------------------------

/// Recovery model specification for configuration and deferred construction.
#[pyclass(
    name = "RecoverySpec",
    module = "finstack_quant.valuations.correlation",
    frozen,
    from_py_object
)]
#[derive(Clone, Debug)]
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
            .map_err(display_to_py)
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
            .map_err(display_to_py)
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

    fn __repr__(&self) -> String {
        format!("RecoverySpec({:?})", self.inner)
    }
}

// ---------------------------------------------------------------------------
// RecoveryModel (trait object wrapper)
// ---------------------------------------------------------------------------

/// Concrete recovery model for credit portfolio pricing.
///
/// Obtain an instance via ``RecoverySpec.build()``.
#[pyclass(
    name = "RecoveryModel",
    module = "finstack_quant.valuations.correlation",
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

// ---------------------------------------------------------------------------
// LatentFactorSpec
// ---------------------------------------------------------------------------

/// Factor model specification for configuration and deferred construction.
#[pyclass(
    name = "LatentFactorSpec",
    module = "finstack_quant.valuations.correlation",
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
            .map_err(display_to_py)
    }

    fn __repr__(&self) -> String {
        format!("LatentFactorSpec({:?})", self.inner)
    }
}

// ---------------------------------------------------------------------------
// LatentFactorKind (concrete dispatch wrapper)
// ---------------------------------------------------------------------------

/// Concrete factor model for correlated behavior.
///
/// Obtain an instance via ``LatentFactorSpec.build()``.
#[pyclass(
    name = "LatentFactorKind",
    module = "finstack_quant.valuations.correlation",
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

// ---------------------------------------------------------------------------
// Concrete factor models with extra methods
// ---------------------------------------------------------------------------

/// Single-factor model (common market factor).
#[pyclass(
    name = "LatentSingleFactor",
    module = "finstack_quant.valuations.correlation",
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
    module = "finstack_quant.valuations.correlation",
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
    module = "finstack_quant.valuations.correlation",
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
            .map_err(display_to_py)
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

// ---------------------------------------------------------------------------
// CorrelatedBernoulli
// ---------------------------------------------------------------------------

/// Correlated Bernoulli distribution for two binary events.
///
/// Wraps ``finstack_quant_core::math::probability::CorrelatedBernoulli``.
#[pyclass(
    name = "CorrelatedBernoulli",
    module = "finstack_quant.valuations.correlation",
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
    fn new(p1: f64, p2: f64, correlation: f64) -> Self {
        Self {
            inner: CorrelatedBernoulli::new(p1, p2, correlation),
        }
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
    fn sample_from_uniform(&self, u: f64) -> (u8, u8) {
        self.inner.sample_from_uniform(u)
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

// ---------------------------------------------------------------------------
// Module-level functions
// ---------------------------------------------------------------------------

/// Fréchet-Hoeffding correlation bounds for two Bernoulli marginals.
///
/// Returns ``(rho_min, rho_max)`` — the feasible correlation range.
#[pyfunction]
#[pyo3(text_signature = "(p1, p2)")]
fn correlation_bounds(p1: f64, p2: f64) -> (f64, f64) {
    corr::correlation_bounds(p1, p2)
}

/// Joint probabilities for two correlated Bernoulli variables.
///
/// Returns ``(p11, p10, p01, p00)`` that sums to 1 and exactly
/// preserves the marginals.
#[pyfunction]
#[pyo3(text_signature = "(p1, p2, correlation)")]
fn joint_probabilities(p1: f64, p2: f64, correlation: f64) -> (f64, f64, f64, f64) {
    corr::joint_probabilities(p1, p2, correlation)
}

/// Validate a correlation matrix (flattened row-major).
///
/// Raises ``ValueError`` if the matrix is invalid.
#[pyfunction]
#[pyo3(text_signature = "(matrix, n)")]
fn validate_correlation_matrix(py: Python<'_>, matrix: Vec<f64>, n: usize) -> PyResult<()> {
    py.detach(|| corr::validate_correlation_matrix(&matrix, n))
        .map_err(display_to_py)
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
/// matrix : list[float]
///     Flattened row-major ``n x n`` input matrix.
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
///     If the input is not square, is grossly asymmetric, the diagonal is
///     far from 1, or the projection does not converge.
#[pyfunction]
#[pyo3(signature = (matrix, n, max_iter=None, tol=None))]
fn nearest_correlation(
    py: Python<'_>,
    matrix: Vec<f64>,
    n: usize,
    max_iter: Option<usize>,
    tol: Option<f64>,
) -> PyResult<Vec<f64>> {
    // Single source of truth for the defaults: the Rust
    // `NearestCorrelationOpts::default()` (max_iter = 200, tol = 1e-10).
    let defaults = corr::NearestCorrelationOpts::default();
    let opts = corr::NearestCorrelationOpts {
        max_iter: max_iter.unwrap_or(defaults.max_iter),
        tol: tol.unwrap_or(defaults.tol),
    };
    py.detach(|| corr::nearest_correlation_matrix(&matrix, n, opts))
        .map_err(display_to_py)
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
/// Raises ``ValueError`` if the matrix shape is wrong or the matrix is
/// indefinite.
#[pyfunction]
#[pyo3(text_signature = "(matrix, n)")]
fn cholesky_decompose(py: Python<'_>, matrix: Vec<f64>, n: usize) -> PyResult<Vec<f64>> {
    py.detach(|| corr::cholesky_decompose(&matrix, n).map(|f| f.factor_matrix().to_vec()))
        .map_err(display_to_py)
}

// ---------------------------------------------------------------------------
// Module registration
// ---------------------------------------------------------------------------

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
    m.add_function(wrap_pyfunction!(correlation_bounds, &m)?)?;
    m.add_function(wrap_pyfunction!(joint_probabilities, &m)?)?;
    m.add_function(wrap_pyfunction!(validate_correlation_matrix, &m)?)?;
    m.add_function(wrap_pyfunction!(nearest_correlation, &m)?)?;
    m.add_function(wrap_pyfunction!(cholesky_decompose, &m)?)?;

    let all = PyList::new(
        py,
        [
            "CopulaSpec",
            "Copula",
            "RecoverySpec",
            "RecoveryModel",
            "LatentFactorSpec",
            "LatentFactorKind",
            "LatentSingleFactor",
            "LatentTwoFactor",
            "LatentMultiFactor",
            "CorrelatedBernoulli",
            "correlation_bounds",
            "joint_probabilities",
            "validate_correlation_matrix",
            "nearest_correlation",
            "cholesky_decompose",
        ],
    )?;
    m.setattr("__all__", all)?;

    crate::bindings::module_utils::register_submodule(
        py,
        parent,
        &m,
        "correlation",
        "finstack_quant.finstack_quant.valuations",
        crate::bindings::module_utils::ParentNameSource::Package,
    )?;

    Ok(())
}
