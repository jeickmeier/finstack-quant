//! Python bindings for schedule generation from [`finstack_quant_core::dates`].

use crate::bindings::core::dates::calendar::extract_business_day_convention;
use crate::bindings::core::dates::tenor::extract_tenor;
use crate::bindings::date_utils::py_to_date;
use crate::errors::core_to_py;
use finstack_quant_core::dates::{Schedule, ScheduleErrorPolicy, ScheduleSpec, StubKind};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyIterator, PyList, PyModule, PyType};

/// Stub positioning rule for schedule generation.
///
/// Immutable, hashable enum-style type. ``str()`` gives the snake_case wire
/// name (``"short_front"``), which ``from_name`` parses.
///
/// Examples
/// --------
/// >>> from finstack_quant.core.dates import StubKind
/// >>> StubKind.from_name("short_front") == StubKind.SHORT_FRONT
/// True
#[pyclass(
    name = "StubKind",
    module = "finstack_quant.core.dates",
    frozen,
    eq,
    hash,
    skip_from_py_object
)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PyStubKind {
    /// Inner stub-kind variant.
    pub(crate) inner: StubKind,
}

impl PyStubKind {
    /// Build from an existing Rust [`StubKind`].
    pub(crate) const fn from_inner(inner: StubKind) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyStubKind {
    /// No stub — periods divide evenly.
    #[classattr]
    const NONE: PyStubKind = PyStubKind {
        inner: StubKind::None,
    };
    /// Short stub at the front.
    #[classattr]
    const SHORT_FRONT: PyStubKind = PyStubKind {
        inner: StubKind::ShortFront,
    };
    /// Short stub at the back.
    #[classattr]
    const SHORT_BACK: PyStubKind = PyStubKind {
        inner: StubKind::ShortBack,
    };
    /// Long stub at the front.
    #[classattr]
    const LONG_FRONT: PyStubKind = PyStubKind {
        inner: StubKind::LongFront,
    };
    /// Long stub at the back.
    #[classattr]
    const LONG_BACK: PyStubKind = PyStubKind {
        inner: StubKind::LongBack,
    };

    /// Parse from a string (e.g. ``"short_front"``, ``"long_back"``); raises
    /// ``ValueError`` for unknown names.
    #[classmethod]
    #[pyo3(text_signature = "(cls, name)")]
    fn from_name(_cls: &Bound<'_, PyType>, name: &str) -> PyResult<Self> {
        name.parse::<StubKind>()
            .map(Self::from_inner)
            .map_err(crate::errors::value_error)
    }

    /// Support ``pickle`` by reconstructing through ``from_name``.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_name = py.get_type::<Self>().getattr("from_name")?;
        Ok((from_name, (self.inner.to_string(),)))
    }

    fn __repr__(&self) -> String {
        format!("StubKind('{}')", self.inner)
    }

    fn __str__(&self) -> String {
        self.inner.to_string()
    }
}

/// Extract a [`StubKind`] from a wrapper or its snake_case name.
fn extract_stub_kind(obj: &Bound<'_, PyAny>) -> PyResult<StubKind> {
    if let Ok(s) = obj.extract::<PyRef<'_, PyStubKind>>() {
        return Ok(s.inner);
    }
    if let Ok(s) = obj.extract::<String>() {
        return s.parse::<StubKind>().map_err(crate::errors::value_error);
    }
    Err(pyo3::exceptions::PyTypeError::new_err(
        "expected StubKind or str",
    ))
}

/// Error handling policy for schedule building.
///
/// Immutable, hashable enum-style type. ``str()`` gives the snake_case wire
/// name (``"strict"``, ``"missing_calendar_warning"``, ``"graceful_empty"``),
/// which ``from_name`` parses.
///
/// Examples
/// --------
/// >>> from finstack_quant.core.dates import ScheduleErrorPolicy
/// >>> ScheduleErrorPolicy.from_name("graceful_empty") == ScheduleErrorPolicy.GRACEFUL_EMPTY
/// True
#[pyclass(
    name = "ScheduleErrorPolicy",
    module = "finstack_quant.core.dates",
    frozen,
    eq,
    hash,
    skip_from_py_object
)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PyScheduleErrorPolicy {
    /// Inner policy variant.
    pub(crate) inner: ScheduleErrorPolicy,
}

impl PyScheduleErrorPolicy {
    fn label(&self) -> PyResult<String> {
        finstack_quant_core::wire::serde_label(&self.inner).map_err(core_to_py)
    }
}

/// Extract a [`ScheduleErrorPolicy`] from a wrapper or its snake_case name.
fn extract_error_policy(obj: &Bound<'_, PyAny>) -> PyResult<ScheduleErrorPolicy> {
    if let Ok(p) = obj.extract::<PyRef<'_, PyScheduleErrorPolicy>>() {
        return Ok(p.inner);
    }
    if let Ok(s) = obj.extract::<String>() {
        return finstack_quant_core::wire::serde_parse(&s.to_ascii_lowercase()).map_err(core_to_py);
    }
    Err(pyo3::exceptions::PyTypeError::new_err(
        "expected ScheduleErrorPolicy or str",
    ))
}

#[pymethods]
impl PyScheduleErrorPolicy {
    /// Strict — errors are immediately propagated.
    #[classattr]
    const STRICT: PyScheduleErrorPolicy = PyScheduleErrorPolicy {
        inner: ScheduleErrorPolicy::Strict,
    };
    /// Emit a warning for missing calendars, but continue.
    #[classattr]
    const MISSING_CALENDAR_WARNING: PyScheduleErrorPolicy = PyScheduleErrorPolicy {
        inner: ScheduleErrorPolicy::MissingCalendarWarning,
    };
    /// Gracefully return an empty schedule on error.
    #[classattr]
    const GRACEFUL_EMPTY: PyScheduleErrorPolicy = PyScheduleErrorPolicy {
        inner: ScheduleErrorPolicy::GracefulEmpty,
    };

    /// Parse from the snake_case name (``"strict"``, ``"missing_calendar_warning"``,
    /// ``"graceful_empty"``), case-insensitively; raises ``ValueError`` otherwise.
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
            ScheduleErrorPolicy::Strict => "STRICT",
            ScheduleErrorPolicy::MissingCalendarWarning => "MISSING_CALENDAR_WARNING",
            ScheduleErrorPolicy::GracefulEmpty => "GRACEFUL_EMPTY",
        };
        format!("ScheduleErrorPolicy.{label}")
    }

    fn __str__(&self) -> PyResult<String> {
        self.label()
    }
}

/// Convert a Python spec (``dict`` or JSON string) into a [`ScheduleSpec`].
fn extract_schedule_spec(obj: &Bound<'_, PyAny>) -> PyResult<ScheduleSpec> {
    if let Ok(json) = obj.extract::<String>() {
        return serde_json::from_str(&json)
            .map_err(|e| crate::errors::serde_json_to_py(e, "invalid ScheduleSpec JSON"));
    }
    crate::bindings::module_utils::py_to_serde(obj.py(), obj, "ScheduleSpec")
}

/// A generated date schedule.
///
/// Immutable value type produced by ``ScheduleBuilder``.
/// ``dates`` is the unadjusted accrual grid (start plus every period end);
/// ``payment_dates`` and ``fixing_dates`` are one per accrual period.
/// Iterating a schedule yields its ``dates``.
///
/// Examples
/// --------
/// >>> from finstack_quant.core.dates import Schedule
/// >>> schedule = Schedule.builder("2025-01-15", "2025-07-15").frequency("3M").build()
/// >>> [d.isoformat() for d in schedule]
/// ['2025-01-15', '2025-04-15', '2025-07-15']
#[pyclass(
    name = "Schedule",
    module = "finstack_quant.core.dates",
    frozen,
    eq,
    skip_from_py_object
)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PySchedule {
    /// Inner Rust schedule.
    pub(crate) inner: Schedule,
}

impl PySchedule {
    /// Build from an existing Rust [`Schedule`].
    pub(crate) fn from_inner(inner: Schedule) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PySchedule {
    /// Start a schedule build between two dates.
    ///
    /// The canonical entry point, mirroring the ``Type.builder()`` form every
    /// other builder-backed type uses. The builder defaults to a monthly
    /// frequency, no stub, no adjustment and the ``STRICT`` policy.
    ///
    /// Parameters
    /// ----------
    /// start : datetime.date | str
    ///     First accrual date.
    /// end : datetime.date | str
    ///     Final accrual date; must not precede ``start``.
    ///
    /// Returns
    /// -------
    /// ScheduleBuilder
    ///     A fresh builder.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``start`` is after ``end`` or either date is invalid.
    #[staticmethod]
    #[pyo3(text_signature = "(start, end)")]
    fn builder(start: &Bound<'_, PyAny>, end: &Bound<'_, PyAny>) -> PyResult<PyScheduleBuilder> {
        PyScheduleBuilder::from_dates(start, end)
    }

    /// Build a schedule from a serialized spec (``dict`` or JSON string)
    /// with the canonical ``ScheduleSpec`` fields (``start``, ``end``,
    /// ``frequency``, ``stub``, ``business_day_convention``, ``calendar_id``,
    /// ``end_of_month``, ``imm_mode``, ``cds_imm_mode``, ``error_policy``,
    /// ``payment_lag_business_days``, ``fixing_lag_business_days``).
    ///
    /// Raises ``ValueError`` when the spec is malformed or the schedule
    /// cannot be built.
    #[staticmethod]
    #[pyo3(text_signature = "(spec)")]
    fn from_spec(spec: &Bound<'_, PyAny>) -> PyResult<Self> {
        extract_schedule_spec(spec)?
            .build()
            .map(Self::from_inner)
            .map_err(core_to_py)
    }

    /// Unadjusted accrual dates as a list of ``datetime.date``.
    #[getter]
    fn dates<'py>(&self, py: Python<'py>) -> PyResult<Vec<Bound<'py, PyAny>>> {
        crate::bindings::pandas_utils::dates_to_pylist(py, &self.inner.dates)
    }

    /// Payment date for each accrual period (one per period end).
    #[getter]
    fn payment_dates<'py>(&self, py: Python<'py>) -> PyResult<Vec<Bound<'py, PyAny>>> {
        crate::bindings::pandas_utils::dates_to_pylist(py, &self.inner.payment_dates)
    }

    /// Fixing dates for each accrual period. Empty when no fixing lag is set.
    #[getter]
    fn fixing_dates<'py>(&self, py: Python<'py>) -> PyResult<Vec<Bound<'py, PyAny>>> {
        crate::bindings::pandas_utils::dates_to_pylist(py, &self.inner.fixing_dates)
    }

    /// Whether any warnings were generated.
    fn has_warnings(&self) -> bool {
        self.inner.has_warnings()
    }

    /// Whether a graceful fallback was used.
    fn used_graceful_fallback(&self) -> bool {
        self.inner.used_graceful_fallback()
    }

    /// Warnings as a list of dicts with ``kind`` (``"graceful_fallback"`` or
    /// ``"missing_calendar_id"``), ``message`` and the warning's own field
    /// (``error_message`` / ``calendar_id``).
    #[getter]
    fn warnings<'py>(&self, py: Python<'py>) -> PyResult<Vec<Bound<'py, PyDict>>> {
        self.inner
            .warnings
            .iter()
            .map(|w| {
                let d = PyDict::new(py);
                let value = serde_json::to_value(w).map_err(|e| {
                    crate::errors::serde_json_to_py(e, "cannot serialize ScheduleWarning")
                })?;
                if let serde_json::Value::Object(map) = value {
                    for (kind, payload) in map {
                        d.set_item("kind", kind)?;
                        if let serde_json::Value::Object(fields) = payload {
                            for (name, field) in fields {
                                d.set_item(
                                    name,
                                    crate::bindings::pandas_utils::serde_to_py(py, &field)?,
                                )?;
                            }
                        }
                    }
                }
                d.set_item("message", w.to_string())?;
                Ok(d)
            })
            .collect()
    }

    /// Accrual periods as a pandas DataFrame with ``datetime64`` columns
    /// ``period_start``, ``period_end``, ``payment_date`` and ``fixing_date``
    /// (``NaT`` when no fixing lag is configured); one row per period.
    #[allow(clippy::wrong_self_convention)]
    #[pyo3(text_signature = "(self)")]
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let n = self.inner.dates.len().saturating_sub(1);
        let starts = &self.inner.dates[..n];
        let ends = self.inner.dates.get(1..).unwrap_or_default();
        let to_datetime = py.import("pandas")?.getattr("to_datetime")?;
        let column = |dates: &[time::Date]| -> PyResult<Bound<'py, PyAny>> {
            to_datetime.call1((crate::bindings::pandas_utils::dates_to_pylist(py, dates)?,))
        };
        let columns = PyDict::new(py);
        columns.set_item("period_start", column(starts)?)?;
        columns.set_item("period_end", column(ends)?)?;
        columns.set_item("payment_date", column(&self.inner.payment_dates)?)?;
        if self.inner.fixing_dates.is_empty() {
            let nones: Vec<Option<()>> = vec![None; n];
            columns.set_item("fixing_date", to_datetime.call1((nones,))?)?;
        } else {
            columns.set_item("fixing_date", column(&self.inner.fixing_dates)?)?;
        }
        crate::bindings::pandas_utils::dict_to_dataframe(py, &columns, None)
    }

    /// Serialize to the canonical JSON wire form (strict field names).
    #[allow(clippy::wrong_self_convention)]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|e| crate::errors::serde_json_to_py(e, "cannot serialize Schedule"))
    }

    /// Deserialize from canonical JSON; raises ``ValueError`` on malformed input.
    #[staticmethod]
    #[pyo3(text_signature = "(json)")]
    fn from_json(json: &str) -> PyResult<Self> {
        serde_json::from_str::<Schedule>(json)
            .map(Self::from_inner)
            .map_err(|e| crate::errors::serde_json_to_py(e, "invalid Schedule JSON"))
    }

    /// Support ``pickle`` through the JSON round-trip.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Iterate over the unadjusted accrual dates.
    fn __iter__<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyIterator>> {
        PyList::new(py, self.dates(py)?)?.into_any().try_iter()
    }

    /// Number of dates in the schedule.
    fn __len__(&self) -> usize {
        self.inner.dates.len()
    }

    fn __repr__(&self) -> String {
        match (self.inner.dates.first(), self.inner.dates.last()) {
            (Some(first), Some(last)) => format!(
                "Schedule('{first}'..'{last}', periods={})",
                self.inner.dates.len().saturating_sub(1)
            ),
            _ => "Schedule(periods=0)".to_string(),
        }
    }
}

/// Registry id for a ``HolidayCalendar`` wrapper or calendar-id string.
///
/// Wrappers contribute their canonical code. Strings are stored as given and
/// resolved by the Rust build under the configured error policy, so
/// ``MISSING_CALENDAR_WARNING`` / ``GRACEFUL_EMPTY`` keep their meaning.
fn calendar_code(obj: &Bound<'_, PyAny>) -> PyResult<String> {
    if let Ok(cal) =
        obj.extract::<PyRef<'_, crate::bindings::core::dates::calendar::PyHolidayCalendar>>()
    {
        return Ok(cal.canonical_code().to_string());
    }
    if let Ok(code) = obj.extract::<String>() {
        return Ok(code);
    }
    Err(pyo3::exceptions::PyTypeError::new_err(
        "expected HolidayCalendar or str calendar code",
    ))
}

/// Builder for constructing date schedules.
///
/// Setters mutate the builder in place and return the same instance, so
/// calls chain. Obtain one through ``Schedule.builder(start, end)``.
///
/// Examples
/// --------
/// >>> from datetime import date
/// >>> from finstack_quant.core.dates import Schedule, StubKind
/// >>> schedule = (
/// ...     Schedule.builder(date(2025, 1, 15), date(2030, 1, 15))
/// ...     .frequency("3M")
/// ...     .stub_rule(StubKind.SHORT_FRONT)
/// ...     .adjust_with("modified_following", "usny")
/// ...     .build()
/// ... )
/// >>> len(schedule)
/// 21
#[pyclass(
    name = "ScheduleBuilder",
    module = "finstack_quant.core.dates",
    skip_from_py_object
)]
pub struct PyScheduleBuilder {
    /// Serializable spec accumulating builder state.
    spec: ScheduleSpec,
}

impl PyScheduleBuilder {
    pub(crate) fn from_dates(start: &Bound<'_, PyAny>, end: &Bound<'_, PyAny>) -> PyResult<Self> {
        let s = py_to_date(start)?;
        let e = py_to_date(end)?;
        Ok(Self {
            spec: ScheduleSpec::new(s, e).map_err(core_to_py)?,
        })
    }
}

#[pymethods]
impl PyScheduleBuilder {
    /// Set the coupon/roll frequency (accepts ``Tenor`` or a string like ``"3M"``).
    fn frequency<'py>(
        mut slf: PyRefMut<'py, Self>,
        frequency: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        slf.spec.frequency = extract_tenor(frequency)?;
        Ok(slf)
    }

    /// Set the stub rule (``StubKind`` or its name such as ``"short_front"``).
    fn stub_rule<'py>(
        mut slf: PyRefMut<'py, Self>,
        stub: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        slf.spec.stub = extract_stub_kind(stub)?;
        Ok(slf)
    }

    /// Set the business-day convention and calendar used to adjust payment dates.
    ///
    /// Parameters
    /// ----------
    /// convention : BusinessDayConvention | str
    ///     Roll rule (``"modified_following"``, short codes ``MF``/``F``/``P``).
    /// calendar : HolidayCalendar | str
    ///     Holiday calendar object or registry id (``"usny"``; ``"nyse+gblo"``
    ///     joins calendars). A string id is resolved at ``build()`` under the
    ///     error policy (``STRICT`` raises ``KeyError`` for unknown ids).
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``convention`` is unknown.
    fn adjust_with<'py>(
        mut slf: PyRefMut<'py, Self>,
        convention: &Bound<'_, PyAny>,
        calendar: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        slf.spec.business_day_convention = Some(extract_business_day_convention(convention)?);
        slf.spec.calendar_id = Some(calendar_code(calendar)?);
        Ok(slf)
    }

    /// Shift each payment date by ``lag`` business days after the adjusted period end.
    fn payment_lag_business_days(mut slf: PyRefMut<'_, Self>, lag: i32) -> PyRefMut<'_, Self> {
        slf.spec.payment_lag_business_days = lag;
        slf
    }

    /// Set a T-minus fixing lag from each period's unadjusted accrual start.
    fn fixing_lag_business_days(mut slf: PyRefMut<'_, Self>, lag: i32) -> PyRefMut<'_, Self> {
        slf.spec.fixing_lag_business_days = Some(lag);
        slf
    }

    /// Enable or disable end-of-month roll logic.
    ///
    /// ``eom=True`` requires a month/year tenor and a regular schedule rule.
    /// ``build`` raises ``ValueError`` for day/week tenors or IMM/CDS modes.
    fn end_of_month(mut slf: PyRefMut<'_, Self>, eom: bool) -> PyRefMut<'_, Self> {
        slf.spec.end_of_month = eom;
        slf
    }

    /// Enable CDS IMM date mode and disable standard IMM mode.
    fn cds_imm(mut slf: PyRefMut<'_, Self>) -> PyRefMut<'_, Self> {
        slf.spec.cds_imm_mode = true;
        slf.spec.imm_mode = false;
        slf
    }

    /// Enable standard IMM date mode and disable CDS IMM mode.
    fn imm(mut slf: PyRefMut<'_, Self>) -> PyRefMut<'_, Self> {
        slf.spec.imm_mode = true;
        slf.spec.cds_imm_mode = false;
        slf
    }

    /// Set the error policy (``ScheduleErrorPolicy`` or its name such as
    /// ``"graceful_empty"``). Setting a policy fully replaces any previous
    /// policy (calls are order-independent and idempotent).
    fn error_policy<'py>(
        mut slf: PyRefMut<'py, Self>,
        policy: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        slf.spec.error_policy = extract_error_policy(policy)?;
        Ok(slf)
    }

    /// Current builder state as a ``ScheduleSpec`` dict (the input accepted by
    /// ``Schedule.from_spec``).
    #[allow(clippy::wrong_self_convention)]
    #[pyo3(text_signature = "(self)")]
    fn to_spec<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::pandas_utils::serde_to_py(py, &self.spec)
    }

    /// Build the schedule.
    ///
    /// Delegates entirely to the canonical Rust ``ScheduleSpec::build``:
    /// under the default ``STRICT`` policy an invalid range or any build
    /// warning raises ``ValueError`` (strict fails closed in Rust). Under
    /// ``MISSING_CALENDAR_WARNING`` or ``GRACEFUL_EMPTY`` the schedule is
    /// returned carrying its warnings (inspect via ``Schedule.warnings`` /
    /// ``Schedule.has_warnings()``).
    fn build(&self) -> PyResult<PySchedule> {
        self.spec
            .build()
            .map(PySchedule::from_inner)
            .map_err(core_to_py)
    }

    fn __repr__(&self) -> String {
        format!(
            "ScheduleBuilder(start='{}', end='{}', frequency='{}')",
            self.spec.start, self.spec.end, self.spec.frequency,
        )
    }
}

/// Register schedule types on the `finstack_quant.core.dates` module.
pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyStubKind>()?;
    m.add_class::<PyScheduleErrorPolicy>()?;
    m.add_class::<PySchedule>()?;
    m.add_class::<PyScheduleBuilder>()?;
    Ok(())
}

/// Names exported from this submodule.
pub const EXPORTS: &[&str] = &[
    "StubKind",
    "ScheduleErrorPolicy",
    "Schedule",
    "ScheduleBuilder",
];
