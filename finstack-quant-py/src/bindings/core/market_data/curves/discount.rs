//! Discount curve bindings.

use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::market_data::traits::Discounting;

use std::sync::Arc;

use finstack_quant_core::math::Compounding;
use finstack_quant_core::math::interp::ExtrapolationPolicy;
use finstack_quant_core::math::interp::InterpStyle;
use pyo3::prelude::*;

use super::helpers::{TimePoint, columns_to_dataframe, extract_time_point};
use crate::bindings::core::dates::daycount::extract_day_count;
use crate::bindings::date_utils::{date_to_py, py_to_date};
use crate::bindings::macros::{impl_repr_html_via_dataframe, wire_methods};
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
/// >>> curve = DiscountCurve(
/// ...     "USD-OIS",
/// ...     datetime.date(2025, 1, 1),
/// ...     [(0.0, 1.0), (1.0, 0.95), (5.0, 0.80)],
/// ...     interp="monotone_convex",
/// ...     extrapolation="extend",
/// ...     day_count="act_365f",
/// ... )
/// >>> round(curve.df(1.0), 4)
/// 0.95
/// >>> round(curve.df("2026-01-01"), 4)
/// 0.95
#[pyclass(
    name = "DiscountCurve",
    module = "finstack_quant.core.market_data",
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
    ///     and must be strictly positive. A ``(0.0, 1.0)`` anchor is conventional.
    /// interp : str
    ///     Interpolation style (``"monotone_convex"``, ``"linear"``,
    ///     ``"log_linear"``, ``"cubic_hermite"``,
    ///     ``"piecewise_quadratic_forward"``). Keyword-only and required.
    /// extrapolation : str
    ///     Extrapolation policy (``"extend"``, ``"flat"``, or
    ///     ``"error"``, which raises outside the pillar range). Keyword-only and required.
    /// day_count : DayCount | str
    ///     Day-count convention used to convert query dates to curve time.
    ///     Keyword-only and required (never inferred from ``id``).
    /// calendar_id : str, optional
    ///     Registered holiday calendar of the curve's time axis (for example
    ///     ``"brbd"``), or a ``+``-joined union. Required when ``day_count`` is
    ///     ``"bus_252"``; every date-based query counts business days on it.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If a knot is non-finite, non-positive, or duplicated, an enum label is
    ///     unknown, or a ``"bus_252"`` curve has no ``calendar_id``.
    /// KeyError
    ///     If ``calendar_id`` names no registered calendar.
    ///
    /// Example
    /// -------
    /// >>> import datetime
    /// >>> from finstack_quant.core.market_data import DiscountCurve
    /// >>> curve = DiscountCurve(
    /// ...     "USD-OIS",
    /// ...     datetime.date(2025, 1, 1),
    /// ...     [(0.0, 1.0), (1.0, 0.95)],
    /// ...     interp="monotone_convex",
    /// ...     extrapolation="extend",
    /// ...     day_count="act_360",
    /// ... )
    /// >>> curve.day_count
    /// 'act_360'
    #[new]
    #[pyo3(signature = (id, base_date, knots, *, interp, extrapolation, day_count, calendar_id=None))]
    fn new(
        id: &str,
        base_date: &Bound<'_, PyAny>,
        knots: Vec<(f64, f64)>,
        interp: &str,
        extrapolation: &str,
        day_count: &Bound<'_, PyAny>,
        calendar_id: Option<String>,
    ) -> PyResult<Self> {
        let base = py_to_date(base_date)?;
        DiscountCurve::builder(id)
            .base_date(base)
            .knots(knots)
            .calendar_id_opt(calendar_id)
            .interp(interp.parse::<InterpStyle>().map_err(core_to_py)?)
            .extrapolation(
                extrapolation
                    .parse::<ExtrapolationPolicy>()
                    .map_err(core_to_py)?,
            )
            .day_count(extract_day_count(day_count)?)
            .build()
            .map(|curve| Self::from_inner(Arc::new(curve)))
            .map_err(core_to_py)
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
    /// day_count : DayCount | str
    ///     Explicit supported day-count convention label or object.
    ///
    /// Returns
    /// -------
    /// DiscountCurve
    ///     Curve with ``df(t) == exp(-continuous_rate * t)``.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the rate is non-finite or ``|rate| > 1``, or ``day_count`` is
    ///     unknown or requires an absent calendar.
    ///
    /// Example
    /// -------
    /// >>> from finstack_quant.core.market_data import DiscountCurve
    /// >>> round(DiscountCurve.flat("USD-OIS", "2025-01-01", 0.05, "act_365f").df(2.0), 6)
    /// 0.904837
    #[staticmethod]
    #[pyo3(text_signature = "(id, base_date, continuous_rate, day_count)")]
    fn flat(
        id: &str,
        base_date: &Bound<'_, PyAny>,
        continuous_rate: f64,
        day_count: &Bound<'_, PyAny>,
    ) -> PyResult<Self> {
        DiscountCurve::flat(
            id,
            py_to_date(base_date)?,
            continuous_rate,
            extract_day_count(day_count)?,
        )
        .map(|curve| Self::from_inner(Arc::new(curve)))
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
    /// compounding : str
    ///     Convention of the zero rates: ``"continuous"``, ``"simple"``,
    ///     ``"periodic(1)"``, ``"semi_annual"``, ``"quarterly"`` or ``"monthly"``.
    /// day_count : DayCount | str
    ///     Explicit curve day-count convention.
    /// interp : str
    ///     Explicit interpolation style.
    /// extrapolation : str
    ///     Explicit tail extrapolation policy (``"flat"``, ``"extend"``, or ``"error"``).
    ///
    /// Returns
    /// -------
    /// DiscountCurve
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``points`` is empty, a pillar or label is invalid, or discount
    ///     factors fail curve construction.
    ///
    /// Example
    /// -------
    /// >>> from finstack_quant.core.market_data import DiscountCurve
    /// >>> curve = DiscountCurve.from_zero_rates(
    /// ...     "USD-OIS", "2025-01-01", [(1.0, 0.05), (2.0, 0.05)],
    /// ...     "periodic(1)", "act_365f", "monotone_convex", "extend",
    /// ... )
    /// >>> round(curve.df(2.0), 6)
    /// 0.907029
    #[staticmethod]
    #[pyo3(
        text_signature = "(id, base_date, points, compounding, day_count, interp, extrapolation)"
    )]
    fn from_zero_rates(
        id: &str,
        base_date: &Bound<'_, PyAny>,
        points: Vec<(f64, f64)>,
        compounding: &str,
        day_count: &Bound<'_, PyAny>,
        interp: &str,
        extrapolation: &str,
    ) -> PyResult<Self> {
        DiscountCurve::from_zero_rates(
            id,
            py_to_date(base_date)?,
            &points,
            compounding.parse::<Compounding>().map_err(core_to_py)?,
            extract_day_count(day_count)?,
            interp.parse::<InterpStyle>().map_err(core_to_py)?,
            extrapolation
                .parse::<ExtrapolationPolicy>()
                .map_err(core_to_py)?,
        )
        .map(|curve| Self::from_inner(Arc::new(curve)))
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
    /// day_count : DayCount | str
    ///     Explicit day count used to convert pillar dates to years.
    /// interp : str
    ///     Explicit interpolation style.
    /// extrapolation : str
    ///     Explicit tail extrapolation policy (``"flat"``, ``"extend"``, or ``"error"``).
    ///
    /// Returns
    /// -------
    /// DiscountCurve
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``points`` is empty, a pillar precedes ``base_date``, or an
    ///     enum label or discount factor is invalid.
    ///
    /// Example
    /// -------
    /// >>> from finstack_quant.core.market_data import DiscountCurve
    /// >>> curve = DiscountCurve.from_dates(
    /// ...     "USD-OIS", "2025-01-01", [("2026-01-01", 0.95)],
    /// ...     "act_365f", "monotone_convex", "extend",
    /// ... )
    /// >>> round(curve.df("2026-01-01"), 4)
    /// 0.95
    #[staticmethod]
    #[pyo3(text_signature = "(id, base_date, points, day_count, interp, extrapolation)")]
    fn from_dates(
        id: &str,
        base_date: &Bound<'_, PyAny>,
        points: Vec<(Bound<'_, PyAny>, f64)>,
        day_count: &Bound<'_, PyAny>,
        interp: &str,
        extrapolation: &str,
    ) -> PyResult<Self> {
        let points = points
            .iter()
            .map(|(date, df)| Ok((py_to_date(date)?, *df)))
            .collect::<PyResult<Vec<_>>>()?;
        DiscountCurve::from_dates(
            id,
            py_to_date(base_date)?,
            &points,
            extract_day_count(day_count)?,
            interp.parse::<InterpStyle>().map_err(core_to_py)?,
            extrapolation
                .parse::<ExtrapolationPolicy>()
                .map_err(core_to_py)?,
        )
        .map(|curve| Self::from_inner(Arc::new(curve)))
        .map_err(core_to_py)
    }

    /// Discount factor at a year fraction or date.
    ///
    /// Parameters
    /// ----------
    /// t : float | datetime.date | str
    ///     Non-negative year fraction from ``base_date``, or a date on or after
    ///     ``base_date`` converted with the curve day count.
    ///
    /// Returns
    /// -------
    /// float
    ///     Unitless discount factor.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``t < 0``, a date precedes ``base_date`` or is not valid, or
    ///     the tail policy rejects out-of-range queries.
    #[pyo3(text_signature = "(self, t)")]
    fn df(&self, t: &Bound<'_, PyAny>) -> PyResult<f64> {
        match extract_time_point(t)? {
            TimePoint::Years(t) => self.inner.df(t).map_err(core_to_py),
            TimePoint::Date(d) => self.inner.df_on_date(d).map_err(core_to_py),
        }
    }

    /// Zero rate at a year fraction or date under an explicit compounding convention.
    ///
    /// Parameters
    /// ----------
    /// t : float | datetime.date | str
    ///     Strictly positive year fraction from ``base_date``, or a date
    ///     strictly after ``base_date`` converted with the curve day count.
    /// compounding : str
    ///     ``"continuous"``, ``"simple"``, ``"periodic(1)"``, ``"semi_annual"``,
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
    ///     If ``t <= 0``, ``t`` is a date on or before ``base_date``, or
    ///     ``compounding`` is not a recognised label.
    #[pyo3(text_signature = "(self, t, compounding)")]
    fn zero_rate(&self, t: &Bound<'_, PyAny>, compounding: &str) -> PyResult<f64> {
        let comp = compounding.parse::<Compounding>().map_err(core_to_py)?;
        match extract_time_point(t)? {
            TimePoint::Years(t) => self.inner.zero_rate(t, comp).map_err(core_to_py),
            TimePoint::Date(d) => self.inner.zero_rate_on_date(d, comp).map_err(core_to_py),
        }
    }

    /// Forward discount factor between two dates: ``DF(0, to) / DF(0, from)``.
    ///
    /// Both dates map to curve time with the curve's own day count and
    /// time-axis calendar (``calendar_id``).
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

    /// Registered holiday calendar of the curve's time axis, or ``None``.
    ///
    /// Set for business-day day counts such as ``"bus_252"`` (e.g. ``"brbd"``).
    #[getter]
    fn calendar_id(&self) -> Option<String> {
        self.inner.calendar_id().map(str::to_owned)
    }

    /// Interpolation style label (e.g. ``"monotone_convex"``).
    #[getter]
    fn interp_style(&self) -> String {
        self.inner.interp_style().to_string()
    }

    /// Extrapolation policy label (e.g. ``"extend"``).
    #[getter]
    fn extrapolation(&self) -> String {
        self.inner.extrapolation().to_string()
    }

    /// Calibration recipe stamped on this curve, as a dict.
    ///
    /// The serde form of the Rust ``RateCalibrationRecipe``: ``currency``,
    /// ``method``, ``curve_day_count``, ``ois_compounding``, ``role`` and the
    /// complete ``quotes`` set needed to replay the calibration. ``None`` for a
    /// curve that was not produced by a calibration step.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the recipe cannot be converted to Python objects.
    #[getter]
    fn rate_calibration<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .rate_calibration()
            .map(|recipe| crate::bindings::pandas_utils::serde_to_py(py, recipe))
            .transpose()
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

wire_methods!(PyDiscountCurve, DiscountCurve, "DiscountCurve", arc);
impl_repr_html_via_dataframe!(PyDiscountCurve);
