//! Python bindings for holiday calendars and business-day adjustment.

use crate::bindings::core::dates::utils::{date_to_py, py_to_date};
use crate::errors::core_to_py;
use finstack_core::dates::{
    adjust, BusinessDayConvention, CalendarMetadata, CalendarRegistry, HolidayCalendar, WeekendRule,
};
use pyo3::prelude::*;
use pyo3::types::{PyModule, PyType};

/// Map [`WeekendRule`] to its stable snake_case serde name.
const fn weekend_rule_str(rule: WeekendRule) -> &'static str {
    match rule {
        WeekendRule::SaturdaySunday => "saturday_sunday",
        WeekendRule::FridaySaturday => "friday_saturday",
        WeekendRule::FridayOnly => "friday_only",
        WeekendRule::None => "none",
    }
}

/// Business-day adjustment convention.
#[pyclass(
    name = "BusinessDayConvention",
    module = "finstack.core.dates",
    frozen,
    eq,
    skip_from_py_object
)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PyBusinessDayConvention {
    /// Inner convention variant.
    pub(crate) inner: BusinessDayConvention,
}

impl PyBusinessDayConvention {
    /// Build from an existing Rust [`BusinessDayConvention`].
    pub(crate) const fn from_inner(inner: BusinessDayConvention) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyBusinessDayConvention {
    /// No adjustment — use the date as given.
    #[classattr]
    const UNADJUSTED: PyBusinessDayConvention = PyBusinessDayConvention {
        inner: BusinessDayConvention::Unadjusted,
    };
    /// Roll forward to the next business day.
    #[classattr]
    const FOLLOWING: PyBusinessDayConvention = PyBusinessDayConvention {
        inner: BusinessDayConvention::Following,
    };
    /// Roll forward unless it crosses a month boundary, then roll backward.
    #[classattr]
    const MODIFIED_FOLLOWING: PyBusinessDayConvention = PyBusinessDayConvention {
        inner: BusinessDayConvention::ModifiedFollowing,
    };
    /// Roll backward to the previous business day.
    #[classattr]
    const PRECEDING: PyBusinessDayConvention = PyBusinessDayConvention {
        inner: BusinessDayConvention::Preceding,
    };
    /// Roll backward unless it crosses a month boundary, then roll forward.
    #[classattr]
    const MODIFIED_PRECEDING: PyBusinessDayConvention = PyBusinessDayConvention {
        inner: BusinessDayConvention::ModifiedPreceding,
    };

    /// Parse from a string (e.g. ``"following"``, ``"modified_following"``).
    #[classmethod]
    #[pyo3(text_signature = "(cls, name)")]
    fn from_name(_cls: &Bound<'_, PyType>, name: &str) -> PyResult<Self> {
        name.parse::<BusinessDayConvention>()
            .map(Self::from_inner)
            .map_err(crate::errors::value_error)
    }

    /// Hash based on discriminant.
    fn __hash__(&self) -> isize {
        match self.inner {
            BusinessDayConvention::Unadjusted => 0,
            BusinessDayConvention::Following => 1,
            BusinessDayConvention::ModifiedFollowing => 2,
            BusinessDayConvention::Preceding => 3,
            BusinessDayConvention::ModifiedPreceding => 4,
            #[allow(unreachable_patterns)]
            _ => 255,
        }
    }

    fn __repr__(&self) -> String {
        format!("BusinessDayConvention('{}')", self.inner)
    }

    fn __str__(&self) -> String {
        self.inner.to_string()
    }
}

/// Extract a [`BusinessDayConvention`] from a Python object (wrapper or string).
pub(crate) fn extract_bdc(obj: &Bound<'_, PyAny>) -> PyResult<BusinessDayConvention> {
    if let Ok(bdc) = obj.extract::<PyRef<'_, PyBusinessDayConvention>>() {
        return Ok(bdc.inner);
    }
    if let Ok(s) = obj.extract::<String>() {
        return s
            .parse::<BusinessDayConvention>()
            .map_err(crate::errors::value_error);
    }
    Err(crate::errors::value_error(
        "expected BusinessDayConvention or str",
    ))
}

/// Metadata for a holiday calendar.
#[pyclass(
    name = "CalendarMetadata",
    module = "finstack.core.dates",
    frozen,
    skip_from_py_object
)]
#[derive(Clone, Debug)]
pub struct PyCalendarMetadata {
    /// Calendar short code.
    id: String,
    /// Human-readable name.
    name: String,
    /// Whether weekends are ignored (all days are potentially business days).
    ignore_weekends: bool,
    /// Weekend convention used by this calendar.
    weekend_rule: String,
}

impl PyCalendarMetadata {
    /// Build from a Rust [`CalendarMetadata`].
    fn from_rust(m: CalendarMetadata) -> Self {
        Self {
            id: m.id.to_string(),
            name: m.name.to_string(),
            ignore_weekends: m.ignore_weekends,
            weekend_rule: weekend_rule_str(m.weekend_rule).to_string(),
        }
    }
}

#[pymethods]
impl PyCalendarMetadata {
    /// Calendar short code.
    #[getter]
    fn id(&self) -> &str {
        &self.id
    }

    /// Human-readable name.
    #[getter]
    fn name(&self) -> &str {
        &self.name
    }

    /// Whether weekends are ignored for this calendar.
    #[getter]
    fn ignore_weekends(&self) -> bool {
        self.ignore_weekends
    }

    /// Weekend convention used by this calendar as a snake_case string
    /// (e.g. ``"saturday_sunday"``, ``"friday_saturday"``, ``"friday_only"``, ``"none"``).
    #[getter]
    fn weekend_rule(&self) -> &str {
        &self.weekend_rule
    }

    fn __repr__(&self) -> String {
        format!("CalendarMetadata(id='{}', name='{}')", self.id, self.name)
    }
}

/// A holiday calendar resolved from the global registry.
#[pyclass(
    name = "HolidayCalendar",
    module = "finstack.core.dates",
    frozen,
    skip_from_py_object
)]
#[derive(Clone, Debug)]
pub struct PyHolidayCalendar {
    /// Calendar code used for registry resolution.
    code: String,
}

#[pymethods]
impl PyHolidayCalendar {
    /// Resolve a calendar by its code (e.g. ``"target2"``, ``"nyse"``).
    #[new]
    #[pyo3(text_signature = "(code)")]
    fn new(code: &str) -> PyResult<Self> {
        let registry = CalendarRegistry::global();
        if registry.resolve_str(code).is_none() {
            return Err(crate::errors::value_error(format!(
                "unknown calendar code: {code:?}"
            )));
        }
        Ok(Self {
            code: code.to_string(),
        })
    }

    /// Check whether a date is a holiday.
    #[pyo3(text_signature = "(self, date)")]
    fn is_holiday(&self, date: &Bound<'_, PyAny>) -> PyResult<bool> {
        let d = py_to_date(date)?;
        let cal = self.resolve()?;
        Ok(cal.is_holiday(d))
    }

    /// Check whether a date is a business day.
    #[pyo3(text_signature = "(self, date)")]
    fn is_business_day(&self, date: &Bound<'_, PyAny>) -> PyResult<bool> {
        let d = py_to_date(date)?;
        let cal = self.resolve()?;
        Ok(cal.is_business_day(d))
    }

    /// Calendar metadata (if available).
    #[getter]
    fn metadata(&self) -> PyResult<Option<PyCalendarMetadata>> {
        let cal = self.resolve()?;
        Ok(cal.metadata().map(PyCalendarMetadata::from_rust))
    }

    /// Calendar code.
    #[getter]
    fn code(&self) -> &str {
        &self.code
    }

    fn __repr__(&self) -> String {
        format!("HolidayCalendar('{}')", self.code)
    }

    fn __str__(&self) -> String {
        self.code.clone()
    }
}

impl PyHolidayCalendar {
    /// Resolve the inner calendar from the global registry.
    fn resolve(&self) -> PyResult<&'static dyn HolidayCalendar> {
        CalendarRegistry::global()
            .resolve_str(&self.code)
            .ok_or_else(|| {
                crate::errors::value_error(format!("calendar not found: {:?}", self.code))
            })
    }
}

/// Adjust a date according to a business-day convention and calendar.
///
/// Arguments:
///   - ``date``: a ``datetime.date``
///   - ``convention``: a ``BusinessDayConvention`` or string
///   - ``calendar``: a ``HolidayCalendar`` or calendar code string
#[pyfunction]
#[pyo3(name = "adjust", text_signature = "(date, convention, calendar)")]
fn py_adjust<'py>(
    py: Python<'py>,
    date: &Bound<'py, PyAny>,
    convention: &Bound<'py, PyAny>,
    calendar: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let d = py_to_date(date)?;
    let bdc = extract_bdc(convention)?;

    let cal_ref: &dyn HolidayCalendar =
        if let Ok(cal) = calendar.extract::<PyRef<'_, PyHolidayCalendar>>() {
            cal.resolve()?
        } else if let Ok(code) = calendar.extract::<String>() {
            CalendarRegistry::global()
                .resolve_str(&code)
                .ok_or_else(|| crate::errors::value_error(format!("unknown calendar: {code:?}")))?
        } else {
            return Err(crate::errors::value_error(
                "expected HolidayCalendar or str calendar code",
            ));
        };

    let adjusted = adjust(d, bdc, cal_ref).map_err(core_to_py)?;
    date_to_py(py, adjusted)
}

/// Return the list of available calendar codes in the global registry.
#[pyfunction]
#[pyo3(name = "available_calendars")]
fn py_available_calendars() -> Vec<String> {
    CalendarRegistry::global()
        .available_ids()
        .iter()
        .map(|s| s.to_string())
        .collect()
}

/// Register calendar types on the `finstack.core.dates` module.
pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyBusinessDayConvention>()?;
    m.add_class::<PyCalendarMetadata>()?;
    m.add_class::<PyHolidayCalendar>()?;
    m.add_function(wrap_pyfunction!(py_adjust, m)?)?;
    m.add_function(wrap_pyfunction!(py_available_calendars, m)?)?;
    Ok(())
}

/// Names exported from this submodule.
pub const EXPORTS: &[&str] = &[
    "BusinessDayConvention",
    "CalendarMetadata",
    "HolidayCalendar",
    "adjust",
    "available_calendars",
];
