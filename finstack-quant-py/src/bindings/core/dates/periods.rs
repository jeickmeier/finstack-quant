//! Python bindings for period types from [`finstack_quant_core::dates`].

use crate::bindings::date_utils::{date_to_py, py_to_date};
use crate::errors::core_to_py;
use finstack_quant_core::dates::{
    build_fiscal_periods, build_periods, FiscalConfig, Period, PeriodId, PeriodKind, PeriodPlan,
};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyIterator, PyList, PyModule, PyType};

/// Period frequency kind (Daily, Weekly, Monthly, Quarterly, Semi-annual, Annual).
///
/// Immutable, hashable enum-style type. ``str()`` gives the snake_case wire
/// name (``"quarterly"``), which ``from_name`` parses along with the pandas
/// offset aliases ``D``/``W``/``M``/``Q``/``A``.
///
/// Examples
/// --------
/// >>> from finstack_quant.core.dates import PeriodKind
/// >>> (PeriodKind.QUARTERLY.periods_per_year, PeriodKind.from_name("m") == PeriodKind.MONTHLY)
/// (4, True)
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
    ///
    /// Raises ``ValueError`` when ``name`` is not a kind name or offset alias.
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

    /// Observation date one period before ``first``.
    ///
    /// Daily and weekly step back 1 and 7 calendar days; monthly, quarterly,
    /// semi-annual and annual step back 1, 3, 6 and 12 months with month-end
    /// clamping. Used to seed a prior observation for period-over-period
    /// series.
    ///
    /// Parameters
    /// ----------
    /// first : datetime.date | str
    ///     First observation date of the series.
    ///
    /// Returns
    /// -------
    /// datetime.date
    ///     The prior observation date.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``first`` is not a valid calendar date or ISO string.
    #[pyo3(text_signature = "(self, first)")]
    fn prior_observation_date<'py>(
        &self,
        py: Python<'py>,
        first: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.prior_observation_date(py_to_date(first)?))
    }

    /// Support ``pickle`` by reconstructing through ``from_name``.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_name = py.get_type::<Self>().getattr("from_name")?;
        Ok((from_name, (self.inner.to_string(),)))
    }

    fn __repr__(&self) -> String {
        format!("PeriodKind('{}')", self.inner)
    }

    fn __str__(&self) -> String {
        self.inner.to_string()
    }
}

/// A period identifier such as ``2025Q1`` or ``2025M03``.
///
/// Immutable, hashable, totally ordered value type (chronological within one
/// kind, then by kind granularity), so identifiers sort and compare with
/// ``<``/``>`` directly.
///
/// Examples
/// --------
/// >>> from finstack_quant.core.dates import PeriodId
/// >>> (PeriodId.parse("2025Q2").code, PeriodId.parse("2025Q2") < PeriodId.parse("2025Q3"))
/// ('2025Q2', True)
#[pyclass(
    name = "PeriodId",
    module = "finstack_quant.core.dates",
    frozen,
    eq,
    ord,
    hash,
    skip_from_py_object
)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
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
    /// Parse a period code string (e.g. ``"2025Q1"`` or fiscal ``"FY2025W53"``).
    ///
    /// Raises ``ValueError`` naming the offending value and the accepted
    /// grammar (``2024``, ``2024Q1``, ``2024M01``, ``2024H1``, ``2024W05``,
    /// ``2024D100``, ``FY2024Q1``).
    #[classmethod]
    #[pyo3(text_signature = "(cls, code)")]
    fn parse(_cls: &Bound<'_, PyType>, code: &str) -> PyResult<Self> {
        code.parse::<PeriodId>()
            .map(Self::from_inner)
            .map_err(core_to_py)
    }

    /// Build a monthly period identifier (``month`` in 1-12).
    #[classmethod]
    #[pyo3(text_signature = "(cls, year, month)")]
    fn month(_cls: &Bound<'_, PyType>, year: i32, month: u8) -> PyResult<Self> {
        PeriodId::month(year, month)
            .map(Self::from_inner)
            .map_err(core_to_py)
    }

    /// Build a quarterly period identifier (``quarter`` in 1-4).
    #[classmethod]
    #[pyo3(text_signature = "(cls, year, quarter)")]
    fn quarter(_cls: &Bound<'_, PyType>, year: i32, quarter: u8) -> PyResult<Self> {
        PeriodId::quarter(year, quarter)
            .map(Self::from_inner)
            .map_err(core_to_py)
    }

    /// Build an annual period identifier.
    #[classmethod]
    #[pyo3(text_signature = "(cls, year)")]
    fn annual(_cls: &Bound<'_, PyType>, year: i32) -> Self {
        Self::from_inner(PeriodId::annual(year))
    }

    /// Build a semi-annual period identifier (``half`` is 1 or 2).
    #[classmethod]
    #[pyo3(text_signature = "(cls, year, half)")]
    fn half(_cls: &Bound<'_, PyType>, year: i32, half: u8) -> PyResult<Self> {
        PeriodId::half(year, half)
            .map(Self::from_inner)
            .map_err(core_to_py)
    }

    /// Build a weekly period identifier (ISO ``week`` in 1-52/53).
    #[classmethod]
    #[pyo3(text_signature = "(cls, year, week)")]
    fn week(_cls: &Bound<'_, PyType>, year: i32, week: u8) -> PyResult<Self> {
        PeriodId::week(year, week)
            .map(Self::from_inner)
            .map_err(core_to_py)
    }

    /// Build a daily period identifier from an ordinal day (1-365/366).
    #[classmethod]
    #[pyo3(text_signature = "(cls, year, ordinal)")]
    fn day(_cls: &Bound<'_, PyType>, year: i32, ordinal: u16) -> PyResult<Self> {
        PeriodId::day(year, ordinal)
            .map(Self::from_inner)
            .map_err(core_to_py)
    }

    /// Period code string (e.g. ``"2025Q1"``).
    #[getter]
    fn code(&self) -> String {
        self.inner.to_string()
    }

    /// Gregorian or fiscal year label.
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

    /// Whether this identifier uses fiscal-year (``FY...``) semantics.
    #[getter]
    fn is_fiscal(&self) -> bool {
        self.inner.is_fiscal()
    }

    /// Number of periods per year for this kind.
    #[getter]
    fn periods_per_year(&self) -> u16 {
        self.inner.periods_per_year()
    }

    /// Next Gregorian/ISO period in sequence (fiscal ids need ``next_fiscal``).
    fn next(&self) -> PyResult<Self> {
        self.inner.next().map(Self::from_inner).map_err(core_to_py)
    }

    /// Previous Gregorian/ISO period in sequence (fiscal ids need ``prev_fiscal``).
    fn prev(&self) -> PyResult<Self> {
        self.inner.prev().map(Self::from_inner).map_err(core_to_py)
    }

    /// Next period using fiscal-year week/day capacity.
    fn next_fiscal(&self, fiscal_config: &PyFiscalConfig) -> PyResult<Self> {
        self.inner
            .next_fiscal(fiscal_config.inner)
            .map(Self::from_inner)
            .map_err(core_to_py)
    }

    /// Previous period using fiscal-year week/day capacity.
    fn prev_fiscal(&self, fiscal_config: &PyFiscalConfig) -> PyResult<Self> {
        self.inner
            .prev_fiscal(fiscal_config.inner)
            .map(Self::from_inner)
            .map_err(core_to_py)
    }

    /// Support ``pickle`` by reconstructing through ``PeriodId.parse``.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let parse = py.get_type::<Self>().getattr("parse")?;
        Ok((parse, (self.inner.to_string(),)))
    }

    fn __repr__(&self) -> String {
        format!("PeriodId('{}')", self.inner)
    }

    fn __str__(&self) -> String {
        self.inner.to_string()
    }
}

/// A concrete period with start/end dates and an actual/forecast flag.
///
/// Examples
/// --------
/// >>> from finstack_quant.core.dates import build_periods
/// >>> period = build_periods("2024Q1..Q1").periods[0]
/// >>> (period.id.code, period.start.isoformat(), period.end.isoformat(), period.is_actual)
/// ('2024Q1', '2024-01-01', '2024-04-01', False)
#[pyclass(
    name = "Period",
    module = "finstack_quant.core.dates",
    frozen,
    eq,
    skip_from_py_object
)]
#[derive(Clone, Debug, PartialEq, Eq)]
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

    /// Serialize to the canonical JSON wire form.
    #[allow(clippy::wrong_self_convention)]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|e| crate::errors::serde_json_to_py(e, "cannot serialize Period"))
    }

    /// Deserialize from canonical JSON; raises ``ValueError`` on malformed input.
    #[staticmethod]
    #[pyo3(text_signature = "(json)")]
    fn from_json(json: &str) -> PyResult<Self> {
        serde_json::from_str::<Period>(json)
            .map(Self::from_inner)
            .map_err(|e| crate::errors::serde_json_to_py(e, "invalid Period JSON"))
    }

    /// Support ``pickle`` through the JSON round-trip.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    fn __repr__(&self) -> String {
        format!(
            "Period(id='{}', start='{}', end='{}', is_actual={})",
            self.inner.id,
            self.inner.start,
            self.inner.end,
            if self.inner.is_actual {
                "True"
            } else {
                "False"
            },
        )
    }

    fn __str__(&self) -> String {
        self.inner.id.to_string()
    }
}

/// A plan containing a contiguous sequence of periods.
///
/// Returned by ``build_periods`` and ``build_fiscal_periods``. Iterating a
/// plan yields its ``Period`` values in ascending order.
///
/// Examples
/// --------
/// >>> from finstack_quant.core.dates import build_periods
/// >>> [period.id.code for period in build_periods("2024Q1..Q2")]
/// ['2024Q1', '2024Q2']
#[pyclass(
    name = "PeriodPlan",
    module = "finstack_quant.core.dates",
    frozen,
    eq,
    skip_from_py_object
)]
#[derive(Clone, Debug, PartialEq, Eq)]
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

    /// Periods as a pandas DataFrame with columns ``id``, ``start``, ``end``,
    /// ``is_actual`` (``start``/``end`` as ``datetime64``).
    #[allow(clippy::wrong_self_convention)]
    #[pyo3(text_signature = "(self)")]
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let ids: Vec<String> = self
            .inner
            .periods
            .iter()
            .map(|p| p.id.to_string())
            .collect();
        let starts: Vec<time::Date> = self.inner.periods.iter().map(|p| p.start).collect();
        let ends: Vec<time::Date> = self.inner.periods.iter().map(|p| p.end).collect();
        let actual: Vec<bool> = self.inner.periods.iter().map(|p| p.is_actual).collect();
        let columns = PyDict::new(py);
        columns.set_item("id", ids)?;
        columns.set_item(
            "start",
            crate::bindings::pandas_utils::dates_to_datetime_index(py, &starts)?,
        )?;
        columns.set_item(
            "end",
            crate::bindings::pandas_utils::dates_to_datetime_index(py, &ends)?,
        )?;
        columns.set_item("is_actual", actual)?;
        crate::bindings::pandas_utils::dict_to_dataframe(py, &columns, None)
    }

    /// Serialize to the canonical JSON wire form.
    #[allow(clippy::wrong_self_convention)]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|e| crate::errors::serde_json_to_py(e, "cannot serialize PeriodPlan"))
    }

    /// Deserialize from canonical JSON; raises ``ValueError`` on malformed input.
    #[staticmethod]
    #[pyo3(text_signature = "(json)")]
    fn from_json(json: &str) -> PyResult<Self> {
        serde_json::from_str::<PeriodPlan>(json)
            .map(Self::from_inner)
            .map_err(|e| crate::errors::serde_json_to_py(e, "invalid PeriodPlan JSON"))
    }

    /// Support ``pickle`` through the JSON round-trip.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Iterate over the periods in ascending order.
    fn __iter__<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyIterator>> {
        PyList::new(py, self.periods())?.into_any().try_iter()
    }

    /// Number of periods.
    fn __len__(&self) -> usize {
        self.inner.periods.len()
    }

    fn __repr__(&self) -> String {
        match (self.inner.periods.first(), self.inner.periods.last()) {
            (Some(first), Some(last)) => format!(
                "PeriodPlan('{}'..'{}', len={})",
                first.id,
                last.id,
                self.inner.periods.len()
            ),
            _ => "PeriodPlan(len=0)".to_string(),
        }
    }
}

/// Fiscal year configuration (the month and day the fiscal year starts).
///
/// Parameters
/// ----------
/// start_month : int
///     Month (1-12) in which the fiscal year starts.
/// start_day : int
///     Day of that month (1-31); a day past the month's end (e.g. 30 February)
///     clamps to the last day when applied to a concrete year.
///
/// Raises
/// ------
/// ValueError
///     If ``start_month`` is outside 1-12 or ``start_day`` outside 1-31.
///
/// Examples
/// --------
/// >>> from finstack_quant.core.dates import FiscalConfig
/// >>> FiscalConfig.us_federal() == FiscalConfig(10, 1)
/// True
#[pyclass(
    name = "FiscalConfig",
    module = "finstack_quant.core.dates",
    frozen,
    eq,
    hash,
    skip_from_py_object
)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
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

    /// Australian fiscal year (July 1).
    #[classmethod]
    fn australia(_cls: &Bound<'_, PyType>) -> Self {
        Self::from_inner(FiscalConfig::australia())
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

    /// Support ``pickle`` by reconstructing through ``FiscalConfig(start_month, start_day)``.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (u8, u8))> {
        Ok((
            py.get_type::<Self>().into_any(),
            (self.inner.start_month, self.inner.start_day),
        ))
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
/// Parameters
/// ----------
/// spec : str
///     Range in the period grammar: ``2024Q1..Q4``, ``2024M01..2025M12``,
///     ``2024..2026``, ``2025W01..W52``, ``FY2024Q1..Q4``.
/// actuals_cutoff : str | None
///     Period code up to and including which periods are flagged
///     ``is_actual``; ``None`` marks every period as forecast.
///
/// Returns
/// -------
/// PeriodPlan
///     Contiguous periods in ascending order.
///
/// Raises
/// ------
/// ValueError
///     If ``spec`` or ``actuals_cutoff`` is malformed; the message names the
///     offending value and the accepted grammar.
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
///
/// Parameters
/// ----------
/// spec : str
///     Range in the period grammar (``FY2025Q1..Q4``); unprefixed ids are
///     interpreted as fiscal under ``fiscal_config``.
/// fiscal_config : FiscalConfig
///     Fiscal-year start used to bound each period.
/// actuals_cutoff : str | None
///     Period code up to and including which periods are flagged ``is_actual``.
///
/// Returns
/// -------
/// PeriodPlan
///     Contiguous fiscal periods in ascending order.
///
/// Raises
/// ------
/// ValueError
///     If ``spec`` or ``actuals_cutoff`` is malformed.
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
