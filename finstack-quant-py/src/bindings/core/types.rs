//! Python bindings for `finstack_quant_core::types`.

use crate::errors::{core_to_py, serde_json_to_py};
use finstack_quant_core::types::{Attributes, Bps, CreditRating, CurveId, InstrumentId, Rate};
use pyo3::basic::CompareOp;
use pyo3::exceptions::{PyKeyError, PyTypeError};
use pyo3::prelude::*;
use pyo3::types::{PyList, PyModule, PyString, PyType};
use pyo3::IntoPyObjectExt;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

#[inline]
fn canonical_f64_bits(value: f64) -> u64 {
    (value + 0.0).to_bits()
}

fn to_json_impl<T: serde::Serialize>(inner: &T, what: &str) -> PyResult<String> {
    serde_json::to_string(inner).map_err(|err| serde_json_to_py(err, what))
}

fn from_json_impl<T: serde::de::DeserializeOwned>(json: &str, what: &str) -> PyResult<T> {
    serde_json::from_str(json).map_err(|err| serde_json_to_py(err, what))
}

/// A financial rate expressed as a decimal fraction (``0.05`` is 5% / 500 bp).
///
/// Immutable, hashable value type. Supports checked arithmetic and conversion
/// between decimal, percent and basis-point representations.
///
/// Parameters
/// ----------
/// value : float | str
///     Decimal fraction (``0.05``) or a quote string: ``"5%"``, ``"25bp"``,
///     ``"25bps"`` or ``"0.05"``.
///
/// Raises
/// ------
/// ValueError
///     If the value is non-finite or the string cannot be parsed.
///
/// Examples
/// --------
/// >>> from finstack_quant.core.types import Rate
/// >>> Rate("5%") == Rate(0.05) == Rate("500bp")
/// True
#[pyclass(
    module = "finstack_quant.core.types",
    name = "Rate",
    frozen,
    eq,
    ord,
    hash,
    skip_from_py_object
)]
#[derive(Clone, Debug, PartialEq, PartialOrd)]
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

    /// Construct a rate from a decimal fraction (``0.05`` for 5%) or a quote
    /// string (``"5%"``, ``"25bp"``, ``"25bps"``, ``"0.05"``).
    ///
    /// Raises ``ValueError`` for non-finite values or unparsable strings.
    #[new]
    #[pyo3(text_signature = "(value)")]
    fn new(value: &Bound<'_, PyAny>) -> PyResult<Self> {
        if let Ok(text) = value.cast::<PyString>() {
            return text
                .to_str()?
                .parse::<Rate>()
                .map(Self::from_inner)
                .map_err(core_to_py);
        }
        let decimal: f64 = value
            .extract()
            .map_err(|_| PyTypeError::new_err("Rate value must be a float or a str quote"))?;
        Rate::from_decimal(decimal)
            .map(Self::from_inner)
            .map_err(core_to_py)
    }

    /// Build from a percent value (e.g. ``5.0`` for 5%).
    #[classmethod]
    #[pyo3(text_signature = "(cls, percent)")]
    fn from_percent(_cls: &Bound<'_, PyType>, percent: f64) -> PyResult<Self> {
        Rate::from_percent(percent)
            .map(Self::from_inner)
            .map_err(core_to_py)
    }

    /// Build from an integer basis-point amount (e.g. ``500`` for 5%).
    #[classmethod]
    #[pyo3(text_signature = "(cls, bp)")]
    fn from_bp(_cls: &Bound<'_, PyType>, bp: i32) -> Self {
        Self::from_inner(Rate::from_bp(bp))
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
    fn as_bp(&self) -> i32 {
        self.inner.as_bp()
    }

    /// Rate in fractional basis points, without rounding (Rust ``Rate::as_bp_f64``).
    ///
    /// ``Rate(0.0029).as_bp_f64 == 29.0`` and ``Rate("62.5bp").as_bp_f64 == 62.5``.
    #[getter]
    fn as_bp_f64(&self) -> f64 {
        self.inner.as_bp_f64()
    }

    /// Absolute value.
    #[pyo3(text_signature = "(self)")]
    fn abs(&self) -> Self {
        Self::from_inner(self.inner.abs())
    }

    /// ``True`` when the rate is exactly zero.
    #[pyo3(text_signature = "(self)")]
    fn is_zero(&self) -> bool {
        self.inner.is_zero()
    }

    /// ``True`` when the rate is strictly positive.
    #[pyo3(text_signature = "(self)")]
    fn is_positive(&self) -> bool {
        self.inner.is_positive()
    }

    /// ``True`` when the rate is strictly negative.
    #[pyo3(text_signature = "(self)")]
    fn is_negative(&self) -> bool {
        self.inner.is_negative()
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!("Rate(decimal={:?})", self.inner.as_decimal())
    }

    /// Return ``str(self)``.
    fn __str__(&self) -> String {
        self.inner.to_string()
    }

    /// Add a ``Rate`` or a ``Bps`` spread: ``Rate(0.05) + Bps(25) == Rate(0.0525)``.
    fn __add__(&self, other: &Bound<'_, PyAny>) -> PyResult<Self> {
        let rhs = extract_rate_or_basis_points(other)?;
        self.inner
            .checked_add(rhs)
            .map(Self::from_inner)
            .map_err(core_to_py)
    }

    /// Subtract a ``Rate`` or a ``Bps`` spread.
    fn __sub__(&self, other: &Bound<'_, PyAny>) -> PyResult<Self> {
        let rhs = extract_rate_or_basis_points(other)?;
        self.inner
            .checked_sub(rhs)
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

    /// Right-multiply by a scalar ``float`` (``2.0 * rate``).
    fn __rmul__(&self, lhs: f64) -> PyResult<Self> {
        self.__mul__(lhs)
    }

    /// Divide by a scalar ``float``; raises ``ValueError`` on zero divisor.
    fn __truediv__(&self, rhs: f64) -> PyResult<Self> {
        self.inner
            .checked_div(rhs)
            .map(Self::from_inner)
            .map_err(core_to_py)
    }

    /// Unary negation.
    fn __neg__(&self) -> Self {
        Self::from_inner(-self.inner)
    }

    /// Serialize to JSON (the bare decimal number, e.g. ``0.05``).
    fn to_json(&self) -> PyResult<String> {
        to_json_impl(&self.inner, "invalid Rate")
    }

    /// Deserialize from JSON (a bare decimal number).
    #[staticmethod]
    #[pyo3(text_signature = "(json)")]
    fn from_json(json: &str) -> PyResult<Self> {
        from_json_impl(json, "invalid Rate JSON").map(Self::from_inner)
    }

    /// Support ``pickle`` via the JSON wire format.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }
}

/// Accept a `Rate` or a `Bps` operand and return it as a `Rate`.
fn extract_rate_or_basis_points(obj: &Bound<'_, PyAny>) -> PyResult<Rate> {
    if let Ok(rate) = obj.extract::<PyRef<'_, PyRate>>() {
        return Ok(rate.inner);
    }
    if let Ok(bps) = obj.extract::<PyRef<'_, PyBps>>() {
        return Ok(bps.inner.as_rate());
    }
    Err(PyTypeError::new_err("expected Rate or Bps"))
}

/// A value measured in whole basis points (1 bp = 0.0001 = 0.01%).
///
/// Immutable, hashable, integer-backed value type. Fractional input is
/// rejected rather than rounded, because silently rounding a sub-bp spread
/// would change instrument economics.
///
/// Parameters
/// ----------
/// bp : float
///     Whole basis-point value (``250`` for 2.5%).
///
/// Raises
/// ------
/// ValueError
///     If *bp* is non-finite or not a whole number of basis points.
///
/// Examples
/// --------
/// >>> from finstack_quant.core.types import Bps
/// >>> Bps(250).as_decimal
/// 0.025
#[pyclass(
    module = "finstack_quant.core.types",
    name = "Bps",
    frozen,
    eq,
    ord,
    hash,
    skip_from_py_object
)]
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
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
        self.inner.as_bp().hash(state);
    }
}

#[pymethods]
impl PyBps {
    /// Zero basis points.
    #[classattr]
    const ZERO: PyBps = PyBps { inner: Bps::ZERO };

    /// Construct from a whole basis-point value.
    ///
    /// Raises ``ValueError`` for fractional input: the canonical Rust
    /// ``Bps::try_new`` is integer-backed and rejects sub-bp quotes. Use a
    /// decimal ``Rate`` (``Rate("62.5bp")``) for sub-bp precision.
    #[new]
    #[pyo3(text_signature = "(bp)")]
    fn new(bp: f64) -> PyResult<Self> {
        Bps::try_new(bp).map(Self::from_inner).map_err(core_to_py)
    }

    /// Value as a decimal fraction.
    #[getter]
    fn as_decimal(&self) -> f64 {
        self.inner.as_decimal()
    }

    /// Value as whole basis points.
    #[getter]
    fn as_bp(&self) -> i32 {
        self.inner.as_bp()
    }

    /// Value in percent units (``Bps(250).as_percent == 2.5``).
    #[getter]
    fn as_percent(&self) -> f64 {
        self.inner.as_percent()
    }

    /// Value as a decimal ``Rate``.
    #[getter]
    fn as_rate(&self) -> PyRate {
        PyRate::from_inner(self.inner.as_rate())
    }

    /// Absolute value.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the quote is ``-2147483648`` bp, whose absolute value has no
    ///     32-bit representation.
    #[pyo3(text_signature = "(self)")]
    fn abs(&self) -> PyResult<Self> {
        self.inner
            .checked_abs()
            .map(Self::from_inner)
            .map_err(core_to_py)
    }

    /// ``True`` when exactly zero basis points.
    #[pyo3(text_signature = "(self)")]
    fn is_zero(&self) -> bool {
        self.inner.is_zero()
    }

    /// ``True`` when strictly positive.
    #[pyo3(text_signature = "(self)")]
    fn is_positive(&self) -> bool {
        self.inner.is_positive()
    }

    /// ``True`` when strictly negative.
    #[pyo3(text_signature = "(self)")]
    fn is_negative(&self) -> bool {
        self.inner.is_negative()
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!("Bps({})", self.inner.as_bp())
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

    /// Right-multiply by an integer scalar (``3 * Bps(10)``).
    fn __rmul__(&self, lhs: i32) -> PyResult<Self> {
        self.__mul__(lhs)
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

    /// Serialize to JSON (the bare integer basis-point quote, e.g. ``250``).
    fn to_json(&self) -> PyResult<String> {
        to_json_impl(&self.inner, "invalid Bps")
    }

    /// Deserialize from JSON (a bare integer).
    #[staticmethod]
    #[pyo3(text_signature = "(json)")]
    fn from_json(json: &str) -> PyResult<Self> {
        from_json_impl(json, "invalid Bps JSON").map(Self::from_inner)
    }

    /// Support ``pickle`` via the JSON wire format.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }
}

/// Standardised credit rating category on the 23-step S&P/Fitch scale.
///
/// Immutable, hashable, ordered enum-style type: ``AAA < AA+ < ... < C < NR < D``
/// (a "smaller" rating is stronger credit; ``NR`` sits between ``C`` and ``D``).
/// Notched ratings (``"BBB+"``, ``"Baa1"``) keep notch-level precision.
/// Equality and ordering compare ``CreditRating`` instances; parse strings
/// explicitly, for example ``CreditRating.BBB == CreditRating("baa2")``.
///
/// Parameters
/// ----------
/// name : str
///     Rating string in S&P/Fitch or Moody's notation, case-insensitive
///     (``"BBB+"``, ``"Baa1"``, ``"nr"``).
///
/// Raises
/// ------
/// ValueError
///     If *name* is not a recognised rating.
///
/// Examples
/// --------
/// >>> from finstack_quant.core.types import CreditRating
/// >>> CreditRating("Baa1") == CreditRating.BBB_PLUS
/// True
/// >>> CreditRating.BBB.notches_to(CreditRating.BB)
/// 3
#[pyclass(
    module = "finstack_quant.core.types",
    name = "CreditRating",
    frozen,
    skip_from_py_object
)]
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
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

    /// Parse a rating string (case-insensitive, S&P/Fitch or Moody's notation).
    ///
    /// Raises ``ValueError`` if *name* is not a recognised rating.
    #[new]
    #[pyo3(text_signature = "(name)")]
    fn new(name: &str) -> PyResult<Self> {
        name.parse::<CreditRating>()
            .map(Self::from_inner)
            .map_err(core_to_py)
    }

    /// Canonical rating name in S&P/Fitch notation (e.g. ``"BBB-"``).
    #[getter]
    fn name(&self) -> String {
        self.inner.to_string()
    }

    /// ``True`` for BBB- and above.
    #[pyo3(text_signature = "(self)")]
    fn is_investment_grade(&self) -> bool {
        self.inner.is_investment_grade()
    }

    /// ``True`` for ratings below BBB- (``NR`` is neither grade).
    #[pyo3(text_signature = "(self)")]
    fn is_speculative_grade(&self) -> bool {
        self.inner.is_speculative_grade()
    }

    /// ``True`` only for ``D``.
    #[pyo3(text_signature = "(self)")]
    fn is_default(&self) -> bool {
        self.inner.is_default()
    }

    /// Moody's-style label (``"Baa1"`` for BBB+).
    #[pyo3(text_signature = "(self)")]
    // PyO3 `#[pymethods]` cannot take `self` by value.
    fn to_moodys_string(&self) -> &'static str {
        self.inner.to_moodys_string()
    }

    /// Signed notch distance to *other*: positive when *other* is weaker.
    ///
    /// ``CreditRating.BBB.notches_to(CreditRating.BB) == 3``. Accepts a
    /// ``CreditRating`` or a rating string.
    #[pyo3(text_signature = "(self, other)")]
    fn notches_to(&self, other: &Bound<'_, PyAny>) -> PyResult<i32> {
        let rhs = extract_credit_rating(other)?;
        Ok(self.inner.notches_to(rhs))
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!("CreditRating({:?})", self.inner.to_string())
    }

    /// Return ``str(self)``.
    fn __str__(&self) -> String {
        self.inner.to_string()
    }

    /// Hash consistent with equality between ``CreditRating`` instances.
    fn __hash__(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        self.inner.hash(&mut hasher);
        hasher.finish()
    }

    /// Rich comparison against another ``CreditRating``.
    ///
    /// Ordering follows credit quality: ``AAA < BBB < D``. Other host types
    /// compare unequal and do not support ordering.
    fn __richcmp__(
        &self,
        other: &Bound<'_, PyAny>,
        op: CompareOp,
        py: Python<'_>,
    ) -> PyResult<Py<PyAny>> {
        if let Ok(rhs) = other.extract::<PyRef<'_, PyCreditRating>>() {
            return op.matches(self.inner.cmp(&rhs.inner)).into_py_any(py);
        }
        Ok(py.NotImplemented())
    }

    /// Serialize to JSON (the quoted S&P/Fitch label, e.g. ``"BBB+"``).
    fn to_json(&self) -> PyResult<String> {
        to_json_impl(&self.inner, "invalid CreditRating")
    }

    /// Deserialize from JSON (a quoted S&P/Fitch label).
    #[staticmethod]
    #[pyo3(text_signature = "(json)")]
    fn from_json(json: &str) -> PyResult<Self> {
        from_json_impl(json, "invalid CreditRating JSON").map(Self::from_inner)
    }

    /// Support ``pickle`` via the JSON wire format.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }
}

/// Extract a [`CreditRating`] from a `CreditRating` instance or a rating string.
pub(crate) fn extract_credit_rating(obj: &Bound<'_, PyAny>) -> PyResult<CreditRating> {
    if let Ok(rating) = obj.extract::<PyRef<'_, PyCreditRating>>() {
        return Ok(rating.inner);
    }
    if let Ok(text) = obj.cast::<PyString>() {
        return text.to_str()?.parse::<CreditRating>().map_err(core_to_py);
    }
    Err(PyTypeError::new_err("expected CreditRating or rating str"))
}

macro_rules! define_py_id_wrapper {
    (
        $(#[$meta:meta])*
        $wrapper:ident,
        $py_name:literal,
        $inner:ty
    ) => {
        $(#[$meta])*
        #[pyclass(
            module = "finstack_quant.core.types",
            name = $py_name,
            frozen,
            eq,
            ord,
            hash,
            skip_from_py_object
        )]
        #[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
        pub struct $wrapper {
            /// Underlying Rust identifier.
            pub(crate) inner: $inner,
        }

        impl $wrapper {
            /// Build a Python wrapper from a Rust identifier.
            pub(crate) fn from_inner(inner: $inner) -> Self {
                Self { inner }
            }
        }

        #[pymethods]
        impl $wrapper {
            /// Create an identifier from its string value (empty ids are accepted).
            #[new]
            #[pyo3(text_signature = "(value)")]
            fn new(value: &str) -> Self {
                Self::from_inner(<$inner>::from(value))
            }

            /// Underlying string value.
            #[pyo3(text_signature = "(self)")]
            fn as_str(&self) -> &str {
                self.inner.as_str()
            }

            /// ``True`` when the identifier is the empty string.
            #[pyo3(text_signature = "(self)")]
            fn is_empty(&self) -> bool {
                self.inner.is_empty()
            }

            /// Length of the identifier in bytes (UTF-8).
            fn __len__(&self) -> usize {
                self.inner.len()
            }

            /// Return ``repr(self)``.
            fn __repr__(&self) -> String {
                format!("{}({:?})", $py_name, self.inner.as_str())
            }

            /// Return ``str(self)`` — the underlying identifier string.
            fn __str__(&self) -> String {
                self.inner.as_str().to_string()
            }

            /// Serialize to JSON (a quoted string).
            fn to_json(&self) -> PyResult<String> {
                to_json_impl(&self.inner, concat!("invalid ", $py_name))
            }

            /// Deserialize from JSON (a quoted string).
            #[staticmethod]
            #[pyo3(text_signature = "(json)")]
            fn from_json(json: &str) -> PyResult<Self> {
                from_json_impl(json, concat!("invalid ", $py_name, " JSON")).map(Self::from_inner)
            }

            /// Support ``pickle`` via the JSON wire format.
            fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
                let from_json = py.get_type::<Self>().getattr("from_json")?;
                crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
            }
        }
    };
}

define_py_id_wrapper!(
    /// A unique identifier for a market data curve.
    ///
    /// Immutable, hashable, ordered string wrapper. Empty identifiers are
    /// accepted (``is_empty()`` reports them); ordering is lexicographic.
    ///
    /// Parameters
    /// ----------
    /// value : str
    ///     Curve identifier text, stored verbatim.
    ///
    /// Examples
    /// --------
    /// >>> from finstack_quant.core.types import CurveId
    /// >>> CurveId("USD-OIS").as_str()
    /// 'USD-OIS'
    PyCurveId,
    "CurveId",
    CurveId
);

define_py_id_wrapper!(
    /// A unique identifier for a financial instrument.
    ///
    /// Immutable, hashable, ordered string wrapper. Empty identifiers are
    /// accepted (``is_empty()`` reports them); ordering is lexicographic.
    ///
    /// Parameters
    /// ----------
    /// value : str
    ///     Instrument identifier text, stored verbatim.
    ///
    /// Examples
    /// --------
    /// >>> from finstack_quant.core.types import InstrumentId
    /// >>> InstrumentId("BOND_A").as_str()
    /// 'BOND_A'
    PyInstrumentId,
    "InstrumentId",
    InstrumentId
);

/// User-defined tags and string metadata attached to instruments.
///
/// Tags are a sorted set of labels; ``meta`` is a sorted ``str -> str`` map.
/// The mapping protocol (``attrs["key"]``, ``"key" in attrs``, ``len(attrs)``,
/// ``keys()``, ``items()``) covers the metadata map. Selectors accept
/// ``"*"``, ``"tag:<name>"`` and ``"meta:<key>=<value>"``.
///
/// Examples
/// --------
/// >>> from finstack_quant.core.types import Attributes
/// >>> attrs = Attributes()
/// >>> attrs.add_tag("energy")
/// >>> attrs.set_meta("region", "NA")
/// >>> attrs.matches_selector("meta:region=NA")
/// True
#[pyclass(
    module = "finstack_quant.core.types",
    name = "Attributes",
    eq,
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
    /// Create an empty attribute set (no tags, no metadata).
    #[new]
    fn new() -> Self {
        Self::from_inner(Attributes::new())
    }

    /// Sorted list of tags.
    #[getter]
    fn tags(&self) -> Vec<String> {
        self.inner.tags.iter().cloned().collect()
    }

    /// Add a tag (no-op if already present).
    #[pyo3(text_signature = "(self, tag)")]
    fn add_tag(&mut self, tag: &str) {
        self.inner.add_tag(tag);
    }

    /// Return whether *tag* is present.
    #[pyo3(text_signature = "(self, tag)")]
    fn has_tag(&self, tag: &str) -> bool {
        self.inner.has_tag(tag)
    }

    /// Match against a selector: ``"*"``, ``"tag:<name>"`` or
    /// ``"meta:<key>=<value>"``. Unknown selector syntax returns ``False``.
    #[pyo3(text_signature = "(self, selector)")]
    fn matches_selector(&self, selector: &str) -> bool {
        self.inner.matches_selector(selector)
    }

    /// Fetch metadata by key, or ``None`` when absent.
    #[pyo3(text_signature = "(self, key)")]
    fn get_meta(&self, key: &str) -> Option<String> {
        self.inner.get_meta(key).map(str::to_string)
    }

    /// Insert or replace a metadata entry.
    ///
    /// *value* may be a ``str``, ``int`` or ``float``; non-string values are
    /// stored as their ``str()`` form.
    #[pyo3(text_signature = "(self, key, value)")]
    fn set_meta(&mut self, key: &str, value: &Bound<'_, PyAny>) -> PyResult<()> {
        let value: String = value.str()?.extract()?;
        self.inner.set_meta(key, value);
        Ok(())
    }

    /// Return whether `key` exists in metadata.
    #[pyo3(text_signature = "(self, key)")]
    fn contains_meta_key(&self, key: &str) -> bool {
        self.inner.contains_meta_key(key)
    }

    /// Metadata keys in sorted order.
    #[pyo3(text_signature = "(self)")]
    fn keys(&self) -> Vec<String> {
        self.inner.meta.keys().cloned().collect()
    }

    /// Metadata ``(key, value)`` pairs in sorted key order.
    #[pyo3(text_signature = "(self)")]
    fn items(&self) -> Vec<(String, String)> {
        self.inner
            .meta
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }

    /// ``attrs[key]`` — metadata lookup; raises ``KeyError`` when absent.
    fn __getitem__(&self, key: &str) -> PyResult<String> {
        self.inner
            .get_meta(key)
            .map(str::to_string)
            .ok_or_else(|| PyKeyError::new_err(key.to_string()))
    }

    /// ``key in attrs`` — metadata key membership.
    fn __contains__(&self, key: &str) -> bool {
        self.inner.contains_meta_key(key)
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "Attributes(tags={:?}, meta={:?})",
            self.tags(),
            self.items()
        )
    }

    /// Number of metadata entries; ``len(attrs) == len(attrs.keys())``.
    fn __len__(&self) -> usize {
        self.inner.meta.len()
    }

    /// Serialize to JSON (``{"tags": [...], "meta": {...}}``; empty parts are omitted).
    fn to_json(&self) -> PyResult<String> {
        to_json_impl(&self.inner, "invalid Attributes")
    }

    /// Deserialize from JSON; unknown fields are rejected.
    #[staticmethod]
    #[pyo3(text_signature = "(json)")]
    fn from_json(json: &str) -> PyResult<Self> {
        from_json_impl(json, "invalid Attributes JSON").map(Self::from_inner)
    }

    /// Support ``pickle`` via the JSON wire format.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }
}

/// Register the `finstack_quant.core.types` submodule.
pub fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = crate::bindings::module_utils::new_submodule(parent, "types")?;
    m.setattr(
        "__doc__",
        "Core finstack-quant types: rates, identifiers, credit ratings, attributes.",
    )?;

    m.add_class::<PyRate>()?;
    m.add_class::<PyBps>()?;
    m.add_class::<PyCreditRating>()?;
    m.add_class::<PyCurveId>()?;
    m.add_class::<PyInstrumentId>()?;
    m.add_class::<PyAttributes>()?;
    let all = PyList::new(
        py,
        [
            "Attributes",
            "Bps",
            "CreditRating",
            "CurveId",
            "InstrumentId",
            "Rate",
        ],
    )?;
    m.setattr("__all__", all)?;

    crate::bindings::module_utils::attach_submodule(
        parent,
        &m,
        crate::bindings::module_utils::Exposure::Compiled,
    )?;

    Ok(())
}
