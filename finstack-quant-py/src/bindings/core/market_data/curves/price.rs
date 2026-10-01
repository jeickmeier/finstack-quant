//! Price curve bindings.

use finstack_quant_core::market_data::term_structures::PriceCurve;

use std::sync::Arc;

use pyo3::prelude::*;

use super::helpers::{
    columns_to_dataframe, extract_time_point, impl_arc_serde_pymethods,
    impl_repr_html_via_dataframe, parse_day_count, parse_extrapolation, parse_interp_style,
    parse_price_curve_kind, price_curve_kind_name, TimePoint,
};
use crate::bindings::date_utils::{date_to_py, py_to_date};
use crate::errors::core_to_py;

/// Forward price curve for commodities, other price-based assets and
/// volatility indices.
///
/// Stores ``(t, forward_price)`` knots in years from ``base_date`` in absolute
/// price units (or index points for ``kind="vol_index"``). ``price`` accepts a
/// year fraction or a date (converted with the curve day count by Rust).
///
/// Example
/// -------
/// >>> from finstack_quant.core.market_data import PriceCurve
/// >>> curve = PriceCurve("WTI", "2025-01-01", [(0.0, 70.0), (1.0, 72.0)])
/// >>> curve.price(0.5)
/// 71.0
/// >>> vix = PriceCurve("VIX", "2025-01-01", [(0.0, 18.0), (1.0, 21.0)], kind="vol_index")
/// >>> vix.kind
/// 'vol_index'
#[pyclass(
    name = "PriceCurve",
    module = "finstack_quant.core.market_data.curves",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyPriceCurve {
    /// Shared Rust curve.
    pub(crate) inner: Arc<PriceCurve>,
}

impl PyPriceCurve {
    /// Build from an existing `Arc<PriceCurve>`.
    pub(crate) fn from_inner(inner: Arc<PriceCurve>) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyPriceCurve {
    /// Construct a price curve from ``(time_years, forward_price)`` knots.
    ///
    /// Parameters
    /// ----------
    /// id : str
    ///     Unique curve identifier (e.g. ``"WTI-FORWARD"`` or ``"VIX"``).
    /// base_date : datetime.date | str
    ///     Valuation date anchoring ``t = 0``.
    /// knots : list[tuple[float, float]]
    ///     ``(time_years, forward_price)`` pairs in absolute price units. At
    ///     least two knots; the first must be at ``t = 0`` unless ``spot_price`` is given.
    /// kind : str, optional
    ///     ``"price"`` (default; signed prices allowed) or ``"vol_index"``
    ///     (non-negative volatility-index levels in vol points, e.g. ``18.0``).
    /// spot_price : float, optional
    ///     Spot level at ``t = 0``; inferred from a ``t = 0`` knot when omitted.
    ///     An explicit spot is inserted at zero if that knot is absent and
    ///     must match any supplied zero-time knot.
    /// extrapolation : str, optional
    ///     Extrapolation policy; default ``"flat_zero"``.
    /// interp : str, optional
    ///     Interpolation style; default ``"linear"``.
    /// day_count : str, optional
    ///     Day-count convention; default ``"act_365f"``.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If fewer than two knots are given, a knot is non-finite or
    ///     duplicated, spot cannot be inferred or conflicts with a zero-time
    ///     knot, a vol-index level is negative,
    ///     or a label is unknown.
    ///
    /// Example
    /// -------
    /// >>> from finstack_quant.core.market_data import PriceCurve
    /// >>> curve = PriceCurve("WTI", "2025-01-01", [(0.5, 70.0), (1.0, 72.0)], spot_price=69.5)
    /// >>> curve.spot_price
    /// 69.5
    #[new]
    #[expect(
        clippy::too_many_arguments,
        reason = "keyword-only curve options mirror the Rust builder setters"
    )]
    #[pyo3(signature = (id, base_date, knots, *, kind=None, spot_price=None, extrapolation=None, interp=None, day_count=None))]
    fn new(
        id: &str,
        base_date: &Bound<'_, PyAny>,
        knots: Vec<(f64, f64)>,
        kind: Option<&str>,
        spot_price: Option<f64>,
        extrapolation: Option<&str>,
        interp: Option<&str>,
        day_count: Option<&str>,
    ) -> PyResult<Self> {
        let mut builder = PriceCurve::builder(id)
            .base_date(py_to_date(base_date)?)
            .knots(knots);
        if let Some(kind) = kind {
            builder = builder.kind(parse_price_curve_kind(kind)?);
        }
        if let Some(spot) = spot_price {
            builder = builder.spot_price(spot);
        }
        if let Some(extrapolation) = extrapolation {
            builder = builder.extrapolation(parse_extrapolation(extrapolation)?);
        }
        if let Some(interp) = interp {
            builder = builder.interp(parse_interp_style(interp)?);
        }
        if let Some(day_count) = day_count {
            builder = builder.day_count(parse_day_count(day_count)?);
        }
        builder
            .build()
            .map(|curve| Self {
                inner: Arc::new(curve),
            })
            .map_err(core_to_py)
    }

    /// Forward price (or vol-index level) at a year fraction or date.
    ///
    /// Parameters
    /// ----------
    /// t : float | datetime.date | str
    ///     Year fraction from ``base_date``, or a date converted with the curve day count.
    ///
    /// Returns
    /// -------
    /// float
    ///     Price in the curve's units.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If a date precedes ``base_date``.
    #[pyo3(text_signature = "(self, t)")]
    fn price(&self, t: &Bound<'_, PyAny>) -> PyResult<f64> {
        match extract_time_point(t)? {
            TimePoint::Years(t) => Ok(self.inner.price(t)),
            TimePoint::Date(d) => self.inner.price_on_date(d).map_err(core_to_py),
        }
    }

    /// Forward price on a date using the curve day count.
    ///
    /// Parameters
    /// ----------
    /// date : datetime.date | str
    ///     Target date on or after ``base_date``.
    ///
    /// Returns
    /// -------
    /// float
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the year fraction cannot be computed.
    #[pyo3(text_signature = "(self, date)")]
    fn price_on_date(&self, date: &Bound<'_, PyAny>) -> PyResult<f64> {
        self.inner
            .price_on_date(py_to_date(date)?)
            .map_err(core_to_py)
    }

    /// Export knots as a pandas ``DataFrame`` with columns ``t`` (years) and ``price``.
    ///
    /// Returns
    /// -------
    /// pandas.DataFrame
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        columns_to_dataframe(
            py,
            &[
                ("t", self.inner.knots().to_vec()),
                ("price", self.inner.prices().to_vec()),
            ],
        )
    }

    /// Curve identifier string.
    #[getter]
    fn id(&self) -> &str {
        self.inner.id().as_str()
    }

    /// Valuation base date (``datetime.date``).
    #[getter]
    fn base_date<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.base_date())
    }

    /// Curve kind label: ``"price"`` or ``"vol_index"``.
    #[getter]
    fn kind(&self) -> &'static str {
        price_curve_kind_name(self.inner.kind())
    }

    /// Spot price (or vol-index level) at ``t = 0``.
    #[getter]
    fn spot_price(&self) -> f64 {
        self.inner.spot_price()
    }

    /// Knot times in years from ``base_date``, ascending.
    #[getter]
    fn knots(&self) -> Vec<f64> {
        self.inner.knots().to_vec()
    }

    /// Forward prices at each knot (absolute price units or vol points).
    #[getter]
    fn prices(&self) -> Vec<f64> {
        self.inner.prices().to_vec()
    }

    /// Day-count convention label (e.g. ``"act_365f"``).
    #[getter]
    fn day_count(&self) -> String {
        self.inner.day_count().to_string()
    }

    /// Interpolation style label (e.g. ``"linear"``).
    #[getter]
    fn interp_style(&self) -> String {
        self.inner.interp_style().to_string()
    }

    /// Extrapolation policy label (e.g. ``"flat_zero"``).
    #[getter]
    fn extrapolation(&self) -> String {
        self.inner.extrapolation().to_string()
    }

    fn __repr__(&self) -> String {
        format!(
            "PriceCurve(id='{}', kind='{}', base_date='{}', knots={}, spot_price={})",
            self.inner.id().as_str(),
            price_curve_kind_name(self.inner.kind()),
            self.inner.base_date(),
            self.inner.knots().len(),
            self.inner.spot_price()
        )
    }
}

impl_arc_serde_pymethods!(PyPriceCurve, PriceCurve, "PriceCurve");
impl_repr_html_via_dataframe!(PyPriceCurve);
