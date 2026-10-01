//! Forward curve bindings.

use finstack_quant_core::market_data::term_structures::ForwardCurve;

use std::sync::Arc;

use pyo3::prelude::*;

use super::helpers::{
    columns_to_dataframe, extract_time_point, impl_arc_serde_pymethods,
    impl_repr_html_via_dataframe, parse_day_count, parse_extrapolation, parse_interp_style,
    TimePoint,
};
use crate::bindings::date_utils::{date_to_py, py_to_date};
use crate::errors::core_to_py;

/// Forward rate curve for a floating-rate index with a fixed tenor.
///
/// Stores ``(t, forward_rate)`` knots in years from ``base_date`` with rates as
/// decimals (``0.04`` is 4%). ``rate`` and ``df`` accept a year fraction or a
/// date; dates are converted with the curve day count by Rust.
///
/// Example
/// -------
/// >>> from finstack_quant.core.market_data import ForwardCurve
/// >>> curve = ForwardCurve("USD-SOFR-3M", 0.25, "2025-01-01", [(0.0, 0.04), (1.0, 0.045)])
/// >>> round(curve.rate(0.5), 4)
/// 0.0425
#[pyclass(
    name = "ForwardCurve",
    module = "finstack_quant.core.market_data.curves",
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

    fn wrap(curve: ForwardCurve) -> Self {
        Self {
            inner: Arc::new(curve),
        }
    }
}

#[pymethods]
impl PyForwardCurve {
    /// Construct a forward rate curve from ``(time_years, forward_rate)`` knots.
    ///
    /// Parameters
    /// ----------
    /// id : str
    ///     Unique curve identifier (e.g. ``"USD-SOFR-3M"``). Day count and
    ///     reset lag are inferred from the ID unless given explicitly.
    /// tenor : float
    ///     Index tenor in years (``0.25`` for 3 months).
    /// base_date : datetime.date | str
    ///     Valuation date anchoring ``t = 0``.
    /// knots : list[tuple[float, float]]
    ///     ``(time_years, forward_rate)`` pairs with rates as decimals.
    /// day_count : str, optional
    ///     Day-count convention (``"act_360"``, ``"act_365f"``, ...). When
    ///     omitted, Rust infers a market default from the curve ID.
    /// interp : str, optional
    ///     Interpolation style; default ``"linear"``.
    /// extrapolation : str, optional
    ///     Extrapolation policy; default ``"flat_forward"``.
    /// projection_grid : list[float] | None, optional
    ///     Contractual reset/end-date boundaries in years. Omit for fixed
    ///     numeric-tenor stepping.
    /// reset_lag : int | None, optional
    ///     Business days from fixing to spot. Omit for curve-ID inference.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If a knot is non-finite or duplicated, ``tenor`` is non-positive,
    ///     or an enum label is unknown.
    ///
    /// Example
    /// -------
    /// >>> from finstack_quant.core.market_data import ForwardCurve
    /// >>> curve = ForwardCurve("USD-SOFR-3M", 0.25, "2025-01-01", [(0.0, 0.04), (1.0, 0.045)], day_count="act_360")
    /// >>> curve.tenor
    /// 0.25
    #[new]
    #[expect(
        clippy::too_many_arguments,
        reason = "keyword-only curve options mirror the Rust builder setters"
    )]
    #[pyo3(signature = (id, tenor, base_date, knots, *, day_count=None, interp=None, extrapolation=None, projection_grid=None, reset_lag=None))]
    fn new(
        id: &str,
        tenor: f64,
        base_date: &Bound<'_, PyAny>,
        knots: Vec<(f64, f64)>,
        day_count: Option<&str>,
        interp: Option<&str>,
        extrapolation: Option<&str>,
        projection_grid: Option<Vec<f64>>,
        reset_lag: Option<i32>,
    ) -> PyResult<Self> {
        let base = py_to_date(base_date)?;

        let mut builder = ForwardCurve::builder(id, tenor)
            .base_date(base)
            .knots(knots)
            .projection_grid_opt(projection_grid);
        if let Some(interp) = interp {
            builder = builder.interp(parse_interp_style(interp)?);
        }
        if let Some(extrapolation) = extrapolation {
            builder = builder.extrapolation(parse_extrapolation(extrapolation)?);
        }
        if let Some(day_count) = day_count {
            builder = builder.day_count(parse_day_count(day_count)?);
        }
        if let Some(reset_lag) = reset_lag {
            builder = builder.reset_lag(reset_lag);
        }

        builder.build().map(Self::wrap).map_err(core_to_py)
    }

    /// Construct a flat forward curve quoting ``rate`` at every maturity.
    ///
    /// Parameters
    /// ----------
    /// id : str
    ///     Unique curve identifier.
    /// tenor : float
    ///     Index tenor in years; must be positive.
    /// base_date : datetime.date | str
    ///     Valuation date anchoring ``t = 0``.
    /// rate : float
    ///     Simple forward rate as a decimal (``0.04`` is 4%).
    ///
    /// Returns
    /// -------
    /// ForwardCurve
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``rate`` is non-finite or ``tenor`` is non-positive.
    ///
    /// Example
    /// -------
    /// >>> from finstack_quant.core.market_data import ForwardCurve
    /// >>> round(ForwardCurve.flat("USD-SOFR-3M", 0.25, "2025-01-01", 0.04).rate(7.0), 6)
    /// 0.04
    #[staticmethod]
    #[pyo3(text_signature = "(id, tenor, base_date, rate)")]
    fn flat(id: &str, tenor: f64, base_date: &Bound<'_, PyAny>, rate: f64) -> PyResult<Self> {
        ForwardCurve::flat(id, tenor, py_to_date(base_date)?, rate)
            .map(Self::wrap)
            .map_err(core_to_py)
    }

    /// Forward rate (decimal) at a year fraction or date.
    ///
    /// The stored rate is annualized on the curve's day-count basis. Convert
    /// its accrual growth to the contractual index basis before calculating a
    /// coupon whose accrual convention differs; Rust instrument pricing performs
    /// that conversion from the contractual schedule.
    ///
    /// Parameters
    /// ----------
    /// t : float | datetime.date | str
    ///     Year fraction from ``base_date``, or a date converted with the curve day count.
    ///
    /// Returns
    /// -------
    /// float
    ///     Simple forward rate as a decimal, annualized on the curve day count.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If a date precedes ``base_date``.
    #[pyo3(text_signature = "(self, t)")]
    fn rate(&self, t: &Bound<'_, PyAny>) -> PyResult<f64> {
        match extract_time_point(t)? {
            TimePoint::Years(t) => Ok(self.inner.rate(t)),
            TimePoint::Date(d) => self.inner.rate_on_date(d).map_err(core_to_py),
        }
    }

    /// Discount-factor-implied simple forward rate (decimal) over ``(t1, t2)``.
    ///
    /// The denominator is the curve-time span ``t2 - t1``. To express the
    /// implied growth on a different contractual index basis, multiply by that
    /// span and divide by the index's accrual fraction for the same dated period.
    ///
    /// Parameters
    /// ----------
    /// t1 : float
    ///     Start year fraction from the curve base date using its day count.
    /// t2 : float
    ///     End year fraction on the same curve basis; must be finite and greater than ``t1``.
    ///
    /// Returns
    /// -------
    /// float
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the interval is empty, reversed or non-finite.
    #[pyo3(text_signature = "(self, t1, t2)")]
    fn rate_between(&self, t1: f64, t2: f64) -> PyResult<f64> {
        self.inner.rate_between(t1, t2).map_err(core_to_py)
    }

    /// Average forward rate (decimal) over ``[t1, t2]`` from the stored knots.
    ///
    /// This integral average remains annualized on the curve's day count.
    /// For an overnight observation on another index basis, multiply by
    /// ``t2 - t1`` and divide by its contractual observation accrual fraction.
    /// Use ``rate_between`` to infer a simple term-period rate from projection DFs.
    ///
    /// Parameters
    /// ----------
    /// t1 : float
    ///     Start year fraction from the curve base date using its day count.
    /// t2 : float
    ///     End year fraction on the same curve basis.
    ///
    /// Returns
    /// -------
    /// float
    #[pyo3(text_signature = "(self, t1, t2)")]
    fn rate_period(&self, t1: f64, t2: f64) -> f64 {
        self.inner.rate_period(t1, t2)
    }

    /// Discount factor implied by compounding the forwards to a year fraction or date.
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
    ///     If ``t`` is negative or non-finite, or a date precedes ``base_date``.
    #[pyo3(text_signature = "(self, t)")]
    fn df(&self, t: &Bound<'_, PyAny>) -> PyResult<f64> {
        match extract_time_point(t)? {
            TimePoint::Years(t) => self.inner.df(t).map_err(core_to_py),
            TimePoint::Date(d) => self.inner.df_on_date_curve(d).map_err(core_to_py),
        }
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

    /// Export knots as a pandas ``DataFrame`` with columns ``t`` (years) and ``forward`` (decimal).
    ///
    /// Returns
    /// -------
    /// pandas.DataFrame
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        columns_to_dataframe(
            py,
            &[
                ("t", self.inner.knots().to_vec()),
                ("forward", self.inner.forwards().to_vec()),
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

    /// Index tenor in years.
    #[getter]
    fn tenor(&self) -> f64 {
        self.inner.tenor()
    }

    /// Knot times in years from ``base_date``, ascending.
    #[getter]
    fn knots(&self) -> Vec<f64> {
        self.inner.knots().to_vec()
    }

    /// Forward rates (decimal) at each knot.
    #[getter]
    fn forwards(&self) -> Vec<f64> {
        self.inner.forwards().to_vec()
    }

    /// Day-count convention label (e.g. ``"act_360"``).
    #[getter]
    fn day_count(&self) -> String {
        self.inner.day_count().to_string()
    }

    /// Interpolation style label (e.g. ``"linear"``).
    #[getter]
    fn interp_style(&self) -> String {
        self.inner.interp_style().to_string()
    }

    /// Extrapolation policy label (e.g. ``"flat_forward"``).
    #[getter]
    fn extrapolation(&self) -> String {
        self.inner.extrapolation().to_string()
    }

    /// Contractual projection boundaries in years, or ``None`` for tenor stepping.
    #[getter]
    fn projection_grid(&self) -> Option<Vec<f64>> {
        self.inner.projection_grid().map(<[f64]>::to_vec)
    }

    /// Business days from fixing to spot.
    #[getter]
    fn reset_lag(&self) -> i32 {
        self.inner.reset_lag()
    }

    fn __repr__(&self) -> String {
        format!(
            "ForwardCurve(id='{}', tenor={}, base_date='{}', knots={}, day_count='{}')",
            self.inner.id().as_str(),
            self.inner.tenor(),
            self.inner.base_date(),
            self.inner.knots().len(),
            self.inner.day_count()
        )
    }
}

impl_arc_serde_pymethods!(PyForwardCurve, ForwardCurve, "ForwardCurve");
impl_repr_html_via_dataframe!(PyForwardCurve);
