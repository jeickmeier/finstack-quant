//! Portfolio loss simulation, correlated Bernoulli, and correlation matrix Python bindings.

use super::copula::PyCopulaSpec;
use super::recovery::PyRecoverySpec;
use crate::bindings::core::table::PyArrowTable;
use crate::bindings::macros::{py_getters, tabular_methods, wire_methods};
use crate::bindings::pydoc::pydoc;
use crate::bindings::table_builders::TableBuilder;
use crate::errors::{core_to_py, correlation_to_py};
use pyo3::prelude::*;

use finstack_quant_core::math::linalg;
use finstack_quant_core::math::probability::{self, CorrelatedBernoulli};
use finstack_quant_models::correlation::CreditExposure;
use finstack_quant_models::correlation::{
    self as corr, PortfolioLossConfig, PortfolioLossResult, TrancheLossStatistics,
};

#[doc = pydoc!("finstack_quant.models.correlation._matrix", "CorrelatedBernoulli")]
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
    #[doc = pydoc!("finstack_quant.models.correlation._matrix", "CorrelatedBernoulli.__init__")]
    #[new]
    #[pyo3(text_signature = "(p1, p2, correlation)")]
    fn new(p1: f64, p2: f64, correlation: f64) -> PyResult<Self> {
        Ok(Self {
            inner: CorrelatedBernoulli::new(p1, p2, correlation).map_err(core_to_py)?,
        })
    }

    #[doc = pydoc!("finstack_quant.models.correlation._matrix", "CorrelatedBernoulli.joint_probabilities")]
    fn joint_probabilities(&self) -> (f64, f64, f64, f64) {
        self.inner.joint_probabilities()
    }

    #[doc = pydoc!("finstack_quant.models.correlation._matrix", "CorrelatedBernoulli.conditional_p2_given_x1")]
    fn conditional_p2_given_x1(&self) -> f64 {
        self.inner.conditional_p2_given_x1()
    }

    #[doc = pydoc!("finstack_quant.models.correlation._matrix", "CorrelatedBernoulli.conditional_p1_given_x2")]
    fn conditional_p1_given_x2(&self) -> f64 {
        self.inner.conditional_p1_given_x2()
    }

    #[doc = pydoc!("finstack_quant.models.correlation._matrix", "CorrelatedBernoulli.sample_from_uniform")]
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

py_getters!(PyCorrelatedBernoulli |s| {
    #[doc = pydoc!("finstack_quant.models.correlation._matrix", "CorrelatedBernoulli.p1")]
    p1 -> f64 = s.inner.p1();

    #[doc = pydoc!("finstack_quant.models.correlation._matrix", "CorrelatedBernoulli.p2")]
    p2 -> f64 = s.inner.p2();

    #[doc = pydoc!("finstack_quant.models.correlation._matrix", "CorrelatedBernoulli.correlation")]
    correlation -> f64 = s.inner.correlation();

    #[doc = pydoc!("finstack_quant.models.correlation._matrix", "CorrelatedBernoulli.joint_p11")]
    joint_p11 -> f64 = s.inner.joint_p11();

    #[doc = pydoc!("finstack_quant.models.correlation._matrix", "CorrelatedBernoulli.joint_p10")]
    joint_p10 -> f64 = s.inner.joint_p10();

    #[doc = pydoc!("finstack_quant.models.correlation._matrix", "CorrelatedBernoulli.joint_p01")]
    joint_p01 -> f64 = s.inner.joint_p01();

    #[doc = pydoc!("finstack_quant.models.correlation._matrix", "CorrelatedBernoulli.joint_p00")]
    joint_p00 -> f64 = s.inner.joint_p00();
});

#[doc = pydoc!("finstack_quant.models.correlation._matrix", "CreditExposure")]
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
    #[doc = pydoc!("finstack_quant.models.correlation._matrix", "CreditExposure.__init__")]
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

    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("CreditExposure", &self.inner)
    }
}

py_getters!(PyCreditExposure |s| {
    #[doc = pydoc!("finstack_quant.models.correlation._matrix", "CreditExposure.id")]
    id -> String = s.inner.id.clone();

    #[doc = pydoc!("finstack_quant.models.correlation._matrix", "CreditExposure.notional")]
    notional -> f64 = s.inner.notional;

    #[doc = pydoc!("finstack_quant.models.correlation._matrix", "CreditExposure.default_probability")]
    default_probability -> f64 = s.inner.default_probability;

    #[doc = pydoc!("finstack_quant.models.correlation._matrix", "CreditExposure.lgd")]
    lgd -> f64 = s.inner.lgd;

    #[doc = pydoc!("finstack_quant.models.correlation._matrix", "CreditExposure.factor_loadings")]
    factor_loadings -> Vec<f64> = s.inner.factor_loadings.clone();
});

wire_methods!(
    PyCreditExposure,
    CreditExposure,
    "CreditExposure",
    module = "finstack_quant.models.correlation._matrix"
);

#[doc = pydoc!("finstack_quant.models.correlation._matrix", "PortfolioLossConfig")]
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
    #[doc = pydoc!("finstack_quant.models.correlation._matrix", "PortfolioLossConfig.__init__")]
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

    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("PortfolioLossConfig", &self.inner)
    }
}

py_getters!(PyPortfolioLossConfig |s| {
    #[doc = pydoc!("finstack_quant.models.correlation._matrix", "PortfolioLossConfig.num_paths")]
    num_paths -> usize = s.inner.num_paths;

    #[doc = pydoc!("finstack_quant.models.correlation._matrix", "PortfolioLossConfig.seed")]
    seed -> u64 = s.inner.seed;

    #[doc = pydoc!("finstack_quant.models.correlation._matrix", "PortfolioLossConfig.confidence")]
    confidence -> f64 = s.inner.confidence;

    #[doc = pydoc!("finstack_quant.models.correlation._matrix", "PortfolioLossConfig.copula")]
    copula -> PyCopulaSpec = PyCopulaSpec::from_inner(s.inner.copula.clone());
});

wire_methods!(
    PyPortfolioLossConfig,
    PortfolioLossConfig,
    "PortfolioLossConfig",
    module = "finstack_quant.models.correlation._matrix"
);

#[doc = pydoc!("finstack_quant.models.correlation._matrix", "PortfolioLossResult")]
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
    #[doc = pydoc!("finstack_quant.models.correlation._matrix", "PortfolioLossResult.from_losses")]
    #[staticmethod]
    #[pyo3(text_signature = "(losses, confidence)")]
    fn from_losses(losses: Vec<f64>, confidence: f64) -> PyResult<Self> {
        PortfolioLossResult::from_losses(losses, confidence)
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.correlation._matrix", "PortfolioLossResult.tranche_loss_statistics")]
    #[pyo3(text_signature = "(attach_pct, detach_pct, pool_notional)")]
    fn tranche_loss_statistics(
        &self,
        py: Python<'_>,
        attach_pct: f64,
        detach_pct: f64,
        pool_notional: f64,
    ) -> PyResult<PyTrancheLossStatistics> {
        py.detach(|| {
            self.inner
                .tranche_loss_statistics(attach_pct, detach_pct, pool_notional)
        })
        .map(|inner| PyTrancheLossStatistics { inner })
        .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.correlation._matrix", "PortfolioLossResult.to_summary_table")]
    fn to_summary_table(&self) -> PyResult<PyArrowTable> {
        let num_paths = i64::try_from(self.inner.losses.len())
            .map_err(|e| crate::errors::value_error(format!("num_paths overflow: {e}")))?;
        TableBuilder::new()
            .f64("expected_loss", vec![self.inner.expected_loss])?
            .f64("var", vec![self.inner.var])?
            .f64("expected_shortfall", vec![self.inner.expected_shortfall])?
            .f64("confidence", vec![self.inner.confidence])?
            .i64("num_paths", vec![num_paths])?
            .build()
    }

    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("PortfolioLossResult", &self.inner)
    }
}

py_getters!(PyPortfolioLossResult |s| {
    #[doc = pydoc!("finstack_quant.models.correlation._matrix", "PortfolioLossResult.losses")]
    losses -> Vec<f64> = s.inner.losses.clone();

    #[doc = pydoc!("finstack_quant.models.correlation._matrix", "PortfolioLossResult.expected_loss")]
    expected_loss -> f64 = s.inner.expected_loss;

    #[doc = pydoc!("finstack_quant.models.correlation._matrix", "PortfolioLossResult.var")]
    var -> f64 = s.inner.var;

    #[doc = pydoc!("finstack_quant.models.correlation._matrix", "PortfolioLossResult.expected_shortfall")]
    expected_shortfall -> f64 = s.inner.expected_shortfall;

    #[doc = pydoc!("finstack_quant.models.correlation._matrix", "PortfolioLossResult.confidence")]
    confidence -> f64 = s.inner.confidence;
});

impl PyPortfolioLossResult {
    fn to_arrow_table(&self) -> PyResult<PyArrowTable> {
        TableBuilder::new()
            .f64("loss", self.inner.losses.clone())?
            .build()
    }
}

tabular_methods!(
    PyPortfolioLossResult,
    "PortfolioLossResult",
    module = "finstack_quant.models.correlation._matrix"
);
wire_methods!(
    PyPortfolioLossResult,
    PortfolioLossResult,
    "PortfolioLossResult",
    module = "finstack_quant.models.correlation._matrix"
);

#[doc = pydoc!("finstack_quant.models.correlation._matrix", "TrancheLossStatistics")]
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
    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("TrancheLossStatistics", &self.inner)
    }
}

py_getters!(PyTrancheLossStatistics |s| {
    #[doc = pydoc!("finstack_quant.models.correlation._matrix", "TrancheLossStatistics.attachment")]
    attachment -> f64 = s.inner.attachment;

    #[doc = pydoc!("finstack_quant.models.correlation._matrix", "TrancheLossStatistics.detachment")]
    detachment -> f64 = s.inner.detachment;

    #[doc = pydoc!("finstack_quant.models.correlation._matrix", "TrancheLossStatistics.tranche_notional")]
    tranche_notional -> f64 = s.inner.tranche_notional;

    #[doc = pydoc!("finstack_quant.models.correlation._matrix", "TrancheLossStatistics.expected_loss_fraction")]
    expected_loss_fraction -> f64 = s.inner.expected_loss_fraction;

    #[doc = pydoc!("finstack_quant.models.correlation._matrix", "TrancheLossStatistics.expected_loss_amount")]
    expected_loss_amount -> f64 = s.inner.expected_loss_amount;

    #[doc = pydoc!("finstack_quant.models.correlation._matrix", "TrancheLossStatistics.var_fraction")]
    var_fraction -> f64 = s.inner.var_fraction;

    #[doc = pydoc!("finstack_quant.models.correlation._matrix", "TrancheLossStatistics.var_amount")]
    var_amount -> f64 = s.inner.var_amount;

    #[doc = pydoc!("finstack_quant.models.correlation._matrix", "TrancheLossStatistics.expected_shortfall_fraction")]
    expected_shortfall_fraction -> f64 = s.inner.expected_shortfall_fraction;

    #[doc = pydoc!("finstack_quant.models.correlation._matrix", "TrancheLossStatistics.expected_shortfall_amount")]
    expected_shortfall_amount -> f64 = s.inner.expected_shortfall_amount;

    #[doc = pydoc!("finstack_quant.models.correlation._matrix", "TrancheLossStatistics.prob_attachment_breached")]
    prob_attachment_breached -> f64 = s.inner.prob_attachment_breached;

    #[doc = pydoc!("finstack_quant.models.correlation._matrix", "TrancheLossStatistics.prob_full_writedown")]
    prob_full_writedown -> f64 = s.inner.prob_full_writedown;
});

impl PyTrancheLossStatistics {
    fn to_arrow_table(&self) -> PyResult<PyArrowTable> {
        TableBuilder::new()
            .f64("attachment", vec![self.inner.attachment])?
            .f64("detachment", vec![self.inner.detachment])?
            .f64("tranche_notional", vec![self.inner.tranche_notional])?
            .f64(
                "expected_loss_fraction",
                vec![self.inner.expected_loss_fraction],
            )?
            .f64(
                "expected_loss_amount",
                vec![self.inner.expected_loss_amount],
            )?
            .f64("var_fraction", vec![self.inner.var_fraction])?
            .f64("var_amount", vec![self.inner.var_amount])?
            .f64(
                "expected_shortfall_fraction",
                vec![self.inner.expected_shortfall_fraction],
            )?
            .f64(
                "expected_shortfall_amount",
                vec![self.inner.expected_shortfall_amount],
            )?
            .f64(
                "prob_attachment_breached",
                vec![self.inner.prob_attachment_breached],
            )?
            .f64("prob_full_writedown", vec![self.inner.prob_full_writedown])?
            .build()
    }
}

tabular_methods!(
    PyTrancheLossStatistics,
    "TrancheLossStatistics",
    module = "finstack_quant.models.correlation._matrix"
);
wire_methods!(
    PyTrancheLossStatistics,
    TrancheLossStatistics,
    "TrancheLossStatistics",
    module = "finstack_quant.models.correlation._matrix"
);

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
    if let Some(records) = crate::bindings::dataframe_utils::table_to_py_records(py, exposures)? {
        return crate::bindings::module_utils::py_to_serde(py, &records, "exposures");
    }
    Err(pyo3::exceptions::PyTypeError::new_err(
        "exposures must be a list of CreditExposure or a pandas DataFrame, Polars DataFrame, or Arrow table with columns id, notional, default_probability, lgd and factor_loadings",
    ))
}

#[doc = pydoc!("finstack_quant.models.correlation._matrix", "simulate_portfolio_loss")]
#[pyfunction]
#[pyo3(signature = (exposures, config, recovery=None))]
#[pyo3(text_signature = "(exposures, config, recovery=None)")]
pub(super) fn simulate_portfolio_loss(
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

#[doc = pydoc!("finstack_quant.models.correlation._matrix", "correlation_bounds")]
#[pyfunction]
#[pyo3(text_signature = "(p1, p2)")]
pub(super) fn correlation_bounds(p1: f64, p2: f64) -> PyResult<(f64, f64)> {
    probability::correlation_bounds(p1, p2).map_err(core_to_py)
}

#[doc = pydoc!("finstack_quant.models.correlation._matrix", "validate_correlation_matrix")]
#[pyfunction]
#[pyo3(text_signature = "(matrix, n)")]
pub(super) fn validate_correlation_matrix(
    py: Python<'_>,
    matrix: &Bound<'_, PyAny>,
    n: usize,
) -> PyResult<()> {
    let matrix = extract_square_matrix(py, matrix, n)?;
    py.detach(|| linalg::validate_correlation_matrix(&matrix, n))
        .map_err(|err| correlation_to_py(corr::Error::from(err)))
}

#[doc = pydoc!("finstack_quant.models.correlation._matrix", "nearest_correlation")]
#[pyfunction]
#[pyo3(signature = (matrix, n, max_iter=None, tol=None))]
pub(super) fn nearest_correlation(
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

#[doc = pydoc!("finstack_quant.models.correlation._matrix", "cholesky_correlation")]
#[pyfunction]
#[pyo3(text_signature = "(matrix, n)")]
pub(super) fn cholesky_correlation(
    py: Python<'_>,
    matrix: &Bound<'_, PyAny>,
    n: usize,
) -> PyResult<Vec<f64>> {
    let matrix = extract_square_matrix(py, matrix, n)?;
    py.detach(|| {
        finstack_quant_core::math::linalg::cholesky_correlation(&matrix, n)
            .map(|f| f.factor_matrix().to_vec())
    })
    .map_err(|e| correlation_to_py(corr::Error::from(e)))
}
