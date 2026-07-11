//! Python bindings for period types from [`finstack_quant_core::dates`].

use crate::bindings::core::dates::utils::date_to_py;
use crate::errors::core_to_py;
use finstack_quant_core::dates::{
    build_fiscal_periods, build_periods, FiscalConfig, Period, PeriodId, PeriodKind, PeriodPlan,
};
use pyo3::prelude::*;
use pyo3::types::{PyModule, PyType};

/// Period frequency kind (Daily, Monthly, Quarterly, etc.).
#[pyclass(
    name = "PeriodKind",
    module = "finstack_quant.core.dates",
    frozen,
    eq,
    hash,
    skip_from_py_object
)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PyPeriodKind {
    /// Inner period kind variant.
    pub(crate) inner: PeriodKind,
}

impl PyPeriodKind {
    /// Build from an existing Rust [`PeriodKind`].
    pub(crate) const fn from_inner(inner: PeriodKind) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyPeriodKind {
    /// Daily periods (252 trading days per year).
    #[classattr]
    const DAILY: PyPeriodKind = PyPeriodKind {
        inner: PeriodKind::Daily,
    };
    /// Weekly periods.
    #[classattr]
    const WEEKLY: PyPeriodKind = PyPeriodKind {
        inner: PeriodKind::Weekly,
    };
    /// Monthly periods.
    #[classattr]
    const MONTHLY: PyPeriodKind = PyPeriodKind {
        inner: PeriodKind::Monthly,
    };
    /// Quarterly periods.
    #[classattr]
    const QUARTERLY: PyPeriodKind = PyPeriodKind {
        inner: PeriodKind::Quarterly,
    };
    /// Semi-annual periods.
    #[classattr]
    const SEMI_ANNUAL: PyPeriodKind = PyPeriodKind {
        inner: PeriodKind::SemiAnnual,
    };
    /// Annual periods.
    #[classattr]
    const ANNUAL: PyPeriodKind = PyPeriodKind {
        inner: PeriodKind::Annual,
    };

    /// Parse a period kind from a string (e.g. ``"quarterly"``, ``"m"``).
    #[classmethod]
    #[pyo3(text_signature = "(cls, name)")]
    fn from_name(_cls: &Bound<'_, PyType>, name: &str) -> PyResult<Self> {
        name.parse::<PeriodKind>()
            .map(Self::from_inner)
            .map_err(core_to_py)
    }

    /// Number of periods per year for this frequency.
    #[getter]
    fn periods_per_year(&self) -> u16 {
        self.inner.periods_per_year()
    }

    /// Annualization factor for this frequency.
    #[getter]
    fn annualization_factor(&self) -> f64 {
        self.inner.annualization_factor()
    }

    fn __repr__(&self) -> String {
        format!("PeriodKind('{}')", self.inner)
    }

    fn __str__(&self) -> String {
        self.inner.to_string()
    }
}

/// A period identifier such as ``2025Q1`` or ``2025M03``.
#[pyclass(
    name = "PeriodId",
    module = "finstack_quant.core.dates",
    frozen,
    eq,
    hash,
    skip_from_py_object
)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PyPeriodId {
    /// Inner Rust period identifier.
    pub(crate) inner: PeriodId,
}

impl PyPeriodId {
    /// Build from an existing Rust [`PeriodId`].
    pub(crate) const fn from_inner(inner: PeriodId) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyPeriodId {
    /// Parse a period code string (e.g. ``"2025Q1"``, ``"2025M06"``).
    #[classmethod]
    #[pyo3(text_signature = "(cls, code)")]
    fn parse(_cls: &Bound<'_, PyType>, code: &str) -> PyResult<Self> {
        code.parse::<PeriodId>()
            .map(Self::from_inner)
            .map_err(core_to_py)
    }

    /// Build a monthly period identifier.
    #[classmethod]
    #[pyo3(text_signature = "(cls, year, month)")]
    fn month(_cls: &Bound<'_, PyType>, year: i32, month: u8) -> PyResult<Self> {
        PeriodId::try_month(year, month)
            .map(Self::from_inner)
            .map_err(core_to_py)
    }

    /// Build a quarterly period identifier.
    #[classmethod]
    #[pyo3(text_signature = "(cls, year, quarter)")]
    fn quarter(_cls: &Bound<'_, PyType>, year: i32, quarter: u8) -> PyResult<Self> {
        PeriodId::try_quarter(year, quarter)
            .map(Self::from_inner)
            .map_err(core_to_py)
    }

    /// Build an annual period identifier.
    #[classmethod]
    #[pyo3(text_signature = "(cls, year)")]
    fn annual(_cls: &Bound<'_, PyType>, year: i32) -> Self {
        Self::from_inner(PeriodId::annual(year))
    }

    /// Build a semi-annual period identifier.
    #[classmethod]
    #[pyo3(text_signature = "(cls, year, half)")]
    fn half(_cls: &Bound<'_, PyType>, year: i32, h: u8) -> PyResult<Self> {
        PeriodId::try_half(year, h)
            .map(Self::from_inner)
            .map_err(core_to_py)
    }

    /// Build a weekly period identifier.
    #[classmethod]
    #[pyo3(text_signature = "(cls, year, week)")]
    fn week(_cls: &Bound<'_, PyType>, year: i32, w: u8) -> PyResult<Self> {
        PeriodId::try_week(year, w)
            .map(Self::from_inner)
            .map_err(core_to_py)
    }

    /// Build a daily period identifier from an ordinal day.
    #[classmethod]
    #[pyo3(text_signature = "(cls, year, ordinal)")]
    fn day(_cls: &Bound<'_, PyType>, year: i32, ordinal: u16) -> PyResult<Self> {
        PeriodId::try_day(year, ordinal)
            .map(Self::from_inner)
            .map_err(core_to_py)
    }

    /// Period code string (e.g. ``"2025Q1"``).
    #[getter]
    fn code(&self) -> String {
        self.inner.to_string()
    }

    /// Gregorian calendar year.
    #[getter]
    fn year(&self) -> i32 {
        self.inner.year
    }

    /// Ordinal index within the year.
    #[getter]
    fn index(&self) -> u16 {
        self.inner.index
    }

    /// Kind (frequency) of this period.
    #[getter]
    fn kind(&self) -> PyPeriodKind {
        PyPeriodKind::from_inner(self.inner.kind())
    }

    /// Number of periods per year for this kind.
    #[getter]
    fn periods_per_year(&self) -> u16 {
        self.inner.periods_per_year()
    }

    /// Next period in sequence.
    fn next(&self) -> PyResult<Self> {
        self.inner.next().map(Self::from_inner).map_err(core_to_py)
    }

    /// Previous period in sequence.
    fn prev(&self) -> PyResult<Self> {
        self.inner.prev().map(Self::from_inner).map_err(core_to_py)
    }

    fn __repr__(&self) -> String {
        format!("PeriodId('{}')", self.inner)
    }

    fn __str__(&self) -> String {
        self.inner.to_string()
    }
}

/// A concrete period with start/end dates and an actual/forecast flag.
#[pyclass(
    name = "Period",
    module = "finstack_quant.core.dates",
    frozen,
    skip_from_py_object
)]
#[derive(Clone, Debug)]
pub struct PyPeriod {
    /// Inner Rust period.
    pub(crate) inner: Period,
}

impl PyPeriod {
    /// Build from an existing Rust [`Period`].
    pub(crate) fn from_inner(inner: Period) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyPeriod {
    /// Period identifier.
    #[getter]
    fn id(&self) -> PyPeriodId {
        PyPeriodId::from_inner(self.inner.id)
    }

    /// Inclusive start date as ``datetime.date``.
    #[getter]
    fn start<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.start)
    }

    /// Exclusive end date as ``datetime.date``.
    #[getter]
    fn end<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.end)
    }

    /// Whether this period is an actual (vs forecast).
    #[getter]
    fn is_actual(&self) -> bool {
        self.inner.is_actual
    }

    fn __repr__(&self) -> String {
        format!(
            "Period(id='{}', start={}, end={}, is_actual={})",
            self.inner.id, self.inner.start, self.inner.end, self.inner.is_actual,
        )
    }

    fn __str__(&self) -> String {
        self.inner.id.to_string()
    }
}

/// A plan containing a contiguous sequence of periods.
///
/// Returned by [`build_periods`] and [`build_fiscal_periods`].
#[pyclass(
    name = "PeriodPlan",
    module = "finstack_quant.core.dates",
    frozen,
    skip_from_py_object
)]
#[derive(Clone, Debug)]
pub struct PyPeriodPlan {
    /// Inner Rust [`PeriodPlan`].
    pub(crate) inner: PeriodPlan,
}

impl PyPeriodPlan {
    /// Build from an existing Rust [`PeriodPlan`].
    pub(crate) const fn from_inner(inner: PeriodPlan) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyPeriodPlan {
    /// List of periods in ascending order.
    #[getter]
    fn periods(&self) -> Vec<PyPeriod> {
        self.inner
            .periods
            .iter()
            .map(|p| PyPeriod::from_inner(p.clone()))
            .collect()
    }

    /// Number of periods.
    fn __len__(&self) -> usize {
        self.inner.periods.len()
    }

    fn __repr__(&self) -> String {
        format!("PeriodPlan(len={})", self.inner.periods.len())
    }
}

/// Fiscal year configuration.
#[pyclass(
    name = "FiscalConfig",
    module = "finstack_quant.core.dates",
    frozen,
    skip_from_py_object
)]
#[derive(Clone, Copy, Debug)]
pub struct PyFiscalConfig {
    /// Inner Rust fiscal configuration.
    pub(crate) inner: FiscalConfig,
}

impl PyFiscalConfig {
    /// Build from an existing Rust [`FiscalConfig`].
    pub(crate) const fn from_inner(inner: FiscalConfig) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyFiscalConfig {
    /// Create a fiscal configuration from a start month and day.
    #[new]
    #[pyo3(text_signature = "(start_month, start_day)")]
    fn new(start_month: u8, start_day: u8) -> PyResult<Self> {
        FiscalConfig::new(start_month, start_day)
            .map(Self::from_inner)
            .map_err(core_to_py)
    }

    /// Standard calendar year (January 1).
    #[classmethod]
    fn calendar_year(_cls: &Bound<'_, PyType>) -> Self {
        Self::from_inner(FiscalConfig::calendar_year())
    }

    /// US Federal fiscal year (October 1).
    #[classmethod]
    fn us_federal(_cls: &Bound<'_, PyType>) -> Self {
        Self::from_inner(FiscalConfig::us_federal())
    }

    /// UK fiscal year (April 6).
    #[classmethod]
    fn uk(_cls: &Bound<'_, PyType>) -> Self {
        Self::from_inner(FiscalConfig::uk())
    }

    /// Japanese fiscal year (April 1).
    #[classmethod]
    fn japan(_cls: &Bound<'_, PyType>) -> Self {
        Self::from_inner(FiscalConfig::japan())
    }

    /// Canadian fiscal year (April 1).
    #[classmethod]
    fn canada(_cls: &Bound<'_, PyType>) -> Self {
        Self::from_inner(FiscalConfig::canada())
    }

    /// Australian fiscal year (July 1).
    #[classmethod]
    fn australia(_cls: &Bound<'_, PyType>) -> Self {
        Self::from_inner(FiscalConfig::australia())
    }

    /// German fiscal year (January 1, same as calendar year).
    #[classmethod]
    fn germany(_cls: &Bound<'_, PyType>) -> Self {
        Self::from_inner(FiscalConfig::germany())
    }

    /// French fiscal year (January 1, same as calendar year).
    #[classmethod]
    fn france(_cls: &Bound<'_, PyType>) -> Self {
        Self::from_inner(FiscalConfig::france())
    }

    /// Month when the fiscal year starts (1-12).
    #[getter]
    fn start_month(&self) -> u8 {
        self.inner.start_month
    }

    /// Day when the fiscal year starts (1-31).
    #[getter]
    fn start_day(&self) -> u8 {
        self.inner.start_day
    }

    fn __repr__(&self) -> String {
        format!(
            "FiscalConfig(start_month={}, start_day={})",
            self.inner.start_month, self.inner.start_day,
        )
    }
}

/// Build periods from a range expression (e.g. ``"2025Q1..Q4"``).
///
/// Returns a [`PeriodPlan`] with actual/forecast split at ``actuals_cutoff``.
#[pyfunction]
#[pyo3(
    name = "build_periods",
    signature = (spec, actuals_cutoff=None),
    text_signature = "(spec, actuals_cutoff=None)"
)]
fn py_build_periods(spec: &str, actuals_cutoff: Option<&str>) -> PyResult<PyPeriodPlan> {
    let plan = build_periods(spec, actuals_cutoff).map_err(core_to_py)?;
    Ok(PyPeriodPlan::from_inner(plan))
}

/// Build fiscal periods with a custom fiscal year configuration.
#[pyfunction]
#[pyo3(
    name = "build_fiscal_periods",
    signature = (spec, fiscal_config, actuals_cutoff=None),
    text_signature = "(spec, fiscal_config, actuals_cutoff=None)"
)]
fn py_build_fiscal_periods(
    spec: &str,
    fiscal_config: &PyFiscalConfig,
    actuals_cutoff: Option<&str>,
) -> PyResult<PyPeriodPlan> {
    let plan =
        build_fiscal_periods(spec, fiscal_config.inner, actuals_cutoff).map_err(core_to_py)?;
    Ok(PyPeriodPlan::from_inner(plan))
}

/// Register period types on the `finstack_quant.core.dates` module.
pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyPeriodKind>()?;
    m.add_class::<PyPeriodId>()?;
    m.add_class::<PyPeriod>()?;
    m.add_class::<PyPeriodPlan>()?;
    m.add_class::<PyFiscalConfig>()?;
    m.add_function(wrap_pyfunction!(py_build_periods, m)?)?;
    m.add_function(wrap_pyfunction!(py_build_fiscal_periods, m)?)?;
    Ok(())
}

/// Names exported from this submodule.
pub const EXPORTS: &[&str] = &[
    "PeriodKind",
    "PeriodId",
    "Period",
    "PeriodPlan",
    "FiscalConfig",
    "build_periods",
    "build_fiscal_periods",
];
