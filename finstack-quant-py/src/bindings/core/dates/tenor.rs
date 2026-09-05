//! Python bindings for [`finstack_quant_core::dates::Tenor`] and [`finstack_quant_core::dates::TenorUnit`].

use crate::bindings::core::dates::calendar::{extract_business_day_convention, extract_calendar};
use crate::bindings::date_utils::{date_to_py, py_to_date};
use crate::errors::core_to_py;
use finstack_quant_core::dates::{BusinessDayConvention, HolidayCalendar, Tenor, TenorUnit};
use pyo3::prelude::*;
use pyo3::types::{PyModule, PyType};

/// Frequency/tenor unit (Days, Weeks, Months, Years).
///
/// Immutable, hashable enum-style type. ``str()`` gives the single-letter
/// designator (``"D"``, ``"W"``, ``"M"``, ``"Y"``) accepted by
/// ``TenorUnit.from_char`` and ``Tenor``.
#[pyclass(
    name = "TenorUnit",
    module = "finstack_quant.core.dates",
    frozen,
    eq,
    hash,
    skip_from_py_object
)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PyTenorUnit {
    /// Inner unit variant.
    pub(crate) inner: TenorUnit,
}

impl PyTenorUnit {
    /// Build from an existing Rust [`TenorUnit`].
    pub(crate) const fn from_inner(inner: TenorUnit) -> Self {
        Self { inner }
    }

    const fn designator(self) -> char {
        match self.inner {
            TenorUnit::Days => 'D',
            TenorUnit::Weeks => 'W',
            TenorUnit::Months => 'M',
            TenorUnit::Years => 'Y',
        }
    }
}

#[pymethods]
impl PyTenorUnit {
    /// Day unit.
    #[classattr]
    const DAYS: PyTenorUnit = PyTenorUnit {
        inner: TenorUnit::Days,
    };
    /// Week unit.
    #[classattr]
    const WEEKS: PyTenorUnit = PyTenorUnit {
        inner: TenorUnit::Weeks,
    };
    /// Month unit.
    #[classattr]
    const MONTHS: PyTenorUnit = PyTenorUnit {
        inner: TenorUnit::Months,
    };
    /// Year unit.
    #[classattr]
    const YEARS: PyTenorUnit = PyTenorUnit {
        inner: TenorUnit::Years,
    };

    /// Parse a single-character tenor unit designator (``D``, ``W``, ``M``, ``Y``),
    /// case-insensitively.
    ///
    /// Raises ``ValueError`` when ``ch`` is not exactly one of those letters.
    #[classmethod]
    #[pyo3(text_signature = "(cls, ch)")]
    fn from_char(_cls: &Bound<'_, PyType>, ch: &str) -> PyResult<Self> {
        let mut chars = ch.chars();
        match (chars.next(), chars.next()) {
            (Some(c), None) => TenorUnit::from_char(c)
                .map(Self::from_inner)
                .map_err(core_to_py),
            _ => Err(crate::errors::value_error(format!(
                "expected a single unit character D, W, M or Y, got {ch:?}"
            ))),
        }
    }

    /// Support ``pickle`` by reconstructing through ``TenorUnit.from_char``.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_char = py.get_type::<Self>().getattr("from_char")?;
        Ok((from_char, (self.designator().to_string(),)))
    }

    fn __repr__(&self) -> String {
        let label = match self.inner {
            TenorUnit::Days => "DAYS",
            TenorUnit::Weeks => "WEEKS",
            TenorUnit::Months => "MONTHS",
            TenorUnit::Years => "YEARS",
        };
        format!("TenorUnit.{label}")
    }

    fn __str__(&self) -> String {
        self.designator().to_string()
    }
}

/// Extract a [`TenorUnit`] from a ``TenorUnit`` wrapper or a one-letter string.
fn extract_tenor_unit(obj: &Bound<'_, PyAny>) -> PyResult<TenorUnit> {
    if let Ok(unit) = obj.extract::<PyRef<'_, PyTenorUnit>>() {
        return Ok(unit.inner);
    }
    if let Ok(s) = obj.extract::<String>() {
        return PyTenorUnit::from_char(&obj.py().get_type::<PyTenorUnit>(), &s).map(|u| u.inner);
    }
    Err(pyo3::exceptions::PyTypeError::new_err(
        "expected TenorUnit or a one-letter unit string (D, W, M, Y)",
    ))
}

/// A tenor such as ``3M``, ``1Y``, or ``2W``.
///
/// Immutable, hashable value type combining a positive count and a
/// ``TenorUnit``. The constructor accepts either a tenor string
/// (``Tenor("3M")``, money-market aliases ``"ON"``/``"TN"``/``"SN"`` map to
/// ``1D``) or a count plus unit (``Tenor(3, "M")``,
/// ``Tenor(3, TenorUnit.MONTHS)``).
///
/// Parameters
/// ----------
/// value : str | int
///     Tenor string such as ``"3M"`` (``unit`` must then be omitted), or the
///     positive integer count when ``unit`` is given.
/// unit : TenorUnit | str | None
///     Calendar unit for an integer ``value``; ``TenorUnit`` or a one-letter
///     designator ``"D"``/``"W"``/``"M"``/``"Y"``.
///
/// Raises
/// ------
/// ValueError
///     If the string does not parse, the count is zero, or the count exceeds
///     the supported range for its unit (200 years).
/// TypeError
///     If ``value`` is neither ``str`` nor ``int``, or ``unit`` is supplied
///     with a string ``value``.
///
/// Examples
/// --------
/// >>> from finstack_quant.core.dates import Tenor, TenorUnit
/// >>> Tenor("3M") == Tenor(3, "M") == Tenor(3, TenorUnit.MONTHS)
/// True
#[pyclass(
    name = "Tenor",
    module = "finstack_quant.core.dates",
    frozen,
    eq,
    hash,
    skip_from_py_object
)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PyTenor {
    /// Inner Rust tenor.
    pub(crate) inner: Tenor,
}

impl PyTenor {
    /// Build from an existing Rust [`Tenor`].
    pub(crate) const fn from_inner(inner: Tenor) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyTenor {
    /// Construct a tenor from a string (``"3M"``) or a count and unit
    /// (``Tenor(3, "M")`` / ``Tenor(3, TenorUnit.MONTHS)``).
    #[new]
    #[pyo3(signature = (value, unit=None), text_signature = "(value, unit=None)")]
    fn new(value: &Bound<'_, PyAny>, unit: Option<&Bound<'_, PyAny>>) -> PyResult<Self> {
        if let Ok(s) = value.extract::<String>() {
            if unit.is_some() {
                return Err(pyo3::exceptions::PyTypeError::new_err(
                    "Tenor(value: str) takes no unit; pass Tenor(count: int, unit) instead",
                ));
            }
            return Tenor::parse(&s).map(Self::from_inner).map_err(core_to_py);
        }
        let count: u32 = value.extract().map_err(|_| {
            pyo3::exceptions::PyTypeError::new_err(format!(
                "expected a tenor string like '3M' or a positive integer count, got {}",
                value
                    .get_type()
                    .name()
                    .map(|n| n.to_string())
                    .unwrap_or_default()
            ))
        })?;
        let unit = unit.ok_or_else(|| {
            pyo3::exceptions::PyTypeError::new_err(
                "Tenor(count: int) requires a unit (TenorUnit or 'D'/'W'/'M'/'Y')",
            )
        })?;
        Tenor::new(count, extract_tenor_unit(unit)?)
            .map(Self::from_inner)
            .map_err(core_to_py)
    }

    /// Parse a tenor string (e.g. ``"3M"``, ``"1Y"``, ``"2W"``; ``"ON"``/``"TN"``/``"SN"`` give ``1D``).
    #[classmethod]
    #[pyo3(text_signature = "(cls, s)")]
    fn parse(_cls: &Bound<'_, PyType>, s: &str) -> PyResult<Self> {
        Tenor::parse(s).map(Self::from_inner).map_err(core_to_py)
    }

    /// 1-day tenor.
    #[classmethod]
    fn daily(_cls: &Bound<'_, PyType>) -> Self {
        Self::from_inner(Tenor::daily())
    }

    /// 1-week tenor.
    #[classmethod]
    fn weekly(_cls: &Bound<'_, PyType>) -> Self {
        Self::from_inner(Tenor::weekly())
    }

    /// 2-week tenor.
    #[classmethod]
    fn biweekly(_cls: &Bound<'_, PyType>) -> Self {
        Self::from_inner(Tenor::biweekly())
    }

    /// 1-month tenor.
    #[classmethod]
    fn monthly(_cls: &Bound<'_, PyType>) -> Self {
        Self::from_inner(Tenor::monthly())
    }

    /// 2-month tenor.
    #[classmethod]
    fn bimonthly(_cls: &Bound<'_, PyType>) -> Self {
        Self::from_inner(Tenor::bimonthly())
    }

    /// 3-month (quarterly) tenor.
    #[classmethod]
    fn quarterly(_cls: &Bound<'_, PyType>) -> Self {
        Self::from_inner(Tenor::quarterly())
    }

    /// 6-month (semi-annual) tenor.
    #[classmethod]
    fn semi_annual(_cls: &Bound<'_, PyType>) -> Self {
        Self::from_inner(Tenor::semi_annual())
    }

    /// 12-month (annual) tenor.
    #[classmethod]
    fn annual(_cls: &Bound<'_, PyType>) -> Self {
        Self::from_inner(Tenor::annual())
    }

    /// Construct from the number of coupon payments per year.
    #[classmethod]
    #[pyo3(text_signature = "(cls, payments)")]
    fn from_payments_per_year(_cls: &Bound<'_, PyType>, payments: u32) -> PyResult<Self> {
        Tenor::from_payments_per_year(payments)
            .map(Self::from_inner)
            .map_err(core_to_py)
    }

    /// Construct from a year fraction using a day-count convention.
    ///
    /// A year fraction that is (within a small epsilon) a whole number of
    /// months gives a month-based tenor; anything else is converted to days
    /// under ``day_count``.
    ///
    /// Parameters
    /// ----------
    /// years : float
    ///     Positive, finite length in years (e.g. ``0.5`` gives ``6M``).
    /// day_count : DayCount | str
    ///     Convention used for the day conversion (``DayCount`` or a
    ///     canonical name such as ``"act_365f"``).
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``years`` is non-positive, non-finite or exceeds 200 years, or
    ///     ``day_count`` is not a recognized convention.
    #[classmethod]
    #[pyo3(text_signature = "(cls, years, day_count)")]
    fn from_years(
        _cls: &Bound<'_, PyType>,
        years: f64,
        day_count: &Bound<'_, PyAny>,
    ) -> PyResult<Self> {
        let day_count = crate::bindings::core::dates::daycount::extract_day_count(day_count)?;
        Tenor::from_years(years, day_count)
            .map(Self::from_inner)
            .map_err(core_to_py)
    }

    /// Positive integer multiplying this tenor's calendar unit.
    #[getter]
    fn count(&self) -> u32 {
        self.inner.count()
    }

    /// Unit of the tenor.
    #[getter]
    fn unit(&self) -> PyTenorUnit {
        PyTenorUnit::from_inner(self.inner.unit())
    }

    /// Equivalent whole months (``None`` for day/week tenors).
    #[getter]
    fn months(&self) -> Option<u32> {
        self.inner.months()
    }

    /// Equivalent whole days (``None`` for month/year tenors).
    #[getter]
    fn days(&self) -> Option<u32> {
        self.inner.days()
    }

    /// Approximate tenor length in years (1D = 1/365, 1W = 7/365, 1M = 1/12, no calendar).
    #[allow(clippy::wrong_self_convention)]
    fn to_years(&self) -> f64 {
        self.inner.to_years()
    }

    /// Coupon payments per year implied by this tenor (``3M`` gives ``4.0``, ``2Y`` gives ``0.5``).
    fn payments_per_year(&self) -> f64 {
        self.inner.payments_per_year()
    }

    /// Approximate tenor length in calendar days.
    #[allow(clippy::wrong_self_convention)]
    fn to_days_approx(&self) -> i64 {
        self.inner.to_days_approx()
    }

    /// Add this tenor to a date with optional business-day adjustment.
    ///
    /// Month and year tenors clamp to the last valid day of the target month
    /// (Jan 31 + 1M gives Feb 28/29).
    ///
    /// Parameters
    /// ----------
    /// date : datetime.date | str
    ///     Anchor date (``datetime.date``, ``pandas.Timestamp`` or ISO
    ///     ``YYYY-MM-DD`` string).
    /// calendar : HolidayCalendar | str | None
    ///     Holiday calendar (object or registered id such as ``"usny"``;
    ///     ``"nyse+gblo"`` joins calendars). ``None`` skips adjustment.
    /// business_day_convention : BusinessDayConvention | str
    ///     Roll rule applied when ``calendar`` is given (default
    ///     ``"modified_following"``; short codes ``MF``/``F``/``P`` accepted).
    ///
    /// Returns
    /// -------
    /// datetime.date
    ///     The (optionally adjusted) end date.
    ///
    /// Raises
    /// ------
    /// KeyError
    ///     If ``calendar`` names an unknown calendar.
    /// ValueError
    ///     If the convention string is unknown, the date is invalid, or no
    ///     business day is found within 100 days.
    #[pyo3(
        signature = (date, calendar=None, business_day_convention=None),
        text_signature = "(self, date, calendar=None, business_day_convention='modified_following')"
    )]
    fn add_to_date<'py>(
        &self,
        py: Python<'py>,
        date: &Bound<'py, PyAny>,
        calendar: Option<&Bound<'py, PyAny>>,
        business_day_convention: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let d = py_to_date(date)?;
        let cal: Option<&'static dyn HolidayCalendar> =
            calendar.map(extract_calendar).transpose()?;
        let conv = match business_day_convention {
            Some(c) => extract_business_day_convention(c)?,
            None => BusinessDayConvention::ModifiedFollowing,
        };
        let end = self.inner.add_to_date(d, cal, conv).map_err(core_to_py)?;
        date_to_py(py, end)
    }

    /// Exact year fraction of this tenor from ``as_of`` under a day count.
    ///
    /// Adds the tenor to ``as_of`` (see ``add_to_date``) and measures the
    /// result with ``day_count``, so calendars and roll conventions are
    /// honoured, unlike the fixed approximation in ``to_years()``.
    ///
    /// Parameters
    /// ----------
    /// as_of : datetime.date | str
    ///     Start date of the measurement.
    /// day_count : DayCount | str
    ///     Convention used to measure the span (required keyword).
    /// calendar : HolidayCalendar | str | None
    ///     Holiday calendar for the end-date roll; ``None`` skips adjustment.
    /// business_day_convention : BusinessDayConvention | str
    ///     Roll rule for the end date (default ``"modified_following"``).
    ///
    /// Returns
    /// -------
    /// float
    ///     Year fraction between ``as_of`` and the rolled end date.
    ///
    /// Raises
    /// ------
    /// KeyError
    ///     If ``calendar`` names an unknown calendar.
    /// ValueError
    ///     If ``day_count`` or the convention is unrecognized, or the
    ///     day-count needs context (e.g. ``BUS_252`` without a calendar).
    #[allow(clippy::wrong_self_convention)]
    #[pyo3(
        signature = (as_of, *, day_count, calendar=None, business_day_convention=None),
        text_signature = "(self, as_of, *, day_count, calendar=None, business_day_convention='modified_following')"
    )]
    fn to_years_with_context(
        &self,
        as_of: &Bound<'_, PyAny>,
        day_count: &Bound<'_, PyAny>,
        calendar: Option<&Bound<'_, PyAny>>,
        business_day_convention: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<f64> {
        let d = py_to_date(as_of)?;
        let cal: Option<&'static dyn HolidayCalendar> =
            calendar.map(extract_calendar).transpose()?;
        let conv = match business_day_convention {
            Some(c) => extract_business_day_convention(c)?,
            None => BusinessDayConvention::ModifiedFollowing,
        };
        let day_count = crate::bindings::core::dates::daycount::extract_day_count(day_count)?;
        self.inner
            .to_years_with_context(d, cal, conv, day_count)
            .map_err(core_to_py)
    }

    /// Support ``pickle`` by reconstructing through ``Tenor("<count><unit>")``.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        Ok((py.get_type::<Self>().into_any(), (self.inner.to_string(),)))
    }

    fn __repr__(&self) -> String {
        format!("Tenor('{}')", self.inner)
    }

    fn __str__(&self) -> String {
        self.inner.to_string()
    }
}

/// Extract a [`Tenor`] from a [`PyTenor`] or a string.
pub(crate) fn extract_tenor(obj: &Bound<'_, PyAny>) -> PyResult<Tenor> {
    if let Ok(t) = obj.extract::<PyRef<'_, PyTenor>>() {
        return Ok(t.inner);
    }
    if let Ok(s) = obj.extract::<String>() {
        return Tenor::parse(&s).map_err(core_to_py);
    }
    Err(pyo3::exceptions::PyTypeError::new_err(
        "expected Tenor or str",
    ))
}

/// Register tenor types on the `finstack_quant.core.dates` module.
pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyTenorUnit>()?;
    m.add_class::<PyTenor>()?;
    Ok(())
}

/// Names exported from this submodule.
pub const EXPORTS: &[&str] = &["TenorUnit", "Tenor"];
