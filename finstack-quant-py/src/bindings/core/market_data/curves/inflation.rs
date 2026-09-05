//! Inflation curve bindings.

use finstack_quant_core::market_data::term_structures::InflationCurve;

use std::sync::Arc;

use pyo3::prelude::*;

use super::helpers::{
    columns_to_dataframe, extract_time_point, impl_arc_serde_pymethods,
    impl_repr_html_via_dataframe, parse_day_count, parse_extrapolation, parse_interp_style,
    TimePoint,
};
use crate::bindings::date_utils::{date_to_py, py_to_date};
use crate::errors::core_to_py;

/// CPI inflation curve for inflation-linked pricing and breakeven analysis.
///
/// Stores ``(t, cpi_level)`` knots in years from ``base_date`` as absolute
/// index levels (e.g. ``300.0``). ``cpi`` accepts a year fraction or a date
/// (converted with the curve day count by Rust).
///
/// Example
/// -------
/// >>> from finstack_quant.core.market_data import InflationCurve
/// >>> curve = InflationCurve("US-CPI", "2025-01-01", 300.0, [(0.0, 300.0), (1.0, 306.0), (2.0, 312.0)])
/// >>> round(curve.inflation_rate(0.0, 1.0), 4)
/// 0.02
#[pyclass(
    name = "InflationCurve",
    module = "finstack_quant.core.market_data.curves",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyInflationCurve {
    /// Shared Rust curve.
    pub(crate) inner: Arc<InflationCurve>,
}

impl PyInflationCurve {
    /// Build from an existing `Arc<InflationCurve>`.
    pub(crate) fn from_inner(inner: Arc<InflationCurve>) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyInflationCurve {
    /// Construct an inflation curve from CPI knot points.
    ///
    /// Parameters
    /// ----------
    /// id : str
    ///     Unique curve identifier (e.g. ``"US-CPI"``).
    /// base_date : datetime.date | str
    ///     Valuation date anchoring ``t = 0``.
    /// base_cpi : float
    ///     Reference CPI level at ``t = 0`` used by ``index_ratio``.
    /// knots : list[tuple[float, float]]
    ///     ``(time_years, cpi_level)`` pairs; levels must be positive.
    /// day_count : str, optional
    ///     Day-count convention; default ``"act_365f"``.
    /// indexation_lag_months : int, optional
    ///     Indexation lag in months applied by ``cpi_with_lag``; default ``3``.
    /// interp : str, optional
    ///     Interpolation style; default ``"log_linear"``.
    /// extrapolation : str, optional
    ///     Extrapolation policy; default ``"flat_forward"``.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If no knots are given, a knot is non-finite, duplicated or
    ///     non-positive, or a label is unknown.
    ///
    /// Example
    /// -------
    /// >>> from finstack_quant.core.market_data import InflationCurve
    /// >>> curve = InflationCurve("US-CPI", "2025-01-01", 300.0, [(0.0, 300.0), (1.0, 306.0)], indexation_lag_months=2)
    /// >>> curve.indexation_lag_months
    /// 2
    #[new]
    #[expect(
        clippy::too_many_arguments,
        reason = "keyword-only curve options mirror the Rust builder setters"
    )]
    #[pyo3(signature = (id, base_date, base_cpi, knots, *, day_count=None, indexation_lag_months=None, interp=None, extrapolation=None))]
    fn new(
        id: &str,
        base_date: &Bound<'_, PyAny>,
        base_cpi: f64,
        knots: Vec<(f64, f64)>,
        day_count: Option<&str>,
        indexation_lag_months: Option<u32>,
        interp: Option<&str>,
        extrapolation: Option<&str>,
    ) -> PyResult<Self> {
        let mut builder = InflationCurve::builder(id)
            .base_date(py_to_date(base_date)?)
            .base_cpi(base_cpi)
            .knots(knots);
        if let Some(day_count) = day_count {
            builder = builder.day_count(parse_day_count(day_count)?);
        }
        if let Some(months) = indexation_lag_months {
            builder = builder.indexation_lag_months(months);
        }
        if let Some(interp) = interp {
            builder = builder.interp(parse_interp_style(interp)?);
        }
        if let Some(extrapolation) = extrapolation {
            builder = builder.extrapolation(parse_extrapolation(extrapolation)?);
        }
        builder
            .build()
            .map(|curve| Self {
                inner: Arc::new(curve),
            })
            .map_err(core_to_py)
    }

    /// CPI level at a year fraction or date, without indexation lag.
    ///
    /// Parameters
    /// ----------
    /// t : float | datetime.date | str
    ///     Year fraction from ``base_date``, or a date converted with the curve day count.
    ///
    /// Returns
    /// -------
    /// float
    ///     Absolute CPI index level.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If a date precedes ``base_date``.
    #[pyo3(text_signature = "(self, t)")]
    fn cpi(&self, t: &Bound<'_, PyAny>) -> PyResult<f64> {
        match extract_time_point(t)? {
            TimePoint::Years(t) => Ok(self.inner.cpi(t)),
            TimePoint::Date(d) => self.inner.cpi_on_date(d).map_err(core_to_py),
        }
    }

    /// CPI level on a date using the curve day count (no indexation lag).
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
    fn cpi_on_date(&self, date: &Bound<'_, PyAny>) -> PyResult<f64> {
        self.inner
            .cpi_on_date(py_to_date(date)?)
            .map_err(core_to_py)
    }

    /// CPI level at year fraction ``t`` with the configured indexation lag applied.
    ///
    /// Parameters
    /// ----------
    /// t : float
    ///     Year fraction from ``base_date``.
    ///
    /// Returns
    /// -------
    /// float
    #[pyo3(text_signature = "(self, t)")]
    fn cpi_with_lag(&self, t: f64) -> f64 {
        self.inner.cpi_with_lag(t)
    }

    /// Principal indexation ratio ``cpi_with_lag(t) / base_cpi`` at year fraction ``t``.
    ///
    /// No deflation floor is applied.
    ///
    /// Parameters
    /// ----------
    /// t : float
    ///     Year fraction from ``base_date``.
    ///
    /// Returns
    /// -------
    /// float
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``base_cpi`` is not strictly positive.
    #[pyo3(text_signature = "(self, t)")]
    fn index_ratio(&self, t: f64) -> PyResult<f64> {
        self.inner.index_ratio(t).map_err(core_to_py)
    }

    /// Annualized inflation rate (decimal, CAGR) between ``t1`` and ``t2``.
    ///
    /// Parameters
    /// ----------
    /// t1 : float
    ///     Start year fraction.
    /// t2 : float
    ///     End year fraction.
    ///
    /// Returns
    /// -------
    /// float
    ///     Annualized CPI growth as a decimal rate.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If times are non-finite or not increasing, CPI levels are invalid,
    ///     or the annualized rate is non-finite.
    #[pyo3(text_signature = "(self, t1, t2)")]
    fn inflation_rate(&self, t1: f64, t2: f64) -> PyResult<f64> {
        self.inner.inflation_rate(t1, t2).map_err(core_to_py)
    }

    /// Export knots as a pandas ``DataFrame`` with columns ``t`` (years) and ``cpi``.
    ///
    /// Returns
    /// -------
    /// pandas.DataFrame
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        columns_to_dataframe(
            py,
            &[
                ("t", self.inner.knots().to_vec()),
                ("cpi", self.inner.cpi_levels().to_vec()),
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

    /// Day-count convention label (e.g. ``"act_365f"``).
    #[getter]
    fn day_count(&self) -> String {
        self.inner.day_count().to_string()
    }

    /// Indexation lag in months.
    #[getter]
    fn indexation_lag_months(&self) -> u32 {
        self.inner.indexation_lag_months()
    }

    /// Base CPI level at ``t = 0``.
    #[getter]
    fn base_cpi(&self) -> f64 {
        self.inner.base_cpi()
    }

    /// Knot times in years from ``base_date``, ascending.
    #[getter]
    fn knots(&self) -> Vec<f64> {
        self.inner.knots().to_vec()
    }

    /// CPI index levels at each knot.
    #[getter]
    fn cpi_levels(&self) -> Vec<f64> {
        self.inner.cpi_levels().to_vec()
    }

    /// Interpolation style label (e.g. ``"log_linear"``).
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
            "InflationCurve(id='{}', base_date='{}', base_cpi={}, knots={}, indexation_lag_months={})",
            self.inner.id().as_str(),
            self.inner.base_date(),
            self.inner.base_cpi(),
            self.inner.knots().len(),
            self.inner.indexation_lag_months()
        )
    }
}

impl_arc_serde_pymethods!(PyInflationCurve, InflationCurve, "InflationCurve");
impl_repr_html_via_dataframe!(PyInflationCurve);
