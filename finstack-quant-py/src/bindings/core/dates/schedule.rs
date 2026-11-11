//! Python bindings for schedule generation from [`finstack_quant_core::dates`].

use crate::bindings::core::dates::calendar::extract_business_day_convention;
use crate::bindings::core::dates::tenor::extract_tenor;
use crate::bindings::date_utils::py_to_date;
use crate::bindings::macros::{label_enum, typed_or_label, wire_methods};
use crate::errors::core_to_py;
use finstack_quant_core::dates::{GridMode, Schedule, ScheduleBuilder, ScheduleSpec, StubKind};
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

/// Regular-grid selection for schedule generation.
///
/// Immutable, hashable enum-style type. ``str()`` gives the snake_case wire
/// name (``"end_of_month"``), which ``from_name`` parses.
///
/// Examples
/// --------
/// >>> from finstack_quant.core.dates import GridMode
/// >>> GridMode.from_name("quarterly_imm") == GridMode.QUARTERLY_IMM
/// True
#[pyclass(
    name = "GridMode",
    module = "finstack_quant.core.dates",
    frozen,
    eq,
    hash,
    skip_from_py_object
)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PyGridMode {
    /// Inner grid-mode variant.
    pub(crate) inner: GridMode,
}

label_enum!(PyGridMode, GridMode, "GridMode", from_name, {
    /// Tenor-arithmetic dates with no snapping.
    NATURAL => Natural,
    /// Last calendar day of each month.
    END_OF_MONTH => EndOfMonth,
    /// Third Wednesday of March, June, September and December.
    QUARTERLY_IMM => QuarterlyImm,
});

typed_or_label!(
    fn extract_grid_mode -> GridMode,
    PyGridMode,
    "GridMode or str",
    |label| label.parse().map_err(core_to_py)
);

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
/// >>> schedule = (
/// ...     Schedule.builder("2025-01-15", "2025-07-15")
/// ...     .frequency("3M").stub_rule("none").grid("natural").unadjusted().build()
/// ... )
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
    /// other builder-backed type uses. There are no financial defaults: the
    /// frequency, stub rule, grid and payment convention must all be chosen
    /// before ``build()``.
    #[staticmethod]
    #[pyo3(text_signature = "(start, end)")]
    fn builder(start: &Bound<'_, PyAny>, end: &Bound<'_, PyAny>) -> PyResult<PyScheduleBuilder> {
        Ok(PyScheduleBuilder {
            inner: Schedule::builder(py_to_date(start)?, py_to_date(end)?).map_err(core_to_py)?,
        })
    }

    /// Build a schedule from a serialized spec (``dict`` or JSON string)
    /// with the canonical ``ScheduleSpec`` fields (``start``, ``end``,
    /// ``frequency``, ``stub``, ``grid``, ``cds_imm_mode``,
    /// ``business_day_convention``, ``calendar_id``, and the optional
    /// ``payment_lag_days`` and ``fixing_lag_business_days``).
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
        crate::bindings::pandas_utils::dates_to_pylist(py, self.inner.get_dates())
    }

    /// Payment date for each accrual period (one per period end).
    #[getter]
    fn payment_dates<'py>(&self, py: Python<'py>) -> PyResult<Vec<Bound<'py, PyAny>>> {
        crate::bindings::pandas_utils::dates_to_pylist(py, self.inner.get_payment_dates())
    }

    /// Fixing dates for each accrual period. Empty when no fixing lag is set.
    #[getter]
    fn fixing_dates<'py>(&self, py: Python<'py>) -> PyResult<Vec<Bound<'py, PyAny>>> {
        crate::bindings::pandas_utils::dates_to_pylist(py, self.inner.get_fixing_dates())
    }

    /// Accrual periods as a pandas DataFrame with ``datetime64`` columns
    /// ``period_start``, ``period_end``, ``payment_date`` and ``fixing_date``
    /// (``NaT`` when no fixing lag is configured); one row per period.
    #[pyo3(text_signature = "(self)")]
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        // A schedule always has at least two grid dates, so one period.
        let dates = self.inner.get_dates();
        let n = dates.len() - 1;
        let starts = &dates[..n];
        let ends = &dates[1..];
        let to_datetime = py.import("pandas")?.getattr("to_datetime")?;
        let column = |dates: &[finstack_quant_core::dates::Date]| -> PyResult<Bound<'py, PyAny>> {
            to_datetime.call1((crate::bindings::pandas_utils::dates_to_pylist(py, dates)?,))
        };
        let columns = PyDict::new(py);
        columns.set_item("period_start", column(starts)?)?;
        columns.set_item("period_end", column(ends)?)?;
        columns.set_item("payment_date", column(self.inner.get_payment_dates())?)?;
        if self.inner.get_fixing_dates().is_empty() {
            let nones: Vec<Option<()>> = vec![None; n];
            columns.set_item("fixing_date", to_datetime.call1((nones,))?)?;
        } else {
            columns.set_item("fixing_date", column(self.inner.get_fixing_dates())?)?;
        }
        crate::bindings::pandas_utils::dict_to_dataframe(py, &columns, None)
    }

    /// Iterate over the unadjusted accrual dates.
    fn __iter__<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyIterator>> {
        PyList::new(py, self.dates(py)?)?.into_any().try_iter()
    }

    /// Number of dates in the schedule.
    fn __len__(&self) -> usize {
        self.inner.get_dates().len()
    }

    fn __repr__(&self) -> String {
        match (
            self.inner.get_dates().first(),
            self.inner.get_dates().last(),
        ) {
            (Some(first), Some(last)) => format!(
                "Schedule('{first}'..'{last}', periods={})",
                self.inner.get_dates().len().saturating_sub(1)
            ),
            _ => "Schedule(periods=0)".to_string(),
        }
    }
}

wire_methods!(PySchedule, Schedule, "Schedule");

/// Registry id for a ``HolidayCalendar`` wrapper or calendar-id string.
///
/// Wrappers contribute their canonical code. Strings are stored as given and
/// resolved strictly by the Rust build.
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
/// ...     .grid("natural")
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
    /// Canonical Rust builder accumulating the choices.
    inner: ScheduleBuilder,
}

#[pymethods]
impl PyScheduleBuilder {
    /// Set the coupon/roll frequency (accepts ``Tenor`` or a string like ``"3M"``).
    fn frequency<'py>(
        mut slf: PyRefMut<'py, Self>,
        frequency: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        slf.inner = slf.inner.clone().frequency(extract_tenor(frequency)?);
        Ok(slf)
    }

    /// Set the stub rule (``StubKind`` or its name such as ``"short_front"``).
    fn stub_rule<'py>(
        mut slf: PyRefMut<'py, Self>,
        stub: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        slf.inner = slf.inner.clone().stub_rule(extract_stub_kind(stub)?);
        Ok(slf)
    }

    /// Select the regular grid (``GridMode`` or its name such as ``"end_of_month"``).
    fn grid<'py>(
        mut slf: PyRefMut<'py, Self>,
        grid: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        slf.inner = slf.inner.clone().grid(extract_grid_mode(grid)?);
        Ok(slf)
    }

    /// Keep payment dates on the raw period ends, with no calendar.
    fn unadjusted(mut slf: PyRefMut<'_, Self>) -> PyRefMut<'_, Self> {
        slf.inner = slf.inner.clone().unadjusted();
        slf
    }

    /// Set the business-day convention and calendar used to adjust payment dates.
    ///
    /// Parameters
    /// ----------
    /// convention : BusinessDayConvention | str
    ///     Roll rule (for example ``"modified_following"``), ignoring outer whitespace and ASCII case.
    /// calendar : HolidayCalendar | str
    ///     Holiday calendar object or registry id (``"usny"``; ``"nyse+gblo"``
    ///     joins calendars). A string id is resolved strictly at ``build()``
    ///     (``KeyError`` for unknown ids).
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
        let convention = extract_business_day_convention(convention)?;
        let calendar = calendar_code(calendar)?;
        slf.inner = slf.inner.clone().adjust_with(convention, &calendar);
        Ok(slf)
    }

    /// Shift each payment date by ``lag`` business days after the adjusted period end.
    fn payment_lag_days(mut slf: PyRefMut<'_, Self>, lag: i32) -> PyRefMut<'_, Self> {
        slf.inner = slf.inner.clone().payment_lag_days(lag);
        slf
    }

    /// Set a T-minus fixing lag from each period's unadjusted accrual start.
    fn fixing_lag_business_days(mut slf: PyRefMut<'_, Self>, lag: i32) -> PyRefMut<'_, Self> {
        slf.inner = slf.inner.clone().fixing_lag_business_days(lag);
        slf
    }

    /// Use CDS IMM dates (the 20th of March, June, September and December).
    ///
    /// Same semantics as the Rust ``ScheduleBuilder::cds_imm``: sets the
    /// frequency to quarterly and the stub rule to ``short_back`` and turns
    /// CDS IMM mode on (it requires the natural grid). A later ``frequency`` or
    /// ``stub_rule`` call wins, and ``to_spec()`` reports what ``build()`` uses.
    fn cds_imm(mut slf: PyRefMut<'_, Self>) -> PyRefMut<'_, Self> {
        slf.inner = slf.inner.clone().cds_imm();
        slf
    }

    /// Current builder state as a ``ScheduleSpec`` dict (the input accepted by
    /// ``Schedule.from_spec``).
    #[pyo3(text_signature = "(self)")]
    fn to_spec<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let spec = self.inner.to_spec().map_err(core_to_py)?;
        crate::bindings::pandas_utils::serde_to_py(py, &spec)
    }

    /// Build the schedule.
    ///
    /// Delegates entirely to the canonical Rust ``ScheduleBuilder::build``,
    /// which returns a complete schedule or an error: a generation failure
    /// raises ``ValueError`` and an unknown calendar id ``KeyError``.
    fn build(&self) -> PyResult<PySchedule> {
        self.inner
            .clone()
            .build()
            .map(PySchedule::from_inner)
            .map_err(core_to_py)
    }

    fn __repr__(&self) -> String {
        match self.inner.to_spec() {
            Ok(spec) => format!(
                "ScheduleBuilder(start='{}', end='{}', frequency='{}', grid='{}')",
                spec.start, spec.end, spec.frequency, spec.grid,
            ),
            Err(_) => "ScheduleBuilder(incomplete)".to_string(),
        }
    }
}

/// Register schedule types on the `finstack_quant.core.dates` module.
pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyStubKind>()?;
    m.add_class::<PyGridMode>()?;
    m.add_class::<PySchedule>()?;
    m.add_class::<PyScheduleBuilder>()?;
    Ok(())
}

/// Names exported from this submodule.
pub const EXPORTS: &[&str] = &["StubKind", "GridMode", "Schedule", "ScheduleBuilder"];
