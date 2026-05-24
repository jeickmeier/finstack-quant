//! Python bindings for schedule generation from [`finstack_core::dates`].

use crate::bindings::core::dates::calendar::PyBusinessDayConvention;
use crate::bindings::core::dates::tenor::extract_tenor;
use crate::bindings::core::dates::utils::{date_to_py, py_to_date};
use crate::errors::core_to_py;
use finstack_core::dates::{Schedule, ScheduleErrorPolicy, ScheduleSpec, StubKind};
use pyo3::prelude::*;
use pyo3::types::{PyModule, PyType};

/// Stub positioning rule for schedule generation.
#[pyclass(
    name = "StubKind",
    module = "finstack.core.dates",
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
    module = "finstack.core.dates",
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
    module = "finstack.core.dates",
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
/// Unlike the Rust [`finstack_core::dates::ScheduleBuilder`], the Python
/// binding mutates the builder **in place**: each setter method returns
/// ``None`` rather than ``self``. Call setters sequentially on the same
/// instance, then call ``build()``.
///
/// # Example
///
/// ```text
/// from datetime import date
/// from finstack.core.dates import (
///     ScheduleBuilder,
///     StubKind,
///     BusinessDayConvention,
///     ScheduleErrorPolicy,
/// )
///
/// builder = ScheduleBuilder(date(2025, 1, 15), date(2030, 1, 15))
/// builder.frequency("3M")
/// builder.stub_rule(StubKind.SHORT_FRONT)
/// builder.adjust_with(BusinessDayConvention.MODIFIED_FOLLOWING, "usny")
/// builder.end_of_month(False)
/// builder.error_policy(ScheduleErrorPolicy.STRICT)
/// schedule = builder.build()
/// assert len(schedule) >= 20  # ~quarterly periods over 5 years
/// ```
#[pyclass(
    name = "ScheduleBuilder",
    module = "finstack.core.dates",
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
                frequency: finstack_core::dates::Tenor::quarterly(),
                stub: StubKind::None,
                business_day_convention: None,
                calendar_id: None,
                end_of_month: false,
                imm_mode: false,
                cds_imm_mode: false,
                graceful: false,
                allow_missing_calendar: false,
            },
        })
    }

    /// Set the coupon/roll frequency (accepts ``Tenor`` or a string like ``"3M"``).
    fn frequency(&mut self, freq: &Bound<'_, PyAny>) -> PyResult<()> {
        self.spec.frequency = extract_tenor(freq)?;
        Ok(())
    }

    /// Set the stub rule.
    fn stub_rule(&mut self, stub: &PyStubKind) {
        self.spec.stub = stub.inner;
    }

    /// Set the business-day convention and calendar for adjustment.
    fn adjust_with(&mut self, convention: &PyBusinessDayConvention, calendar_id: &str) {
        self.spec.business_day_convention = Some(convention.inner);
        self.spec.calendar_id = Some(calendar_id.to_string());
    }

    /// Enable or disable end-of-month roll logic.
    fn end_of_month(&mut self, eom: bool) {
        self.spec.end_of_month = eom;
    }

    /// Enable CDS IMM date mode.
    fn cds_imm(&mut self) {
        self.spec.cds_imm_mode = true;
    }

    /// Enable IMM date mode.
    fn imm(&mut self) {
        self.spec.imm_mode = true;
    }

    /// Set the error policy.
    fn error_policy(&mut self, policy: &PyScheduleErrorPolicy) {
        match policy.inner {
            ScheduleErrorPolicy::Strict => {
                self.spec.graceful = false;
                self.spec.allow_missing_calendar = false;
            }
            ScheduleErrorPolicy::MissingCalendarWarning => {
                self.spec.allow_missing_calendar = true;
            }
            ScheduleErrorPolicy::GracefulEmpty => {
                self.spec.graceful = true;
            }
        }
    }

    /// Build the schedule.
    fn build(&self) -> PyResult<PySchedule> {
        let schedule = self.spec.build().map_err(core_to_py)?;
        if schedule.has_warnings() {
            let warnings = schedule
                .warnings
                .iter()
                .map(|w| w.to_string())
                .collect::<Vec<_>>()
                .join("; ");
            return Err(crate::errors::value_error(format!(
                "schedule build produced warnings; Python bindings fail closed: {warnings}"
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

/// Register schedule types on the `finstack.core.dates` module.
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
