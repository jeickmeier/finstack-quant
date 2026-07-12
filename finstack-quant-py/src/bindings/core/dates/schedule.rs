//! Python bindings for schedule generation from [`finstack_quant_core::dates`].

use crate::bindings::core::dates::calendar::PyBusinessDayConvention;
use crate::bindings::core::dates::tenor::extract_tenor;
use crate::bindings::core::dates::utils::{date_to_py, py_to_date};
use crate::errors::core_to_py;
use finstack_quant_core::dates::{Schedule, ScheduleErrorPolicy, ScheduleSpec, StubKind};
use pyo3::prelude::*;
use pyo3::types::{PyModule, PyType};

/// Stub positioning rule for schedule generation.
#[pyclass(
    name = "StubKind",
    module = "finstack_quant.core.dates",
    frozen,
    eq,
    skip_from_py_object
)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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

    /// Parse from a string (e.g. ``"short_front"``, ``"long_back"``).
    #[classmethod]
    #[pyo3(text_signature = "(cls, name)")]
    fn from_name(_cls: &Bound<'_, PyType>, name: &str) -> PyResult<Self> {
        name.parse::<StubKind>()
            .map(Self::from_inner)
            .map_err(crate::errors::value_error)
    }

    /// Hash based on discriminant.
    fn __hash__(&self) -> isize {
        match self.inner {
            StubKind::None => 0,
            StubKind::ShortFront => 1,
            StubKind::ShortBack => 2,
            StubKind::LongFront => 3,
            StubKind::LongBack => 4,
            #[allow(unreachable_patterns)]
            _ => 255,
        }
    }

    fn __repr__(&self) -> String {
        format!("StubKind('{}')", self.inner)
    }

    fn __str__(&self) -> String {
        self.inner.to_string()
    }
}

/// Error handling policy for schedule building.
#[pyclass(
    name = "ScheduleErrorPolicy",
    module = "finstack_quant.core.dates",
    frozen,
    eq,
    skip_from_py_object
)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PyScheduleErrorPolicy {
    /// Inner policy variant.
    pub(crate) inner: ScheduleErrorPolicy,
}

impl PyScheduleErrorPolicy {
    /// Build from an existing Rust [`ScheduleErrorPolicy`].
    #[allow(dead_code)]
    pub(crate) const fn from_inner(inner: ScheduleErrorPolicy) -> Self {
        Self { inner }
    }
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

    /// Hash based on discriminant.
    fn __hash__(&self) -> isize {
        match self.inner {
            ScheduleErrorPolicy::Strict => 0,
            ScheduleErrorPolicy::MissingCalendarWarning => 1,
            ScheduleErrorPolicy::GracefulEmpty => 2,
        }
    }

    fn __repr__(&self) -> String {
        let label = match self.inner {
            ScheduleErrorPolicy::Strict => "STRICT",
            ScheduleErrorPolicy::MissingCalendarWarning => "MISSING_CALENDAR_WARNING",
            ScheduleErrorPolicy::GracefulEmpty => "GRACEFUL_EMPTY",
        };
        format!("ScheduleErrorPolicy.{label}")
    }
}

/// A generated date schedule.
#[pyclass(
    name = "Schedule",
    module = "finstack_quant.core.dates",
    frozen,
    skip_from_py_object
)]
#[derive(Clone, Debug)]
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
    /// Schedule dates as a list of ``datetime.date``.
    #[getter]
    fn dates<'py>(&self, py: Python<'py>) -> PyResult<Vec<Bound<'py, PyAny>>> {
        self.inner
            .dates
            .iter()
            .map(|d| date_to_py(py, *d))
            .collect()
    }

    /// Whether any warnings were generated.
    fn has_warnings(&self) -> bool {
        self.inner.has_warnings()
    }

    /// Whether a graceful fallback was used.
    fn used_graceful_fallback(&self) -> bool {
        self.inner.used_graceful_fallback()
    }

    /// Warning messages (if any).
    #[getter]
    fn warnings(&self) -> Vec<String> {
        self.inner.warnings.iter().map(|w| w.to_string()).collect()
    }

    /// Number of dates in the schedule.
    fn __len__(&self) -> usize {
        self.inner.dates.len()
    }

    fn __repr__(&self) -> String {
        format!("Schedule(dates={})", self.inner.dates.len())
    }
}

/// Builder for constructing date schedules.
///
/// Like the Rust [`finstack_quant_core::dates::ScheduleBuilder`], setters are
/// fluent. The Python binding preserves its existing in-place mutation
/// behavior and returns the same builder instance from every setter.
///
/// # Example
///
/// ```text
/// from datetime import date
/// from finstack_quant.core.dates import (
///     ScheduleBuilder,
///     StubKind,
///     BusinessDayConvention,
///     ScheduleErrorPolicy,
/// )
///
/// schedule = (
///     ScheduleBuilder(date(2025, 1, 15), date(2030, 1, 15))
///     .frequency("3M")
///     .stub_rule(StubKind.SHORT_FRONT)
///     .adjust_with(BusinessDayConvention.MODIFIED_FOLLOWING, "usny")
///     .end_of_month(False)
///     .error_policy(ScheduleErrorPolicy.STRICT)
///     .build()
/// )
/// assert len(schedule) >= 20  # ~quarterly periods over 5 years
/// ```
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
    /// Start a new schedule builder with start and end dates.
    #[new]
    #[pyo3(text_signature = "(start, end)")]
    fn new(start: &Bound<'_, PyAny>, end: &Bound<'_, PyAny>) -> PyResult<Self> {
        let s = py_to_date(start)?;
        let e = py_to_date(end)?;
        if s >= e {
            return Err(crate::errors::value_error("start must be before end"));
        }
        Ok(Self {
            spec: ScheduleSpec {
                start: s,
                end: e,
                frequency: finstack_quant_core::dates::Tenor::monthly(),
                stub: StubKind::None,
                business_day_convention: None,
                calendar_id: None,
                end_of_month: false,
                imm_mode: false,
                cds_imm_mode: false,
                error_policy: ScheduleErrorPolicy::Strict,
            },
        })
    }

    /// Set the coupon/roll frequency (accepts ``Tenor`` or a string like ``"3M"``).
    fn frequency<'py>(
        mut slf: PyRefMut<'py, Self>,
        freq: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        slf.spec.frequency = extract_tenor(freq)?;
        Ok(slf)
    }

    /// Set the stub rule.
    fn stub_rule<'py>(mut slf: PyRefMut<'py, Self>, stub: &PyStubKind) -> PyRefMut<'py, Self> {
        slf.spec.stub = stub.inner;
        slf
    }

    /// Set the business-day convention and calendar for adjustment.
    fn adjust_with<'py>(
        mut slf: PyRefMut<'py, Self>,
        convention: &PyBusinessDayConvention,
        calendar_id: &str,
    ) -> PyRefMut<'py, Self> {
        slf.spec.business_day_convention = Some(convention.inner);
        slf.spec.calendar_id = Some(calendar_id.to_string());
        slf
    }

    /// Enable or disable end-of-month roll logic.
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

    /// Set the error policy. Setting a policy fully replaces any previous
    /// policy (calls are order-independent and idempotent).
    fn error_policy<'py>(
        mut slf: PyRefMut<'py, Self>,
        policy: &PyScheduleErrorPolicy,
    ) -> PyRefMut<'py, Self> {
        slf.spec.error_policy = policy.inner;
        slf
    }

    /// Build the schedule.
    ///
    /// Under the default ``STRICT`` policy any build warnings raise
    /// ``ValueError``. Under ``MISSING_CALENDAR_WARNING`` or
    /// ``GRACEFUL_EMPTY`` the schedule is returned carrying its warnings
    /// (inspect via ``Schedule.warnings`` / ``Schedule.has_warnings()``).
    fn build(&self) -> PyResult<PySchedule> {
        let schedule = self.spec.build().map_err(core_to_py)?;
        let strict = self.spec.error_policy == ScheduleErrorPolicy::Strict;
        if strict && schedule.has_warnings() {
            let warnings = schedule
                .warnings
                .iter()
                .map(|w| w.to_string())
                .collect::<Vec<_>>()
                .join("; ");
            return Err(crate::errors::value_error(format!(
                "schedule build produced warnings; strict policy fails closed: {warnings}"
            )));
        }
        Ok(PySchedule::from_inner(schedule))
    }

    fn __repr__(&self) -> String {
        format!(
            "ScheduleBuilder(start={}, end={}, freq={})",
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
