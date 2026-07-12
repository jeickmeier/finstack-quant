//! Python bindings for [`finstack_quant_core::dates::Tenor`] and [`finstack_quant_core::dates::TenorUnit`].

use crate::errors::core_to_py;
use finstack_quant_core::dates::{Tenor, TenorUnit};
use pyo3::prelude::*;
use pyo3::types::{PyModule, PyType};

/// Frequency/tenor unit (Days, Weeks, Months, Years).
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

    /// Parse a single-character tenor unit designator (``D``, ``W``, ``M``, ``Y``).
    #[classmethod]
    #[pyo3(text_signature = "(cls, ch)")]
    fn from_char(_cls: &Bound<'_, PyType>, ch: char) -> PyResult<Self> {
        TenorUnit::from_char(ch)
            .map(Self::from_inner)
            .map_err(core_to_py)
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
        match self.inner {
            TenorUnit::Days => "D".to_string(),
            TenorUnit::Weeks => "W".to_string(),
            TenorUnit::Months => "M".to_string(),
            TenorUnit::Years => "Y".to_string(),
        }
    }
}

/// A tenor such as ``3M``, ``1Y``, or ``2W``.
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
    /// Construct a tenor from a count and unit.
    #[new]
    #[pyo3(text_signature = "(count, unit)")]
    fn new(count: u32, unit: PyRef<PyTenorUnit>) -> PyResult<Self> {
        Tenor::try_new(count, unit.inner)
            .map(Self::from_inner)
            .map_err(core_to_py)
    }

    /// Parse a tenor string (e.g. ``"3M"``, ``"1Y"``, ``"2W"``).
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

    /// Numeric count.
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

    /// Approximate tenor length in years (simple estimate, no calendar).
    #[allow(clippy::wrong_self_convention)]
    fn to_years_simple(&self) -> f64 {
        self.inner.to_years_simple()
    }

    /// Approximate tenor length in calendar days.
    #[allow(clippy::wrong_self_convention)]
    fn to_days_approx(&self) -> i64 {
        self.inner.to_days_approx()
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
