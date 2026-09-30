//! Discount curve bindings.

use finstack_quant_core::market_data::term_structures::{DiscountCurve, ValidationMode};

use std::sync::Arc;

use pyo3::prelude::*;

use super::forward::PyForwardCurve;
use super::helpers::{
    columns_to_dataframe, extract_time_point, impl_arc_serde_pymethods,
    impl_repr_html_via_dataframe, parse_compounding, parse_day_count, parse_extrapolation,
    parse_interp_style, TimePoint,
};
use crate::bindings::date_utils::{date_to_py, py_to_date};
use crate::errors::core_to_py;

/// Discount factor curve for present-value calculations.
///
/// Stores ``(t, DF)`` knots in years from ``base_date`` and interpolates
/// between them. Query methods accept either a year fraction (``float``) or a
/// date (``datetime.date`` or ISO ``"YYYY-MM-DD"`` string); dates are converted
/// with the curve's own day count by Rust.
///
/// Example
/// -------
/// >>> import datetime
/// >>> from finstack_quant.core.market_data import DiscountCurve
/// >>> curve = DiscountCurve("USD-OIS", datetime.date(2025, 1, 1), [(0.0, 1.0), (1.0, 0.95), (5.0, 0.80)])
/// >>> round(curve.df(1.0), 4)
/// 0.95
/// >>> round(curve.df("2026-01-01"), 4)
/// 0.95
#[pyclass(
    name = "DiscountCurve",
    module = "finstack_quant.core.market_data.curves",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyDiscountCurve {
    /// Shared Rust curve.
    pub(crate) inner: Arc<DiscountCurve>,
}

impl PyDiscountCurve {
    /// Build from an existing `Arc<DiscountCurve>`.
    pub(crate) fn from_inner(inner: Arc<DiscountCurve>) -> Self {
        Self { inner }
    }

    fn wrap(curve: DiscountCurve) -> Self {
        Self {
            inner: Arc::new(curve),
        }
    }
}

#[pymethods]
impl PyDiscountCurve {
    /// Construct a discount curve from ``(time_years, discount_factor)`` knots.
    ///
    /// Parameters
    /// ----------
    /// id : str
    ///     Unique curve identifier (e.g. ``"USD-OIS"``).
    /// base_date : datetime.date | str
    ///     Valuation date anchoring ``t = 0``; ISO ``"YYYY-MM-DD"`` strings are accepted.
    /// knots : list[tuple[float, float]]
    ///     ``(time_years, discount_factor)`` pairs; discount factors are unitless
    ///     and must be positive. A ``(0.0, 1.0)`` anchor is conventional.
    /// interp : str, optional
    ///     Interpolation style (``"monotone_convex"``, ``"linear"``,
    ///     ``"log_linear"``, ``"cubic_hermite"``,
    ///     ``"piecewise_quadratic_forward"``). Default ``"monotone_convex"``.
    /// extrapolation : str, optional
    ///     Extrapolation policy (``"flat_forward"``, ``"flat_zero"``, or
    ///     ``"none"``, which returns NaN outside the pillar range). Default
    ///     ``"flat_forward"``.
    /// day_count : str, optional
    ///     Day-count convention used to convert query dates to curve time.
    ///     Default is fixed at ``"act_365f"`` (not inferred from the ID).
    /// validation_mode : str, optional
    ///     ``"market_standard"`` (default: monotonic DFs, -50bp forward floor)
    ///     or ``"negative_rate_friendly"``.
    /// forward_floor : float | None, optional
    ///     Required minimum implied forward (decimal) for ``"negative_rate_friendly"``.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If a knot is non-finite or duplicated, discount factors violate the
    ///     validation mode, or an enum label is unknown.
    ///
    /// Example
    /// -------
    /// >>> import datetime
    /// >>> from finstack_quant.core.market_data import DiscountCurve
    /// >>> curve = DiscountCurve("USD-OIS", datetime.date(2025, 1, 1), [(0.0, 1.0), (1.0, 0.95)], day_count="act_360")
    /// >>> curve.day_count
    /// 'act_360'
    #[new]
    #[expect(
        clippy::too_many_arguments,
        reason = "keyword-only curve options mirror the Rust builder setters"
    )]
    #[pyo3(signature = (id, base_date, knots, *, interp=None, extrapolation=None, day_count=None, validation_mode=None, forward_floor=None))]
    fn new(
        id: &str,
        base_date: &Bound<'_, PyAny>,
        knots: Vec<(f64, f64)>,
        interp: Option<&str>,
        extrapolation: Option<&str>,
        day_count: Option<&str>,
        validation_mode: Option<&str>,
        forward_floor: Option<f64>,
    ) -> PyResult<Self> {
        let base = py_to_date(base_date)?;

        let mut builder = DiscountCurve::builder(id).base_date(base).knots(knots);
        if let Some(interp) = interp {
            builder = builder.interp(parse_interp_style(interp)?);
        }
        if let Some(extrapolation) = extrapolation {
            builder = builder.extrapolation(parse_extrapolation(extrapolation)?);
        }
        if let Some(day_count) = day_count {
            builder = builder.day_count(parse_day_count(day_count)?);
        }
        builder = builder.validation(
            ValidationMode::from_preset(validation_mode, forward_floor).map_err(core_to_py)?,
        );

        builder.build().map(Self::wrap).map_err(core_to_py)
    }

    /// Construct a flat continuously-compounded discount curve.
    ///
    /// Parameters
    /// ----------
    /// id : str
    ///     Unique curve identifier.
    /// base_date : datetime.date | str
    ///     Valuation date anchoring ``t = 0``.
    /// continuous_rate : float
    ///     Continuously-compounded zero rate as a decimal (``0.05`` is 5%).
    ///
    /// Returns
    /// -------
    /// DiscountCurve
    ///     Curve with ``df(t) == exp(-continuous_rate * t)``.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the rate is non-finite or has magnitude greater than ``1.0``
    ///     (a percentage passed where a decimal was expected).
    ///
    /// Example
    /// -------
    /// >>> from finstack_quant.core.market_data import DiscountCurve
    /// >>> round(DiscountCurve.flat("USD-OIS", "2025-01-01", 0.05).df(2.0), 6)
    /// 0.904837
    #[staticmethod]
    #[pyo3(text_signature = "(id, base_date, continuous_rate)")]
    fn flat(id: &str, base_date: &Bound<'_, PyAny>, continuous_rate: f64) -> PyResult<Self> {
        DiscountCurve::flat(id, py_to_date(base_date)?, continuous_rate)
            .map(Self::wrap)
            .map_err(core_to_py)
    }

    /// Construct a discount curve from zero-rate pillars.
    ///
    /// Parameters
    /// ----------
    /// id : str
    ///     Unique curve identifier.
    /// base_date : datetime.date | str
    ///     Valuation date anchoring ``t = 0``.
    /// points : list[tuple[float, float]]
    ///     ``(time_years, zero_rate)`` pillars with rates as decimals; a
    ///     ``(0, 1.0)`` discount-factor anchor is added when no ``t = 0`` pillar is given.
    /// compounding : str, optional
    ///     Convention of the zero rates: ``"continuous"`` (default), ``"simple"``,
    ///     ``"annual"``, ``"semi_annual"``, ``"quarterly"`` or ``"monthly"``.
    ///
    /// Returns
    /// -------
    /// DiscountCurve
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``points`` is empty, a pillar is non-finite, or the implied
    ///     discount factors fail market-standard validation.
    ///
    /// Example
    /// -------
    /// >>> from finstack_quant.core.market_data import DiscountCurve
    /// >>> curve = DiscountCurve.from_zero_rates("USD-OIS", "2025-01-01", [(1.0, 0.05), (2.0, 0.05)], compounding="annual")
    /// >>> round(curve.df(2.0), 6)
    /// 0.907029
    #[staticmethod]
    #[pyo3(signature = (id, base_date, points, compounding="continuous"))]
    fn from_zero_rates(
        id: &str,
        base_date: &Bound<'_, PyAny>,
        points: Vec<(f64, f64)>,
        compounding: &str,
    ) -> PyResult<Self> {
        DiscountCurve::from_zero_rates(
            id,
            py_to_date(base_date)?,
            &points,
            parse_compounding(compounding)?,
        )
        .map(Self::wrap)
        .map_err(core_to_py)
    }

    /// Construct a discount curve from dated discount-factor pillars.
    ///
    /// Parameters
    /// ----------
    /// id : str
    ///     Unique curve identifier.
    /// base_date : datetime.date | str
    ///     Valuation date anchoring ``t = 0``.
    /// points : list[tuple[datetime.date | str, float]]
    ///     ``(date, discount_factor)`` pillars on or after ``base_date``.
    /// day_count : str, optional
    ///     Day count used to convert pillar dates to years; default ``"act_365f"``.
    ///
    /// Returns
    /// -------
    /// DiscountCurve
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``points`` is empty, a pillar precedes ``base_date``, or the
    ///     discount factors fail validation.
    ///
    /// Example
    /// -------
    /// >>> from finstack_quant.core.market_data import DiscountCurve
    /// >>> curve = DiscountCurve.from_dates("USD-OIS", "2025-01-01", [("2026-01-01", 0.95)])
    /// >>> round(curve.df("2026-01-01"), 4)
    /// 0.95
    #[staticmethod]
    #[pyo3(signature = (id, base_date, points, day_count=None))]
    fn from_dates(
        id: &str,
        base_date: &Bound<'_, PyAny>,
        points: Vec<(Bound<'_, PyAny>, f64)>,
        day_count: Option<&str>,
    ) -> PyResult<Self> {
        let points = points
            .iter()
            .map(|(date, df)| Ok((py_to_date(date)?, *df)))
            .collect::<PyResult<Vec<_>>>()?;
        let day_count = day_count.map(parse_day_count).transpose()?;
        DiscountCurve::from_dates(id, py_to_date(base_date)?, &points, day_count)
            .map(Self::wrap)
            .map_err(core_to_py)
    }

    /// Discount factor at a year fraction or date.
    ///
    /// Parameters
    /// ----------
    /// t : float | datetime.date | str
    ///     Year fraction from ``base_date``, or a date converted with the curve day count.
    ///
    /// Returns
    /// -------
    /// float
    ///     Unitless discount factor.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If a date precedes ``base_date`` or is not a valid date.
    #[pyo3(text_signature = "(self, t)")]
    fn df(&self, t: &Bound<'_, PyAny>) -> PyResult<f64> {
        match extract_time_point(t)? {
            TimePoint::Years(t) => Ok(self.inner.df(t)),
            TimePoint::Date(d) => self.inner.df_on_date_curve(d).map_err(core_to_py),
        }
    }

    /// Continuously-compounded zero rate (decimal) at a year fraction or date.
    ///
    /// Parameters
    /// ----------
    /// t : float | datetime.date | str
    ///     Year fraction from ``base_date``, or a date converted with the curve day count.
    ///
    /// Returns
    /// -------
    /// float
    ///     Zero rate as a decimal (``0.05`` is 5%); ``0.0`` at ``t = 0``.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If a date precedes ``base_date``.
    #[pyo3(text_signature = "(self, t)")]
    fn zero(&self, t: &Bound<'_, PyAny>) -> PyResult<f64> {
        match extract_time_point(t)? {
            TimePoint::Years(t) => Ok(self.inner.zero(t)),
            TimePoint::Date(d) => self
                .inner
                .zero_rate_on_date(d, finstack_quant_core::math::Compounding::Continuous)
                .map_err(core_to_py),
        }
    }

    /// Annually-compounded zero rate (decimal) at year fraction ``t``.
    ///
    /// Parameters
    /// ----------
    /// t : float
    ///     Year fraction from ``base_date``.
    ///
    /// Returns
    /// -------
    /// float
    ///     Zero rate as a decimal under annual compounding.
    #[pyo3(text_signature = "(self, t)")]
    fn zero_annual(&self, t: f64) -> f64 {
        self.inner.zero_annual(t)
    }

    /// Zero rate at year fraction ``t`` under an explicit compounding convention.
    ///
    /// Parameters
    /// ----------
    /// t : float
    ///     Year fraction from ``base_date``.
    /// compounding : str, optional
    ///     ``"continuous"`` (default), ``"simple"``, ``"annual"``, ``"semi_annual"``,
    ///     ``"quarterly"`` or ``"monthly"``.
    ///
    /// Returns
    /// -------
    /// float
    ///     Zero rate as a decimal.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``compounding`` is not a recognised label.
    #[pyo3(signature = (t, compounding="continuous"))]
    fn zero_rate(&self, t: f64, compounding: &str) -> PyResult<f64> {
        Ok(self.inner.zero_rate(t, parse_compounding(compounding)?))
    }

    /// Zero rate on a date under an explicit compounding convention.
    ///
    /// Parameters
    /// ----------
    /// date : datetime.date | str
    ///     Target date, converted with the curve day count.
    /// compounding : str, optional
    ///     Compounding label; see :meth:`zero_rate`. Default ``"continuous"``.
    ///
    /// Returns
    /// -------
    /// float
    ///     Zero rate as a decimal.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``date`` precedes ``base_date`` or the label is unknown.
    #[pyo3(signature = (date, compounding="continuous"))]
    fn zero_rate_on_date(&self, date: &Bound<'_, PyAny>, compounding: &str) -> PyResult<f64> {
        self.inner
            .zero_rate_on_date(py_to_date(date)?, parse_compounding(compounding)?)
            .map_err(core_to_py)
    }

    /// Discount factor on a date using the curve day count.
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
    fn df_on_date_curve(&self, date: &Bound<'_, PyAny>) -> PyResult<f64> {
        self.inner
            .df_on_date_curve(py_to_date(date)?)
            .map_err(core_to_py)
    }

    /// Forward discount factor between two dates: ``DF(0, to) / DF(0, from)``.
    ///
    /// Parameters
    /// ----------
    /// from_date : datetime.date | str
    ///     Start date.
    /// to_date : datetime.date | str
    ///     End date; may precede ``from_date`` (the ratio inverts). Returns ``1.0`` when equal.
    ///
    /// Returns
    /// -------
    /// float
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If either year fraction cannot be computed or a discount factor is non-positive.
    #[pyo3(text_signature = "(self, from_date, to_date)")]
    fn df_between_dates(
        &self,
        from_date: &Bound<'_, PyAny>,
        to_date: &Bound<'_, PyAny>,
    ) -> PyResult<f64> {
        self.inner
            .df_between_dates(py_to_date(from_date)?, py_to_date(to_date)?)
            .map_err(core_to_py)
    }

    /// Continuously-compounded forward rate (decimal) between ``t1`` and ``t2``.
    ///
    /// Parameters
    /// ----------
    /// t1 : float
    ///     Start year fraction.
    /// t2 : float
    ///     End year fraction; must exceed ``t1`` by at least the curve's minimum forward tenor.
    ///
    /// Returns
    /// -------
    /// float
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the interval is invalid.
    #[pyo3(text_signature = "(self, t1, t2)")]
    fn forward(&self, t1: f64, t2: f64) -> PyResult<f64> {
        self.inner.forward(t1, t2).map_err(core_to_py)
    }

    /// Derive a simple forward-rate curve for a fixed tenor from this curve.
    ///
    /// Parameters
    /// ----------
    /// forward_id : str
    ///     Identifier for the resulting forward curve.
    /// tenor : float
    ///     Forward tenor in years (``0.25`` for 3M); must be positive.
    /// interp : str, optional
    ///     Interpolation style of the forward curve; default ``"linear"``.
    ///
    /// Returns
    /// -------
    /// ForwardCurve
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``tenor`` is non-positive, the curve has fewer than two knots,
    ///     or a derived forward is non-finite.
    #[pyo3(signature = (forward_id, tenor, interp=None))]
    fn to_forward_curve(
        &self,
        forward_id: &str,
        tenor: f64,
        interp: Option<&str>,
    ) -> PyResult<PyForwardCurve> {
        let style = interp.map(parse_interp_style).transpose()?;
        self.inner
            .to_forward_curve(forward_id, tenor, style)
            .map(|curve| PyForwardCurve::from_inner(Arc::new(curve)))
            .map_err(core_to_py)
    }

    /// Export knots as a pandas ``DataFrame`` with columns ``t`` (years) and ``df``.
    ///
    /// Returns
    /// -------
    /// pandas.DataFrame
    ///     One row per knot, in ascending time order.
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        columns_to_dataframe(
            py,
            &[
                ("t", self.inner.knots().to_vec()),
                ("df", self.inner.dfs().to_vec()),
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

    /// Knot times in years from ``base_date``, ascending.
    #[getter]
    fn knots(&self) -> Vec<f64> {
        self.inner.knots().to_vec()
    }

    /// Unitless discount factors at each knot (same order as ``knots``).
    #[getter]
    fn dfs(&self) -> Vec<f64> {
        self.inner.dfs().to_vec()
    }

    /// Day-count convention label used to convert dates to curve time (e.g. ``"act_365f"``).
    #[getter]
    fn day_count(&self) -> String {
        self.inner.day_count().to_string()
    }

    /// Interpolation style label (e.g. ``"monotone_convex"``).
    #[getter]
    fn interp_style(&self) -> String {
        self.inner.interp_style().to_string()
    }

    /// Extrapolation policy label (e.g. ``"flat_forward"``).
    #[getter]
    fn extrapolation(&self) -> String {
        self.inner.extrapolation().to_string()
    }

    fn __repr__(&self) -> String {
        format!(
            "DiscountCurve(id='{}', base_date='{}', knots={}, day_count='{}')",
            self.inner.id().as_str(),
            self.inner.base_date(),
            self.inner.knots().len(),
            self.inner.day_count()
        )
    }
}

impl_arc_serde_pymethods!(PyDiscountCurve, DiscountCurve, "DiscountCurve");
impl_repr_html_via_dataframe!(PyDiscountCurve);
