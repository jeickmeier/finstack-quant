//! Python bindings for [`finstack_core::money::Money`].

use std::str::FromStr;

use finstack_core::currency::Currency;
use finstack_core::money::Money;
use pyo3::basic::CompareOp;
use pyo3::exceptions::PyTypeError;
use pyo3::prelude::*;
use pyo3::sync::PyOnceLock;
use pyo3::types::{PyFloat, PyInt, PyList, PyTuple, PyType};
use pyo3::IntoPyObjectExt;

use crate::bindings::core::currency::{extract_currency, PyCurrency};
use crate::errors::{core_to_py, display_to_py};

static DECIMAL_TYPE: PyOnceLock<Py<PyType>> = PyOnceLock::new();

fn decimal_type<'py>(py: Python<'py>) -> PyResult<&'py Bound<'py, PyType>> {
    DECIMAL_TYPE.import(py, "decimal", "Decimal")
}

/// Wrapper for [`Money`] exposed to Python as `finstack.core.money.Money`.
#[pyclass(name = "Money", module = "finstack.core.money", frozen, from_py_object)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PyMoney {
    /// Inner currency-tagged amount.
    pub(crate) inner: Money,
}

impl PyMoney {
    /// Build a [`PyMoney`] from an existing [`Money`].
    pub(crate) const fn from_inner(inner: Money) -> Self {
        Self { inner }
    }
}

/// Parse `obj` as a [`PyMoney`] and return the wrapped [`Money`].
fn extract_money(obj: &Bound<'_, PyAny>) -> PyResult<Money> {
    obj.extract::<PyRef<'_, PyMoney>>()
        .map(|m| m.inner)
        .map_err(|_| PyTypeError::new_err("expected Money"))
}

/// Convert a Python ``decimal.Decimal`` into a `rust_decimal::Decimal` without
/// going through `f64`.
fn decimal_from_py(obj: &Bound<'_, PyAny>) -> PyResult<rust_decimal::Decimal> {
    if !is_python_decimal(obj)? {
        return Err(PyTypeError::new_err("expected decimal.Decimal"));
    }
    let s: String = obj.str()?.extract()?;
    rust_decimal::Decimal::from_str(&s)
        .map_err(|e| crate::errors::value_error(format!("Invalid Decimal value {s:?}: {e}")))
}

/// Return true if `obj` is an instance of `decimal.Decimal` (or any subclass).
///
/// Uses the Python `isinstance` check rather than a string compare on
/// `type(obj).__name__`, so `MyDecimal(Decimal)` subclasses and any third-party
/// `Decimal`-named classes are distinguished correctly. The import resolves
/// through a cached `decimal.Decimal` type after the first call.
fn is_python_decimal(obj: &Bound<'_, PyAny>) -> PyResult<bool> {
    let py = obj.py();
    obj.is_instance(decimal_type(py)?)
}

/// Build a [`Money`] from a Python amount that may be `float`, `int`, or
/// `decimal.Decimal`. `Decimal` inputs (including subclasses) preserve full
/// precision; numeric inputs follow IEEE 754 semantics and later ``amount``
/// accessors expose an ``f64`` view.
fn money_from_amount(obj: &Bound<'_, PyAny>, ccy: Currency) -> PyResult<Money> {
    if obj.is_instance_of::<PyFloat>() || obj.is_instance_of::<PyInt>() {
        let amount: f64 = obj.extract().map_err(|_| {
            PyTypeError::new_err("Money amount must be float, int, or decimal.Decimal")
        })?;
        return Money::try_new(amount, ccy).map_err(core_to_py);
    }
    if is_python_decimal(obj)? {
        let d = decimal_from_py(obj)?;
        return Money::from_decimal(d, ccy).map_err(core_to_py);
    }
    let amount: f64 = obj
        .extract()
        .map_err(|_| PyTypeError::new_err("Money amount must be float, int, or decimal.Decimal"))?;
    Money::try_new(amount, ccy).map_err(core_to_py)
}

#[pymethods]
impl PyMoney {
    /// Construct from a finite ``amount`` and a [`PyCurrency`] or ISO code string.
    ///
    /// ``amount`` may be a Python ``float``, ``int``, or ``decimal.Decimal``.
    /// ``Decimal`` inputs are parsed without going through ``f64``. Floats and
    /// ints follow standard IEEE 754 rounding; returned numeric accessors are
    /// finite ``float`` views of the internal Decimal amount.
    #[new]
    #[pyo3(text_signature = "(amount, currency)")]
    fn new(amount: &Bound<'_, PyAny>, currency: &Bound<'_, PyAny>) -> PyResult<Self> {
        let ccy = extract_currency(currency)?;
        money_from_amount(amount, ccy).map(Self::from_inner)
    }

    /// Zero amount in the given currency.
    #[classmethod]
    #[pyo3(text_signature = "(cls, currency)")]
    fn zero(_cls: &Bound<'_, PyType>, currency: &Bound<'_, PyAny>) -> PyResult<Self> {
        let ccy = extract_currency(currency)?;
        Money::try_new(0.0, ccy)
            .map(Self::from_inner)
            .map_err(core_to_py)
    }

    /// Construct from a ``decimal.Decimal`` amount, preserving full precision.
    ///
    /// This requires an actual ``decimal.Decimal`` instance.
    #[classmethod]
    #[pyo3(text_signature = "(cls, amount, currency)")]
    fn from_decimal(
        _cls: &Bound<'_, PyType>,
        amount: &Bound<'_, PyAny>,
        currency: &Bound<'_, PyAny>,
    ) -> PyResult<Self> {
        let ccy = extract_currency(currency)?;
        let d = decimal_from_py(amount)?;
        Money::from_decimal(d, ccy)
            .map(Self::from_inner)
            .map_err(core_to_py)
    }

    /// Numeric amount as ``float`` (derived from the internal decimal representation).
    #[getter]
    fn amount(&self) -> f64 {
        self.inner.amount()
    }

    /// Lossless amount as a Python ``decimal.Decimal``.
    ///
    /// The internal Rust ``Decimal`` is rendered to a string and parsed by
    /// ``decimal.Decimal``, so no ``float`` round-trip occurs.
    #[getter]
    fn amount_decimal<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let s = self.inner.amount_decimal().to_string();
        Ok(decimal_type(py)?.call1((s,))?.into_any())
    }

    /// Currency tag.
    #[getter]
    fn currency(&self, py: Python<'_>) -> PyResult<Py<PyCurrency>> {
        Py::new(py, PyCurrency::from_inner(self.inner.currency()))
    }

    /// Format with ``decimals`` places and optional currency prefix.
    ///
    /// When ``decimals`` is omitted, the currency's ISO minor-unit precision is used.
    #[pyo3(signature = (decimals=None, show_currency=true))]
    fn format(&self, decimals: Option<usize>, show_currency: bool) -> String {
        let dp = decimals.unwrap_or(self.inner.currency().decimals() as usize);
        self.inner.format(dp, show_currency)
    }

    /// Return a debug-style representation.
    fn __repr__(&self) -> String {
        format!(
            "Money({}, '{}')",
            self.inner.amount(),
            self.inner.currency()
        )
    }

    /// Human-readable amount with currency (ISO minor units).
    fn __str__(&self) -> String {
        self.inner.to_string()
    }

    /// Hash combining the amount bits and currency numeric code.
    fn __hash__(&self) -> isize {
        let bits = self.inner.amount().to_bits() as i64;
        let code = i64::from(self.inner.currency().numeric());
        (bits.rotate_left(17) ^ code) as isize
    }

    /// Rich comparison; ordering requires matching currencies.
    fn __richcmp__(
        &self,
        other: Bound<'_, PyAny>,
        op: CompareOp,
        py: Python<'_>,
    ) -> PyResult<Py<PyAny>> {
        let Ok(rhs) = other.extract::<PyRef<'_, PyMoney>>() else {
            return Ok(py.NotImplemented());
        };
        match op {
            CompareOp::Eq => Ok((self.inner == rhs.inner).into_py_any(py)?),
            CompareOp::Ne => Ok((self.inner != rhs.inner).into_py_any(py)?),
            CompareOp::Lt | CompareOp::Le | CompareOp::Gt | CompareOp::Ge => {
                if self.inner.currency() != rhs.inner.currency() {
                    return Err(crate::errors::value_error(
                        "cannot order Money values with different currencies",
                    ));
                }
                let ord = self
                    .inner
                    .amount()
                    .partial_cmp(&rhs.inner.amount())
                    .ok_or_else(|| crate::errors::value_error("non-comparable Money amounts"))?;
                Ok(op.matches(ord).into_py_any(py)?)
            }
        }
    }

    /// Serialize to JSON.
    #[allow(clippy::wrong_self_convention)]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    /// Deserialize from JSON.
    #[classmethod]
    #[pyo3(text_signature = "(cls, json)")]
    fn from_json(_cls: &Bound<'_, PyType>, json: &str) -> PyResult<Self> {
        let inner: Money = serde_json::from_str(json).map_err(display_to_py)?;
        Ok(Self::from_inner(inner))
    }

    /// ``(amount, currency_code)`` tuple.
    #[allow(clippy::wrong_self_convention)]
    fn to_tuple(&self) -> (f64, String) {
        (self.inner.amount(), self.inner.currency().to_string())
    }

    /// Build from ``(amount, currency_code)``.
    #[classmethod]
    #[pyo3(text_signature = "(cls, tup)")]
    fn from_tuple(_cls: &Bound<'_, PyType>, tup: &Bound<'_, PyTuple>) -> PyResult<Self> {
        let (amount, code): (f64, String) = tup.extract()?;
        let ccy = Currency::from_str(&code).map_err(|e| {
            crate::errors::value_error(format!("Invalid currency code {code:?}: {e}"))
        })?;
        Money::try_new(amount, ccy)
            .map(Self::from_inner)
            .map_err(core_to_py)
    }

    /// Add two amounts (same currency); maps [`Money::checked_add`] errors to Python.
    fn __add__(&self, other: &Bound<'_, PyAny>) -> PyResult<Self> {
        let rhs = extract_money(other)?;
        self.inner
            .checked_add(rhs)
            .map(Self::from_inner)
            .map_err(core_to_py)
    }

    /// Subtract two amounts (same currency).
    fn __sub__(&self, other: &Bound<'_, PyAny>) -> PyResult<Self> {
        let rhs = extract_money(other)?;
        self.inner
            .checked_sub(rhs)
            .map(Self::from_inner)
            .map_err(core_to_py)
    }

    /// Scale by a scalar ``float``.
    fn __mul__(&self, other: &Bound<'_, PyAny>) -> PyResult<Self> {
        let scalar: f64 = other.extract()?;
        self.inner
            .checked_mul_f64(scalar)
            .map(Self::from_inner)
            .map_err(core_to_py)
    }

    /// Divide by a scalar ``float``.
    fn __truediv__(&self, other: &Bound<'_, PyAny>) -> PyResult<Self> {
        let scalar: f64 = other.extract()?;
        self.inner
            .checked_div_f64(scalar)
            .map(Self::from_inner)
            .map_err(core_to_py)
    }

    /// Unary negation.
    fn __neg__(&self) -> PyResult<Self> {
        self.inner
            .checked_mul_f64(-1.0)
            .map(Self::from_inner)
            .map_err(core_to_py)
    }

    /// Right-add; supports ``Money + Money`` and ``0 + money``.
    fn __radd__(&self, other: &Bound<'_, PyAny>) -> PyResult<Self> {
        if let Ok(rhs) = other.extract::<PyRef<'_, PyMoney>>() {
            return rhs
                .inner
                .checked_add(self.inner)
                .map(Self::from_inner)
                .map_err(core_to_py);
        }
        let scalar: f64 = other.extract()?;
        if scalar == 0.0 {
            Ok(*self)
        } else {
            Err(PyTypeError::new_err(
                "unsupported right operand for Money addition (expected Money or 0)",
            ))
        }
    }

    /// Right-subtract: ``scalar - money`` in this money's currency.
    fn __rsub__(&self, other: &Bound<'_, PyAny>) -> PyResult<Self> {
        let scalar: f64 = other.extract()?;
        let zero = Money::try_new(scalar, self.inner.currency()).map_err(core_to_py)?;
        zero.checked_sub(self.inner)
            .map(Self::from_inner)
            .map_err(core_to_py)
    }

    /// Right-multiply by a scalar.
    fn __rmul__(&self, other: &Bound<'_, PyAny>) -> PyResult<Self> {
        let scalar: f64 = other.extract()?;
        self.inner
            .checked_mul_f64(scalar)
            .map(Self::from_inner)
            .map_err(core_to_py)
    }

    // Note: `PyMoney` is `frozen`, so in-place ops are not provided.
    // Python's `+=`/`-=`/`*=`/`/=` will fall back to the non-in-place dunders
    // (`__add__`, etc.) and rebind the variable to a fresh `Money`.
}

/// Register the `finstack.core.money` submodule.
pub fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let module = PyModule::new(py, "money")?;
    module.setattr("__doc__", "Currency-tagged money bindings (finstack-core).")?;
    module.add_class::<PyMoney>()?;

    let all = PyList::new(py, ["Money"])?;
    module.setattr("__all__", all)?;

    crate::bindings::module_utils::register_submodule(
        py,
        parent,
        &module,
        "money",
        "finstack.core",
        crate::bindings::module_utils::ParentNameSource::Package,
    )?;

    Ok(())
}
