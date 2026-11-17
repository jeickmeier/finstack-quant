//! Forward curve bindings.

use finstack_quant_core::market_data::term_structures::ForwardCurve;

use std::sync::Arc;

use finstack_quant_core::math::Compounding;
use finstack_quant_core::math::interp::ExtrapolationPolicy;
use finstack_quant_core::math::interp::InterpStyle;
use pyo3::prelude::*;

use super::discount::PyDiscountCurve;
use super::helpers::{TimePoint, columns_to_dataframe, extract_time_point};
use crate::bindings::core::dates::daycount::extract_day_count;
use crate::bindings::core::dates::tenor::{PyTenor, extract_tenor};
use crate::bindings::date_utils::{date_to_py, py_to_date};
use crate::bindings::macros::{impl_repr_html_via_dataframe, wire_methods};
use crate::errors::core_to_py;

/// Projection-factor curve for one floating-rate index.
///
/// Stores strictly positive projection factors ``P_x(t)`` at knot times in
/// years from ``base_date``, with ``P_x(0) = 1``. The growth
/// ``P_x(t1) / P_x(t2)`` over an index period drives its projected fixing;
/// ``rate_between`` converts it to a simple rate on an explicit contractual
/// accrual fraction. ``df`` accepts a year fraction or a date; dates are
/// converted with the curve day count by Rust.
///
/// Example
/// -------
/// >>> import math
/// >>> from finstack_quant.core.market_data import ForwardCurve
/// >>> curve = ForwardCurve(
/// ...     "USD-SOFR-3M",
/// ...     "2025-01-01",
/// ...     [(0.0, 1.0), (1.0, math.exp(-0.04))],
/// ...     interp="log_linear",
/// ...     extrapolation="extend",
/// ...     day_count="act_360",
/// ...     index_tenor="3M",
/// ... )
/// >>> round(curve.rate_between(0.0, 0.25, 0.25), 6)
/// 0.040201
#[pyclass(
    name = "ForwardCurve",
    module = "finstack_quant.core.market_data",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyForwardCurve {
    /// Shared Rust curve.
    pub(crate) inner: Arc<ForwardCurve>,
}

impl PyForwardCurve {
    /// Build from an existing `Arc<ForwardCurve>`.
    pub(crate) fn from_inner(inner: Arc<ForwardCurve>) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyForwardCurve {
    /// Construct a forward curve from ``(time_years, projection_factor)`` knots.
    ///
    /// Parameters
    /// ----------
    /// id : str
    ///     Unique curve identifier (e.g. ``"USD-SOFR-3M"``).
    /// base_date : datetime.date | str
    ///     Valuation date anchoring ``t = 0``; ISO ``"YYYY-MM-DD"`` strings are accepted.
    /// knots : list[tuple[float, float]]
    ///     Strictly ascending ``(time_years, projection_factor)`` pairs with
    ///     finite, strictly positive factors. ``(0.0, 1.0)`` is prepended when
    ///     absent; a supplied ``t = 0`` knot must have factor ``1.0``.
    /// interp : str
    ///     Interpolation style (``"log_linear"``, ``"linear"``,
    ///     ``"monotone_convex"``, ``"cubic_hermite"``,
    ///     ``"piecewise_quadratic_forward"``). Keyword-only and required.
    /// extrapolation : str
    ///     Tail policy outside the knots (``"extend"``, ``"flat"``, or
    ///     ``"error"``, which raises outside the pillar range). Keyword-only and required.
    /// day_count : DayCount | str
    ///     Day-count convention used to convert query dates to curve time.
    ///     Keyword-only and required (never inferred from ``id``).
    /// index_tenor : Tenor | str, optional
    ///     Accrual tenor of the projected index (for example ``"3M"``).
    ///     Term-rate consumers such as statement rate bindings require it;
    ///     ``None`` leaves the curve without index-period metadata.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If a knot is non-finite, non-positive, unsorted or duplicated, a
    ///     ``t = 0`` factor is not ``1.0``, a label is unknown, or the day count
    ///     needs a holiday calendar.
    /// TypeError
    ///     If ``index_tenor`` is neither a ``Tenor`` nor a string.
    ///
    /// Example
    /// -------
    /// >>> import math
    /// >>> from finstack_quant.core.market_data import ForwardCurve
    /// >>> curve = ForwardCurve(
    /// ...     "USD-SOFR-3M",
    /// ...     "2025-01-01",
    /// ...     [(1.0, math.exp(-0.04))],
    /// ...     interp="log_linear",
    /// ...     extrapolation="extend",
    /// ...     day_count="act_360",
    /// ...     index_tenor="3M",
    /// ... )
    /// >>> curve.knots
    /// [0.0, 1.0]
    #[new]
    #[pyo3(signature = (id, base_date, knots, *, interp, extrapolation, day_count, index_tenor=None))]
    fn new(
        id: &str,
        base_date: &Bound<'_, PyAny>,
        knots: Vec<(f64, f64)>,
        interp: &str,
        extrapolation: &str,
        day_count: &Bound<'_, PyAny>,
        index_tenor: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Self> {
        let base = py_to_date(base_date)?;
        ForwardCurve::builder(id)
            .base_date(base)
            .knots(knots)
            .index_tenor_opt(index_tenor.map(extract_tenor).transpose()?)
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

    /// Construct a flat continuously-compounded projection curve.
    ///
    /// Parameters
    /// ----------
    /// id : str
    ///     Unique curve identifier.
    /// base_date : datetime.date | str
    ///     Valuation date anchoring ``t = 0``.
    /// continuous_rate : float
    ///     Continuously-compounded projection zero rate as a decimal (``0.04`` is 4%).
    /// day_count : DayCount | str
    ///     Explicit supported day-count convention label or object.
    ///
    /// Returns
    /// -------
    /// ForwardCurve
    ///     Log-linear curve with ``df(t) == exp(-continuous_rate * t)``,
    ///     ``"extend"`` tails and no ``index_tenor``.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the rate is non-finite, its one-year factor overflows or
    ///     underflows, or ``day_count`` is unknown or requires a calendar.
    ///
    /// Example
    /// -------
    /// >>> from finstack_quant.core.market_data import ForwardCurve
    /// >>> round(ForwardCurve.flat("USD-SOFR-3M", "2025-01-01", 0.04, "act_360").df(2.0), 6)
    /// 0.923116
    #[staticmethod]
    #[pyo3(text_signature = "(id, base_date, continuous_rate, day_count)")]
    fn flat(
        id: &str,
        base_date: &Bound<'_, PyAny>,
        continuous_rate: f64,
        day_count: &Bound<'_, PyAny>,
    ) -> PyResult<Self> {
        ForwardCurve::flat(
            id,
            py_to_date(base_date)?,
            continuous_rate,
            extract_day_count(day_count)?,
        )
        .map(|curve| Self::from_inner(Arc::new(curve)))
        .map_err(core_to_py)
    }

    /// Promote a discount curve into a projection curve without refitting.
    ///
    /// The forward curve reuses the discount curve's factors, interpolator,
    /// day count, calendar, tail policy and any retained shock, so
    /// ``df(t)`` matches ``discount.df(t)`` everywhere. Use it for
    /// single-curve setups where the index projects off the discount curve.
    ///
    /// Parameters
    /// ----------
    /// id : str
    ///     Identifier of the resulting forward curve.
    /// discount : DiscountCurve
    ///     Source curve whose factor state is copied; its calibration recipe is dropped.
    /// index_tenor : Tenor | str, optional
    ///     Accrual tenor of the projected index (for example ``"3M"``);
    ///     ``None`` leaves the curve without index-period metadata.
    ///
    /// Returns
    /// -------
    /// ForwardCurve
    ///     Projection curve sharing ``discount``'s factor function.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``index_tenor`` is not a valid tenor label.
    /// TypeError
    ///     If ``index_tenor`` is neither a ``Tenor`` nor a string.
    ///
    /// Example
    /// -------
    /// >>> from finstack_quant.core.market_data import DiscountCurve, ForwardCurve
    /// >>> ois = DiscountCurve.flat("USD-OIS", "2025-01-01", 0.04, "act_365f")
    /// >>> fwd = ForwardCurve.from_discount_curve("USD-SOFR-3M", ois, "3M")
    /// >>> fwd.df(2.0) == ois.df(2.0)
    /// True
    #[staticmethod]
    #[pyo3(signature = (id, discount, index_tenor=None))]
    fn from_discount_curve(
        id: &str,
        discount: &Bound<'_, PyDiscountCurve>,
        index_tenor: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Self> {
        let index_tenor = index_tenor.map(extract_tenor).transpose()?;
        Ok(Self::from_inner(Arc::new(
            ForwardCurve::from_discount_curve(id, &discount.get().inner, index_tenor),
        )))
    }

    /// Construct a forward curve from zero-rate pillars.
    ///
    /// Parameters
    /// ----------
    /// id : str
    ///     Unique curve identifier.
    /// base_date : datetime.date | str
    ///     Valuation date anchoring ``t = 0``.
    /// points : list[tuple[float, float]]
    ///     Strictly ascending ``(time_years, zero_rate)`` pillars with decimal
    ///     zero rates (``0.04`` is 4%); ``(0.0, 1.0)`` is prepended when absent.
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
    /// ForwardCurve
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``points`` is empty, unsorted or non-finite, a label is unknown,
    ///     or projection-factor construction fails.
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
        ForwardCurve::from_zero_rates(
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

    /// Construct a forward curve from dated projection-factor pillars.
    ///
    /// Parameters
    /// ----------
    /// id : str
    ///     Unique curve identifier.
    /// base_date : datetime.date | str
    ///     Valuation date anchoring ``t = 0``.
    /// points : list[tuple[datetime.date | str, float]]
    ///     Strictly ascending ``(date, projection_factor)`` pillars on or after
    ///     ``base_date`` with strictly positive finite factors.
    /// day_count : DayCount | str
    ///     Explicit day count used to convert pillar dates to years.
    /// interp : str
    ///     Explicit interpolation style.
    /// extrapolation : str
    ///     Explicit tail extrapolation policy (``"flat"``, ``"extend"``, or ``"error"``).
    ///
    /// Returns
    /// -------
    /// ForwardCurve
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``points`` is empty, a pillar precedes ``base_date``, mapped
    ///     pillar times do not strictly increase, or a label or factor is invalid.
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
            .map(|(date, factor)| Ok((py_to_date(date)?, *factor)))
            .collect::<PyResult<Vec<_>>>()?;
        ForwardCurve::from_dates(
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

    /// Projection factor ``P_x(t)`` at a year fraction or date.
    ///
    /// Parameters
    /// ----------
    /// t : float | datetime.date | str
    ///     Year fraction from ``base_date``, or a date converted with the curve
    ///     day count; times before the base date follow the tail policy.
    ///
    /// Returns
    /// -------
    /// float
    ///     Unitless, strictly positive projection factor (``1.0`` at ``t = 0``).
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``t`` is non-finite, outside the knots under ``"error"``
    ///     extrapolation, or the date's day-count fraction cannot be computed.
    #[pyo3(text_signature = "(self, t)")]
    fn df(&self, t: &Bound<'_, PyAny>) -> PyResult<f64> {
        match extract_time_point(t)? {
            TimePoint::Years(t) => self.inner.df(t).map_err(core_to_py),
            TimePoint::Date(d) => self.inner.df_on_date(d).map_err(core_to_py),
        }
    }

    /// Relative projection factor between two dates: ``P_x(to_date) / P_x(from_date)``.
    ///
    /// Parameters
    /// ----------
    /// from_date : datetime.date | str
    ///     Start date.
    /// to_date : datetime.date | str
    ///     End date; returns ``1.0`` when equal.
    ///
    /// Returns
    /// -------
    /// float
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If either date's projection factor cannot be admitted or the ratio overflows.
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

    /// Simple forward rate (decimal) over ``[t1, t2]`` on a contractual accrual fraction.
    ///
    /// Evaluates ``(P_x(t1) / P_x(t2) - 1) / accrual_year_fraction``. The
    /// interval endpoints are curve times; the accrual fraction is the index's
    /// own day-count fraction for the same dated period, which may differ from
    /// ``t2 - t1`` when the index and curve conventions differ.
    ///
    /// Parameters
    /// ----------
    /// t1 : float
    ///     Start year fraction from the curve base date using its day count.
    /// t2 : float
    ///     End year fraction on the same curve basis; must be finite and greater than ``t1``.
    /// accrual_year_fraction : float
    ///     Strictly positive accrual year fraction of the index period under
    ///     its contractual day count (for example ``91 / 360``).
    ///
    /// Returns
    /// -------
    /// float
    ///     Simple forward rate as a decimal, annualized on ``accrual_year_fraction``.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the interval is empty, reversed or non-finite, the accrual fraction
    ///     is not finite and positive, or a factor or the rate cannot be computed.
    ///
    /// Example
    /// -------
    /// >>> from finstack_quant.core.market_data import ForwardCurve
    /// >>> curve = ForwardCurve.flat("USD-SOFR-3M", "2025-01-01", 0.04, "act_360")
    /// >>> round(curve.rate_between(0.0, 0.25, 91 / 360), 6)
    /// 0.039759
    #[pyo3(text_signature = "(self, t1, t2, accrual_year_fraction)")]
    fn rate_between(&self, t1: f64, t2: f64, accrual_year_fraction: f64) -> PyResult<f64> {
        self.inner
            .rate_between(t1, t2, accrual_year_fraction)
            .map_err(core_to_py)
    }

    /// Simple forward rate (decimal) over ``[start, end]`` on a contractual accrual fraction.
    ///
    /// Parameters
    /// ----------
    /// start : datetime.date | str
    ///     Period start date.
    /// end : datetime.date | str
    ///     Exclusive period end date strictly after ``start``.
    /// accrual_year_fraction : float
    ///     Strictly positive accrual year fraction of the index period.
    ///
    /// Returns
    /// -------
    /// float
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``end <= start``, mapped coordinates reverse order, ``accrual_year_fraction``
    ///     is not finite and positive, or factors/rate fail admission.
    #[pyo3(text_signature = "(self, start, end, accrual_year_fraction)")]
    fn rate_between_dates(
        &self,
        start: &Bound<'_, PyAny>,
        end: &Bound<'_, PyAny>,
        accrual_year_fraction: f64,
    ) -> PyResult<f64> {
        self.inner
            .rate_between_dates(py_to_date(start)?, py_to_date(end)?, accrual_year_fraction)
            .map_err(core_to_py)
    }

    /// Instantaneous projection forward ``-P_x'(t) / P_x(t)`` at a year fraction.
    ///
    /// Parameters
    /// ----------
    /// t : float
    ///     Curve time in years from ``base_date``; at a knot a one-sided
    ///     derivative of the interpolator is used.
    ///
    /// Returns
    /// -------
    /// float
    ///     Continuously-compounded instantaneous forward rate as a decimal.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``t`` is non-finite, the factor at ``t`` cannot be evaluated, or
    ///     the derivative overflows.
    ///
    /// Example
    /// -------
    /// >>> from finstack_quant.core.market_data import ForwardCurve
    /// >>> curve = ForwardCurve.flat("USD-SOFR-3M", "2025-01-01", 0.04, "act_360")
    /// >>> round(curve.instantaneous_forward(3.0), 6)
    /// 0.04
    #[pyo3(text_signature = "(self, t)")]
    fn instantaneous_forward(&self, t: f64) -> PyResult<f64> {
        self.inner.instantaneous_forward(t).map_err(core_to_py)
    }

    /// Export knots as a pandas ``DataFrame`` with columns ``t`` (years) and ``factor``.
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
                ("factor", self.inner.factors().to_vec()),
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

    /// Accrual tenor of the projected index, or ``None`` when not recorded.
    #[getter]
    fn index_tenor(&self) -> Option<PyTenor> {
        self.inner.index_tenor().map(PyTenor::from_inner)
    }

    /// Knot times in years from ``base_date``, ascending and starting at ``0.0``.
    #[getter]
    fn knots(&self) -> Vec<f64> {
        self.inner.knots().to_vec()
    }

    /// Projection factors at each knot (``1.0`` at ``t = 0``).
    #[getter]
    fn factors(&self) -> Vec<f64> {
        self.inner.factors().to_vec()
    }

    /// Day-count convention label (e.g. ``"act_360"``).
    #[getter]
    fn day_count(&self) -> String {
        self.inner.day_count().to_string()
    }

    /// Registered holiday calendar of the curve's time axis, or ``None``.
    #[getter]
    fn calendar_id(&self) -> Option<String> {
        self.inner.calendar_id().map(str::to_owned)
    }

    /// Interpolation style label (e.g. ``"log_linear"``).
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
        let index_tenor = self
            .inner
            .index_tenor()
            .map_or_else(|| "None".to_string(), |tenor| format!("'{tenor}'"));
        format!(
            "ForwardCurve(id='{}', base_date='{}', knots={}, day_count='{}', index_tenor={})",
            self.inner.id().as_str(),
            self.inner.base_date(),
            self.inner.knots().len(),
            self.inner.day_count(),
            index_tenor
        )
    }
}

wire_methods!(PyForwardCurve, ForwardCurve, "ForwardCurve", arc);
impl_repr_html_via_dataframe!(PyForwardCurve);
