//! Python bindings for day-count conventions from [`finstack_quant_core::dates`].

use crate::bindings::core::dates::tenor::{PyTenor, extract_tenor};
use crate::bindings::date_utils::{date_to_py, py_to_date};
use crate::bindings::macros::{label_enum, typed_or_label, wire_methods};
use crate::errors::core_to_py;
use finstack_quant_core::dates::{DayCount, DayCountContext, DayCountContextState};
use pyo3::prelude::*;
use pyo3::types::PyModule;

/// Day-count convention for year-fraction calculations.
///
/// Immutable, hashable enum-style type with a class attribute per supported
/// convention. ``str()`` gives the canonical snake_case name
/// (``"act_360"``), which ``from_name`` parses after trimming surrounding
/// whitespace and normalizing ASCII case. Other spellings are rejected.
///
/// Examples
/// --------
/// >>> import datetime
/// >>> from finstack_quant.core.dates import DayCount
/// >>> DayCount.ACT_360.year_fraction(datetime.date(2024, 1, 1), datetime.date(2024, 7, 1))
/// 0.5055555555555555
/// >>> DayCount.from_name(" ACT_ACT_ISMA ") == DayCount.ACT_ACT_ISMA
/// True
#[pyclass(
    name = "DayCount",
    module = "finstack_quant.core.dates",
    frozen,
    eq,
    hash,
    skip_from_py_object
)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PyDayCount {
    /// Inner day-count convention.
    pub(crate) inner: DayCount,
}

impl PyDayCount {
    /// Build from an existing Rust [`DayCount`].
    pub(crate) const fn from_inner(inner: DayCount) -> Self {
        Self { inner }
    }
}

typed_or_label!(
    pub(crate) fn extract_day_count -> DayCount,
    PyDayCount,
    "DayCount or str",
    |label| label.parse().map_err(core_to_py)
);

/// Resolve the optional ``ctx`` argument of a year-fraction call.
pub(crate) fn resolve_context(
    ctx: Option<&PyDayCountContext>,
) -> PyResult<DayCountContext<'static>> {
    match ctx {
        Some(c) => c.to_rust_ctx(),
        None => Ok(DayCountContext::default()),
    }
}

label_enum!(PyDayCount, DayCount, "DayCount", from_name, {
    /// One unit per nonempty contractual accrual period; empty periods return zero.
    ONE_ONE => OneOne,
    /// Actual/360 (money market).
    ACT_360 => Act360,
    /// Actual/365 Fixed.
    ACT_365F => Act365F,
    /// Actual/365L (ISDA coupon-end-year rule). Requires the enclosing
    /// ``coupon_period`` in ``DayCountContext``; no frequency is used. The
    /// denominator is 366 exactly when ``coupon_end``'s year is a leap year,
    /// otherwise 365, for every coupon frequency. This is explicitly not
    /// ACT/ACT AFB, which uses sub-period splitting.
    ACT_365L => Act365L,
    /// NL/365 (Actual/365 No Leap): actual days excluding every February 29
    /// in ``(start, end]``, divided by 365.
    NL_365 => Nl365,
    /// Fixed US 30U/360 EOM (QuantLib ``Thirty360.USA``); not 30/360 PSA.
    THIRTY_360 => Thirty360,
    /// 30/360 ISDA bond basis, ISDA 2006 §4.16(f) (QuantLib
    /// ``Thirty360.BondBasis``); no February rule.
    THIRTY_360_ISDA => Thirty360Isda,
    /// 30E/360 (Eurobond Basis), ISDA 2006 §4.16(g).
    THIRTY_E_360 => ThirtyE360,
    /// 30E/360 ISDA, ISDA 2006 §4.16(h). Requires an explicit
    /// ``end_is_termination_date`` in ``DayCountContext``.
    THIRTY_E_360_ISDA => ThirtyE360Isda,
    /// Actual/Actual (ISDA).
    ACT_ACT => ActAct,
    /// Actual/Actual (ICMA/ISMA).
    ACT_ACT_ISMA => ActActIsma,
    /// Actual/Actual AFB (Actual/Actual Euro).
    ///
    /// Walks whole years backwards from the end date (QuantLib
    /// ``ActualActual::AFB``). A year-step landing on 28 February of a leap
    /// year is bumped to 29 February. The residual uses denominator 366 if
    /// 29 February lies in ``[start, residual_end)``, else 365.
    ACT_ACT_AFB => ActActAfb,
    /// 30/360 Italian.
    ///
    /// Day 31 becomes 30, and any February day after the 27th becomes 30
    /// (QuantLib ``Thirty360::Italian``). Distinct from US SIA and 30E/360.
    THIRTY_360_IT => Thirty360It,
    /// Business/252 (Brazilian market convention).
    BUS_252 => Bus252,
});

#[pymethods]
impl PyDayCount {
    /// Compute the year fraction between two dates under this convention.
    ///
    /// Parameters
    /// ----------
    /// start : datetime.date | str
    ///     Accrual start (inclusive).
    /// end : datetime.date | str
    ///     Accrual end (exclusive); must not precede ``start``.
    /// ctx : DayCountContext | None
    ///     Context for conventions that need one: ``ACT_ACT_ISMA`` reads
    ///     ``frequency``, ``ACT_365L`` the enclosing contractual
    ///     ``coupon_period`` (partial accrual retains that coupon's
    ///     denominator), ``BUS_252`` reads ``calendar_id``.
    ///
    /// Returns
    /// -------
    /// float
    ///     Non-negative year fraction (``0.0`` when ``start == end``).
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``start > end`` or the convention needs context that was not
    ///     supplied (ISMA without a frequency, ACT/365L without enclosing
    ///     coupon dates, Bus/252 without a calendar),
    ///     ACT/365L dates lie outside the coupon, or ICMA reference endpoints
    ///     do not share an unadjusted nominal month grid.
    /// KeyError
    ///     If ``ctx`` names an unknown calendar.
    #[pyo3(signature = (start, end, ctx=None), text_signature = "(self, start, end, ctx=None)")]
    fn year_fraction(
        &self,
        start: &Bound<'_, PyAny>,
        end: &Bound<'_, PyAny>,
        ctx: Option<&PyDayCountContext>,
    ) -> PyResult<f64> {
        let s = py_to_date(start)?;
        let e = py_to_date(end)?;
        let context = resolve_context(ctx)?;
        self.inner.year_fraction(s, e, context).map_err(core_to_py)
    }

    /// Compute the signed year fraction (negative when ``start > end``).
    ///
    /// Accepts the same ``ctx`` as ``year_fraction`` and raises the same
    /// exceptions, except that an inverted range is allowed.
    #[pyo3(signature = (start, end, ctx=None), text_signature = "(self, start, end, ctx=None)")]
    fn signed_year_fraction(
        &self,
        start: &Bound<'_, PyAny>,
        end: &Bound<'_, PyAny>,
        ctx: Option<&PyDayCountContext>,
    ) -> PyResult<f64> {
        let s = py_to_date(start)?;
        let e = py_to_date(end)?;
        let context = resolve_context(ctx)?;
        self.inner
            .signed_year_fraction(s, e, context)
            .map_err(core_to_py)
    }
}

/// Optional context for day-count calculations.
///
/// Certain conventions require additional information:
///
/// - ``BUS_252`` requires a holiday calendar (``calendar_id``).
/// - ``ACT_ACT_ISMA`` requires the coupon ``frequency`` and, for irregular
///   or mid-coupon accruals, the unadjusted regular reference ``coupon_period``.
/// - ``ACT_365L`` requires the full enclosing contractual ``coupon_period``,
///   including for partial accrual.
/// - ``THIRTY_E_360_ISDA`` requires an explicit ``end_is_termination_date``
///   (``True`` or ``False``) for its end-of-February rule.
///
/// Parameters
/// ----------
/// calendar_id : str | None
///     Registered calendar id (``"usny"``; ``"nyse+gblo"`` joins calendars).
///     Resolved on each use, so an unknown id raises ``KeyError`` at
///     calculation time.
/// frequency : Tenor | str | None
///     Coupon frequency (``Tenor`` or ``"6M"``).
/// bus_basis : int | None
///     Positive business-day divisor for ``BUS_252``; ``None`` selects 252.
/// coupon_period : tuple[datetime.date | str, datetime.date | str] | None
///     Unadjusted regular reference coupon period for ACT/ACT (ICMA), or the enclosing
///     contractual coupon for ACT/365L, as ``(start, end)``;
///     ``start`` must precede ``end``. ICMA endpoints must share one nominal
///     month grid; business-day-adjusted payment dates are not reference dates.
/// end_is_termination_date : bool | None
///     Whether the chronologically later accrual endpoint is the contractual
///     termination date. ``None`` (the default) means unspecified, which
///     ``THIRTY_E_360_ISDA`` rejects; other conventions ignore it.
///
/// Raises
/// ------
/// ValueError
///     If ``coupon_period`` is inverted, ``frequency`` does not parse or
///     ``bus_basis`` is zero.
///
/// Examples
/// --------
/// >>> from finstack_quant.core.dates import DayCountContext
/// >>> context = DayCountContext("usny", "3M", 252)
/// >>> (context.calendar_id, context.frequency.months, context.bus_basis)
/// ('usny', 3, 252)
#[pyclass(
    name = "DayCountContext",
    module = "finstack_quant.core.dates",
    frozen,
    eq,
    skip_from_py_object
)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PyDayCountContext {
    /// Serializable state; the live calendar is resolved on each use.
    pub(crate) inner: DayCountContextState,
}

impl PyDayCountContext {
    /// Resolve to a runtime [`DayCountContext`] using the global calendar registry.
    ///
    /// # Errors
    ///
    /// Raises ``KeyError`` when ``calendar_id`` is set but cannot be resolved
    /// in the global calendar registry.
    fn to_rust_ctx(&self) -> PyResult<DayCountContext<'static>> {
        // Routes through the core registry error so unknown codes surface
        // "Did you mean …?" suggestions instead of a bare message.
        self.inner.to_ctx().map_err(core_to_py)
    }
}

#[pymethods]
impl PyDayCountContext {
    /// Create a day-count context (see the class docstring for parameters).
    #[new]
    #[pyo3(signature = (calendar_id=None, frequency=None, bus_basis=None, coupon_period=None, end_is_termination_date=None))]
    fn new(
        calendar_id: Option<String>,
        frequency: Option<&Bound<'_, PyAny>>,
        bus_basis: Option<u16>,
        coupon_period: Option<(Bound<'_, PyAny>, Bound<'_, PyAny>)>,
        end_is_termination_date: Option<bool>,
    ) -> PyResult<Self> {
        let coupon_period = coupon_period
            .map(|(s, e)| Ok::<_, PyErr>((py_to_date(&s)?, py_to_date(&e)?)))
            .transpose()?;
        let frequency = frequency.map(extract_tenor).transpose()?;
        let inner = DayCountContextState::try_new(
            calendar_id,
            frequency,
            bus_basis,
            coupon_period,
            end_is_termination_date,
        )
        .map_err(core_to_py)?;
        Ok(Self { inner })
    }

    /// Optional calendar identifier.
    #[getter]
    fn calendar_id(&self) -> Option<&str> {
        self.inner.calendar_id.as_deref()
    }

    /// Optional coupon frequency.
    #[getter]
    fn frequency(&self) -> Option<PyTenor> {
        self.inner.frequency.map(PyTenor::from_inner)
    }

    /// Optional custom business-day divisor.
    #[getter]
    fn bus_basis(&self) -> Option<u16> {
        self.inner.bus_basis
    }

    /// Optional unadjusted ICMA reference or ACT/365L enclosing coupon as ``(start, end)`` dates.
    #[getter]
    fn coupon_period<'py>(
        &self,
        py: Python<'py>,
    ) -> PyResult<Option<(Bound<'py, PyAny>, Bound<'py, PyAny>)>> {
        self.inner
            .coupon_period
            .map(|(s, e)| Ok((date_to_py(py, s)?, date_to_py(py, e)?)))
            .transpose()
    }

    /// Whether the later accrual endpoint is the termination date; ``None`` if unspecified.
    #[getter]
    fn end_is_termination_date(&self) -> Option<bool> {
        self.inner.end_is_termination_date
    }

    fn __repr__(&self) -> String {
        let state = &self.inner;
        let calendar_id = match &state.calendar_id {
            Some(s) => format!("'{s}'"),
            None => "None".to_string(),
        };
        let frequency = match state.frequency {
            Some(t) => format!("Tenor('{t}')"),
            None => "None".to_string(),
        };
        let bus_basis = state
            .bus_basis
            .map_or_else(|| "None".to_string(), |b| b.to_string());
        let coupon_period = match state.coupon_period {
            Some((s, e)) => format!("('{s}', '{e}')"),
            None => "None".to_string(),
        };
        let flag = match state.end_is_termination_date {
            Some(true) => "True",
            Some(false) => "False",
            None => "None",
        };
        format!(
            "DayCountContext(calendar_id={calendar_id}, frequency={frequency}, bus_basis={bus_basis}, coupon_period={coupon_period}, end_is_termination_date={flag})"
        )
    }
}

wire_methods!(PyDayCountContext, DayCountContextState, "DayCountContext");

/// Register day-count types on the `finstack_quant.core.dates` module.
pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyDayCount>()?;
    m.add_class::<PyDayCountContext>()?;
    Ok(())
}

/// Names exported from this submodule.
pub const EXPORTS: &[&str] = &["DayCount", "DayCountContext"];
