//! Base-correlation curve and credit index data bindings.

use std::collections::BTreeMap;
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
    /// Construct credit index data, optionally with issuer-level detail.
    ///
    /// Without the keyword arguments the bundle is homogeneous: every
    /// constituent uses the index curve, the index recovery and weight
    /// ``1 / num_constituents``. Supplying ``issuer_curves`` enables
    /// heterogeneous (bespoke) tranche pricing.
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
    /// issuer_curves : dict[str, HazardCurve], optional
    ///     Hazard curve per issuer identifier (ticker, CUSIP, ...). When given
    ///     it must cover exactly ``num_constituents`` distinct issuers.
    /// issuer_recovery_rates : dict[str, float], optional
    ///     Recovery per issuer as a decimal in ``[0, 1]``; requires
    ///     ``issuer_curves`` and only its identifiers. Issuers left out use
    ///     ``recovery_rate``.
    /// issuer_weights : dict[str, float], optional
    ///     Non-negative notional weight per issuer (decimal fractions summing
    ///     to ``1.0`` within ``1e-9``); requires ``issuer_curves`` and must
    ///     cover exactly its identifiers.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``num_constituents`` is zero, a recovery is outside ``[0, 1]``,
    ///     the issuer curves do not cover ``num_constituents`` issuers, the
    ///     issuer recoveries or weights name an unknown issuer (or are given
    ///     without ``issuer_curves``), or the weights are negative, do not
    ///     cover every issuer or do not sum to ``1.0``.
    ///
    /// Example
    /// -------
    /// >>> from finstack_quant.core.market_data import BaseCorrelationCurve, CreditIndexData, HazardCurve
    /// >>> hazard = HazardCurve.flat("CDX-IG", "2025-01-01", 0.01, 0.4)
    /// >>> base_corr = BaseCorrelationCurve("CDX-IG-BC", [(3.0, 0.25), (10.0, 0.55)])
    /// >>> CreditIndexData(125, 0.4, hazard, base_corr).recovery_rate
    /// 0.4
    /// >>> data = CreditIndexData(
    /// ...     2, 0.4, hazard, base_corr,
    /// ...     issuer_curves={"A": HazardCurve.flat("A", "2025-01-01", 0.02, 0.3), "B": hazard},
    /// ...     issuer_recovery_rates={"A": 0.3},
    /// ...     issuer_weights={"A": 0.25, "B": 0.75},
    /// ... )
    /// >>> data.issuer_ids(), data.get_issuer_recovery("A"), data.get_issuer_recovery("B")
    /// (['A', 'B'], 0.3, 0.4)
    #[new]
    #[pyo3(signature = (num_constituents, recovery_rate, index_credit_curve, base_correlation_curve, *, issuer_curves=None, issuer_recovery_rates=None, issuer_weights=None))]
    fn new(
        num_constituents: u16,
        recovery_rate: f64,
        index_credit_curve: &PyHazardCurve,
        base_correlation_curve: &PyBaseCorrelationCurve,
        issuer_curves: Option<BTreeMap<String, PyRef<'_, PyHazardCurve>>>,
        issuer_recovery_rates: Option<BTreeMap<String, f64>>,
        issuer_weights: Option<BTreeMap<String, f64>>,
    ) -> PyResult<Self> {
        let mut builder = CreditIndexData::builder()
            .num_constituents(num_constituents)
            .recovery_rate(recovery_rate)
            .index_credit_curve(Arc::clone(&index_credit_curve.inner))
            .base_correlation_curve(Arc::clone(&base_correlation_curve.inner));
        if let Some(curves) = issuer_curves {
            builder = builder.issuer_curves(
                curves
                    .into_iter()
                    .map(|(issuer, curve)| (issuer, Arc::clone(&curve.inner))),
            );
        }
        if let Some(rates) = issuer_recovery_rates {
            builder = builder.issuer_recovery_rates(rates);
        }
        if let Some(weights) = issuer_weights {
            builder = builder.issuer_weights(weights);
        }
        let data = builder.build().map_err(core_to_py)?;
        Ok(Self {
            inner: Arc::new(data),
        })
    }

    /// Hazard curve of one issuer, falling back to the index curve.
    ///
    /// Parameters
    /// ----------
    /// issuer_id : str
    ///     Issuer identifier as supplied in ``issuer_curves``.
    ///
    /// Returns
    /// -------
    /// HazardCurve
    ///     The issuer's curve, or ``index_credit_curve`` when the bundle has
    ///     no curve for ``issuer_id`` (homogeneous assumption).
    #[pyo3(text_signature = "(self, issuer_id)")]
    fn get_issuer_curve(&self, issuer_id: &str) -> PyHazardCurve {
        PyHazardCurve::from_inner(Arc::new(self.inner.get_issuer_curve(issuer_id).clone()))
    }

    /// Whether issuer-level curves are present (heterogeneous pricing mode).
    ///
    /// Returns
    /// -------
    /// bool
    #[pyo3(text_signature = "(self)")]
    fn has_issuer_curves(&self) -> bool {
        self.inner.has_issuer_curves()
    }

    /// Issuer identifiers with their own curve, sorted.
    ///
    /// Returns
    /// -------
    /// list[str]
    ///     Empty for a homogeneous bundle.
    #[pyo3(text_signature = "(self)")]
    fn issuer_ids(&self) -> Vec<String> {
        self.inner.issuer_ids()
    }

    /// Recovery rate of one issuer, falling back to the index recovery.
    ///
    /// Parameters
    /// ----------
    /// issuer_id : str
    ///     Issuer identifier.
    ///
    /// Returns
    /// -------
    /// float
    ///     The issuer's recovery as a decimal, or ``recovery_rate`` when none
    ///     was supplied for ``issuer_id``.
    #[pyo3(text_signature = "(self, issuer_id)")]
    fn get_issuer_recovery(&self, issuer_id: &str) -> f64 {
        self.inner.get_issuer_recovery(issuer_id)
    }

    /// Notional weight of one issuer, falling back to equal weighting.
    ///
    /// Parameters
    /// ----------
    /// issuer_id : str
    ///     Issuer identifier.
    ///
    /// Returns
    /// -------
    /// float
    ///     The issuer's weight as a decimal fraction, or
    ///     ``1 / num_constituents`` when none was supplied for ``issuer_id``.
    #[pyo3(text_signature = "(self, issuer_id)")]
    fn get_issuer_weight(&self, issuer_id: &str) -> f64 {
        self.inner.get_issuer_weight(issuer_id)
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
