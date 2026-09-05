//! Python bindings for day-count conventions from [`finstack_quant_core::dates`].

use crate::bindings::core::dates::calendar::extract_calendar;
use crate::bindings::core::dates::tenor::{extract_tenor, PyTenor};
use crate::bindings::date_utils::{date_to_py, py_to_date};
use crate::errors::core_to_py;
use finstack_quant_core::dates::{
    days_30_360, days_30e_360_isda, DayCount, DayCountContext, DayCountContextState,
    Thirty360Convention,
};
use pyo3::prelude::*;
use pyo3::types::{PyModule, PyType};

/// Day-count convention for year-fraction calculations.
///
/// Immutable, hashable enum-style type with a class attribute per supported
/// convention. ``str()`` gives the canonical snake_case name
/// (``"act_360"``), which ``from_name`` parses strictly and ``parse``
/// leniently (``"ACT/360"``, ``"Act/Act ICMA"``).
///
/// Examples
/// --------
/// >>> import datetime
/// >>> from finstack_quant.core.dates import DayCount
/// >>> DayCount.ACT_360.year_fraction(datetime.date(2024, 1, 1), datetime.date(2024, 7, 1))
/// 0.5055555555555555
/// >>> DayCount.parse("ACT/ACT ICMA") == DayCount.ACT_ACT_ISMA
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

/// Extract a [`DayCount`] from a ``DayCount`` wrapper or a (lenient) name string.
pub(crate) fn extract_day_count(obj: &Bound<'_, PyAny>) -> PyResult<DayCount> {
    if let Ok(day_count) = obj.extract::<PyRef<'_, PyDayCount>>() {
        return Ok(day_count.inner);
    }
    if let Ok(s) = obj.extract::<String>() {
        return DayCount::parse(&s).map_err(core_to_py);
    }
    Err(pyo3::exceptions::PyTypeError::new_err(
        "expected DayCount or str",
    ))
}

/// Build a [`DayCountContext`] for a year-fraction call from the optional
/// ``ctx`` object or the ``frequency``/``calendar`` keywords.
fn context_from_args(
    ctx: Option<&PyDayCountContext>,
    frequency: Option<&Bound<'_, PyAny>>,
    calendar: Option<&Bound<'_, PyAny>>,
) -> PyResult<DayCountContext<'static>> {
    match ctx {
        Some(c) => {
            if frequency.is_some() || calendar.is_some() {
                return Err(crate::errors::value_error(
                    "pass either ctx or the frequency/calendar keywords, not both",
                ));
            }
            c.to_rust_ctx()
        }
        None => Ok(DayCountContext {
            calendar: calendar.map(extract_calendar).transpose()?,
            frequency: frequency.map(extract_tenor).transpose()?,
            ..DayCountContext::default()
        }),
    }
}

#[pymethods]
impl PyDayCount {
    /// Actual/360 (money market).
    #[classattr]
    const ACT_360: PyDayCount = PyDayCount {
        inner: DayCount::Act360,
    };
    /// Actual/365 Fixed.
    #[classattr]
    const ACT_365F: PyDayCount = PyDayCount {
        inner: DayCount::Act365F,
    };
    /// Actual/365L (ICMA Rule 251). Annual periods (or periods without a
    /// supplied frequency) use denominator 366 exactly when February 29 falls
    /// in ``(start, end]``; non-annual periods use 366 exactly when the end
    /// date's year is a leap year. Otherwise the denominator is 365. This is
    /// explicitly not ACT/ACT AFB, which uses sub-period splitting.
    #[classattr]
    const ACT_365L: PyDayCount = PyDayCount {
        inner: DayCount::Act365L,
    };
    /// NL/365 (Actual/365 No Leap): actual days excluding every February 29
    /// in ``(start, end]``, divided by 365.
    #[classattr]
    const NL_365: PyDayCount = PyDayCount {
        inner: DayCount::Nl365,
    };
    /// 30/360 US (Bond Basis).
    #[classattr]
    const THIRTY_360: PyDayCount = PyDayCount {
        inner: DayCount::Thirty360,
    };
    /// 30E/360 (Eurobond Basis).
    #[classattr]
    const THIRTY_E_360: PyDayCount = PyDayCount {
        inner: DayCount::ThirtyE360,
    };
    /// 30E/360 ISDA.
    #[classattr]
    const THIRTY_E_360_ISDA: PyDayCount = PyDayCount {
        inner: DayCount::ThirtyE360Isda,
    };
    /// Actual/Actual (ISDA).
    #[classattr]
    const ACT_ACT: PyDayCount = PyDayCount {
        inner: DayCount::ActAct,
    };
    /// Actual/Actual (ICMA/ISMA).
    #[classattr]
    const ACT_ACT_ISMA: PyDayCount = PyDayCount {
        inner: DayCount::ActActIsma,
    };
    /// Actual/Actual AFB (Actual/Actual Euro).
    ///
    /// Walks whole years backwards from the end date (QuantLib
    /// ``ActualActual::AFB``). A year-step landing on 28 February of a leap
    /// year is bumped to 29 February. The residual uses denominator 366 if
    /// 29 February lies in ``[start, residual_end)``, else 365.
    #[classattr]
    const ACT_ACT_AFB: PyDayCount = PyDayCount {
        inner: DayCount::ActActAfb,
    };
    /// 30/360 Italian.
    ///
    /// Day 31 becomes 30, and any February day after the 27th becomes 30
    /// (QuantLib ``Thirty360::Italian``). Distinct from US SIA and 30E/360.
    #[classattr]
    const THIRTY_360_IT: PyDayCount = PyDayCount {
        inner: DayCount::Thirty360It,
    };
    /// Business/252 (Brazilian market convention).
    #[classattr]
    const BUS_252: PyDayCount = PyDayCount {
        inner: DayCount::Bus252,
    };

    /// Parse a day-count convention from its canonical name (strict).
    ///
    /// Parameters
    /// ----------
    /// name : str
    ///     Exact snake_case identifier such as ``"act_360"``,
    ///     ``"act_act_isma"``, ``"30e_360_isda"`` or ``"nl_365"``.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``name`` is not one of the canonical names; the message lists
    ///     them. Use ``DayCount.parse`` for term-sheet spellings.
    #[classmethod]
    #[pyo3(text_signature = "(cls, name)")]
    fn from_name(_cls: &Bound<'_, PyType>, name: &str) -> PyResult<Self> {
        name.parse::<DayCount>()
            .map(Self::from_inner)
            .map_err(crate::errors::value_error)
    }

    /// Leniently parse a day-count label as written on term sheets.
    ///
    /// Case-insensitive; ``/``, ``-`` and spaces are treated as ``_``, and
    /// the market spellings ``"ACT/ACT ICMA"``, ``"ACT/ACT ISDA"``,
    /// ``"ACT/365"``, ``"30/360"``, ``"30E/360 ISDA"`` are recognised.
    ///
    /// Parameters
    /// ----------
    /// s : str
    ///     Day-count label (``"ACT/360"``, ``"Act/Act ICMA"``, or any
    ///     canonical name).
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If no spelling matches; the message lists the canonical names.
    #[classmethod]
    #[pyo3(text_signature = "(cls, s)")]
    fn parse(_cls: &Bound<'_, PyType>, s: &str) -> PyResult<Self> {
        DayCount::parse(s).map(Self::from_inner).map_err(core_to_py)
    }

    /// Compute the year fraction between two dates under this convention.
    ///
    /// Parameters
    /// ----------
    /// start : datetime.date | str
    ///     Accrual start (inclusive).
    /// end : datetime.date | str
    ///     Accrual end (exclusive); must not precede ``start``.
    /// ctx : DayCountContext | None
    ///     Full context object (calendar, frequency, coupon period, …).
    ///     Mutually exclusive with the keywords below.
    /// frequency : Tenor | str | None
    ///     Coupon frequency for ``ACT_ACT_ISMA``/``ACT_365L`` (e.g. ``"6M"``).
    /// calendar : HolidayCalendar | str | None
    ///     Holiday calendar required by ``BUS_252``.
    ///
    /// Returns
    /// -------
    /// float
    ///     Non-negative year fraction (``0.0`` when ``start == end``).
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``start > end``, both ``ctx`` and keywords are given, or the
    ///     convention needs context that was not supplied (ISMA without a
    ///     frequency, Bus/252 without a calendar).
    /// KeyError
    ///     If ``calendar`` names an unknown calendar.
    #[pyo3(
        signature = (start, end, ctx=None, *, frequency=None, calendar=None),
        text_signature = "(self, start, end, ctx=None, *, frequency=None, calendar=None)"
    )]
    fn year_fraction(
        &self,
        start: &Bound<'_, PyAny>,
        end: &Bound<'_, PyAny>,
        ctx: Option<&PyDayCountContext>,
        frequency: Option<&Bound<'_, PyAny>>,
        calendar: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<f64> {
        let s = py_to_date(start)?;
        let e = py_to_date(end)?;
        let context = context_from_args(ctx, frequency, calendar)?;
        self.inner.year_fraction(s, e, context).map_err(core_to_py)
    }

    /// Compute the signed year fraction (negative when ``start > end``).
    ///
    /// Accepts the same ``ctx`` / ``frequency`` / ``calendar`` inputs as
    /// ``year_fraction`` and raises the same exceptions, except that an
    /// inverted range is allowed.
    #[pyo3(
        signature = (start, end, ctx=None, *, frequency=None, calendar=None),
        text_signature = "(self, start, end, ctx=None, *, frequency=None, calendar=None)"
    )]
    fn signed_year_fraction(
        &self,
        start: &Bound<'_, PyAny>,
        end: &Bound<'_, PyAny>,
        ctx: Option<&PyDayCountContext>,
        frequency: Option<&Bound<'_, PyAny>>,
        calendar: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<f64> {
        let s = py_to_date(start)?;
        let e = py_to_date(end)?;
        let context = context_from_args(ctx, frequency, calendar)?;
        self.inner
            .signed_year_fraction(s, e, context)
            .map_err(core_to_py)
    }

    /// Count the calendar days between two dates (``end - start``, signed).
    #[staticmethod]
    #[pyo3(text_signature = "(start, end)")]
    fn calendar_days(start: &Bound<'_, PyAny>, end: &Bound<'_, PyAny>) -> PyResult<i64> {
        let s = py_to_date(start)?;
        let e = py_to_date(end)?;
        Ok(DayCount::calendar_days(s, e))
    }

    /// Support ``pickle`` by reconstructing through ``from_name``.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_name = py.get_type::<Self>().getattr("from_name")?;
        Ok((from_name, (self.inner.to_string(),)))
    }

    fn __repr__(&self) -> String {
        format!("DayCount('{}')", self.inner)
    }

    fn __str__(&self) -> String {
        self.inner.to_string()
    }
}

/// Shared constructor for the two context wrappers: validates through
/// [`DayCountContextState::try_new`] so an inverted ``coupon_period`` is
/// rejected in Rust.
fn build_state(
    calendar_id: Option<String>,
    frequency: Option<&Bound<'_, PyAny>>,
    bus_basis: Option<u16>,
    coupon_period: Option<(Bound<'_, PyAny>, Bound<'_, PyAny>)>,
    end_is_termination_date: bool,
) -> PyResult<DayCountContextState> {
    let coupon_period = coupon_period
        .map(|(s, e)| Ok::<_, PyErr>((py_to_date(&s)?, py_to_date(&e)?)))
        .transpose()?;
    let frequency = frequency.map(extract_tenor).transpose()?;
    DayCountContextState::try_new(
        calendar_id,
        frequency,
        bus_basis,
        coupon_period,
        end_is_termination_date,
    )
    .map_err(core_to_py)
}

/// Python-style rendering shared by both context wrappers.
fn state_repr(name: &str, state: &DayCountContextState) -> String {
    let opt_str = |v: &Option<String>| match v {
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
    let flag = if state.end_is_termination_date {
        "True"
    } else {
        "False"
    };
    format!(
        "{name}(calendar_id={}, frequency={frequency}, bus_basis={bus_basis}, coupon_period={coupon_period}, end_is_termination_date={flag})",
        opt_str(&state.calendar_id),
    )
}

/// Optional context for day-count calculations.
///
/// Certain conventions require additional information:
///
/// - ``BUS_252`` requires a holiday calendar (``calendar_id``).
/// - ``ACT_ACT_ISMA`` requires the coupon ``frequency`` and, for irregular
///   or mid-coupon accruals, the reference ``coupon_period``.
/// - ``THIRTY_E_360_ISDA`` uses ``end_is_termination_date`` for its
///   end-of-February rule.
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
///     Business-day divisor for ``BUS_252``; ``None`` selects 252.
/// coupon_period : tuple[datetime.date | str, datetime.date | str] | None
///     Reference coupon period ``(start, end)`` for ACT/ACT (ICMA);
///     ``start`` must precede ``end``.
/// end_is_termination_date : bool
///     Whether the accrual end is the instrument termination date.
///
/// Raises
/// ------
/// ValueError
///     If ``coupon_period`` is inverted or ``frequency`` does not parse.
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
    #[pyo3(signature = (calendar_id=None, frequency=None, bus_basis=None, coupon_period=None, end_is_termination_date=false))]
    fn new(
        calendar_id: Option<String>,
        frequency: Option<&Bound<'_, PyAny>>,
        bus_basis: Option<u16>,
        coupon_period: Option<(Bound<'_, PyAny>, Bound<'_, PyAny>)>,
        end_is_termination_date: bool,
    ) -> PyResult<Self> {
        Ok(Self {
            inner: build_state(
                calendar_id,
                frequency,
                bus_basis,
                coupon_period,
                end_is_termination_date,
            )?,
        })
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

    /// Optional reference coupon period as ``(start, end)`` dates.
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

    /// Whether the accrual end is the instrument termination date.
    #[getter]
    fn end_is_termination_date(&self) -> bool {
        self.inner.end_is_termination_date
    }

    /// Serialize to the canonical JSON wire form (strict field names).
    #[allow(clippy::wrong_self_convention)]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|e| crate::errors::serde_json_to_py(e, "cannot serialize DayCountContext"))
    }

    /// Deserialize from canonical JSON; raises ``ValueError`` on malformed input.
    #[staticmethod]
    #[pyo3(text_signature = "(json)")]
    fn from_json(json: &str) -> PyResult<Self> {
        serde_json::from_str::<DayCountContextState>(json)
            .map(|inner| Self { inner })
            .map_err(|e| crate::errors::serde_json_to_py(e, "invalid DayCountContext JSON"))
    }

    /// Support ``pickle`` through the JSON round-trip.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    fn __repr__(&self) -> String {
        state_repr("DayCountContext", &self.inner)
    }
}

/// 30/360 sub-convention (US SIA / Bond Basis, ISDA, European, Italian).
///
/// Immutable, hashable enum-style type used by ``days_30_360``. ``str()``
/// gives the snake_case wire name (``"us_sia"``), which ``from_name`` parses.
///
/// Examples
/// --------
/// >>> from finstack_quant.core.dates import Thirty360Convention
/// >>> str(Thirty360Convention.US_SIA)
/// 'us_sia'
#[pyclass(
    name = "Thirty360Convention",
    module = "finstack_quant.core.dates",
    frozen,
    eq,
    hash,
    skip_from_py_object
)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PyThirty360Convention {
    /// Inner convention variant.
    pub(crate) inner: Thirty360Convention,
}

impl PyThirty360Convention {
    fn label(&self) -> PyResult<String> {
        finstack_quant_core::wire::serde_label(&self.inner).map_err(core_to_py)
    }
}

/// Extract a [`Thirty360Convention`] from a wrapper or its snake_case name.
fn extract_thirty360(obj: &Bound<'_, PyAny>) -> PyResult<Thirty360Convention> {
    if let Ok(c) = obj.extract::<PyRef<'_, PyThirty360Convention>>() {
        return Ok(c.inner);
    }
    if let Ok(s) = obj.extract::<String>() {
        return finstack_quant_core::wire::serde_parse(&s.to_ascii_lowercase()).map_err(core_to_py);
    }
    Err(pyo3::exceptions::PyTypeError::new_err(
        "expected Thirty360Convention or str",
    ))
}

#[pymethods]
impl PyThirty360Convention {
    /// US 30/360 SIA / Bond Basis convention.
    #[classattr]
    const US_SIA: PyThirty360Convention = PyThirty360Convention {
        inner: Thirty360Convention::UsSia,
    };
    /// 30/360 ISDA convention.
    #[classattr]
    const ISDA: PyThirty360Convention = PyThirty360Convention {
        inner: Thirty360Convention::Isda,
    };
    /// European 30E/360 convention.
    #[classattr]
    const EUROPEAN: PyThirty360Convention = PyThirty360Convention {
        inner: Thirty360Convention::European,
    };
    /// 30/360 Italian convention.
    #[classattr]
    const ITALIAN: PyThirty360Convention = PyThirty360Convention {
        inner: Thirty360Convention::Italian,
    };

    /// Parse from the snake_case name (``"us_sia"``, ``"isda"``, ``"european"``, ``"italian"``),
    /// case-insensitively. Raises ``ValueError`` for anything else.
    #[classmethod]
    #[pyo3(text_signature = "(cls, name)")]
    fn from_name(_cls: &Bound<'_, PyType>, name: &str) -> PyResult<Self> {
        finstack_quant_core::wire::serde_parse(&name.to_ascii_lowercase())
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    /// Support ``pickle`` by reconstructing through ``from_name``.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_name = py.get_type::<Self>().getattr("from_name")?;
        Ok((from_name, (self.label()?,)))
    }

    fn __repr__(&self) -> String {
        let label = match self.inner {
            Thirty360Convention::UsSia => "US_SIA",
            Thirty360Convention::Isda => "ISDA",
            Thirty360Convention::European => "EUROPEAN",
            Thirty360Convention::Italian => "ITALIAN",
        };
        format!("Thirty360Convention.{label}")
    }

    fn __str__(&self) -> PyResult<String> {
        self.label()
    }
}

/// 30/360 day count between ``start`` (inclusive) and ``end`` (exclusive).
///
/// Parameters
/// ----------
/// start : datetime.date | str
///     Accrual start.
/// end : datetime.date | str
///     Accrual end; an earlier ``end`` gives a negative count.
/// convention : Thirty360Convention | str
///     Variant governing the month-end and February rules
///     (``"us_sia"``, ``"isda"``, ``"european"``, ``"italian"``).
///
/// Returns
/// -------
/// int
///     Signed 30/360 day count (divide by 360 for the year fraction).
///
/// Raises
/// ------
/// ValueError
///     If a date or the convention name is invalid.
///
/// Examples
/// --------
/// >>> from finstack_quant.core.dates import days_30_360
/// >>> days_30_360("2025-01-31", "2025-03-31", "isda")
/// 60
#[pyfunction(name = "days_30_360")]
#[pyo3(text_signature = "(start, end, convention)")]
fn py_days_30_360(
    start: &Bound<'_, PyAny>,
    end: &Bound<'_, PyAny>,
    convention: &Bound<'_, PyAny>,
) -> PyResult<i32> {
    Ok(days_30_360(
        py_to_date(start)?,
        py_to_date(end)?,
        extract_thirty360(convention)?,
    ))
}

/// 30E/360 ISDA day count between ``start`` (inclusive) and ``end`` (exclusive).
///
/// Parameters
/// ----------
/// start : datetime.date | str
///     Accrual start.
/// end : datetime.date | str
///     Accrual end; an earlier ``end`` gives a negative count.
/// end_is_termination_date : bool
///     Whether ``end`` is the instrument termination date, which switches off
///     the end-of-February 30-day rule for ``end`` (ISDA 2006 §4.16(h)).
///
/// Returns
/// -------
/// int
///     Signed 30E/360 ISDA day count.
///
/// Raises
/// ------
/// ValueError
///     If a date is invalid.
///
/// Examples
/// --------
/// >>> from finstack_quant.core.dates import days_30e_360_isda
/// >>> days_30e_360_isda("2024-01-31", "2024-02-29", False)
/// 30
#[pyfunction(name = "days_30e_360_isda")]
#[pyo3(text_signature = "(start, end, end_is_termination_date)")]
fn py_days_30e_360_isda(
    start: &Bound<'_, PyAny>,
    end: &Bound<'_, PyAny>,
    end_is_termination_date: bool,
) -> PyResult<i32> {
    Ok(days_30e_360_isda(
        py_to_date(start)?,
        py_to_date(end)?,
        end_is_termination_date,
    ))
}

/// Register day-count types on the `finstack_quant.core.dates` module.
pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyDayCount>()?;
    m.add_class::<PyDayCountContext>()?;
    m.add_class::<PyThirty360Convention>()?;
    m.add_function(wrap_pyfunction!(py_days_30_360, m)?)?;
    m.add_function(wrap_pyfunction!(py_days_30e_360_isda, m)?)?;
    Ok(())
}

/// Names exported from this submodule.
pub const EXPORTS: &[&str] = &[
    "DayCount",
    "DayCountContext",
    "Thirty360Convention",
    "days_30_360",
    "days_30e_360_isda",
];
