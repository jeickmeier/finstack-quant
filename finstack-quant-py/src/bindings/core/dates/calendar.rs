//! Python bindings for holiday calendars and business-day adjustment.

use crate::bindings::date_utils::{date_to_py, py_to_date};
use crate::errors::core_to_py;
use finstack_quant_core::dates::{
    adjust, available_calendars, canonical_calendar_id, fx::resolve_calendar,
    BusinessDayConvention, CalendarMetadata, HolidayCalendar, WeekendRule,
};
use pyo3::prelude::*;
use pyo3::types::{PyModule, PyType};

/// Serde name of a [`WeekendRule`] (the stable snake_case wire form).
fn weekend_rule_str(rule: WeekendRule) -> PyResult<String> {
    finstack_quant_core::wire::serde_label(&rule).map_err(crate::errors::core_to_py)
}

/// Business-day adjustment convention (ISDA 2006 Definitions §4.12).
///
/// Immutable, hashable enum-style type. ``str()`` gives the snake_case wire
/// name (``"modified_following"``), which ``from_name`` parses back; the
/// parser also accepts the short codes ``MF``, ``F``, ``P``, ``MP`` and
/// ``NONE`` case-insensitively.
///
/// Examples
/// --------
/// >>> from finstack_quant.core.dates import BusinessDayConvention
/// >>> str(BusinessDayConvention.MODIFIED_FOLLOWING)
/// 'modified_following'
/// >>> BusinessDayConvention.from_name("MF") == BusinessDayConvention.MODIFIED_FOLLOWING
/// True
#[pyclass(
    name = "BusinessDayConvention",
    module = "finstack_quant.core.dates",
    frozen,
    eq,
    hash,
    skip_from_py_object
)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
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
    /// Closer business day; a tie rolls following.
    #[classattr]
    const NEAREST: PyBusinessDayConvention = PyBusinessDayConvention {
        inner: BusinessDayConvention::Nearest,
    };

    /// Parse from the snake_case name (``"modified_following"``) or a short
    /// code (``"MF"``, ``"F"``, ``"P"``, ``"MP"``, ``"NONE"``), case-insensitively.
    ///
    /// Raises ``ValueError`` listing the accepted names when ``name`` is unknown.
    #[classmethod]
    #[pyo3(text_signature = "(cls, name)")]
    fn from_name(_cls: &Bound<'_, PyType>, name: &str) -> PyResult<Self> {
        name.parse::<BusinessDayConvention>()
            .map(Self::from_inner)
            .map_err(crate::errors::value_error)
    }

    /// Support ``pickle`` by reconstructing through ``from_name``.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_name = py.get_type::<Self>().getattr("from_name")?;
        Ok((from_name, (self.inner.to_string(),)))
    }

    fn __repr__(&self) -> String {
        format!("BusinessDayConvention('{}')", self.inner)
    }

    fn __str__(&self) -> String {
        self.inner.to_string()
    }
}

/// Extract a [`BusinessDayConvention`] from a Python object (wrapper or string).
pub(crate) fn extract_business_day_convention(
    obj: &Bound<'_, PyAny>,
) -> PyResult<BusinessDayConvention> {
    if let Ok(business_day_convention) = obj.extract::<PyRef<'_, PyBusinessDayConvention>>() {
        return Ok(business_day_convention.inner);
    }
    if let Ok(s) = obj.extract::<String>() {
        return s
            .parse::<BusinessDayConvention>()
            .map_err(crate::errors::value_error);
    }
    Err(pyo3::exceptions::PyTypeError::new_err(
        "expected BusinessDayConvention or str",
    ))
}

/// Resolve a calendar from a ``HolidayCalendar`` wrapper or a registry id.
///
/// String ids may join several calendars with ``+`` (``"nyse+gblo"``), which
/// resolves to the union calendar. Unknown ids raise ``KeyError`` with the
/// core registry's "Did you mean …?" suggestions.
pub(crate) fn extract_calendar(obj: &Bound<'_, PyAny>) -> PyResult<&'static dyn HolidayCalendar> {
    if let Ok(cal) = obj.extract::<PyRef<'_, PyHolidayCalendar>>() {
        return Ok(cal.cal);
    }
    if let Ok(code) = obj.extract::<String>() {
        return resolve_calendar(Some(&code)).map_err(core_to_py);
    }
    Err(pyo3::exceptions::PyTypeError::new_err(
        "expected HolidayCalendar or str calendar code",
    ))
}

/// Metadata describing a registered holiday calendar.
///
/// Examples
/// --------
/// >>> from finstack_quant.core.dates import HolidayCalendar
/// >>> meta = HolidayCalendar("usny").metadata
/// >>> (meta.id, meta.weekend_rule)
/// ('usny', 'saturday_sunday')
#[pyclass(
    name = "CalendarMetadata",
    module = "finstack_quant.core.dates",
    frozen,
    eq,
    skip_from_py_object
)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PyCalendarMetadata {
    /// Inner Rust metadata.
    pub(crate) inner: CalendarMetadata,
}

impl PyCalendarMetadata {
    /// Build from a Rust [`CalendarMetadata`].
    pub(crate) const fn from_inner(inner: CalendarMetadata) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyCalendarMetadata {
    /// Calendar short code (registry id such as ``"usny"``).
    #[getter]
    fn id(&self) -> &'static str {
        self.inner.id
    }

    /// Human-readable name.
    #[getter]
    fn name(&self) -> &'static str {
        self.inner.name
    }

    /// Whether weekends are ignored for this calendar.
    #[getter]
    fn ignore_weekends(&self) -> bool {
        self.inner.ignore_weekends
    }

    /// Weekend convention used by this calendar as a snake_case string
    /// (e.g. ``"saturday_sunday"``, ``"friday_saturday"``, ``"friday_only"``, ``"none"``).
    #[getter]
    fn weekend_rule(&self) -> PyResult<String> {
        weekend_rule_str(self.inner.weekend_rule)
    }

    fn __repr__(&self) -> String {
        format!(
            "CalendarMetadata(id='{}', name='{}')",
            self.inner.id, self.inner.name
        )
    }
}

/// A holiday calendar resolved from the global registry.
///
/// The calendar is resolved once at construction and cached, so
/// ``is_business_day``/``is_holiday`` are direct lookups. Ids may join
/// several calendars with ``+`` (``"nyse+gblo"``): the result is a business
/// day only when every member is.
///
/// Parameters
/// ----------
/// code : str
///     Registered calendar id (``"usny"``, ``"target2"``, ``"nyse"``, …; see
///     ``available_calendars()``), or a ``+``-joined union such as
///     ``"nyse+gblo"``. Matching is ASCII case-insensitive.
///
/// Raises
/// ------
/// KeyError
///     If ``code`` (or any ``+`` member) is not a registered calendar; the
///     message carries "Did you mean …?" suggestions.
///
/// Examples
/// --------
/// >>> import datetime
/// >>> from finstack_quant.core.dates import HolidayCalendar
/// >>> calendar = HolidayCalendar("usny")
/// >>> (calendar.is_holiday(datetime.date(2025, 1, 1)), calendar.is_business_day(datetime.date(2025, 1, 6)))
/// (True, True)
#[pyclass(
    name = "HolidayCalendar",
    module = "finstack_quant.core.dates",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyHolidayCalendar {
    /// Resolved registry calendar (built-in or interned union).
    cal: &'static dyn HolidayCalendar,
    /// Canonical id: the registry id for built-ins, the normalized
    /// ``a+b`` form for unions.
    code: String,
}

impl std::fmt::Debug for PyHolidayCalendar {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PyHolidayCalendar")
            .field("code", &self.code)
            .finish()
    }
}

impl PyHolidayCalendar {
    /// Canonical registry (or normalized `a+b`) id.
    pub(crate) fn canonical_code(&self) -> &str {
        &self.code
    }
}

#[pymethods]
impl PyHolidayCalendar {
    /// Resolve a calendar by its registry id (e.g. ``"target2"``, ``"nyse"``,
    /// or a union such as ``"nyse+gblo"``).
    #[new]
    #[pyo3(text_signature = "(code)")]
    fn new(code: &str) -> PyResult<Self> {
        let cal = resolve_calendar(Some(code)).map_err(core_to_py)?;
        let code = canonical_calendar_id(code).map_err(core_to_py)?;
        Ok(Self { cal, code })
    }

    /// Whether ``date`` is a holiday (weekends follow the calendar's weekend rule).
    ///
    /// Raises ``TypeError`` for a non-date-like argument and ``ValueError``
    /// for an invalid calendar date or ISO string.
    #[pyo3(text_signature = "(self, date)")]
    fn is_holiday(&self, date: &Bound<'_, PyAny>) -> PyResult<bool> {
        Ok(self.cal.is_holiday(py_to_date(date)?))
    }

    /// Whether ``date`` is a business day (neither a weekend nor a holiday).
    ///
    /// Raises ``TypeError`` for a non-date-like argument and ``ValueError``
    /// for an invalid calendar date or ISO string.
    #[pyo3(text_signature = "(self, date)")]
    fn is_business_day(&self, date: &Bound<'_, PyAny>) -> PyResult<bool> {
        Ok(self.cal.is_business_day(py_to_date(date)?))
    }

    /// Count business days in ``[start, end)``.
    ///
    /// Parameters
    /// ----------
    /// start : datetime.date | str
    ///     First date included in the count.
    /// end : datetime.date | str
    ///     Exclusive boundary; ``end <= start`` gives ``0``.
    ///
    /// Returns
    /// -------
    /// int
    ///     Number of business days from ``start`` up to but excluding ``end``.
    ///
    /// Raises
    /// ------
    /// TypeError
    ///     If either argument is not date-like.
    /// ValueError
    ///     If either argument is not a valid calendar date or ISO string.
    #[pyo3(text_signature = "(self, start, end)")]
    fn count_business_days(
        &self,
        start: &Bound<'_, PyAny>,
        end: &Bound<'_, PyAny>,
    ) -> PyResult<i32> {
        Ok(self
            .cal
            .count_business_days(py_to_date(start)?, py_to_date(end)?))
    }

    /// Calendar metadata; ``None`` for ``+``-joined union calendars.
    #[getter]
    fn metadata(&self) -> Option<PyCalendarMetadata> {
        self.cal.metadata().map(PyCalendarMetadata::from_inner)
    }

    /// Canonical registry id (``"usny"``, ``"target2"``, ``"weekends_only"``),
    /// or the normalized ``a+b`` form for union calendars.
    #[getter]
    fn code(&self) -> &str {
        &self.code
    }

    /// Support ``pickle`` by reconstructing through ``HolidayCalendar(code)``.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        Ok((py.get_type::<Self>().into_any(), (self.code.clone(),)))
    }

    fn __eq__(&self, other: &Bound<'_, PyAny>) -> bool {
        other
            .extract::<PyRef<'_, Self>>()
            .map(|o| o.code == self.code)
            .unwrap_or(false)
    }

    fn __hash__(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.code.hash(&mut hasher);
        hasher.finish()
    }

    fn __repr__(&self) -> String {
        format!("HolidayCalendar('{}')", self.code)
    }

    fn __str__(&self) -> String {
        self.code.clone()
    }
}

/// Adjust a date according to a business-day convention and calendar.
///
/// Parameters
/// ----------
/// date : datetime.date | str
///     Date to adjust (``datetime.date``, ``pandas.Timestamp`` or ISO
///     ``YYYY-MM-DD`` string).
/// convention : BusinessDayConvention | str
///     Roll rule: a ``BusinessDayConvention`` or its name
///     (``"modified_following"``, short codes ``MF``/``F``/``P``/``MP``/``NONE``).
/// calendar : HolidayCalendar | str
///     Holiday calendar object or registry id (``"usny"``; ``"nyse+gblo"``
///     joins calendars).
///
/// Returns
/// -------
/// datetime.date
///     The adjusted date (unchanged when already a business day or under
///     ``UNADJUSTED``).
///
/// Raises
/// ------
/// KeyError
///     If ``calendar`` names an unknown calendar.
/// ValueError
///     If ``convention`` is unknown, ``date`` is invalid, or no business day
///     exists within 100 days.
/// TypeError
///     If an argument has an unsupported type.
///
/// Examples
/// --------
/// >>> import datetime
/// >>> from finstack_quant.core.dates import adjust
/// >>> adjust(datetime.date(2025, 1, 4), "following", "usny")
/// datetime.date(2025, 1, 6)
#[pyfunction]
#[pyo3(name = "adjust", text_signature = "(date, convention, calendar)")]
fn py_adjust<'py>(
    py: Python<'py>,
    date: &Bound<'py, PyAny>,
    convention: &Bound<'py, PyAny>,
    calendar: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let d = py_to_date(date)?;
    let business_day_convention = extract_business_day_convention(convention)?;
    let cal_ref = extract_calendar(calendar)?;
    let adjusted = adjust(d, business_day_convention, cal_ref).map_err(core_to_py)?;
    date_to_py(py, adjusted)
}

/// Return the list of available calendar codes in the global registry.
#[pyfunction]
#[pyo3(name = "available_calendars")]
fn py_available_calendars() -> Vec<String> {
    available_calendars()
        .iter()
        .map(|s| s.to_string())
        .collect()
}

/// Register calendar types on the `finstack_quant.core.dates` module.
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
