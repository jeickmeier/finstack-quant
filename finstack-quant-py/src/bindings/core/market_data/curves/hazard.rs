//! Hazard curve bindings.

use finstack_quant_core::market_data::term_structures::HazardCurve;

use std::sync::Arc;

use finstack_quant_core::market_data::term_structures::Seniority;
use pyo3::prelude::*;

use super::helpers::{TimePoint, columns_to_dataframe, extract_time_point};
use crate::bindings::core::currency::{PyCurrency, extract_currency};
use crate::bindings::core::dates::daycount::extract_day_count;
use crate::bindings::date_utils::{date_to_py, py_to_date};
use crate::bindings::macros::{impl_repr_html_via_dataframe, wire_methods};
use crate::errors::core_to_py;

/// Credit hazard-rate curve for default-probability modelling.
///
/// Stores piecewise-constant hazard rates ``(t, lambda)`` at strictly positive
/// ending times ``t`` in years from ``base_date`` with ``lambda`` as an annual
/// default intensity (decimal). Each ``lambda`` applies to the segment *ending*
/// at its knot, so ``sp(t) = exp(-integral of lambda)``. Query methods accept a
/// year fraction or a date (converted with the curve day count by Rust).
///
/// Example
/// -------
/// >>> from finstack_quant.core.market_data import HazardCurve
/// >>> curve = HazardCurve("ACME-HZD", "2025-01-01", [(1.0, 0.02), (5.0, 0.03)], recovery_rate=0.4, day_count="act_365f")
/// >>> round(curve.sp(1.0), 6)
/// 0.980199
#[pyclass(
    name = "HazardCurve",
    module = "finstack_quant.core.market_data",
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
    ///     ``(time_years, hazard_rate)`` pairs with strictly positive increasing
    ///     ending times ``t > 0`` and finite non-negative annual default
    ///     intensities as decimals (``0.02`` is 2% per year).
    /// recovery_rate : float
    ///     Recovery on default as a decimal fraction in ``[0, 1]`` (keyword-only).
    /// day_count : DayCount | str
    ///     Explicit day-count convention (keyword-only and required).
    /// calendar_id : str, optional
    ///     Registered holiday calendar of the curve's time axis (required with ``"bus_252"``).
    /// issuer : str, optional
    ///     Issuer name metadata.
    /// seniority : str, optional
    ///     ``"senior_secured"``, ``"senior"``, ``"subordinated"`` or ``"junior"``.
    /// currency : Currency | str, optional
    ///     Currency of the protection leg (metadata).
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If a knot has non-positive/non-increasing time or negative/non-finite
    ///     hazard rate, ``recovery_rate`` is outside ``[0, 1]``, ``day_count`` is
    ///     unknown or missing a required calendar, or an enum label is unknown.
    ///
    /// Example
    /// -------
    /// >>> from finstack_quant.core.market_data import HazardCurve
    /// >>> curve = HazardCurve("ACME-HZD", "2025-01-01", [(1.0, 0.02)], recovery_rate=0.4, day_count="act_365f", seniority="senior")
    /// >>> curve.seniority
    /// 'senior'
    #[new]
    #[expect(
        clippy::too_many_arguments,
        reason = "keyword-only curve options mirror the Rust builder setters"
    )]
    #[pyo3(signature = (id, base_date, knots, *, recovery_rate, day_count, calendar_id=None, issuer=None, seniority=None, currency=None))]
    fn new(
        id: &str,
        base_date: &Bound<'_, PyAny>,
        knots: Vec<(f64, f64)>,
        recovery_rate: f64,
        day_count: &Bound<'_, PyAny>,
        calendar_id: Option<String>,
        issuer: Option<&str>,
        seniority: Option<&str>,
        currency: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Self> {
        let mut builder = HazardCurve::builder(id)
            .base_date(py_to_date(base_date)?)
            .day_count(extract_day_count(day_count)?)
            .calendar_id_opt(calendar_id)
            .knots(knots)
            .recovery_rate(recovery_rate);
        if let Some(issuer) = issuer {
            builder = builder.issuer(issuer);
        }
        if let Some(seniority) = seniority {
            builder = builder.seniority(seniority.parse::<Seniority>().map_err(core_to_py)?);
        }
        if let Some(currency) = currency {
            builder = builder.currency(extract_currency(currency)?);
        }
        builder
            .build()
            .map(|curve| Self::from_inner(Arc::new(curve)))
            .map_err(core_to_py)
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
    /// day_count : DayCount | str
    ///     Explicit supported curve day-count convention.
    ///
    /// Returns
    /// -------
    /// HazardCurve
    ///     Curve with ``sp(t) == exp(-hazard_rate * t)``.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``hazard_rate`` is non-finite or negative, ``recovery_rate`` is
    ///     outside ``[0, 1]``, or ``day_count`` is invalid or requires an absent calendar.
    ///
    /// Example
    /// -------
    /// >>> from finstack_quant.core.market_data import HazardCurve
    /// >>> round(HazardCurve.flat("ACME", "2025-01-01", 0.02, 0.4, "act_365f").sp(5.0), 6)
    /// 0.904837
    #[staticmethod]
    #[pyo3(text_signature = "(id, base_date, hazard_rate, recovery_rate, day_count)")]
    fn flat(
        id: &str,
        base_date: &Bound<'_, PyAny>,
        hazard_rate: f64,
        recovery_rate: f64,
        day_count: &Bound<'_, PyAny>,
    ) -> PyResult<Self> {
        HazardCurve::flat(
            id,
            py_to_date(base_date)?,
            hazard_rate,
            recovery_rate,
            extract_day_count(day_count)?,
        )
        .map(|curve| Self::from_inner(Arc::new(curve)))
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
    ///     ``(0, 1]`` and non-increasing in time. An optional ``t = 0`` pillar
    ///     must be ``1.0``; at least one ``t > 0`` pillar is required.
    /// recovery_rate : float
    ///     Recovery on default as a decimal fraction in ``[0, 1]``.
    /// day_count : DayCount | str
    ///     Explicit supported curve day-count convention.
    ///
    /// Returns
    /// -------
    /// HazardCurve
    ///     Piecewise-constant hazard curve reproducing every pillar exactly.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``points`` is empty or lacks a positive time, a probability is
    ///     outside ``(0, 1]`` or increases with time, ``recovery_rate`` is
    ///     outside ``[0, 1]``, or ``day_count`` is invalid.
    ///
    /// Example
    /// -------
    /// >>> from finstack_quant.core.market_data import HazardCurve
    /// >>> curve = HazardCurve.from_survival_probs("ACME", "2025-01-01", [(1.0, 0.98), (5.0, 0.90)], 0.4, "act_365f")
    /// >>> round(curve.sp(5.0), 6)
    /// 0.9
    #[staticmethod]
    #[pyo3(text_signature = "(id, base_date, points, recovery_rate, day_count)")]
    fn from_survival_probs(
        id: &str,
        base_date: &Bound<'_, PyAny>,
        points: Vec<(f64, f64)>,
        recovery_rate: f64,
        day_count: &Bound<'_, PyAny>,
    ) -> PyResult<Self> {
        HazardCurve::from_survival_probs(
            id,
            py_to_date(base_date)?,
            &points,
            recovery_rate,
            extract_day_count(day_count)?,
        )
        .map(|curve| Self::from_inner(Arc::new(curve)))
        .map_err(core_to_py)
    }

    /// Survival probability at a year fraction or date.
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
    ///     Probability in ``[0, 1]``; ``1.0`` at ``t = 0``.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``t < 0``, ``t`` is non-finite, or a date precedes ``base_date``.
    #[pyo3(text_signature = "(self, t)")]
    fn sp(&self, t: &Bound<'_, PyAny>) -> PyResult<f64> {
        match extract_time_point(t)? {
            TimePoint::Years(t) => self.inner.sp(t).map_err(core_to_py),
            TimePoint::Date(d) => self.inner.sp_on_date(d).map_err(core_to_py),
        }
    }

    /// Instantaneous hazard rate (decimal per year) at a year fraction or date.
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
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``t < 0``, ``t`` is non-finite, or a date precedes ``base_date``.
    #[pyo3(text_signature = "(self, t)")]
    fn hazard_rate(&self, t: &Bound<'_, PyAny>) -> PyResult<f64> {
        match extract_time_point(t)? {
            TimePoint::Years(t) => self.inner.hazard_rate(t).map_err(core_to_py),
            TimePoint::Date(d) => self.inner.hazard_rate_on_date(d).map_err(core_to_py),
        }
    }

    /// Survival probabilities on several dates.
    ///
    /// Parameters
    /// ----------
    /// dates : list[datetime.date | str]
    ///     Target dates; a date on or before ``base_date`` gives ``1.0``.
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

    #[doc = crate::bindings::pydoc::pydoc!(
        "finstack_quant.core.market_data._curves.hazard",
        "HazardCurve.conditional_default_prob"
    )]
    #[pyo3(text_signature = "(self, start, end)")]
    fn conditional_default_prob(
        &self,
        start: &Bound<'_, PyAny>,
        end: &Bound<'_, PyAny>,
    ) -> PyResult<f64> {
        self.inner
            .conditional_default_prob(py_to_date(start)?, py_to_date(end)?)
            .map_err(core_to_py)
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
            .map(|curve| Self::from_inner(Arc::new(curve)))
            .map_err(core_to_py)
    }

    /// Export knots as a pandas ``DataFrame`` with columns ``t`` (years) and ``hazard_rate`` (decimal).
    ///
    /// Returns
    /// -------
    /// pandas.DataFrame
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        columns_to_dataframe(
            py,
            &[
                ("t", self.inner.knots().to_vec()),
                ("hazard_rate", self.inner.hazard_rates().to_vec()),
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

    /// Recovery rate assumed on default (decimal fraction).
    #[getter]
    fn recovery_rate(&self) -> f64 {
        self.inner.recovery_rate()
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.core.market_data._curves.hazard", "HazardCurve.knots")]
    #[getter]
    fn knots(&self) -> Vec<f64> {
        self.inner.knots().to_vec()
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.core.market_data._curves.hazard", "HazardCurve.hazard_rates")]
    #[getter]
    fn hazard_rates(&self) -> Vec<f64> {
        self.inner.hazard_rates().to_vec()
    }

    /// Day-count convention label (e.g. ``"act_365f"``).
    #[getter]
    fn day_count(&self) -> String {
        self.inner.day_count().to_string()
    }

    /// Registered holiday calendar of the curve's time axis, or ``None``.
    #[getter]
    fn calendar_id(&self) -> Option<String> {
        self.inner.calendar_id().map(str::to_owned)
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
        self.inner.seniority().map(|s| s.to_string())
    }

    /// Calibration recipe stamped on this curve, as a dict.
    ///
    /// The serde form of the Rust ``HazardCalibrationRecipe``:
    /// ``hazard_params``, ``calibration_inputs``, ``spread_risk_inputs`` and
    /// ``calibration_config``. ``None`` for a curve that was not produced by a
    /// calibration step.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the recipe cannot be converted to Python objects.
    #[getter]
    fn hazard_calibration<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .hazard_calibration()
            .map(|recipe| crate::bindings::pandas_utils::serde_to_py(py, recipe))
            .transpose()
    }

    fn __repr__(&self) -> String {
        format!(
            "HazardCurve(id='{}', base_date='{}', knots={}, recovery_rate={})",
            self.inner.id().as_str(),
            self.inner.base_date(),
            self.inner.knots().len(),
            self.inner.recovery_rate()
        )
    }
}

wire_methods!(PyHazardCurve, HazardCurve, "HazardCurve", arc);
impl_repr_html_via_dataframe!(PyHazardCurve);
