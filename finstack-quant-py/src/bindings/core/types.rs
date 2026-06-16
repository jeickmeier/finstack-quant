//! Python bindings for `finstack_quant_core::types`.

use crate::errors::core_to_py;
use finstack_quant_core::types::{
    Attributes, Bps, CreditRating, CurveId, InstrumentId, Percentage, Rate,
};
use finstack_quant_core::InputError;
use finstack_quant_core::NonFiniteKind;
use pyo3::prelude::*;
use pyo3::types::{PyList, PyModule, PyType};
use std::hash::{Hash, Hasher};

#[inline]
fn canonical_f64_bits(value: f64) -> u64 {
    (value + 0.0).to_bits()
}

/// Wrapper for [`Rate`].
#[pyclass(
    module = "finstack_quant.core.types",
    name = "Rate",
    frozen,
    eq,
    ord,
    hash,
    skip_from_py_object
)]
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub struct PyRate {
    /// Underlying Rust rate.
    pub(crate) inner: Rate,
}

impl PyRate {
    /// Build a Python wrapper from a Rust [`Rate`].
    pub(crate) fn from_inner(inner: Rate) -> Self {
        Self { inner }
    }
}

impl Hash for PyRate {
    fn hash<H: Hasher>(&self, state: &mut H) {
        canonical_f64_bits(self.inner.as_decimal()).hash(state);
    }
}

#[pymethods]
impl PyRate {
    /// Zero rate (0% as a decimal rate).
    #[classattr]
    const ZERO: PyRate = PyRate { inner: Rate::ZERO };

    #[new]
    #[pyo3(text_signature = "(decimal)")]
    /// Construct a rate from a decimal fraction (e.g. ``0.05`` for 5%).
    fn new(decimal: f64) -> PyResult<Self> {
        Rate::try_from_decimal(decimal)
            .map(Self::from_inner)
            .map_err(core_to_py)
    }

    /// Build from a percent value (e.g. ``5.0`` for 5%).
    #[classmethod]
    #[pyo3(text_signature = "(cls, percent)")]
    fn from_percent(_cls: &Bound<'_, PyType>, percent: f64) -> PyResult<Self> {
        if !percent.is_finite() {
            let kind = if percent.is_nan() {
                NonFiniteKind::NaN
            } else if percent.is_sign_positive() {
                NonFiniteKind::PosInfinity
            } else {
                NonFiniteKind::NegInfinity
            };
            return Err(core_to_py(InputError::NonFiniteValue { kind }.into()));
        }
        Ok(Self::from_inner(Rate::from_percent(percent)))
    }

    /// Build from an integer basis-point amount (e.g. ``500`` for 5%).
    #[classmethod]
    #[pyo3(text_signature = "(cls, bps)")]
    fn from_bps(_cls: &Bound<'_, PyType>, bps: i32) -> Self {
        Self::from_inner(Rate::from_bps(bps))
    }

    /// Rate as a decimal fraction.
    #[getter]
    fn as_decimal(&self) -> f64 {
        self.inner.as_decimal()
    }

    /// Rate as a percent value.
    #[getter]
    fn as_percent(&self) -> f64 {
        self.inner.as_percent()
    }

    /// Rate rounded to the nearest basis point.
    #[getter]
    fn as_bps(&self) -> i32 {
        self.inner.as_bps()
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!("Rate(decimal={:?})", self.inner.as_decimal())
    }

    /// Return ``str(self)``.
    fn __str__(&self) -> String {
        self.inner.to_string()
    }

    /// Add two rates: ``Rate(a) + Rate(b) == Rate(a + b)``.
    fn __add__(&self, other: PyRef<Self>) -> PyResult<Self> {
        self.inner
            .checked_add(other.inner)
            .map(Self::from_inner)
            .map_err(core_to_py)
    }

    /// Subtract two rates: ``Rate(a) - Rate(b) == Rate(a - b)``.
    fn __sub__(&self, other: PyRef<Self>) -> PyResult<Self> {
        self.inner
            .checked_sub(other.inner)
            .map(Self::from_inner)
            .map_err(core_to_py)
    }

    /// Multiply by a scalar ``float``.
    fn __mul__(&self, rhs: f64) -> PyResult<Self> {
        self.inner
            .checked_mul(rhs)
            .map(Self::from_inner)
            .map_err(core_to_py)
    }

    /// Divide by a scalar ``float``; raises ``ValueError`` on zero divisor.
    fn __truediv__(&self, rhs: f64) -> PyResult<Self> {
        self.inner
            .checked_div(rhs)
            .map(Self::from_inner)
            .map_err(core_to_py)
    }

    /// Unary negation.
    fn __neg__(&self) -> PyResult<Self> {
        self.inner
            .checked_neg()
            .map(Self::from_inner)
            .map_err(core_to_py)
    }
}

/// Wrapper for [`Bps`].
#[pyclass(
    module = "finstack_quant.core.types",
    name = "Bps",
    frozen,
    eq,
    ord,
    hash,
    skip_from_py_object
)]
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct PyBps {
    /// Underlying Rust basis-point value.
    pub(crate) inner: Bps,
}

impl PyBps {
    /// Build a Python wrapper from a Rust [`Bps`].
    pub(crate) fn from_inner(inner: Bps) -> Self {
        Self { inner }
    }
}

impl Hash for PyBps {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.inner.as_bps().hash(state);
    }
}

#[pymethods]
impl PyBps {
    /// Zero basis points.
    #[classattr]
    const ZERO: PyBps = PyBps { inner: Bps::ZERO };

    #[new]
    #[pyo3(text_signature = "(bps)")]
    /// Construct from a floating basis-point value (rounded to the nearest integer bp).
    fn new(bps: f64) -> PyResult<Self> {
        Bps::try_new(bps).map(Self::from_inner).map_err(core_to_py)
    }

    /// Value as a decimal fraction.
    #[getter]
    fn as_decimal(&self) -> f64 {
        self.inner.as_decimal()
    }

    /// Value as whole basis points.
    #[getter]
    fn as_bps(&self) -> i32 {
        self.inner.as_bps()
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!("Bps({})", self.inner.as_bps())
    }

    /// Return ``str(self)``.
    fn __str__(&self) -> String {
        self.inner.to_string()
    }

    /// Add two basis-point values.
    fn __add__(&self, other: PyRef<Self>) -> PyResult<Self> {
        self.inner
            .checked_add(other.inner)
            .map(Self::from_inner)
            .map_err(core_to_py)
    }

    /// Subtract two basis-point values.
    fn __sub__(&self, other: PyRef<Self>) -> PyResult<Self> {
        self.inner
            .checked_sub(other.inner)
            .map(Self::from_inner)
            .map_err(core_to_py)
    }

    /// Multiply basis points by an integer scalar.
    fn __mul__(&self, rhs: i32) -> PyResult<Self> {
        self.inner
            .checked_mul(rhs)
            .map(Self::from_inner)
            .map_err(core_to_py)
    }

    /// Divide basis points by an integer scalar; raises ``ValueError`` on zero.
    fn __truediv__(&self, rhs: i32) -> PyResult<Self> {
        self.inner
            .checked_div(rhs)
            .map(Self::from_inner)
            .map_err(core_to_py)
    }

    /// Unary negation.
    fn __neg__(&self) -> PyResult<Self> {
        self.inner
            .checked_neg()
            .map(Self::from_inner)
            .map_err(core_to_py)
    }
}

/// Wrapper for [`Percentage`].
#[pyclass(
    module = "finstack_quant.core.types",
    name = "Percentage",
    frozen,
    eq,
    ord,
    hash,
    skip_from_py_object
)]
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub struct PyPercentage {
    /// Underlying Rust percentage.
    pub(crate) inner: Percentage,
}

impl PyPercentage {
    /// Build a Python wrapper from a Rust [`Percentage`].
    pub(crate) fn from_inner(inner: Percentage) -> Self {
        Self { inner }
    }
}

impl Hash for PyPercentage {
    fn hash<H: Hasher>(&self, state: &mut H) {
        canonical_f64_bits(self.inner.as_percent()).hash(state);
    }
}

#[pymethods]
impl PyPercentage {
    /// Zero percent.
    #[classattr]
    const ZERO: PyPercentage = PyPercentage {
        inner: Percentage::ZERO,
    };

    #[new]
    #[pyo3(text_signature = "(percent)")]
    /// Construct from a percent value (e.g. ``12.5`` for 12.5%).
    fn new(percent: f64) -> PyResult<Self> {
        Percentage::try_new(percent)
            .map(Self::from_inner)
            .map_err(core_to_py)
    }

    /// Value as a decimal fraction.
    #[getter]
    fn as_decimal(&self) -> f64 {
        self.inner.as_decimal()
    }

    /// Value in percent terms.
    #[getter]
    fn as_percent(&self) -> f64 {
        self.inner.as_percent()
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!("Percentage(percent={:?})", self.inner.as_percent())
    }

    /// Return ``str(self)``.
    fn __str__(&self) -> String {
        self.inner.to_string()
    }
}

/// Wrapper for [`CreditRating`].
#[pyclass(
    module = "finstack_quant.core.types",
    name = "CreditRating",
    frozen,
    eq,
    hash,
    skip_from_py_object
)]
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct PyCreditRating {
    /// Underlying Rust credit rating.
    pub(crate) inner: CreditRating,
}

impl PyCreditRating {
    /// Build a Python wrapper from a Rust [`CreditRating`].
    pub(crate) fn from_inner(inner: CreditRating) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyCreditRating {
    /// AAA / Aaa
    #[classattr]
    const AAA: PyCreditRating = PyCreditRating {
        inner: CreditRating::AAA,
    };
    /// AA+ / Aa1
    #[classattr]
    const AA_PLUS: PyCreditRating = PyCreditRating {
        inner: CreditRating::AAPlus,
    };
    /// AA / Aa2
    #[classattr]
    const AA: PyCreditRating = PyCreditRating {
        inner: CreditRating::AA,
    };
    /// AA- / Aa3
    #[classattr]
    const AA_MINUS: PyCreditRating = PyCreditRating {
        inner: CreditRating::AAMinus,
    };
    /// A+ / A1
    #[classattr]
    const A_PLUS: PyCreditRating = PyCreditRating {
        inner: CreditRating::APlus,
    };
    /// A / A2
    #[classattr]
    const A: PyCreditRating = PyCreditRating {
        inner: CreditRating::A,
    };
    /// A- / A3
    #[classattr]
    const A_MINUS: PyCreditRating = PyCreditRating {
        inner: CreditRating::AMinus,
    };
    /// BBB+ / Baa1
    #[classattr]
    const BBB_PLUS: PyCreditRating = PyCreditRating {
        inner: CreditRating::BBBPlus,
    };
    /// BBB / Baa2
    #[classattr]
    const BBB: PyCreditRating = PyCreditRating {
        inner: CreditRating::BBB,
    };
    /// BBB- / Baa3
    #[classattr]
    const BBB_MINUS: PyCreditRating = PyCreditRating {
        inner: CreditRating::BBBMinus,
    };
    /// BB+ / Ba1
    #[classattr]
    const BB_PLUS: PyCreditRating = PyCreditRating {
        inner: CreditRating::BBPlus,
    };
    /// BB / Ba2
    #[classattr]
    const BB: PyCreditRating = PyCreditRating {
        inner: CreditRating::BB,
    };
    /// BB- / Ba3
    #[classattr]
    const BB_MINUS: PyCreditRating = PyCreditRating {
        inner: CreditRating::BBMinus,
    };
    /// B+ / B1
    #[classattr]
    const B_PLUS: PyCreditRating = PyCreditRating {
        inner: CreditRating::BPlus,
    };
    /// B / B2
    #[classattr]
    const B: PyCreditRating = PyCreditRating {
        inner: CreditRating::B,
    };
    /// B- / B3
    #[classattr]
    const B_MINUS: PyCreditRating = PyCreditRating {
        inner: CreditRating::BMinus,
    };
    /// CCC+ / Caa1
    #[classattr]
    const CCC_PLUS: PyCreditRating = PyCreditRating {
        inner: CreditRating::CCCPlus,
    };
    /// CCC / Caa2
    #[classattr]
    const CCC: PyCreditRating = PyCreditRating {
        inner: CreditRating::CCC,
    };
    /// CCC- / Caa3
    #[classattr]
    const CCC_MINUS: PyCreditRating = PyCreditRating {
        inner: CreditRating::CCCMinus,
    };
    /// CC / Ca
    #[classattr]
    const CC: PyCreditRating = PyCreditRating {
        inner: CreditRating::CC,
    };
    /// C
    #[classattr]
    const C: PyCreditRating = PyCreditRating {
        inner: CreditRating::C,
    };
    /// Default rating.
    #[classattr]
    const D: PyCreditRating = PyCreditRating {
        inner: CreditRating::D,
    };
    /// Not rated.
    #[classattr]
    const NR: PyCreditRating = PyCreditRating {
        inner: CreditRating::NR,
    };

    /// Parse a rating string (case-insensitive, accepts S&P/Fitch and Moody's notation).
    ///
    /// Examples: ``"BBB"``, ``"BBB-"``, ``"Baa3"``, ``"B+"``, ``"B1"``
    #[classmethod]
    #[pyo3(text_signature = "(cls, name)")]
    fn from_name(_cls: &Bound<'_, PyType>, name: &str) -> PyResult<Self> {
        name.parse::<CreditRating>()
            .map(Self::from_inner)
            .map_err(core_to_py)
    }

    /// Canonical rating name in S&P/Fitch notation (e.g. ``"BBB-"``).
    #[getter]
    fn name(&self) -> String {
        self.inner.to_string()
    }

    /// Moody's WARF factor for this rating.
    #[getter]
    fn warf(&self) -> PyResult<f64> {
        self.inner.warf().map_err(core_to_py)
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!("CreditRating({})", self.inner)
    }

    /// Return ``str(self)``.
    fn __str__(&self) -> String {
        self.inner.to_string()
    }
}

/// Wrapper for [`CurveId`].
#[pyclass(
    module = "finstack_quant.core.types",
    name = "CurveId",
    frozen,
    eq,
    hash,
    skip_from_py_object
)]
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct PyCurveId {
    /// Underlying Rust curve identifier.
    pub(crate) inner: CurveId,
}

impl PyCurveId {
    /// Build a Python wrapper from a Rust [`CurveId`].
    pub(crate) fn from_inner(inner: CurveId) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyCurveId {
    #[new]
    #[pyo3(text_signature = "(value)")]
    /// Create a curve identifier from its string value.
    fn new(value: &str) -> Self {
        Self::from_inner(CurveId::from(value))
    }

    /// Underlying string value.
    #[pyo3(text_signature = "(self)")]
    fn as_str(&self) -> &str {
        self.inner.as_str()
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!("CurveId({:?})", self.inner.as_str())
    }

    /// Return ``str(self)`` — the underlying identifier string.
    fn __str__(&self) -> String {
        self.inner.as_str().to_string()
    }
}

/// Wrapper for [`InstrumentId`].
#[pyclass(
    module = "finstack_quant.core.types",
    name = "InstrumentId",
    frozen,
    eq,
    hash,
    skip_from_py_object
)]
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct PyInstrumentId {
    /// Underlying Rust instrument identifier.
    pub(crate) inner: InstrumentId,
}

impl PyInstrumentId {
    /// Build a Python wrapper from a Rust [`InstrumentId`].
    pub(crate) fn from_inner(inner: InstrumentId) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyInstrumentId {
    #[new]
    #[pyo3(text_signature = "(value)")]
    /// Create an instrument identifier from its string value.
    fn new(value: &str) -> Self {
        Self::from_inner(InstrumentId::from(value))
    }

    /// Underlying string value.
    #[pyo3(text_signature = "(self)")]
    fn as_str(&self) -> &str {
        self.inner.as_str()
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!("InstrumentId({:?})", self.inner.as_str())
    }

    /// Return ``str(self)`` — the underlying identifier string.
    fn __str__(&self) -> String {
        self.inner.as_str().to_string()
    }
}

/// Wrapper for [`Attributes`].
#[pyclass(
    module = "finstack_quant.core.types",
    name = "Attributes",
    skip_from_py_object
)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PyAttributes {
    /// Underlying Rust attribute bag.
    pub(crate) inner: Attributes,
}

impl PyAttributes {
    /// Build a Python wrapper from Rust [`Attributes`].
    pub(crate) fn from_inner(inner: Attributes) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyAttributes {
    #[new]
    /// Create an empty attribute set.
    fn new() -> Self {
        Self::from_inner(Attributes::new())
    }

    /// Fetch metadata by key.
    #[pyo3(text_signature = "(self, key)")]
    fn get_meta(&self, key: &str) -> Option<String> {
        self.inner.get_meta(key).map(str::to_string)
    }

    /// Insert or replace a metadata entry.
    #[pyo3(text_signature = "(self, key, value)")]
    fn set_meta(&mut self, key: &str, value: &str) {
        self.inner.set_meta(key, value);
    }

    /// Return whether `key` exists in metadata.
    #[pyo3(text_signature = "(self, key)")]
    fn contains_meta_key(&self, key: &str) -> bool {
        self.inner.contains_meta_key(key)
    }

    /// Metadata keys in sorted order.
    fn keys(&self) -> Vec<String> {
        self.inner.meta.keys().cloned().collect()
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "Attributes(tags={}, meta_keys={})",
            self.inner.tags.len(),
            self.inner.meta.len()
        )
    }

    /// Number of metadata entries; ``len(attrs) == len(attrs.keys())``.
    fn __len__(&self) -> usize {
        self.inner.meta.len()
    }
}

/// Register the `finstack_quant.core.types` submodule.
pub fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(py, "types")?;
    m.setattr(
        "__doc__",
        "Core finstack-quant types: rates, identifiers, credit ratings, attributes.",
    )?;

    m.add_class::<PyRate>()?;
    m.add_class::<PyBps>()?;
    m.add_class::<PyPercentage>()?;
    m.add_class::<PyCreditRating>()?;
    m.add_class::<PyCurveId>()?;
    m.add_class::<PyInstrumentId>()?;
    m.add_class::<PyAttributes>()?;
    let all = PyList::new(
        py,
        [
            "Rate",
            "Bps",
            "Percentage",
            "CreditRating",
            "CurveId",
            "InstrumentId",
            "Attributes",
        ],
    )?;
    m.setattr("__all__", all)?;

    crate::bindings::module_utils::register_submodule(
        py,
        parent,
        &m,
        "types",
        "finstack_quant.core",
        crate::bindings::module_utils::ParentNameSource::Package,
    )?;

    Ok(())
}
