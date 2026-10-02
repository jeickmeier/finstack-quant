//! Base-correlation curve and credit index data bindings.

use std::sync::Arc;

use finstack_quant_core::market_data::term_structures::{BaseCorrelationCurve, CreditIndexData};
use pyo3::prelude::*;

use super::hazard::PyHazardCurve;
use super::helpers::{
    columns_to_dataframe, impl_arc_serde_pymethods, impl_repr_html_via_dataframe,
};
use crate::errors::core_to_py;

/// Base-correlation curve for synthetic credit index tranche pricing.
///
/// Stores ``(detachment_pct, correlation)`` knots where detachment points are
/// in **percent** of the index notional (``3.0`` for a 0-3% tranche) and
/// correlations are decimals in ``[0, 1]``.
///
/// Example
/// -------
/// >>> from finstack_quant.core.market_data import BaseCorrelationCurve
/// >>> curve = BaseCorrelationCurve("CDX-IG", [(3.0, 0.25), (7.0, 0.40), (10.0, 0.55)])
/// >>> round(curve.correlation(5.0), 4)
/// 0.325
#[pyclass(
    name = "BaseCorrelationCurve",
    module = "finstack_quant.core.market_data.curves",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyBaseCorrelationCurve {
    /// Shared Rust curve.
    pub(crate) inner: Arc<BaseCorrelationCurve>,
}

impl PyBaseCorrelationCurve {
    /// Build from an existing shared Rust curve.
    pub(crate) fn from_inner(inner: Arc<BaseCorrelationCurve>) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyBaseCorrelationCurve {
    /// Construct a base-correlation curve from ``(detachment_pct, correlation)`` knots.
    ///
    /// Parameters
    /// ----------
    /// id : str
    ///     Unique curve identifier (typically index name plus maturity).
    /// knots : list[tuple[float, float]]
    ///     ``(detachment_pct, correlation)`` pairs; detachment in percent of
    ///     notional (``3.0`` for 3%), correlation as a decimal in ``[0, 1]``.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If fewer than two knots are supplied, values are non-finite, correlations
    ///     are outside ``[0, 1]``, or detachments are duplicated or outside ``[0, 100]``.
    ///
    /// Example
    /// -------
    /// >>> from finstack_quant.core.market_data import BaseCorrelationCurve
    /// >>> BaseCorrelationCurve("CDX-IG", [(3.0, 0.25), (10.0, 0.55)]).detachment_points
    /// [3.0, 10.0]
    #[new]
    #[pyo3(signature = (id, knots))]
    fn new(id: &str, knots: Vec<(f64, f64)>) -> PyResult<Self> {
        let curve = BaseCorrelationCurve::builder(id)
            .knots(knots)
            .build()
            .map_err(core_to_py)?;
        Ok(Self {
            inner: Arc::new(curve),
        })
    }

    /// Interpolated base correlation (decimal) at a detachment point.
    ///
    /// Parameters
    /// ----------
    /// detachment_pct : float
    ///     Detachment point in percent of index notional.
    ///
    /// Returns
    /// -------
    /// float
    ///     Base correlation as a decimal in ``[0, 1]``.
    #[pyo3(text_signature = "(self, detachment_pct)")]
    fn correlation(&self, detachment_pct: f64) -> f64 {
        self.inner.correlation(detachment_pct)
    }

    /// Export knots as a pandas ``DataFrame`` with columns ``detachment_pct`` and ``correlation``.
    ///
    /// Returns
    /// -------
    /// pandas.DataFrame
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        columns_to_dataframe(
            py,
            &[
                ("detachment_pct", self.inner.detachment_points().to_vec()),
                ("correlation", self.inner.correlations().to_vec()),
            ],
        )
    }

    /// Curve identifier string.
    #[getter]
    fn id(&self) -> &str {
        self.inner.id().as_str()
    }

    /// Detachment points in percent of index notional, ascending.
    #[getter]
    fn detachment_points(&self) -> Vec<f64> {
        self.inner.detachment_points().to_vec()
    }

    /// Base correlations (decimal) at each detachment point.
    #[getter]
    fn correlations(&self) -> Vec<f64> {
        self.inner.correlations().to_vec()
    }

    /// Interpolation style label.
    #[getter]
    fn interp_style(&self) -> String {
        self.inner.interp_style().to_string()
    }

    /// Extrapolation policy label.
    #[getter]
    fn extrapolation(&self) -> String {
        self.inner.extrapolation().to_string()
    }

    fn __repr__(&self) -> String {
        format!(
            "BaseCorrelationCurve(id='{}', knots={})",
            self.inner.id().as_str(),
            self.inner.detachment_points().len()
        )
    }
}

impl_arc_serde_pymethods!(
    PyBaseCorrelationCurve,
    BaseCorrelationCurve,
    "BaseCorrelationCurve"
);
impl_repr_html_via_dataframe!(PyBaseCorrelationCurve);

/// Credit index data bundle for synthetic tranche pricing.
///
/// Groups the index hazard curve, the base-correlation curve, the number of
/// constituents and the index recovery assumption. The bundle holds shared
/// curve handles and is not JSON-serializable on its own; serialize the
/// ``MarketContext`` it is inserted into instead.
///
/// Example
/// -------
/// >>> from finstack_quant.core.market_data import BaseCorrelationCurve, CreditIndexData, HazardCurve
/// >>> hazard = HazardCurve.flat("CDX-IG", "2025-01-01", 0.01, 0.4)
/// >>> base_corr = BaseCorrelationCurve("CDX-IG-BC", [(3.0, 0.25), (10.0, 0.55)])
/// >>> data = CreditIndexData(125, 0.4, hazard, base_corr)
/// >>> data.num_constituents
/// 125
#[pyclass(
    name = "CreditIndexData",
    module = "finstack_quant.core.market_data.curves",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyCreditIndexData {
    /// Rust credit index bundle (shared, so `MarketContext` getters hand out
    /// `Arc` clones instead of deep copies).
    pub(crate) inner: Arc<CreditIndexData>,
}

impl PyCreditIndexData {
    /// Build from an existing Rust credit-index bundle.
    pub(crate) fn from_inner(inner: Arc<CreditIndexData>) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyCreditIndexData {
    /// Construct homogeneous credit index data.
    ///
    /// Parameters
    /// ----------
    /// num_constituents : int
    ///     Number of names in the index (e.g. ``125`` for CDX IG).
    /// recovery_rate : float
    ///     Index recovery assumption as a decimal fraction in ``[0, 1]``.
    /// index_credit_curve : HazardCurve
    ///     Hazard curve for the index as a whole.
    /// base_correlation_curve : BaseCorrelationCurve
    ///     Base correlations by detachment point.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``num_constituents`` is zero or ``recovery_rate`` is outside ``[0, 1]``.
    ///
    /// Example
    /// -------
    /// >>> from finstack_quant.core.market_data import BaseCorrelationCurve, CreditIndexData, HazardCurve
    /// >>> hazard = HazardCurve.flat("CDX-IG", "2025-01-01", 0.01, 0.4)
    /// >>> base_corr = BaseCorrelationCurve("CDX-IG-BC", [(3.0, 0.25), (10.0, 0.55)])
    /// >>> CreditIndexData(125, 0.4, hazard, base_corr).recovery_rate
    /// 0.4
    #[new]
    #[pyo3(signature = (num_constituents, recovery_rate, index_credit_curve, base_correlation_curve))]
    fn new(
        num_constituents: u16,
        recovery_rate: f64,
        index_credit_curve: &PyHazardCurve,
        base_correlation_curve: &PyBaseCorrelationCurve,
    ) -> PyResult<Self> {
        let data = CreditIndexData::builder()
            .num_constituents(num_constituents)
            .recovery_rate(recovery_rate)
            .index_credit_curve(Arc::clone(&index_credit_curve.inner))
            .base_correlation_curve(Arc::clone(&base_correlation_curve.inner))
            .build()
            .map_err(core_to_py)?;
        Ok(Self {
            inner: Arc::new(data),
        })
    }

    /// Number of constituents in the credit index.
    #[getter]
    fn num_constituents(&self) -> u16 {
        self.inner.num_constituents
    }

    /// Index recovery rate (decimal fraction).
    #[getter]
    fn recovery_rate(&self) -> f64 {
        self.inner.recovery_rate
    }

    /// Hazard curve for the index as a whole.
    #[getter]
    fn index_credit_curve(&self) -> PyHazardCurve {
        PyHazardCurve::from_inner(Arc::clone(&self.inner.index_credit_curve))
    }

    /// Base-correlation curve by detachment point.
    #[getter]
    fn base_correlation_curve(&self) -> PyBaseCorrelationCurve {
        PyBaseCorrelationCurve::from_inner(Arc::clone(&self.inner.base_correlation_curve))
    }

    fn __repr__(&self) -> String {
        format!(
            "CreditIndexData(num_constituents={}, recovery_rate={}, index_credit_curve='{}', base_correlation_curve='{}')",
            self.inner.num_constituents,
            self.inner.recovery_rate,
            self.inner.index_credit_curve.id().as_str(),
            self.inner.base_correlation_curve.id().as_str()
        )
    }
}
