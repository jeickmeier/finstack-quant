//! Hazard curve bindings.

use finstack_quant_core::market_data::term_structures::HazardCurve;

use std::sync::Arc;

use pyo3::prelude::*;

use super::helpers::{
    columns_to_dataframe, extract_time_point, impl_arc_serde_pymethods,
    impl_repr_html_via_dataframe, par_interp_name, parse_day_count, parse_interp_style,
    parse_par_interp, parse_seniority, TimePoint,
};
use crate::bindings::core::currency::{extract_currency, PyCurrency};
use crate::bindings::date_utils::{date_to_py, py_to_date};
use crate::errors::core_to_py;

/// Options accepted by the ``HazardCurve`` constructor beyond the knots.
struct HazardCurveOptions<'a> {
    day_count: Option<&'a str>,
    par_spreads: Option<Vec<(f64, f64)>>,
    interp: Option<&'a str>,
    par_interp: Option<&'a str>,
    issuer: Option<&'a str>,
    seniority: Option<&'a str>,
    currency: Option<&'a Bound<'a, PyAny>>,
    max_hazard_rate: Option<f64>,
}

/// Credit hazard-rate curve for default-probability modelling.
///
/// Stores piecewise-constant hazard rates ``(t, lambda)`` in years from
/// ``base_date`` with ``lambda`` as an annual default intensity (decimal).
/// Each ``lambda`` applies to the segment *ending* at its knot, so
/// ``sp(t) = exp(-integral of lambda)``. Query methods accept a year fraction
/// or a date (converted with the curve day count by Rust).
///
/// Example
/// -------
/// >>> from finstack_quant.core.market_data import HazardCurve
/// >>> curve = HazardCurve("ACME-HZD", "2025-01-01", [(1.0, 0.02), (5.0, 0.03)], recovery_rate=0.4)
/// >>> round(curve.sp(1.0), 6)
/// 0.980199
#[pyclass(
    name = "HazardCurve",
    module = "finstack_quant.core.market_data.curves",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyHazardCurve {
    /// Shared Rust curve.
    pub(crate) inner: Arc<HazardCurve>,
}

impl PyHazardCurve {
    /// Build from an existing `Arc<HazardCurve>`.
    pub(crate) fn from_inner(inner: Arc<HazardCurve>) -> Self {
        Self { inner }
    }

    fn wrap(curve: HazardCurve) -> Self {
        Self {
            inner: Arc::new(curve),
        }
    }

    fn build(
        id: &str,
        base_date: &Bound<'_, PyAny>,
        knots: Vec<(f64, f64)>,
        recovery_rate: f64,
        options: HazardCurveOptions<'_>,
    ) -> PyResult<Self> {
        let mut builder = HazardCurve::builder(id)
            .base_date(py_to_date(base_date)?)
            .knots(knots)
            .recovery_rate(recovery_rate);
        if let Some(day_count) = options.day_count {
            builder = builder.day_count(parse_day_count(day_count)?);
        }
        if let Some(points) = options.par_spreads {
            builder = builder.par_spreads(points);
        }
        if let Some(interp) = options.interp {
            builder = builder.interp(parse_interp_style(interp)?);
        }
        if let Some(par_interp) = options.par_interp {
            builder = builder.par_interp(parse_par_interp(par_interp)?);
        }
        if let Some(issuer) = options.issuer {
            builder = builder.issuer(issuer);
        }
        if let Some(seniority) = options.seniority {
            builder = builder.seniority(parse_seniority(seniority)?);
        }
        if let Some(currency) = options.currency {
            builder = builder.currency(extract_currency(currency)?);
        }
        if let Some(max_hazard_rate) = options.max_hazard_rate {
            builder = builder.max_hazard_rate(max_hazard_rate);
        }
        builder.build().map(Self::wrap).map_err(core_to_py)
    }
}

#[pymethods]
impl PyHazardCurve {
    /// Construct a hazard curve from ``(time_years, hazard_rate)`` knots.
    ///
    /// Parameters
    /// ----------
    /// id : str
    ///     Unique curve identifier (e.g. ``"ACME-HZD"``).
    /// base_date : datetime.date | str
    ///     Valuation date anchoring ``t = 0``.
    /// knots : list[tuple[float, float]]
    ///     ``(time_years, hazard_rate)`` pairs; hazard rates are annual default
    ///     intensities as decimals (``0.02`` is 2% per year), non-negative.
    /// recovery_rate : float
    ///     Recovery on default as a decimal fraction in ``[0, 1]`` (keyword-only).
    /// day_count : str, optional
    ///     Day-count convention; default ``"act_365f"``.
    /// par_spreads : list[tuple[float, float]], optional
    ///     ``(time_years, par_spread_bp)`` market quotes in **basis points**
    ///     kept for reporting and re-bootstrap risk.
    /// interp : str, optional
    ///     Survival-probability interpolation; only ``"log_linear"`` is supported
    ///     to preserve piecewise-constant hazards.
    /// par_interp : str, optional
    ///     Par-spread readout interpolation: ``"linear"`` (default) or ``"log_linear"``.
    /// issuer : str, optional
    ///     Issuer name metadata.
    /// seniority : str, optional
    ///     ``"senior_secured"``, ``"senior"``, ``"subordinated"`` or ``"junior"``.
    /// currency : Currency | str, optional
    ///     Currency of the protection leg (metadata).
    /// max_hazard_rate : float, optional
    ///     Sanity ceiling on any ``hazard_rate``; default ``10.0``.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If a knot is non-finite, negative, duplicated or above
    ///     ``max_hazard_rate``, ``recovery_rate`` is outside ``[0, 1]``, or a
    ///     label is unknown.
    ///
    /// Example
    /// -------
    /// >>> from finstack_quant.core.market_data import HazardCurve
    /// >>> curve = HazardCurve("ACME-HZD", "2025-01-01", [(1.0, 0.02)], recovery_rate=0.4, seniority="senior")
    /// >>> curve.seniority
    /// 'senior'
    #[new]
    #[expect(
        clippy::too_many_arguments,
        reason = "keyword-only curve options mirror the Rust builder setters"
    )]
    #[pyo3(signature = (id, base_date, knots, *, recovery_rate, day_count=None, par_spreads=None, interp=None, par_interp=None, issuer=None, seniority=None, currency=None, max_hazard_rate=None))]
    fn new(
        id: &str,
        base_date: &Bound<'_, PyAny>,
        knots: Vec<(f64, f64)>,
        recovery_rate: f64,
        day_count: Option<&str>,
        par_spreads: Option<Vec<(f64, f64)>>,
        interp: Option<&str>,
        par_interp: Option<&str>,
        issuer: Option<&str>,
        seniority: Option<&str>,
        currency: Option<&Bound<'_, PyAny>>,
        max_hazard_rate: Option<f64>,
    ) -> PyResult<Self> {
        Self::build(
            id,
            base_date,
            knots,
            recovery_rate,
            HazardCurveOptions {
                day_count,
                par_spreads,
                interp,
                par_interp,
                issuer,
                seniority,
                currency,
                max_hazard_rate,
            },
        )
    }

    /// Construct a flat (constant-intensity) hazard curve.
    ///
    /// Parameters
    /// ----------
    /// id : str
    ///     Unique curve identifier.
    /// base_date : datetime.date | str
    ///     Valuation date anchoring ``t = 0``.
    /// hazard_rate : float
    ///     Constant annual default intensity as a decimal (``0.02`` is 2%).
    /// recovery_rate : float
    ///     Recovery on default as a decimal fraction in ``[0, 1]``.
    ///
    /// Returns
    /// -------
    /// HazardCurve
    ///     Curve with ``sp(t) == exp(-hazard_rate * t)``.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``hazard_rate`` is non-finite or negative, or ``recovery_rate`` is outside ``[0, 1]``.
    ///
    /// Example
    /// -------
    /// >>> from finstack_quant.core.market_data import HazardCurve
    /// >>> round(HazardCurve.flat("ACME", "2025-01-01", 0.02, 0.4).sp(5.0), 6)
    /// 0.904837
    #[staticmethod]
    #[pyo3(text_signature = "(id, base_date, hazard_rate, recovery_rate)")]
    fn flat(
        id: &str,
        base_date: &Bound<'_, PyAny>,
        hazard_rate: f64,
        recovery_rate: f64,
    ) -> PyResult<Self> {
        HazardCurve::flat(id, py_to_date(base_date)?, hazard_rate, recovery_rate)
            .map(Self::wrap)
            .map_err(core_to_py)
    }

    /// Construct a hazard curve from survival-probability pillars.
    ///
    /// Parameters
    /// ----------
    /// id : str
    ///     Unique curve identifier.
    /// base_date : datetime.date | str
    ///     Valuation date anchoring ``t = 0``.
    /// points : list[tuple[float, float]]
    ///     ``(time_years, survival_probability)`` pillars with probabilities in
    ///     ``(0, 1]`` and non-increasing in time. A ``t = 0`` pillar must be ``1.0``.
    /// recovery_rate : float
    ///     Recovery on default as a decimal fraction in ``[0, 1]``.
    ///
    /// Returns
    /// -------
    /// HazardCurve
    ///     Piecewise-constant hazard curve reproducing every pillar exactly.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``points`` is empty, a probability is outside ``(0, 1]`` or
    ///     increases with time, or ``recovery_rate`` is outside ``[0, 1]``.
    ///
    /// Example
    /// -------
    /// >>> from finstack_quant.core.market_data import HazardCurve
    /// >>> curve = HazardCurve.from_survival_probs("ACME", "2025-01-01", [(1.0, 0.98), (5.0, 0.90)], 0.4)
    /// >>> round(curve.sp(5.0), 6)
    /// 0.9
    #[staticmethod]
    #[pyo3(text_signature = "(id, base_date, points, recovery_rate)")]
    fn from_survival_probs(
        id: &str,
        base_date: &Bound<'_, PyAny>,
        points: Vec<(f64, f64)>,
        recovery_rate: f64,
    ) -> PyResult<Self> {
        HazardCurve::from_survival_probs(id, py_to_date(base_date)?, &points, recovery_rate)
            .map(Self::wrap)
            .map_err(core_to_py)
    }

    /// Survival probability at a year fraction or date.
    ///
    /// Parameters
    /// ----------
    /// t : float | datetime.date | str
    ///     Year fraction from ``base_date``, or a date converted with the curve day count.
    ///
    /// Returns
    /// -------
    /// float
    ///     Probability in ``(0, 1]``; ``1.0`` at or before ``t = 0``.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If a date precedes ``base_date``.
    #[pyo3(text_signature = "(self, t)")]
    fn sp(&self, t: &Bound<'_, PyAny>) -> PyResult<f64> {
        match extract_time_point(t)? {
            TimePoint::Years(t) => Ok(self.inner.sp(t)),
            TimePoint::Date(d) => self.inner.sp_on_date(d).map_err(core_to_py),
        }
    }

    /// Instantaneous hazard rate (decimal per year) at a year fraction or date.
    ///
    /// Parameters
    /// ----------
    /// t : float | datetime.date | str
    ///     Year fraction from ``base_date``, or a date converted with the curve day count.
    ///
    /// Returns
    /// -------
    /// float
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If a date precedes ``base_date``.
    #[pyo3(text_signature = "(self, t)")]
    fn hazard_rate(&self, t: &Bound<'_, PyAny>) -> PyResult<f64> {
        match extract_time_point(t)? {
            TimePoint::Years(t) => Ok(self.inner.hazard_rate(t)),
            TimePoint::Date(d) => self.inner.hazard_rate_on_date(d).map_err(core_to_py),
        }
    }

    /// Survival probability on a date using the curve day count.
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
    fn sp_on_date(&self, date: &Bound<'_, PyAny>) -> PyResult<f64> {
        self.inner.sp_on_date(py_to_date(date)?).map_err(core_to_py)
    }

    /// Hazard rate (decimal per year) on a date using the curve day count.
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
    fn hazard_rate_on_date(&self, date: &Bound<'_, PyAny>) -> PyResult<f64> {
        self.inner
            .hazard_rate_on_date(py_to_date(date)?)
            .map_err(core_to_py)
    }

    /// Survival probabilities on several dates.
    ///
    /// Parameters
    /// ----------
    /// dates : list[datetime.date | str]
    ///     Target dates on or after ``base_date``.
    ///
    /// Returns
    /// -------
    /// list[float]
    ///     One survival probability per input date, in order.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If any year fraction cannot be computed.
    #[pyo3(text_signature = "(self, dates)")]
    fn survival_at_dates(&self, dates: Vec<Bound<'_, PyAny>>) -> PyResult<Vec<f64>> {
        let dates = dates.iter().map(py_to_date).collect::<PyResult<Vec<_>>>()?;
        self.inner.survival_at_dates(&dates).map_err(core_to_py)
    }

    /// Probability of default in ``[t1, t2]``: ``sp(t1) - sp(t2)``.
    ///
    /// Parameters
    /// ----------
    /// t1 : float
    ///     Start year fraction.
    /// t2 : float
    ///     End year fraction; must not precede ``t1``.
    ///
    /// Returns
    /// -------
    /// float
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``t2 < t1``.
    #[pyo3(text_signature = "(self, t1, t2)")]
    fn default_prob(&self, t1: f64, t2: f64) -> PyResult<f64> {
        self.inner.default_prob(t1, t2).map_err(core_to_py)
    }

    /// Interpolated par CDS spread in **basis points** at year fraction ``t``.
    ///
    /// Uses the stored ``par_spreads`` quotes; when fewer than two quotes are
    /// stored, falls back to a hazard-based approximation.
    ///
    /// Parameters
    /// ----------
    /// t : float
    ///     Year fraction from ``base_date``.
    /// method : str, optional
    ///     ``"linear"`` or ``"log_linear"``; defaults to the curve's ``par_interp``.
    ///
    /// Returns
    /// -------
    /// float
    ///     Spread in basis points.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``method`` is not a recognised label.
    #[pyo3(signature = (t, method=None))]
    fn cds_quote_bp(&self, t: f64, method: Option<&str>) -> PyResult<f64> {
        let method = match method {
            Some(label) => parse_par_interp(label)?,
            None => self.inner.par_interp(),
        };
        Ok(self.inner.cds_quote_bp(t, method))
    }

    /// Copy of this curve with a different recovery-rate metadata value.
    ///
    /// Survival probabilities are unchanged; only the reported recovery differs.
    ///
    /// Parameters
    /// ----------
    /// recovery_rate : float
    ///     New recovery as a decimal fraction in ``[0, 1]``.
    ///
    /// Returns
    /// -------
    /// HazardCurve
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``recovery_rate`` is outside ``[0, 1]``.
    #[pyo3(text_signature = "(self, recovery_rate)")]
    fn with_recovery_rate(&self, recovery_rate: f64) -> PyResult<Self> {
        self.inner
            .with_recovery_rate(recovery_rate)
            .map(Self::wrap)
            .map_err(core_to_py)
    }

    /// Export knots as a pandas ``DataFrame`` with columns ``t`` (years) and ``hazard_rate`` (decimal).
    ///
    /// Returns
    /// -------
    /// pandas.DataFrame
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let (t, lambdas): (Vec<f64>, Vec<f64>) = self.inner.knot_points().unzip();
        columns_to_dataframe(py, &[("t", t), ("hazard_rate", lambdas)])
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

    /// Recovery rate assumed on default (decimal fraction).
    #[getter]
    fn recovery_rate(&self) -> f64 {
        self.inner.recovery_rate()
    }

    /// ``(time_years, hazard_rate)`` knots with hazard rates as decimals.
    #[getter]
    fn knot_points(&self) -> Vec<(f64, f64)> {
        self.inner.knot_points().collect()
    }

    /// ``(time_years, par_spread_bp)`` market quotes in basis points (may be empty).
    #[getter]
    fn par_spread_points(&self) -> Vec<(f64, f64)> {
        self.inner.par_spread_points().collect()
    }

    /// Day-count convention label (e.g. ``"act_365f"``).
    #[getter]
    fn day_count(&self) -> String {
        self.inner.day_count().to_string()
    }

    /// Currency of the protection leg, or ``None``.
    #[getter]
    fn currency(&self) -> Option<PyCurrency> {
        self.inner.currency().map(PyCurrency::from_inner)
    }

    /// Issuer name metadata, or ``None``.
    #[getter]
    fn issuer(&self) -> Option<String> {
        self.inner.issuer().map(str::to_owned)
    }

    /// Debt seniority label (``"senior_secured"``, ``"senior"``, ``"subordinated"``, ``"junior"``), or ``None``.
    #[getter]
    fn seniority(&self) -> Option<String> {
        self.inner.seniority.map(|s| s.to_string())
    }

    /// Par-spread readout interpolation label (``"linear"`` or ``"log_linear"``).
    #[getter]
    fn par_interp(&self) -> PyResult<String> {
        par_interp_name(self.inner.par_interp())
    }

    fn __repr__(&self) -> String {
        format!(
            "HazardCurve(id='{}', base_date='{}', knots={}, recovery_rate={})",
            self.inner.id().as_str(),
            self.inner.base_date(),
            self.inner.knot_points().count(),
            self.inner.recovery_rate()
        )
    }
}

impl_arc_serde_pymethods!(PyHazardCurve, HazardCurve, "HazardCurve");
impl_repr_html_via_dataframe!(PyHazardCurve);
