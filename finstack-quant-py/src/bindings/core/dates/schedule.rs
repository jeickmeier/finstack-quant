//! Python bindings for schedule generation from [`finstack_quant_core::dates`].

use crate::bindings::core::dates::calendar::extract_business_day_convention;
use crate::bindings::core::dates::tenor::extract_tenor;
use crate::bindings::date_utils::py_to_date;
use crate::bindings::macros::{label_enum, typed_or_label, wire_methods};
use crate::errors::core_to_py;
use finstack_quant_core::dates::{
    Schedule, ScheduleBuilder, ScheduleErrorPolicy, ScheduleSpec, StubKind,
};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyIterator, PyList, PyModule};

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

label_enum!(PyStubKind, StubKind, "StubKind", from_name, {
    /// No stub — periods divide evenly.
    NONE => None,
    /// Short stub at the front.
    SHORT_FRONT => ShortFront,
    /// Short stub at the back.
    SHORT_BACK => ShortBack,
    /// Long stub at the front.
    LONG_FRONT => LongFront,
    /// Long stub at the back.
    LONG_BACK => LongBack,
});

typed_or_label!(
    fn extract_stub_kind -> StubKind,
    PyStubKind,
    "StubKind or str",
    |label| label.parse().map_err(core_to_py)
);

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

typed_or_label!(
    fn extract_error_policy -> ScheduleErrorPolicy,
    PyScheduleErrorPolicy,
    "ScheduleErrorPolicy or str",
    |label| label.parse().map_err(core_to_py)
);

label_enum!(PyScheduleErrorPolicy, ScheduleErrorPolicy, "ScheduleErrorPolicy", from_name, {
    /// Strict — errors are immediately propagated.
    STRICT => Strict,
    /// Emit a warning for missing calendars, but continue.
    MISSING_CALENDAR_WARNING => MissingCalendarWarning,
    /// Gracefully return an empty schedule on error.
    GRACEFUL_EMPTY => GracefulEmpty,
});

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
        Ok(PyScheduleBuilder {
            spec: ScheduleSpec::new(py_to_date(start)?, py_to_date(end)?).map_err(core_to_py)?,
        })
    }

    /// Build a schedule from a serialized spec (``dict`` or JSON string)
    /// with the canonical ``ScheduleSpec`` fields (``start``, ``end``,
    /// ``frequency``, ``stub``, ``business_day_convention``, ``calendar_id``,
    /// ``end_of_month``, ``imm_mode``, ``cds_imm_mode``, ``error_policy``,
    /// ``payment_lag_days``, ``fixing_lag_business_days``).
    ///
    /// Raises ``ValueError`` when the spec is malformed or the schedule
    /// cannot be built.
    #[staticmethod]
    #[pyo3(text_signature = "(spec)")]
    fn from_spec(spec: &Bound<'_, PyAny>) -> PyResult<Self> {
        ScheduleBuilder::from(extract_schedule_spec(spec)?)
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
    #[pyo3(text_signature = "(self)")]
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let n = self.inner.dates.len().saturating_sub(1);
        let starts = &self.inner.dates[..n];
        let ends = self.inner.dates.get(1..).unwrap_or_default();
        let to_datetime = py.import("pandas")?.getattr("to_datetime")?;
        let column = |dates: &[finstack_quant_core::dates::Date]| -> PyResult<Bound<'py, PyAny>> {
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

wire_methods!(PySchedule, Schedule, "Schedule");

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
    ///     Roll rule (for example ``"modified_following"``), ignoring outer whitespace and ASCII case.
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
    fn payment_lag_days(mut slf: PyRefMut<'_, Self>, lag: i32) -> PyRefMut<'_, Self> {
        slf.spec.payment_lag_days = lag;
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
    /// With ``StubKind.NONE``, maturity must lie on the generated month-end
    /// roll grid; otherwise an explicit stub rule is required.
    fn end_of_month(mut slf: PyRefMut<'_, Self>, eom: bool) -> PyRefMut<'_, Self> {
        slf.spec.end_of_month = eom;
        slf
    }

    /// Use CDS IMM dates (the 20th of March, June, September and December).
    ///
    /// Same semantics as the Rust ``ScheduleBuilder::cds_imm``: sets the
    /// frequency to quarterly and the stub rule to ``short_back``, turns CDS
    /// IMM mode on and standard IMM mode off. A later ``frequency`` or
    /// ``stub_rule`` call wins, and ``to_spec()`` reports what ``build()`` uses.
    fn cds_imm(mut slf: PyRefMut<'_, Self>) -> PyRefMut<'_, Self> {
        slf.spec = slf.spec.clone().with_cds_imm();
        slf
    }

    /// Roll on standard IMM dates (the third Wednesday of Mar, Jun, Sep, Dec).
    ///
    /// Same semantics as the Rust ``ScheduleBuilder::imm``: sets the IMM
    /// defaults (quarterly frequency, ``short_back`` stub rule), turns standard
    /// IMM mode on and CDS IMM mode off. A later ``frequency`` or ``stub_rule``
    /// call wins. ``build()`` keeps ``start`` and ``end`` as the first and last
    /// dates and lays the regular periods on IMM dates at the frequency, which
    /// must be a whole number of quarters: forward from the first IMM date on
    /// or after ``start`` for back-stub rules, backward from the last IMM date
    /// on or before ``end`` for front-stub rules. With no IMM date strictly
    /// between ``start`` and ``end`` the schedule is the single period
    /// ``[start, end]``.
    fn imm(mut slf: PyRefMut<'_, Self>) -> PyRefMut<'_, Self> {
        slf.spec = slf.spec.clone().with_imm();
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
    #[pyo3(text_signature = "(self)")]
    fn to_spec<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::pandas_utils::serde_to_py(py, &self.spec)
    }

    /// Build the schedule.
    ///
    /// Delegates entirely to the canonical Rust ``ScheduleBuilder::build``:
    /// under the default ``STRICT`` policy a generation failure raises
    /// ``ValueError`` and an unknown calendar id ``KeyError``. Under
    /// ``MISSING_CALENDAR_WARNING`` or ``GRACEFUL_EMPTY`` the schedule is
    /// returned carrying its warnings (inspect via ``Schedule.warnings`` /
    /// ``Schedule.has_warnings()``).
    fn build(&self) -> PyResult<PySchedule> {
        ScheduleBuilder::from(self.spec.clone())
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
